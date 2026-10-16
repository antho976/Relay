"""Any GGUF behind the engine's own local-model path (contract: "Any model").

The engine starts `threads.local.server` with its arguments for model `qwen35-4b`, reading
`Qwen3.5-4B-Q8_0.gguf` from `threads.local.dir`. The bench points that folder at a temp one holding a
symlink of that name to the GGUF under test, and the server at a wrapper that execs the real
llama-server with the engine's arguments, except:

- `-np 1 -c 32768` become `-np N -c N×32768`: N slots of the product's context each;
- `--chat-template-file` is the product's (kept) for a Qwen3.5 GGUF, `--template` when given, and
  dropped (the GGUF's own template) for any other model;
- libfaketime is taken out of llama-server's environment (it needs no date, and CUDA no preload).

With a capture folder the wrapper also stands a small proxy on the engine's port that forwards to
the server and writes every request and its assembled answer to `requests.jsonl`, which is how
"What the model sees" (README) is measured.

Also here: the GPU lock every bench run holds while a llama-server is up, the free-memory check
that picks N, and what the server's log says it did.
"""

import fcntl
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

from . import BENCH_DIR, GPU_LOCK, MODEL_FILE, log

DEFAULT_SERVER = Path.home() / ".local/bin/llama-server"
#: The context of one slot, as the product gives its one conversation (local_models::CONTEXT).
SLOT_CONTEXT = 32768
#: VRAM a slot of SLOT_CONTEXT takes beyond the weights, for picking N; measured for Qwen3.5 4B Q8
#: (README, "GPU memory"), conservative for others. The server fits what does not fit, slowly.
SLOT_MIB = 1024
#: The weights' overhead on the card (CUDA context, compute buffers), beside the file's size.
BASE_MIB = 900
MARGIN_MIB = 512


# ── what to run ───────────────────────────────────────────────────────────


class ModelSpec:
    """A GGUF to test, and how to run it."""

    def __init__(self, gguf, *, name=None, template=None, keep_template=None, server=None, parallel=3,
                 extra_args=None, capture=None):
        self.gguf = Path(gguf).expanduser().resolve()
        if not self.gguf.is_file():
            raise FileNotFoundError(f"no GGUF at {self.gguf}")
        self.name = name or self.gguf.stem
        self.template = Path(template).expanduser().resolve() if template else None
        if self.template and not self.template.is_file():
            raise FileNotFoundError(f"no chat template at {self.template}")
        # The product's template is written for Qwen3.5; any other model renders with its own.
        if keep_template is None:
            keep_template = bool(re.search(r"qwen3[._-]?5", self.gguf.name, re.I))
        self.keep_template = keep_template and not self.template
        self.server = Path(server or shutil.which("llama-server") or DEFAULT_SERVER).expanduser()
        if not self.server.is_file():
            raise FileNotFoundError(f"no llama-server at {self.server}")
        self.parallel = parallel
        self.extra_args = list(extra_args or [])
        self.capture = Path(capture).resolve() if capture else None

    def template_mode(self):
        if self.template:
            return str(self.template)
        return "product" if self.keep_template else "gguf"

    def describe(self):
        st = self.gguf.stat()
        h = hashlib.sha256()
        with open(self.gguf, "rb") as f:
            h.update(f.read(64 << 20))
        info = {
            "name": self.name,
            "file": self.gguf.name,
            "path": str(self.gguf),
            "bytes": st.st_size,
            "sha256_first_64mib": h.hexdigest(),
            "template": self.template_mode(),
            "server": str(self.server),
            "server_version": server_version(self.server),
        }
        if self.template:
            info["template_sha256"] = hashlib.sha256(self.template.read_bytes()).hexdigest()
        return info

    def install(self, root):
        """Lay out the models folder and the wrapper under `root`; returns (models_dir, wrapper)."""
        models = root / "models"
        models.mkdir(exist_ok=True)
        link = models / MODEL_FILE
        if link.is_symlink() or link.exists():
            link.unlink()
        link.symlink_to(self.gguf)
        cfg = {
            "server": str(self.server),
            "parallel": self.parallel,
            "slot_context": SLOT_CONTEXT,
            "template": "keep" if self.keep_template else (str(self.template) if self.template else "drop"),
            "extra_args": self.extra_args,
            "capture": str(self.capture) if self.capture else None,
            "argv_out": str(root / "llama-argv.json"),
        }
        (root / "wrapper.json").write_text(json.dumps(cfg, indent=1))
        wrapper = root / "llama-server-wrapper"
        wrapper.write_text(
            "#!/bin/sh\n"
            "# The bench's stand-in for llama-server (bench/threads/relaybench/model.py).\n"
            f"exec python3 -c 'import sys; sys.path.insert(0, sys.argv.pop(1)); "
            f"from relaybench.model import wrap_main; wrap_main()' '{BENCH_DIR}' '{root / 'wrapper.json'}' \"$@\"\n"
        )
        wrapper.chmod(0o700)
        if self.capture:
            self.capture.mkdir(parents=True, exist_ok=True)
        return models, wrapper


