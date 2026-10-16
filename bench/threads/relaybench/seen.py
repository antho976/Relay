"""What the model is shown, in its own tokens (README, "What the model sees").

From the requests a capture run recorded (`bench.py run --capture DIR`), while its llama-server is
still up: the server's own `/v1/messages/count_tokens` renders a request through the chat template
and counts it, so taking a part out and counting again gives that part's exact share. Tool answers
are counted with `/tokenize` on their text, as the door wrote it.
"""

import copy
import json
import urllib.request
from collections import defaultdict
from pathlib import Path

RELAY_PROMPT_MARK = "You are the agent in a Relay thread"
#: The readings whose size decides what to make easier to read first.
TYPICAL = ["money.summary", "money.tx.list", "money.lists", "money.invest.summary", "money.series",
           "gym.summary", "gym.lifts", "gym.sessions", "gym.lift.get"]


class Server:
    def __init__(self, port, key=None):
        self.base = f"http://127.0.0.1:{port}"
        self.key = key

    def post(self, path, body):
        req = urllib.request.Request(self.base + path, data=json.dumps(body).encode(), method="POST",
                                     headers={"content-type": "application/json",
                                              **({"authorization": f"Bearer {self.key}", "x-api-key": self.key} if self.key else {})})
        with urllib.request.urlopen(req, timeout=120) as r:
            return json.loads(r.read())

    def count(self, body):
        keep = {k: v for k, v in body.items() if k in ("model", "system", "messages", "tools", "tool_choice")}
        return self.post("/v1/messages/count_tokens", keep)["input_tokens"]

    def tokens(self, text):
        return len(self.post("/tokenize", {"content": text, "add_special": False})["tokens"])


def load(capture):
    out = []
    with open(Path(capture) / "requests.jsonl") as f:
        for line in f:
            rec = json.loads(line)
            if isinstance(rec.get("request"), dict) and rec["path"].startswith("/v1/messages") and "count_tokens" not in rec["path"]:
                out.append(rec)
    return out


def _system_blocks(body):
    s = body.get("system")
    if isinstance(s, str):
        return [{"type": "text", "text": s}]
    return list(s or [])


def _split_relay(body):
    """(system without Relay's prompt, Relay's prompt text)."""
    blocks = _system_blocks(body)
    rest, relay = [], ""
    for b in blocks:
        t = b.get("text", "")
        at = t.find(RELAY_PROMPT_MARK)
        if at >= 0:
            relay = t[at:]
            if t[:at].strip():
                rest.append(dict(b, text=t[:at]))
        else:
            rest.append(b)
    return rest, relay


def _tool_results(body):
    """(op, text) of each tool answer in the request's messages, with the call it answers."""
    names = {}
    out = []
    for m in body.get("messages") or []:
        content = m.get("content")
        if not isinstance(content, list):
            continue
        for b in content:
            if b.get("type") == "tool_use":
                names[b.get("id")] = (b.get("name"), b.get("input"))
            elif b.get("type") == "tool_result":
                c = b.get("content")
                text = c if isinstance(c, str) else "".join(p.get("text", "") for p in c or [] if isinstance(p, dict))
                name, args = names.get(b.get("tool_use_id"), (None, None))
                out.append((name, args, text))
    return out


def op_name(tool_name):
    n = (tool_name or "").removeprefix("mcp__relay__")
    return n


