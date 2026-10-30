"""Tasks × runs against the throwaway engine (contract: "A task").

Each task-run is a real thread on the local model: `thread.create` with the prompt, its followups
with `thread.send`, each turn waited for by polling `thread.get` (only the messages after the last
one seen) until the thread stops working, then graded and deleted. Read-only tasks run side by side
up to the server's slot count; a task that `mutates` runs alone and the ledger is restored from the
snapshot after it. A read-only task whose agent changed the ledger anyway is restored too, once the
tasks beside it are done, and those are flagged.
"""

import fnmatch
import glob
import gzip
import hashlib
import json
import os
import threading
import time
import traceback
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from . import MODEL_ID, TASKS_DIR, TODAY, log
from . import fixtures
from . import grade as grading
from .engine import OpError

#: Ops an agent may call that change the ledger (or Arbiter); a read-only task that called one
#: successfully left the world changed.
MUTATING_OPS = {"money.tx.add", "money.tx.update", "money.tx.restore", "money.invest.add",
                "arbiter.propose", "arbiter.order.place", "arbiter.halt"}
TURN_TIMEOUT = 240.0
POLL = 0.5
#: Kept of each tool answer in the result file (the raw transcript keeps them whole).
RESULT_TEXT = 4000
#: Consecutive task-runs ending in the same error before the run gives up.
BREAKER = 6


# ── tasks ────────────────────────────────────────────────────────────────


def split_of(task_id):
    """`holdout` for ≈30% of tasks, by their id alone, stable forever; `dev` for the rest."""
    return "holdout" if int(hashlib.sha1(task_id.encode()).hexdigest(), 16) % 10 < 3 else "dev"


#: Suites `all` leaves out: the smoke starter repeats questions the suite asks.
NOT_IN_ALL = {"smoke"}
#: The second suite, in its own world (worlds/v2): tasks/v2/*.json.
V2 = "v2"
#: The third, harder suite, asked in the v2 world: tasks/v3/*.json.
V3 = "v3"
#: Each suite with a folder of its own, and the world its tasks are asked in.
WORLD_OF_SUITE = {V2: V2, V3: V2}


def suite_files(suite, ids=None):
    """`all` (every tasks/*.json but the smoke starter), a suite name (tasks/<name>.json), a comma
    list of names, or paths. `v2` is every tasks/v2/*.json but its smoke starter, which joins it when
    `ids` name a `v2.smoke.` task; `v2/<name>` is one of them; `v3` likewise for tasks/v3/."""
    if suite in (None, "", "all"):
        return sorted(Path(p) for p in glob.glob(str(TASKS_DIR / "*.json")) if Path(p).stem not in NOT_IN_ALL)
    out = []
    for part in suite.split(","):
        part = part.strip()
        p = Path(part)
        if part in WORLD_OF_SUITE:
            smoke = any(i.strip().startswith(f"{part}.smoke") for chunk in (ids or []) for i in chunk.split(","))
            out += sorted(Path(f) for f in glob.glob(str(TASKS_DIR / part / "*.json")) if smoke or Path(f).stem not in NOT_IN_ALL)
        else:
            out.append(p if p.suffix == ".json" and p.exists() else TASKS_DIR / f"{part}.json")
    return out


def suite_name(path):
    """`tally` for tasks/tally.json, `v2/tally` for tasks/v2/tally.json (and v3's the same way)."""
    path = Path(path).resolve()
    own = any(path.parent == (TASKS_DIR / suite).resolve() for suite in WORLD_OF_SUITE)
    return f"{path.parent.name}/{path.stem}" if own else path.stem


def world_of(tasks):
    """The world the tasks are asked in: `v2` for tasks/v2/ and tasks/v3/, `v1` (the sample household)
    otherwise. Raises when they are from both."""
    worlds = {WORLD_OF_SUITE.get((t.get("suite") or "").split("/")[0], "v1") if "/" in (t.get("suite") or "") else "v1" for t in tasks}
    if len(worlds) > 1:
        raise ValueError("these tasks are from two worlds (v1 and v2): run them as separate suites")
    return worlds.pop() if worlds else "v1"