def server_version(server):
    try:
        r = subprocess.run([str(server), "--version"], capture_output=True, text=True, timeout=20,
                           env={k: v for k, v in os.environ.items() if k != "LD_PRELOAD"})
        text = (r.stdout + r.stderr).strip().splitlines()
        return next((l.strip() for l in text if l.startswith("version")), text[-1] if text else "?")
    except (OSError, subprocess.TimeoutExpired):
        return "?"


# ── the GPU ───────────────────────────────────────────────────────────────


def nvidia(query, kind="gpu"):
    flag = "--query-gpu" if kind == "gpu" else "--query-compute-apps"
    try:
        r = subprocess.run(["nvidia-smi", f"{flag}={query}", "--format=csv,noheader,nounits"],
                           capture_output=True, text=True, timeout=15)
    except (OSError, subprocess.TimeoutExpired):
        return []
    return [[c.strip() for c in line.split(",")] for line in r.stdout.strip().splitlines() if line.strip()]


def gpu_info():
    rows = nvidia("name,memory.total,memory.used,memory.free,driver_version")
    if not rows:
        return None
    name, total, used, free, driver = rows[0]
    return {"name": name, "total_mib": int(total), "used_mib": int(used), "free_mib": int(free), "driver": driver}


def llama_servers(gpu_only=False):
    """Pids of every llama-server of this user, wherever it came from. With `gpu_only`, only the
    ones that hold the card or may: on nvidia-smi's list, or not started CPU-only (`-ngl 0`)."""
    on_card = {int(row[0]) for row in nvidia("pid", kind="apps") if row and row[0].isdigit()}
    found = []
    for entry in os.listdir("/proc"):
        if not entry.isdigit():
            continue
        try:
            comm = Path(f"/proc/{entry}/comm").read_text().strip()
            argv = Path(f"/proc/{entry}/cmdline").read_bytes().split(b"\0")
        except OSError:
            continue
        if not comm.startswith("llama-server"):
            continue
        from .engine import finished
        if finished(int(entry)):
            continue  # exited, not yet reaped: it holds nothing
        argv = [a.decode(errors="replace") for a in argv]
        cpu_only = any(a in ("-ngl", "--n-gpu-layers", "--gpu-layers") and i + 1 < len(argv) and argv[i + 1] == "0"
                       for i, a in enumerate(argv)) or any(a in ("-dev", "--device") and i + 1 < len(argv) and argv[i + 1] == "none"
                                                            for i, a in enumerate(argv))
        if gpu_only and cpu_only and int(entry) not in on_card:
            continue
        found.append(int(entry))
    return found


def llama_on_card():
    """Pids of llama-servers nvidia-smi lists on the card (any user's view of them: by name)."""
    return [int(r[0]) for r in nvidia("pid,process_name", kind="apps")
            if len(r) > 1 and r[0].isdigit() and "llama" in r[1].lower()]


def wait_card_clear(timeout=60.0, quiet=False):
    """Wait until no llama-server is on the card. A server that was just stopped can hold its memory
    for some seconds after the run before has let go of the lock. True when clear."""
    deadline = time.monotonic() + timeout
    said = False
    while True:
        # nvidia-smi stops naming a server whose main thread has exited before its CUDA threads let
        # go of the memory; /proc still shows it (a zombie leader with threads).
        held = sorted(set(llama_on_card()) | set(llama_servers(gpu_only=True)))
        if not held:
            return True
        if time.monotonic() >= deadline:
            return False
        if not said and not quiet:
            log(f"waiting for llama-server {held} to leave the GPU")
            said = True
        time.sleep(1)


