"""Checks: does a thread's answer hold what the task's truth says? (contract: "A task")

A task passes when every check passes; each check is also scored on its own. Grading never calls
the engine: the runner hands in the expected values (from the truth function, before the turn), the
outcome of `effect` checks (the truth function after the turn), and the thread, collected as

    {"turns": [{"prompt", "text", "tool_calls": [{"op", "name", "input", "result", "is_error"}],
                "error", "timeout", "seconds"}],
     "error": str | None, "timeout": bool}

Check types: number, text_any, text_all, text_none, tool, no_tool, effect, chart, no_error. A task
may also set `max_tools` (an integer): it then fails when the thread made more tool calls than
that, scored as a check of its own type `max_tools`.
Common keys: `turn` ("last" (default for answers), "all", or an index) picks which reply a text,
number or chart check reads; tool checks read every turn unless given one.
"""

import json
import re

CHECK_TYPES = ("number", "text_any", "text_all", "text_none", "tool", "no_tool", "effect", "chart", "no_error")
TOOL_PREFIX = "mcp__relay__"

# ── reading numbers ──────────────────────────────────────────────────────

_FENCE = re.compile(r"```(chart|import)\b.*?```", re.S | re.I)
_DATE = re.compile(r"\b\d{4}-\d{2}-\d{2}(?:[T ]\d{2}:\d{2}(?::\d{2})?(?:\.\d+)?Z?)?\b")
_TIME = re.compile(r"\b\d{1,2}:\d{2}(?::\d{2})?\s?(?:[ap]\.?m\.?)?", re.I)
# A hyphen or the minus sign, right against the number or its currency: "- $45" is a list item.
_MINUS = "-−"
_NUMBER = re.compile(
    r"(?P<paren>\()?"
    rf"(?P<sign1>(?<![\w.])[{_MINUS}])?"
    r"(?:(?P<cur>(?:CA|C|US|A|CAD\s?|USD\s?)?\$\s?)"
    rf"(?P<sign2>[{_MINUS}])?)?"
    # "1,234.56" in English; "1 234,56" and "109,15" in French: a comma before one or two digits
    # (and nothing more) is a decimal comma, before three a thousands separator.
    r"(?<![\w.,])(?P<int>\d{1,3}(?:,\d{3})+|\d{1,3}(?:[ \u00a0\u202f]\d{3})+(?=,\d{1,2}(?![\d,]))|\d+)(?P<dec>\.\d+|,\d{1,2}(?![\d,]))?(?![\d])"
    r"(?P<suffix>\s?(?:k|K|M|mn)\b)?"
    r"(?P<close>\))?"
)


def numbers(text):
    """Every number in `text` as a float: "$1,234.56", "1234.56", "1,234.56 CAD", "-$12.30",
    "−12.30", "($12.30)" (negative), "$1.2k", "12.50%", and French "109,15 $" and "1 234,56 $". Dates and clock times are left out, and so
    are the JSON of ```chart and ```import blocks."""
    text = _FENCE.sub(" ", text or "")
    text = _DATE.sub(" ", text)
    text = _TIME.sub(" ", text)
    out = []
    for m in _NUMBER.finditer(text):
        whole = re.sub(r"[,\s\u00a0\u202f]", "", m.group("int"))
        value = float(whole + (m.group("dec") or "").replace(",", "."))
        suffix = (m.group("suffix") or "").strip()
        if suffix in ("k", "K"):
            value *= 1_000
        elif suffix in ("M", "mn"):
            value *= 1_000_000
        negative = bool(m.group("sign1") or m.group("sign2")) or bool(m.group("paren") and m.group("close") and m.group("cur"))
        out.append(-value if negative else value)
    return out


def money_text(value, digits=2):
    """1234.5 -> "1,234.50", for messages."""
    return f"{value:,.{digits}f}"


# ── helpers ──────────────────────────────────────────────────────────────


def op_of(name):
    """A tool name as Claude Code reports it ("mcp__relay__money_tx_add") → a comparable key."""
    name = name or ""
    if name.startswith(TOOL_PREFIX):
        name = name[len(TOOL_PREFIX):]
    return name.replace(".", "_").lower()


def _as_list(v):
    if v is None:
        return []
    return list(v) if isinstance(v, (list, tuple)) else [v]


def _num(v):
    if isinstance(v, bool):
        return None
    if isinstance(v, (int, float)):
        return float(v)
    if isinstance(v, str):
        s = v.strip().replace(",", "").lstrip("$")
        try:
            return float(s)
        except ValueError:
            return None
    return None


def resolve(value, expected):
    """`{"$truth": "key"}` → the truth's value for key (anywhere inside `value`)."""
    if isinstance(value, dict):
        if set(value) == {"$truth"}:
            return expected.get(value["$truth"])
        return {k: resolve(v, expected) for k, v in value.items()}
    if isinstance(value, list):
        return [resolve(v, expected) for v in value]
    return value


