"""A disposable Relay engine for the bench, and calls to it as the person.

The engine runs `relay --instance test serve` with its data, socket and Claude Code's config under
a fresh `/tmp/relay-bench/<n>/`, never the user's dev instance or ~/.claude. Everything it starts
(Claude Code, Relay's MCP server, llama-server) inherits that environment, including libfaketime at
the fixed anchor, and a marker variable the teardown sweeps for, so nothing outlives the bench.

Every op call is `relay --instance test --actor user q <op> <payload>` with the XDG environment:
the person's ops on the throwaway engine (approved for the bench: Relay guardrail request #43).
"""

import json
import os
import shutil
import signal
import subprocess
import tempfile
import time
from pathlib import Path

from . import ANCHOR, WORK_ROOT, ZONE, log

#: Where libfaketime may be. RELAYBENCH_FAKETIME names the library itself.
FAKETIME_DIRS = [
    Path.home() / ".local/lib/faketime",
    Path("/usr/lib/faketime"),
    Path("/usr/lib/x86_64-linux-gnu/faketime"),
    Path("/usr/local/lib/faketime"),
]
#: The variable every process of one bench engine carries, so teardown finds them all.
MARKER = "RELAYBENCH_ROOT"
#: Variables of the session that runs the bench that must not reach the throwaway engine or the
#: agent it starts: Relay's own session identity and store, and the calling Claude Code's state.
DROP_PREFIXES = ("RELAY_", "CLAUDE", "ANTHROPIC_", "FAKETIME", "LLAMA_", "OTEL_", "XDG_")
DROP_EXACT = {"LD_PRELOAD", "NO_FAKE_STAT", "TZ", MARKER}


class OpError(Exception):
    """A typed refusal from the engine (`kind`, `code`, `message`), or a CLI failure."""

    def __init__(self, op, code, message, exit_code=None, detail=None):
        super().__init__(f"{op}: {code}: {message}")
        self.op, self.code, self.message, self.exit_code, self.detail = op, code, message, exit_code, detail


def find_faketime():
    """The libfaketime shared object, or None."""
    explicit = os.environ.get("RELAYBENCH_FAKETIME")
    if explicit:
        return Path(explicit) if Path(explicit).is_file() else None
    for d in FAKETIME_DIRS:
        lib = d / "libfaketime.so.1"
        if lib.is_file():
            return lib
    return None


def clean_env(base=None):
    """The caller's environment without anything that names another Relay, Claude Code session or
    fake clock."""
    env = dict(base if base is not None else os.environ)
    for key in list(env):
        if key in DROP_EXACT or key.startswith(DROP_PREFIXES):
            del env[key]
    return env


def faketime_env(lib, anchor=ANCHOR):
    return {
        "LD_PRELOAD": str(lib),
        "FAKETIME": "@" + anchor,
        # Timeouts, sleeps and Instant keep real time; only the wall clock is moved.
        "FAKETIME_DONT_FAKE_MONOTONIC": "1",
        # File times stay real: SQLite and Claude Code's own files do not need a fake mtime.
        "NO_FAKE_STAT": "1",
    }


def _private_dir(path):
    path.mkdir(parents=True, exist_ok=True)
    os.chmod(path, 0o700)
    return path


def new_root():
    """A fresh, private `/tmp/relay-bench/<n>/` (short, for the socket path)."""
    _private_dir(WORK_ROOT)
    return Path(tempfile.mkdtemp(prefix="", dir=WORK_ROOT))


def processes_marked(root):
    """Pids of this user's processes whose environment carries MARKER=root."""
    needle = f"{MARKER}={root}".encode() + b"\0"
    mine = []
    for entry in os.listdir("/proc"):
        if not entry.isdigit() or int(entry) == os.getpid():
            continue
        try:
            with open(f"/proc/{entry}/environ", "rb") as f:
                if needle in f.read() + b"\0":
                    mine.append(int(entry))
        except OSError:
            continue
    return mine


def _die_with_parent():
    """In the engine's child, before exec: it gets SIGTERM if the bench dies without its teardown
    (SIGKILL), and its model server and agents go with it."""
    try:
        import ctypes
        ctypes.CDLL("libc.so.6", use_errno=True).prctl(1, signal.SIGTERM)  # PR_SET_PDEATHSIG
    except Exception:
        pass