def pick_parallel(requested, gguf_bytes, free_mib, slot_mib=SLOT_MIB):
    """The most slots up to `requested` whose memory fits what the card has free."""
    need = gguf_bytes / (1 << 20) + BASE_MIB + MARGIN_MIB
    fits = int((free_mib - need) // slot_mib)
    return max(1, min(requested, fits))


class GpuLock:
    """An exclusive flock held for as long as a bench's llama-server may be up. Several people run
    benches on one card; the second waits, and says for whom."""

    def __init__(self, path=GPU_LOCK, what="relay bench"):
        self.path = Path(path)
        self.what = what
        self.f = None

    def __enter__(self):
        self.f = open(self.path, "a+")
        try:
            fcntl.flock(self.f, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            self.f.seek(0)
            try:
                holder = json.loads(self.f.read() or "{}")
            except json.JSONDecodeError:
                holder = {}
            log(f"waiting for the GPU (held by pid {holder.get('pid', '?')}, since {holder.get('since', '?')}"
                f"{', ' + holder['what'] if holder.get('what') else ''})")
            fcntl.flock(self.f, fcntl.LOCK_EX)
        self.f.seek(0)
        self.f.truncate()
        self.f.write(json.dumps({"pid": os.getpid(), "since": time.strftime("%Y-%m-%d %H:%M:%S"), "what": self.what}))
        self.f.flush()
        return self

    def __exit__(self, *exc):
        if self.f:
            try:
                self.f.seek(0)
                self.f.truncate()
                self.f.flush()
                fcntl.flock(self.f, fcntl.LOCK_UN)
            finally:
                self.f.close()
                self.f = None
        return False


# ── what the server said ──────────────────────────────────────────────────


def read_server_log(path):
    """The facts of a llama-server log worth keeping with a result."""
    try:
        text = Path(path).read_text(errors="replace")
    except OSError:
        return {}
    facts = {}
    patterns = {
        "n_slots": r"n_slots = (\d+)",
        "n_ctx_slot": r"n_ctx_slot = (\d+)",
        "kv_unified": r"kv_unified = '(\w+)'",
        "n_ctx": r"n_ctx\s*=\s*(\d+)",
        "n_ctx_seq": r"n_ctx_seq\s*(?:\(n_ctx_slot\))?\s*=\s*(\d+)",
        "n_parallel": r"n_seq_max\s*=\s*(\d+)",
        "offloaded": r"offloaded (\d+/\d+) layers to GPU",
        "kv_mib": r"KV buffer size\s*=\s*([\d.]+) MiB",
        "model_mib": r"CUDA0 model buffer size\s*=\s*([\d.]+) MiB",
        "compute_mib": r"CUDA0 compute buffer size\s*=\s*([\d.]+) MiB",
        "chat_template": r"chat template, chat_template: (.{0,60})",
    }
    for key, pat in patterns.items():
        m = re.findall(pat, text)
        if m:
            facts[key] = m[-1]
    return facts


# ── the wrapper (runs in place of llama-server, started by the engine) ─────


def rewrite_args(args, cfg):
    """The engine's llama-server arguments, with the bench's slots, context and template."""
    out, i, port = [], 0, None
    total_ctx = str(cfg["parallel"] * cfg["slot_context"])
    while i < len(args):
        a = args[i]
        nxt = args[i + 1] if i + 1 < len(args) else None
        if a in ("-c", "--ctx-size") and nxt is not None:
            out += [a, total_ctx]
            i += 2
        elif a in ("-np", "--parallel") and nxt is not None:
            out += [a, str(cfg["parallel"])]
            i += 2
        elif a == "--chat-template-file" and nxt is not None:
            if cfg["template"] == "keep":
                out += [a, nxt]
            elif cfg["template"] != "drop":
                out += [a, cfg["template"]]
            i += 2
        else:
            if a == "--port" and nxt is not None:
                port = int(nxt)
            out.append(a)
            i += 1
    return out + list(cfg.get("extra_args") or []), port


def server_env():
    env = dict(os.environ)
    for key in ("LD_PRELOAD", "FAKETIME", "FAKETIME_DONT_FAKE_MONOTONIC", "NO_FAKE_STAT"):
        env.pop(key, None)
    return env


def _die_with_parent():
    try:
        import ctypes
        import signal
        ctypes.CDLL("libc.so.6", use_errno=True).prctl(1, signal.SIGTERM)  # PR_SET_PDEATHSIG
    except Exception:
        pass


def wrap_main():
    cfg = json.loads(Path(sys.argv[1]).read_text())
    args, port = rewrite_args(sys.argv[2:], cfg)
    server = cfg["server"]
    Path(cfg["argv_out"]).write_text(json.dumps({"engine": sys.argv[2:], "server": args}, indent=1))
    if not cfg.get("capture"):
        os.execve(server, [server] + args, server_env())
    _proxy_main(server, args, port, Path(cfg["capture"]))


def _free_port():
    import socket
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def assemble_sse(raw):
    """An Anthropic Messages stream, put back together: content blocks, stop reason, usage."""
    msg = {"content": [], "usage": {}}
    blocks = {}
    for chunk in raw.split("\n\n"):
        data = "".join(line[5:].strip() for line in chunk.splitlines() if line.startswith("data:"))
        if not data:
            continue
        try:
            ev = json.loads(data)
        except json.JSONDecodeError:
            continue
        t = ev.get("type")
        if t == "message_start":
            m = ev.get("message", {})
            msg["usage"].update(m.get("usage") or {})
            msg["model"] = m.get("model")
        elif t == "content_block_start":
            b = dict(ev.get("content_block") or {})
            b.setdefault("text", "") if b.get("type") == "text" else None
            b["_json"] = ""
            blocks[ev.get("index", len(blocks))] = b
        elif t == "content_block_delta":
            b = blocks.setdefault(ev.get("index", 0), {"type": "text", "text": "", "_json": ""})
            d = ev.get("delta") or {}
            if d.get("type") == "text_delta":
                b["text"] = b.get("text", "") + d.get("text", "")
            elif d.get("type") == "input_json_delta":
                b["_json"] += d.get("partial_json", "")
            elif d.get("type") == "thinking_delta":
                b["thinking"] = b.get("thinking", "") + d.get("thinking", "")
        elif t == "message_delta":
            msg["stop_reason"] = (ev.get("delta") or {}).get("stop_reason")
            msg["usage"].update(ev.get("usage") or {})
    for i in sorted(blocks):
        b = blocks[i]
        js = b.pop("_json", "")
        if js:
            try:
                b["input"] = json.loads(js)
            except json.JSONDecodeError:
                b["input_raw"] = js
        msg["content"].append(b)
    return msg


def _proxy_main(server, args, port, capture):
    """Run llama-server on a port of its own and stand on the engine's port in front of it,
    writing each request and its answer to capture/requests.jsonl."""
    import http.client
    import http.server
    import signal
    import threading

    backend = _free_port()
    args = [str(backend) if (i > 0 and args[i - 1] == "--port") else a for i, a in enumerate(args)]
    # For counting tokens on the server itself once the tasks are done (seen.py): its port and key.
    endpoint = capture / "endpoint.json"
    fd = os.open(endpoint, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w") as f:
        json.dump({"port": backend, "key": os.environ.get("LLAMA_API_KEY")}, f)
    child = subprocess.Popen([server] + args, env=server_env(), preexec_fn=_die_with_parent)
    out_lock = threading.Lock()
    out_path = capture / "requests.jsonl"

    def stop(*_):
        child.terminate()
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            child.kill()
        os._exit(0)

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    threading.Thread(target=lambda: (child.wait(), os._exit(child.returncode or 0)), daemon=True).start()

    class Proxy(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, *a):
            pass

        def _forward(self):
            n = int(self.headers.get("content-length") or 0)
            body = self.rfile.read(n) if n else b""
            started = time.time()
            headers = {k: v for k, v in self.headers.items() if k.lower() not in ("host", "connection", "content-length")}
            try:
                conn = http.client.HTTPConnection("127.0.0.1", backend, timeout=900)
                conn.request(self.command, self.path, body=body or None, headers=headers)
                resp = conn.getresponse()
            except OSError:
                self.send_response(503)
                self.send_header("content-length", "0")
                self.end_headers()
                return
            self.send_response(resp.status)
            for k, v in resp.getheaders():
                if k.lower() not in ("transfer-encoding", "connection", "content-length"):
                    self.send_header(k, v)
            self.send_header("transfer-encoding", "chunked")
            self.end_headers()
            raw = bytearray()
            gone = False  # the client hung up (the engine's health check reads a status line and goes)
            while True:
                piece = resp.read1(65536)
                if not piece:
                    break
                raw += piece
                if not gone:
                    try:
                        self.wfile.write(b"%x\r\n%s\r\n" % (len(piece), piece))
                        self.wfile.flush()
                    except OSError:
                        gone = True
            if not gone:
                try:
                    self.wfile.write(b"0\r\n\r\n")
                    self.wfile.flush()
                except OSError:
                    pass
            conn.close()
            if self.command == "POST":
                try:
                    request = json.loads(body)
                except (json.JSONDecodeError, UnicodeDecodeError):
                    request = body.decode(errors="replace")
                text = raw.decode(errors="replace")
                is_sse = "event:" in text[:200] or "data:" in text[:200]
                try:
                    answer = assemble_sse(text) if is_sse else json.loads(text)
                except json.JSONDecodeError:
                    answer = text[:4000]
                record = {"t": started, "seconds": round(time.time() - started, 3), "path": self.path,
                          "status": resp.status, "request": request, "response": answer}
                with out_lock, open(out_path, "a") as f:
                    f.write(json.dumps(record) + "\n")

        do_GET = do_POST = _forward

    httpd = http.server.ThreadingHTTPServer(("127.0.0.1", port), Proxy)
    httpd.daemon_threads = True
    httpd.serve_forever()