def load_tasks(suite="all", ids=None, split="all"):
    tasks, seen = [], {}
    for path in suite_files(suite, ids):
        for task in json.loads(Path(path).read_text()):
            if task["id"] in seen:
                raise ValueError(f"task {task['id']} is in both {seen[task['id']]} and {path.name}")
            seen[task["id"]] = path.name
            task = dict(task, suite=suite_name(path), split=split_of(task["id"]))
            tasks.append(task)
    if ids:
        pats = [p.strip() for chunk in ids for p in chunk.split(",") if p.strip()]
        tasks = [t for t in tasks if any(fnmatch.fnmatchcase(t["id"], p) for p in pats)]
    if split in ("dev", "holdout"):
        tasks = [t for t in tasks if t["split"] == split]
    return tasks


# ── reading a thread ─────────────────────────────────────────────────────


def op_names(relay_bin):
    """Tool name as Claude Code reports it → the bus op, from the bus's own schema."""
    import subprocess
    try:
        r = subprocess.run([str(relay_bin), "schema"], capture_output=True, text=True, timeout=60)
        doc = json.loads(r.stdout)
        names = set()
        for key in ("ops", "operations"):
            v = doc.get(key)
            if isinstance(v, dict):
                names |= set(v)
            elif isinstance(v, list):
                names |= {o.get("name") or o.get("op") for o in v if isinstance(o, dict)}
        if not names:
            names = {m for m in _walk_strings(doc) if "." in m and m.replace(".", "").replace("_", "").isalnum()}
    except Exception:
        names = set()
    return {grading.op_of(n): n for n in names if n}


def _walk_strings(node):
    if isinstance(node, dict):
        for k, v in node.items():
            yield k
            yield from _walk_strings(v)
    elif isinstance(node, list):
        for v in node:
            yield from _walk_strings(v)


def turns_of(messages, ops):
    """A thread's stored messages → turns, each with its reply text and tool calls."""
    turns = []
    calls_by_id = {}
    for m in messages:
        role, body = m["role"], m.get("body") or {}
        if role == "user":
            turns.append({"prompt": body.get("text", ""), "texts": [], "tool_calls": [], "error": None})
            continue
        if not turns:
            turns.append({"prompt": None, "texts": [], "tool_calls": [], "error": None})
        turn = turns[-1]
        if role == "assistant":
            for b in body.get("blocks") or []:
                if b.get("type") == "text":
                    turn["texts"].append(b.get("text", ""))
                elif b.get("type") == "tool_use":
                    name = b.get("name", "")
                    call = {"id": b.get("id"), "name": name, "op": ops.get(grading.op_of(name), grading.op_of(name)),
                            "input": b.get("input"), "result": None, "is_error": None}
                    turn["tool_calls"].append(call)
                    calls_by_id[call["id"]] = call
                    turn["texts"].append(None)  # where the call stood
        elif role == "tool":
            call = calls_by_id.get(body.get("tool_use_id"))
            if call is not None:
                call["result"] = body.get("text", "")
                call["is_error"] = bool(body.get("is_error"))
        elif role == "error":
            turn["error"] = body.get("text", "")
    for t in turns:
        texts = t.pop("texts")
        t["text"] = "\n\n".join(x for x in texts if x)
        last_call = max((i for i, x in enumerate(texts) if x is None), default=-1)
        t["final"] = "\n\n".join(x for x in texts[last_call + 1:] if x)
    return turns


# ── token usage from Claude Code's own transcripts ────────────────────────