def measure(capture, server):
    reqs = load(capture)
    main = [r for r in reqs if r["request"].get("tools")]
    if not main:
        raise RuntimeError("no request with tools was captured")
    first = min(main, key=lambda r: len(r["request"].get("messages") or []))
    body = first["request"]
    rest, relay = _split_relay(body)
    tools = body.get("tools") or []
    user0 = body["messages"][0]
    full = server.count(body)
    no_tools = server.count({**body, "tools": []})
    no_relay = server.count({**body, "tools": [], "system": rest})
    bare = server.count({"model": body.get("model"), "messages": body["messages"][:1]})
    minimal = server.count({"model": body.get("model"), "messages": [{"role": "user", "content": "x"}]})
    per_tool = {}
    for t in tools:
        per_tool[t["name"]] = server.count({**body, "tools": [t]}) - no_tools
    # Relay's prompt, paragraph by paragraph: which parts of it cost what.
    sections = []
    for para in [p for p in relay.split("\n\n") if p.strip()]:
        sections.append({"tokens": server.tokens(para), "chars": len(para), "head": para.strip()[:90]})
    seen = {
        "relay_prompt_sections": sections,
        "request": {"path": first["path"], "model": body.get("model"), "messages": len(body.get("messages") or []),
                    "tools": len(tools), "reported_input_tokens": (first.get("response") or {}).get("usage", {}).get("input_tokens")},
        "total_first_request": full,
        "claude_code_system": no_relay - bare,
        "relay_prompt": no_tools - no_relay,
        "tool_schemas": full - no_tools,
        "first_user_message": bare - minimal,
        "relay_prompt_chars": len(relay),
        "tool_schema_chars": len(json.dumps(tools)),
        "per_tool": dict(sorted(per_tool.items(), key=lambda kv: -kv[1])),
        "system_blocks": [{"chars": len(b.get("text", "")), "head": b.get("text", "")[:80]} for b in _system_blocks(body)],
        "first_user_blocks": ([{"chars": len(b.get("text", "")), "head": b.get("text", "")[:80]} for b in user0["content"]]
                              if isinstance(user0.get("content"), list) else [{"chars": len(user0.get("content") or "")}]),
    }
    # Tool answers: every distinct one captured, by op, with its tokens.
    answers = defaultdict(list)
    done = set()
    for r in reqs:
        for name, args, text in _tool_results(r["request"]):
            key = (name, json.dumps(args, sort_keys=True), len(text))
            if key in done:
                continue
            done.add(key)
            answers[op_name(name)].append({"args": args, "chars": len(text), "tokens": server.tokens(text)})
    seen["tool_results"] = {op: {"n": len(v), "median_tokens": sorted(x["tokens"] for x in v)[len(v) // 2],
                                 "max_tokens": max(x["tokens"] for x in v), "samples": v[:6]}
                            for op, v in sorted(answers.items())}
    # Every request the agent made, by kind: the thread's turns, and anything else Claude Code asked.
    kinds = defaultdict(lambda: {"n": 0, "input_tokens": 0, "output_tokens": 0, "seconds": 0.0})
    for r in reqs:
        kind = "with tools" if r["request"].get("tools") else "no tools"
        u = (r.get("response") or {}).get("usage") or {}
        k = kinds[kind]
        k["n"] += 1
        k["input_tokens"] += u.get("input_tokens", 0) or 0
        k["output_tokens"] += u.get("output_tokens", 0) or 0
        k["seconds"] += r.get("seconds", 0)
    seen["requests"] = dict(kinds)
    side = [r for r in reqs if not r["request"].get("tools")]
    seen["side_requests"] = [{"system_head": (_system_blocks(r["request"]) or [{}])[0].get("text", "")[:120],
                              "max_tokens": r["request"].get("max_tokens"), "seconds": r.get("seconds")} for r in side[:5]]
    (Path(capture) / "seen.json").write_text(json.dumps(seen, indent=1))
    return seen


class OfflineServer:
    """A CPU-only llama-server for counting tokens without the GPU: the GGUF's tokenizer and the
    chat template are all count_tokens needs. CUDA sees no device, so the card is not touched."""

    def __init__(self, gguf, template=None, server=None):
        self.gguf, self.template, self.server = gguf, template, server
        self.proc = None

    def __enter__(self):
        import os
        import socket
        import subprocess
        import time
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        args = [str(self.server), "-m", str(self.gguf), "--host", "127.0.0.1", "--port", str(port), "-c", "32768",
                "-np", "1", "--jinja", "--reasoning", "off", "--no-webui", "-ngl", "0", "--device", "none"]
        if self.template:
            args += ["--chat-template-file", str(self.template)]
        env = {k: v for k, v in os.environ.items() if k not in ("LD_PRELOAD", "LLAMA_API_KEY")}
        env["CUDA_VISIBLE_DEVICES"] = ""
        self.proc = subprocess.Popen(args, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        srv = Server(port)
        deadline = time.monotonic() + 180
        while time.monotonic() < deadline:
            if self.proc.poll() is not None:
                raise RuntimeError(f"llama-server exited ({self.proc.returncode}) before it loaded")
            try:
                with urllib.request.urlopen(srv.base + "/health", timeout=2) as r:
                    if r.status == 200:
                        return srv
            except OSError:
                pass
            time.sleep(0.5)
        raise RuntimeError("the CPU llama-server did not load within 180 s")

    def __exit__(self, *exc):
        if self.proc and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=15)
            except Exception:
                self.proc.kill()
                self.proc.wait(timeout=5)
        return False


def format_seen(seen):
    lines = [
        f"first request of a thread: {seen['total_first_request']} tokens "
        f"(server reported {seen['request']['reported_input_tokens']}), {seen['request']['tools']} tools",
        f"  Claude Code's own system prompt   {seen['claude_code_system']:>6}",
        f"  Relay's appended PROMPT           {seen['relay_prompt']:>6}  ({seen['relay_prompt_chars']} chars)",
        f"  the tool schemas                  {seen['tool_schemas']:>6}  ({seen['tool_schema_chars']} chars of JSON)",
        f"  the first user message            {seen['first_user_message']:>6}  (Claude Code's reminders + the question)",
        "largest tool schemas (each with the template's ~200-token tool preamble): "
        + ", ".join(f"{k.removeprefix('mcp__relay__')} {v}" for k, v in list(seen["per_tool"].items())[:10]),
        "Relay's prompt by paragraph (largest first): "
        + "; ".join(f"{x['tokens']} «{x['head'][:40]}»" for x in sorted(seen.get("relay_prompt_sections", []), key=lambda x: -x["tokens"])[:8]),
        "tool answers (tokens, median / max, distinct calls):",
    ]
    for op, v in seen["tool_results"].items():
        lines.append(f"  {op:<28} {v['median_tokens']:>6} / {v['max_tokens']:<6} n={v['n']}")
    for kind, v in seen["requests"].items():
        lines.append(f"requests {kind}: {v['n']}, {v['input_tokens']} tokens in, {v['output_tokens']} out, {v['seconds']:.0f}s")
    return "\n".join(lines)