def subset(want, got, tol=1e-6):
    """Does `got` hold `want`? Dicts by key, lists element by element (any order), numbers and
    numeric strings by value, other strings without case; a list in `want` where `got` has a
    scalar means any of them."""
    if isinstance(want, dict):
        return isinstance(got, dict) and all(k in got and subset(v, got[k], tol) for k, v in want.items())
    if isinstance(want, list):
        if isinstance(got, list):
            return all(any(subset(w, g, tol) for g in got) for w in want)
        return any(subset(w, got, tol) for w in want)
    if want is None:
        return got is None
    wn, gn = _num(want), _num(got)
    if wn is not None and gn is not None:
        return abs(wn - gn) <= tol
    if isinstance(want, str) and isinstance(got, str):
        return want.strip().lower() == got.strip().lower()
    return want == got


def reply_text(thread, turn="last"):
    turns = thread.get("turns") or []
    if not turns:
        return ""
    if turn == "all":
        return "\n\n".join(t.get("text") or "" for t in turns)
    if turn == "last":
        return turns[-1].get("text") or ""
    try:
        return turns[int(turn)].get("text") or ""
    except (IndexError, ValueError):
        return ""


def tool_calls(thread, turn=None):
    turns = thread.get("turns") or []
    if turn not in (None, "all"):
        turns = [turns[-1]] if turn == "last" else ([turns[int(turn)]] if -len(turns) <= int(turn) < len(turns) else [])
    return [c for t in turns for c in (t.get("tool_calls") or [])]


def chart_blocks(text):
    """Each ```chart block's JSON (or the error reading it)."""
    out = []
    for body in re.findall(r"```chart[ \t]*\n(.*?)```", text or "", re.S):
        try:
            out.append(json.loads(body))
        except json.JSONDecodeError as e:
            out.append({"__invalid__": str(e)})
    return out


# ── the checks ───────────────────────────────────────────────────────────


def _expected_values(check, expected):
    vals = []
    for key in _as_list(check.get("of")):
        vals += _as_list(expected.get(key)) if isinstance(key, str) else [key]
    for item in _as_list(check.get("any_of")):
        vals += _as_list(expected.get(item)) if isinstance(item, str) else [item]
    if "value" in check:
        vals += _as_list(check["value"])
    return [v for v in vals if _num(v) is not None]


def check_number(check, expected, thread):
    want = _expected_values(check, expected)
    if not want:
        return False, f"no expected value for {check.get('of') or check.get('any_of')}"
    tol = float(check.get("tol", 0.01))
    signed = bool(check.get("signed", False))
    seen = numbers(reply_text(thread, check.get("turn", "last")))
    for w in want:
        w = _num(w)
        for n in seen:
            if (abs(n - w) if signed else abs(abs(n) - abs(w))) <= tol + 1e-9:
                return True, f"found {n:g} (want {w:g})"
    shown = ", ".join(f"{n:g}" for n in seen[:12]) + ("…" if len(seen) > 12 else "")
    return False, f"want {' or '.join(f'{_num(w):g}' for w in want)} ±{tol:g}; the reply has [{shown}]"


def _patterns(check, expected):
    subs = []
    for v in _as_list(check.get("values", check.get("value"))):
        subs += [str(x) for x in _as_list(v)]
    for key in _as_list(check.get("of")):
        subs += [str(x) for x in _as_list(expected.get(key))]
    regexes = [re.compile(r, re.I | re.S) for r in _as_list(check.get("regex"))]
    return subs, regexes


def _hits(text, subs, regexes):
    low = text.lower()
    hits = [s for s in subs if s.lower() in low] + [r.pattern for r in regexes if r.search(text)]
    return hits


def check_text(check, expected, thread):
    kind = check["type"]
    subs, regexes = _patterns(check, expected)
    if not subs and not regexes:
        return False, "no text to look for"
    text = reply_text(thread, check.get("turn", "last"))
    hits = _hits(text, subs, regexes)
    every = subs + [r.pattern for r in regexes]
    if kind == "text_any":
        return bool(hits), (f"has {hits[0]!r}" if hits else f"none of {every}")
    if kind == "text_all":
        missing = [p for p in every if p not in hits]
        return not missing, ("has all" if not missing else f"missing {missing}")
    return not hits, ("none present" if not hits else f"has {hits}")