class Transcripts:
    """Claude Code writes each conversation to CLAUDE_CONFIG_DIR/projects/<dir>/<session>.jsonl,
    every model answer with the usage the server reported. A task-run's file is the one that
    started with its prompt, written since it began."""

    def __init__(self, claude_dir):
        self.root = Path(claude_dir) / "projects"
        self.claimed = set()
        self.lock = threading.Lock()

    @staticmethod
    def _first_prompt(path):
        try:
            with open(path) as f:
                for line in f:
                    try:
                        ev = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    if ev.get("type") == "user":
                        c = (ev.get("message") or {}).get("content")
                        if isinstance(c, list):
                            c = "".join(p.get("text", "") for p in c if isinstance(p, dict))
                        return c
        except OSError:
            return None
        return None

    def usage(self, prompt, since):
        with self.lock:
            found = None
            for path in sorted(self.root.glob("*/*.jsonl"), key=lambda p: p.stat().st_mtime):
                if path in self.claimed or path.stat().st_mtime < since - 2:
                    continue
                # A thread on a local model may start with Relay's data ahead of the person's
                # words (prefetch), so the first prompt ends with them rather than being them.
                if (self._first_prompt(path) or "").strip().endswith(prompt.strip()):
                    found = path
                    break
            if not found:
                return None
            self.claimed.add(found)
        calls = {}
        with open(found) as f:
            for line in f:
                try:
                    ev = json.loads(line)
                except json.JSONDecodeError:
                    continue
                msg = ev.get("message") or {}
                if ev.get("type") == "assistant" and msg.get("usage"):
                    calls[msg.get("id") or len(calls)] = msg["usage"]
        ins = [u.get("input_tokens", 0) + u.get("cache_read_input_tokens", 0) + u.get("cache_creation_input_tokens", 0) for u in calls.values()]
        outs = [u.get("output_tokens", 0) for u in calls.values()]
        return {"api_calls": len(calls), "input_tokens": sum(ins), "output_tokens": sum(outs),
                "max_input_tokens": max(ins, default=0), "transcript": found.name}


# ── side by side, or alone ────────────────────────────────────────────────


class Gate:
    """Read-only task-runs share the ledger; a restore (or a task that mutates) has it alone."""

    def __init__(self):
        self.cond = threading.Condition()
        self.shared = 0
        self.exclusive = False

    def enter(self):
        with self.cond:
            self.cond.wait_for(lambda: not self.exclusive)
            self.shared += 1

    def leave(self):
        with self.cond:
            self.shared -= 1
            self.cond.notify_all()

    def alone(self):
        gate = self

        class _Alone:
            def __enter__(self):
                with gate.cond:
                    gate.cond.wait_for(lambda: not gate.exclusive)
                    gate.exclusive = True
                    gate.cond.wait_for(lambda: gate.shared == 0)

            def __exit__(self, *exc):
                with gate.cond:
                    gate.exclusive = False
                    gate.cond.notify_all()
                return False

        return _Alone()