def become_subreaper():
    """Orphans of the throwaway engine (its agents, its model server) are re-parented to this
    process instead of to whatever Relay engine runs the bench, so teardown can reap them."""
    try:
        import ctypes
        ctypes.CDLL("libc.so.6", use_errno=True).prctl(36, 1)  # PR_SET_CHILD_SUBREAPER
    except Exception:
        pass


def reap():
    """Collect every child that has exited. Only at teardown: it would take the exit status of a
    subprocess another thread is waiting on."""
    while True:
        try:
            pid, _ = os.waitpid(-1, os.WNOHANG)
        except ChildProcessError:
            return
        if pid == 0:
            return


def _alive(pid):
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return not finished(pid)


def finished(pid):
    """Gone, or a zombie with nothing left running. A multi-threaded process whose main thread has
    exited also reads as a zombie while its other threads (llama-server's CUDA ones) still hold
    what they hold: that one has not finished."""
    try:
        status = open(f"/proc/{pid}/status").read()
    except OSError:
        return True
    fields = dict(line.split(":", 1) for line in status.splitlines() if ":" in line)
    state = fields.get("State", "").strip()[:1]
    threads = int(fields.get("Threads", "1").strip() or 1)
    return state in ("Z", "X") and threads <= 1


class Engine:
    """One throwaway engine. Use as a context manager, or call start() and stop()."""

    def __init__(self, relay_bin, *, root=None, anchor=ANCHOR, zone=ZONE, faketime=True, extra_env=None, keep=False):
        self.relay_bin = Path(relay_bin).resolve()
        if not self.relay_bin.is_file():
            raise FileNotFoundError(f"no relay binary at {self.relay_bin} (cargo build -p relay-cli)")
        self.root = Path(root) if root else None
        self.anchor = anchor
        self.zone = zone
        self.keep = keep
        self.extra_env = dict(extra_env or {})
        self.faketime_lib = None
        if faketime:
            self.faketime_lib = find_faketime()
            if not self.faketime_lib:
                raise RuntimeError(
                    "libfaketime is not installed: build it into ~/.local/lib/faketime (bench/threads/README.md, "
                    "'Installing libfaketime'), or set RELAYBENCH_FAKETIME to libfaketime.so.1")
        self.proc = None
        self.calls = 0

    # ── paths and environments ────────────────────────────────────────────

    @property
    def data_home(self):
        return self.root / "data"

    @property
    def store_dir(self):
        """Where the engine keeps store.db, threads.db and local-model.log."""
        return self.data_home / "relay-v4" / "test"

    @property
    def claude_dir(self):
        return self.root / "claude"

    def cli_env(self):
        """For the bench's own CLI calls: the throwaway engine's XDG dirs and nothing else of
        Relay's or Claude's."""
        env = clean_env()
        env.update({
            "XDG_DATA_HOME": str(self.data_home),
            "XDG_RUNTIME_DIR": str(self.root / "run"),
            "TZ": self.zone,
            MARKER: str(self.root),
        })
        return env

    def engine_env(self):
        """For the engine, and so for everything it starts: Claude Code, the MCP server, llama-server."""
        env = self.cli_env()
        env.update({
            "CLAUDE_CONFIG_DIR": str(self.claude_dir),
            # The thread agent's MCP server is started from RELAY_BIN (relay-core hooks::relay_bin):
            # the binary under test, never the one of the session running the bench.
            "RELAY_BIN": str(self.relay_bin),
        })
        if self.faketime_lib:
            env.update(faketime_env(self.faketime_lib, self.anchor))
        env.update(self.extra_env)
        return env

    # ── lifecycle ─────────────────────────────────────────────────────────

    def start(self, timeout=60):
        if self.root is None:
            self.root = new_root()
        _private_dir(self.root)
        for sub in ("data", "run", "claude", "fixtures"):
            _private_dir(self.root / sub)
        self.log_path = self.root / "engine.log"
        out = open(self.log_path, "ab")
        self.proc = subprocess.Popen(
            [str(self.relay_bin), "--instance", "test", "serve"],
            cwd=self.root, env=self.engine_env(), stdin=subprocess.DEVNULL, stdout=out, stderr=out,
            start_new_session=True, preexec_fn=_die_with_parent,
        )
        out.close()
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if self.proc.poll() is not None:
                raise RuntimeError(f"the engine exited ({self.proc.returncode}); see {self.log_path}:\n{self.log_tail()}")
            if self.ping():
                log(f"engine up: pid {self.proc.pid}, root {self.root}")
                return self
            time.sleep(0.2)
        raise RuntimeError(f"the engine did not answer within {timeout}s; see {self.log_path}:\n{self.log_tail()}")

    def ping(self):
        try:
            r = subprocess.run([str(self.relay_bin), "--instance", "test", "ping"], env=self.cli_env(),
                               capture_output=True, timeout=5)
            return r.returncode == 0
        except subprocess.TimeoutExpired:
            return False

    def log_tail(self, n=20):
        try:
            return "\n".join(self.log_path.read_text(errors="replace").splitlines()[-n:])
        except OSError:
            return ""

    def stop(self):
        """Stop the model server and the agents, then the engine, then anything else that still
        carries this engine's marker. Removes the root unless `keep`."""
        if self.proc is None:
            return
        if self.proc.poll() is None:
            for op, payload in (("thread.pick", {"model": None}), ("app.quit", {"force": True})):
                try:
                    self.q(op, payload, timeout=10)
                except Exception:
                    pass
            try:
                self.proc.wait(timeout=8)
            except subprocess.TimeoutExpired:
                self._killpg(self.proc.pid, signal.SIGTERM)
                try:
                    self.proc.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    self._killpg(self.proc.pid, signal.SIGKILL)
                    self.proc.wait(timeout=5)
        self.sweep()
        if not self.keep and self.root and str(self.root).startswith(str(WORK_ROOT) + "/"):
            shutil.rmtree(self.root, ignore_errors=True)
        self.proc = None

    def sweep(self, grace=10.0):
        """Kill every process still carrying this engine's marker (agents, MCP servers,
        llama-server, a proxy), politely first, and reap what was left to this process
        (`become_subreaper`). llama-server may take seconds to let go of the card."""
        if not self.root:
            return []
        pids = [p for p in processes_marked(self.root) if _alive(p)]
        for pid in pids:
            try:
                os.kill(pid, signal.SIGTERM)
            except OSError:
                pass
        deadline = time.monotonic() + grace
        while time.monotonic() < deadline and any(_alive(p) for p in pids):
            time.sleep(0.1)
        for pid in pids:
            if _alive(pid):
                try:
                    os.kill(pid, signal.SIGKILL)
                except OSError:
                    pass
        time.sleep(0.2)
        reap()
        return pids

    @staticmethod
    def _killpg(pid, sig):
        try:
            os.killpg(pid, sig)
        except OSError:
            pass

    def __enter__(self):
        return self.start()

    def __exit__(self, *exc):
        self.stop()
        return False

    # ── calls ─────────────────────────────────────────────────────────────

    def q(self, op, payload=None, timeout=60):
        """Run one op as the person and return its result, parsed. Raises OpError on a refusal."""
        argv = [str(self.relay_bin), "--instance", "test", "--actor", "user", "q", op, json.dumps(payload or {})]
        self.calls += 1
        try:
            r = subprocess.run(argv, env=self.cli_env(), capture_output=True, text=True, timeout=timeout)
        except subprocess.TimeoutExpired:
            raise OpError(op, "bench.timeout", f"no answer within {timeout}s")
        if r.returncode != 0:
            err = r.stderr.strip()
            try:
                e = json.loads(err)
                raise OpError(op, e.get("code", "?"), e.get("message", err), r.returncode, e)
            except json.JSONDecodeError:
                raise OpError(op, "bench.cli", err or f"exit {r.returncode}", r.returncode)
        out = r.stdout.strip()
        return json.loads(out) if out else None

    def setting(self, path, value):
        return self.q("settings.set", {"path": path, "value": value})