def _matching_calls(check, expected, thread):
    ops = {op_of(o) for o in _as_list(check.get("ops", check.get("op")))}
    args = resolve(check.get("args"), expected) if check.get("args") is not None else None
    calls = tool_calls(thread, check.get("turn"))
    found = [c for c in calls if (not ops or op_of(c.get("op") or c.get("name")) in ops)
             and (args is None or subset(args, c.get("input") or {}, float(check.get("tol", 1e-6))))]
    if check.get("ok_only"):
        found = [c for c in found if not c.get("is_error")]
    return found, calls


def check_tool(check, expected, thread):
    found, calls = _matching_calls(check, expected, thread)
    least = int(check.get("min", 1))
    called = [c.get("op") or c.get("name") for c in calls]
    if check["type"] == "tool":
        return len(found) >= least, f"{len(found)} matching of {called}"
    return not found, ("not called" if not found else f"called: {[c.get('op') for c in found]} {[c.get('input') for c in found][:2]}")


def check_chart(check, expected, thread):
    charts = chart_blocks(reply_text(thread, check.get("turn", "last")))
    if not charts:
        return False, "no chart block"
    valid = [c for c in charts if "__invalid__" not in c]
    if not valid:
        return False, f"chart JSON invalid: {charts[0]['__invalid__']}"
    wants = _as_list(check.get("any_of")) or [check.get("match", check.get("fields", {}))]
    wants = [resolve(w, expected) for w in wants]
    for c in valid:
        if any(subset(w, c) for w in wants):
            return True, "matches"
    return False, f"no chart matches {wants}: {json.dumps(valid[0])[:300]}"


def check_no_error(check, expected, thread):
    if thread.get("timeout"):
        return False, "timed out"
    if thread.get("error"):
        return False, f"error: {thread['error'][:200]}"
    return True, "ok"


def check_effect(check, effect):
    if effect is None:
        return False, "the effect was not evaluated"
    if "error" in effect and "ok" not in effect:
        return False, f"truth failed: {effect['error']}"
    return bool(effect.get("ok")), str(effect.get("why", ""))


def run_check(check, expected, thread, effect=None):
    """(passed, why) for one check."""
    kind = check.get("type")
    if kind == "number":
        return check_number(check, expected, thread)
    if kind in ("text_any", "text_all", "text_none"):
        return check_text(check, expected, thread)
    if kind in ("tool", "no_tool"):
        return check_tool(check, expected, thread)
    if kind == "chart":
        return check_chart(check, expected, thread)
    if kind == "no_error":
        return check_no_error(check, expected, thread)
    if kind == "effect":
        return check_effect(check, effect)
    return False, f"unknown check type {kind!r}"


def grade(task, expected, thread, effects=None):
    """(passed, [check results]) for a task. `effects[i]` is the effect truth's verdict for the
    i-th check when it is an `effect` check."""
    results = []
    for i, check in enumerate(task.get("checks") or []):
        try:
            ok, why = run_check(check, expected or {}, thread, (effects or {}).get(i))
        except Exception as e:  # a broken check fails itself, not the run
            ok, why = False, f"check crashed: {type(e).__name__}: {e}"
        results.append({"type": check.get("type"), "pass": bool(ok), "why": why,
                        **({"of": check["of"]} if "of" in check else {})})
    if task.get("max_tools") is not None:
        n, most = len(tool_calls(thread)), int(task["max_tools"])
        results.append({"type": "max_tools", "pass": n <= most, "why": f"{n} tool calls, at most {most}"})
    return all(r["pass"] for r in results) and bool(results), results


def validate_task(task, truths=None):
    """Problems with a task's shape (empty when it is fine)."""
    problems = []
    for key in ("id", "source", "category", "difficulty", "prompt", "checks"):
        if key not in task:
            problems.append(f"missing {key}")
    if task.get("source") not in ("tally", "avex", "both"):
        problems.append(f"source {task.get('source')!r}")
    if "max_tools" in task and not (isinstance(task["max_tools"], int) and not isinstance(task["max_tools"], bool) and task["max_tools"] >= 0):
        problems.append(f"max_tools {task['max_tools']!r}: a whole number of calls, 0 or more")
    if truths is not None and task.get("truth") and task["truth"] not in truths:
        problems.append(f"unknown truth {task['truth']!r}")
    for i, check in enumerate(task.get("checks") or []):
        kind = check.get("type")
        if kind not in CHECK_TYPES:
            problems.append(f"check {i}: unknown type {kind!r}")
        if kind == "effect" and truths is not None and check.get("truth") not in truths:
            problems.append(f"check {i}: unknown effect truth {check.get('truth')!r}")
        if kind == "number" and not (check.get("of") or check.get("any_of") or "value" in check):
            problems.append(f"check {i}: number check needs `of`, `any_of` or `value`")
        if kind in ("tool", "no_tool") and not (check.get("ops") or check.get("op")):
            problems.append(f"check {i}: {kind} needs `ops`")
    return problems