class Runner:
    def __init__(self, engine, tasks, *, truths, runs=1, parallel=1, currency="CAD", fingerprint=None,
                 turn_timeout=TURN_TIMEOUT, poll=POLL, raw_path=None, model=MODEL_ID):
        self.engine = engine
        self.tasks = tasks
        self.truths = truths
        self.runs = runs
        self.parallel = max(1, parallel)
        self.ctx = {"today": TODAY, "currency": currency}
        self.fingerprint = fingerprint
        self.turn_timeout = turn_timeout
        self.poll = poll
        self.model = model
        self.ops = op_names(engine.relay_bin)
        self.transcripts = Transcripts(engine.claude_dir)
        self.gate = Gate()
        self.raw = gzip.open(raw_path, "at") if raw_path else None
        self.raw_lock = threading.Lock()
        self.active = {}
        self.active_lock = threading.Lock()
        self.records = []
        self.done = 0
        self.total = 0
        self.broken = None
        self.streak = (None, 0)
        self.streak_lock = threading.Lock()

    def q(self, op, payload=None):
        return self.engine.q(op, payload)

    # ── one task-run ─────────────────────────────────────────────────────

    def _truth(self, name, ctx):
        fn = self.truths.get(name)
        if fn is None:
            raise KeyError(f"unknown truth {name!r}")
        return fn(self.q, ctx)

    def _wait(self, thread_id, after, started):
        """Poll until the thread stops working. Returns (new messages, timed_out)."""
        got = []
        deadline = started + self.turn_timeout
        while True:
            r = self.q("thread.get", {"id": thread_id, "after": after})
            if r["messages"]:
                got += r["messages"]
                after = r["messages"][-1]["id"]
            if not r["thread"]["working"]:
                return got, False
            if time.monotonic() >= deadline:
                try:
                    self.q("thread.stop", {"id": thread_id})
                except OpError:
                    pass
                r = self.q("thread.get", {"id": thread_id, "after": after})
                return got + r["messages"], True
            time.sleep(self.poll)

    def run_task(self, task, run):
        rec = {k: task.get(k) for k in ("id", "suite", "source", "category", "difficulty", "split", "mutates")}
        rec.update({"run": run, "pass": False, "checks": [], "turns": [], "error": None, "timeout": False})
        ctx = dict(self.ctx)
        try:
            expected = self._truth(task["truth"], ctx) if task.get("truth") else {}
        except Exception as e:
            rec["error"] = f"truth {task.get('truth')}: {type(e).__name__}: {e}"
            rec["truth_error"] = True
            return rec
        rec["expected"] = expected
        effect_checks = {i: c for i, c in enumerate(task.get("checks") or []) if c.get("type") == "effect"}
        before = {}
        for i, c in effect_checks.items():
            try:
                before[i] = self._truth(c["truth"], ctx)
            except Exception as e:
                before[i] = {"__error__": f"{type(e).__name__}: {e}"}

        prompts = [task["prompt"]] + list(task.get("followups") or [])
        messages, seconds, timed_out = [], [], False
        wall = time.time()
        thread_id = None
        try:
            for n, text in enumerate(prompts):
                started = time.monotonic()
                if n == 0:
                    thread = self.q("thread.create", {"text": text, "model": self.model})
                    thread_id = thread["id"]
                    after = None
                else:
                    self.q("thread.send", {"id": thread_id, "text": text})
                got, timed_out = self._wait(thread_id, after, started)
                seconds.append(round(time.monotonic() - started, 2))
                if thread_id is not None:
                    full = self.q("thread.get", {"id": thread_id})["messages"]
                    messages = full
                    after = full[-1]["id"] if full else None
                if timed_out or any(m["role"] == "error" for m in got):
                    break
        except OpError as e:
            rec["error"] = f"{e.op}: {e.code}: {e.message}"
        turns = turns_of(messages, self.ops)
        for t, s in zip(turns, seconds):
            t["seconds"] = s
        thread = {"turns": turns, "error": rec["error"] or next((t["error"] for t in turns if t["error"]), None),
                  "timeout": timed_out}
        rec["error"] = thread["error"]
        rec["timeout"] = timed_out
        rec["seconds"] = round(sum(seconds), 2)
        rec["turn_seconds"] = seconds
        calls = [c for t in turns for c in t["tool_calls"]]
        rec["tool_calls"] = len(calls)
        rec["ops"] = [c["op"] for c in calls]
        rec["mutated"] = any(c["op"] in MUTATING_OPS and c["is_error"] is False for c in calls)

        effects = {}
        for i, c in effect_checks.items():
            if "__error__" in before[i]:
                effects[i] = {"error": before[i]["__error__"]}
                continue
            try:
                effects[i] = self._truth(c["truth"], dict(ctx, before=before[i], thread=thread))
            except Exception as e:
                effects[i] = {"error": f"{type(e).__name__}: {e}"}
        rec["pass"], rec["checks"] = grading.grade(task, expected, thread, effects)
        rec["final_text"] = turns[-1]["text"] if turns else ""
        rec["turns"] = [{"prompt": t["prompt"], "text": t["text"], "seconds": t.get("seconds"), "error": t["error"],
                         "tool_calls": [{"op": c["op"], "input": c["input"], "is_error": c["is_error"],
                                         "result": (c["result"] or "")[:RESULT_TEXT]} for c in t["tool_calls"]]}
                        for t in turns]
        rec["tokens"] = self.transcripts.usage(task["prompt"], wall)
        rec["thread_id"] = thread_id
        with self.raw_lock:
            if self.raw:
                self.raw.write(json.dumps({"id": task["id"], "run": run, "messages": messages, "tokens": rec["tokens"]}) + "\n")
        if thread_id is not None:
            try:
                self.q("thread.delete", {"id": thread_id})
            except OpError:
                pass
        return rec

    # ── scheduling ───────────────────────────────────────────────────────

    def _restore(self, why):
        fixtures.restore(self.engine, self.fingerprint)
        log(f"  ledger restored ({why})")

    def _note(self, rec):
        with self.streak_lock:
            self.done += 1
            self.records.append(rec)
            err = rec.get("error") if not rec.get("pass") else None
            key = (err or "")[:80] if err and not rec.get("tool_calls") else None
            last, n = self.streak
            self.streak = (key, n + 1 if key and key == last else (1 if key else 0))
            if self.streak[1] >= BREAKER and not self.broken:
                self.broken = f"{BREAKER} task-runs in a row ended with: {err}"
                log(f"!! giving up: {self.broken}")
        flag = "PASS" if rec["pass"] else ("TIMEOUT" if rec.get("timeout") else ("ERROR" if rec.get("error") else "fail"))
        failed = [c["type"] for c in rec.get("checks", []) if not c["pass"]]
        tok = rec.get("tokens") or {}
        log(f"[{self.done}/{self.total}] {rec['id']} #{rec['run']}: {flag} {rec.get('seconds', 0):.1f}s "
            f"{rec.get('tool_calls', 0)} tools {tok.get('input_tokens', '?')}/{tok.get('output_tokens', '?')} tok"
            + (f"  failed: {failed}" if failed else "") + (f"  ({rec['error'][:120]})" if rec.get("error") else ""))

    def _shared(self, task, run):
        if self.broken:
            return
        self.gate.enter()
        with self.active_lock:
            self.active[(task["id"], run)] = None
        try:
            rec = self._safe(task, run)
        finally:
            with self.active_lock:
                self.active.pop((task["id"], run), None)
                others = list(self.active)
            self.gate.leave()
        if rec.get("mutated"):
            rec["dirty_others"] = [f"{i}#{r}" for i, r in others]
            with self.gate.alone():
                self._restore(f"{task['id']} changed it")
        self._note(rec)

    def _alone(self, task, run):
        if self.broken:
            return
        with self.gate.alone():
            rec = self._safe(task, run)
            self._restore(f"after {task['id']}")
        self._note(rec)

    def _safe(self, task, run):
        try:
            return self.run_task(task, run)
        except Exception as e:
            return {**{k: task.get(k) for k in ("id", "suite", "source", "category", "difficulty", "split", "mutates")},
                    "run": run, "pass": False, "checks": [], "turns": [], "timeout": False,
                    "error": f"bench: {type(e).__name__}: {e}", "trace": traceback.format_exc()[-2000:]}

    def run(self):
        shared = [(t, r) for r in range(1, self.runs + 1) for t in self.tasks if not t.get("mutates")]
        alone = [(t, r) for r in range(1, self.runs + 1) for t in self.tasks if t.get("mutates")]
        self.total = len(shared) + len(alone)
        pool = ThreadPoolExecutor(max_workers=self.parallel)
        try:
            for f in [pool.submit(self._shared, t, r) for t, r in shared]:
                f.result()
            pool.shutdown()
            for t, r in alone:
                self._alone(t, r)
        except KeyboardInterrupt:
            # No new task starts; the ones running end when the engine is stopped.
            self.broken = "interrupted"
            pool.shutdown(wait=False, cancel_futures=True)
        finally:
            if self.raw:
                with self.raw_lock:
                    self.raw.close()
                    self.raw = None
        order = {(t["id"], r): i for i, (t, r) in enumerate(shared + alone)}
        self.records.sort(key=lambda rec: order.get((rec["id"], rec["run"]), 1 << 30))
        return self.records
