"""Summaries of a result file, and the comparison of two (contract: "Results").

Pass rates overall and by split, source, category and difficulty; a Wilson 95% interval over all
task-runs and a bootstrap interval over tasks (task-runs of one task are not independent, so the
task is the unit that is resampled); each task's stability (k of n); seconds and tokens.
"""

import math
import random
import statistics
from collections import defaultdict

BOOTSTRAP = 4000
SEED = 20260918


def wilson(k, n, z=1.96):
    """Wilson score interval for k successes of n."""
    if n == 0:
        return (0.0, 0.0)
    p = k / n
    d = 1 + z * z / n
    centre = (p + z * z / (2 * n)) / d
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (max(0.0, centre - half), min(1.0, centre + half))


def per_task(records):
    """task id → [pass, pass, …] over its runs."""
    out = defaultdict(list)
    for r in records:
        out[r["id"]].append(bool(r.get("pass")))
    return dict(out)


def bootstrap_mean(values, n=BOOTSTRAP, seed=SEED):
    """95% percentile interval of the mean of `values`, resampled with replacement."""
    if not values:
        return (0.0, 0.0)
    rng = random.Random(seed)
    k = len(values)
    means = sorted(sum(values[rng.randrange(k)] for _ in range(k)) / k for _ in range(n))
    return (means[int(0.025 * n)], means[int(0.975 * n) - 1])


def rate(records):
    k = sum(1 for r in records if r.get("pass"))
    n = len(records)
    return {"pass": k, "n": n, "rate": (k / n if n else 0.0), "wilson95": wilson(k, n)}


def _median(xs):
    xs = [x for x in xs if x is not None]
    if not xs:
        return None
    m = statistics.median(xs)
    return int(m) if all(isinstance(x, int) for x in xs) and m == int(m) else round(m, 1)


def _pct(xs, p):
    xs = sorted(x for x in xs if x is not None)
    if not xs:
        return None
    return xs[min(len(xs) - 1, int(round(p * (len(xs) - 1))))]


def summarize(records):
    graded = [r for r in records if not r.get("truth_error")]
    tasks = per_task(graded)
    task_means = [sum(v) / len(v) for v in tasks.values()]
    s = {"overall": rate(graded), "tasks": len(tasks), "task_runs": len(graded),
         "bootstrap95_tasks": bootstrap_mean(task_means)}
    for key in ("split", "source", "category", "difficulty"):
        groups = defaultdict(list)
        for r in graded:
            groups[str(r.get(key))].append(r)
        s["by_" + key] = {g: rate(rs) for g, rs in sorted(groups.items())}
    checks = defaultdict(lambda: [0, 0])
    for r in graded:
        for c in r.get("checks") or []:
            checks[c["type"]][0] += bool(c["pass"])
            checks[c["type"]][1] += 1
    s["by_check"] = {t: {"pass": k, "n": n, "rate": k / n if n else 0.0} for t, (k, n) in sorted(checks.items())}
    s["stability"] = {t: f"{sum(v)}/{len(v)}" for t, v in sorted(tasks.items())}
    s["always"] = sum(1 for v in tasks.values() if all(v))
    s["never"] = sum(1 for v in tasks.values() if not any(v))
    s["flaky"] = sum(1 for v in tasks.values() if any(v) and not all(v))
    secs = [r.get("seconds") for r in graded]
    s["seconds"] = {"median": _median(secs), "p90": _pct(secs, 0.9), "max": max((x for x in secs if x is not None), default=None),
                    "total": round(sum(x for x in secs if x is not None), 1)}
    s["timeouts"] = sum(1 for r in graded if r.get("timeout"))
    s["errors"] = sum(1 for r in graded if r.get("error") and not r.get("timeout"))
    s["truth_errors"] = sum(1 for r in records if r.get("truth_error"))
    s["tool_calls"] = {"mean": (statistics.mean([r.get("tool_calls", 0) for r in graded]) if graded else None),
                       "zero": sum(1 for r in graded if not r.get("tool_calls"))}
    toks = [r.get("tokens") or {} for r in graded]
    s["tokens"] = {"median_input": _median([t.get("input_tokens") for t in toks]),
                   "median_output": _median([t.get("output_tokens") for t in toks]),
                   "max_prompt": max((t.get("max_input_tokens") or 0 for t in toks), default=0),
                   "median_api_calls": _median([t.get("api_calls") for t in toks])}
    s["dirty"] = sum(1 for r in graded if r.get("mutated") and not r.get("mutates"))
    return s


def pct(x):
    return f"{100 * x:.1f}%"


def ci(pair):
    return f"[{100 * pair[0]:.1f}, {100 * pair[1]:.1f}]"


def format_summary(result):
    s = result["summary"]
    m = result.get("model", {})
    o = s["overall"]
    lines = [
        f"# {result.get('name')}  ({m.get('file')}, variant {result.get('variant', {}).get('name')}, runs {result.get('runs')})",
        f"relay {result.get('relay', {}).get('commit', '?')[:12]}  ·  {result.get('llama', {}).get('version', '?')}  ·  "
        f"{(result.get('gpu') or {}).get('name', 'no GPU')}  ·  slots {result.get('parallel')}  ·  anchor {result.get('anchor')}",
        "",
        f"pass {o['pass']}/{o['n']} = {pct(o['rate'])}  Wilson95 {ci(o['wilson95'])}  bootstrap-over-tasks {ci(s['bootstrap95_tasks'])}",
    ]
    for key in ("split", "source", "difficulty", "category"):
        parts = [f"{g} {pct(v['rate'])} ({v['pass']}/{v['n']})" for g, v in s["by_" + key].items()]
        lines.append(f"by {key}: " + ", ".join(parts))
    lines.append("by check: " + ", ".join(f"{t} {pct(v['rate'])} ({v['pass']}/{v['n']})" for t, v in s["by_check"].items()))
    sec = s["seconds"]
    lines.append(f"seconds per task-run: median {sec['median']}, p90 {sec['p90']}, max {sec['max']}, total {sec['total']}  ·  "
                 f"timeouts {s['timeouts']}  ·  errors {s['errors']}  ·  truth errors {s['truth_errors']}")
    t = s["tokens"]
    lines.append(f"tokens per task-run: median in {t['median_input']}, out {t['median_output']}, largest prompt {t['max_prompt']}, "
                 f"median model calls {t['median_api_calls']}  ·  mean tool calls {s['tool_calls']['mean'] and round(s['tool_calls']['mean'], 2)}, "
                 f"none {s['tool_calls']['zero']}")
    lines.append(f"tasks: {s['always']} always pass, {s['never']} never, {s['flaky']} flaky")
    lines.append("")
    by_task = defaultdict(list)
    for r in result["records"]:
        by_task[r["id"]].append(r)
    for tid, rs in by_task.items():
        k = sum(1 for r in rs if r.get("pass"))
        secs = [r.get("seconds") for r in rs]
        failed = sorted({c["type"] for r in rs for c in r.get("checks", []) if not c["pass"]})
        why = next((c["why"] for r in rs for c in r.get("checks", []) if not c["pass"]), "")
        err = next((r["error"] for r in rs if r.get("error")), "")
        lines.append(f"  {'PASS' if k == len(rs) else ('fail' if k == 0 else 'flaky')} {k}/{len(rs)}  {tid:<44} "
                     f"{_median(secs) or 0:6.1f}s  {(rs[0].get('split') or '')[:4]}"
                     + (f"  failed {failed}: {why[:110]}" if failed else "") + (f"  [{err[:100]}]" if err else ""))
    return "\n".join(lines)


def compare(a, b):
    """b against a: deltas with intervals, and the tasks that flipped."""
    ra, rb = per_task(a["records"]), per_task(b["records"])
    common = sorted(set(ra) & set(rb))
    diffs = [sum(rb[t]) / len(rb[t]) - sum(ra[t]) / len(ra[t]) for t in common]
    out = {"common_tasks": len(common), "only_a": sorted(set(ra) - set(rb)), "only_b": sorted(set(rb) - set(ra))}
    sa = summarize([r for r in a["records"] if r["id"] in common])
    sb = summarize([r for r in b["records"] if r["id"] in common])
    out["a"], out["b"] = sa["overall"], sb["overall"]
    out["delta"] = sb["overall"]["rate"] - sa["overall"]["rate"]
    out["delta_bootstrap95"] = bootstrap_mean(diffs)
    out["groups"] = {}
    for key in ("split", "source", "category", "difficulty"):
        rows = {}
        for g in sorted(set(sa["by_" + key]) | set(sb["by_" + key])):
            x, y = sa["by_" + key].get(g), sb["by_" + key].get(g)
            gd = [d for t, d in zip(common, diffs) if _group_of(a, t, key) == g]
            rows[g] = {"a": x and x["rate"], "b": y and y["rate"], "n_tasks": len(gd),
                       "delta": (y["rate"] - x["rate"]) if x and y else None, "delta_bootstrap95": bootstrap_mean(gd) if gd else None}
        out["groups"][key] = rows
    fixed, broken, moved = [], [], []
    for t in common:
        pa, pb = sum(ra[t]) / len(ra[t]), sum(rb[t]) / len(rb[t])
        tag = f"{t} {sum(ra[t])}/{len(ra[t])} → {sum(rb[t])}/{len(rb[t])}"
        if pa < 0.5 <= pb:
            fixed.append(tag)
        elif pb < 0.5 <= pa:
            broken.append(tag)
        elif pa != pb:
            moved.append(tag)
    out["fixed"], out["broken"], out["moved"] = fixed, broken, moved
    sec_a, sec_b = sa["seconds"]["median"], sb["seconds"]["median"]
    out["seconds"] = {"a": sec_a, "b": sec_b}
    out["tokens"] = {"a": sa["tokens"], "b": sb["tokens"]}
    return out


def _group_of(result, task_id, key):
    for r in result["records"]:
        if r["id"] == task_id:
            return str(r.get(key))
    return None


def format_compare(a, b, c):
    lines = [
        f"A: {a.get('name')}  ({a.get('model', {}).get('file')}, {a.get('variant', {}).get('name')}, relay {a.get('relay', {}).get('commit', '?')[:10]})",
        f"B: {b.get('name')}  ({b.get('model', {}).get('file')}, {b.get('variant', {}).get('name')}, relay {b.get('relay', {}).get('commit', '?')[:10]})",
        f"{c['common_tasks']} tasks in both" + (f"; only in A: {len(c['only_a'])}" if c["only_a"] else "") + (f"; only in B: {len(c['only_b'])}" if c["only_b"] else ""),
        "",
        f"overall: {pct(c['a']['rate'])} → {pct(c['b']['rate'])}  Δ {100 * c['delta']:+.1f} pts  bootstrap95 over tasks "
        f"[{100 * c['delta_bootstrap95'][0]:+.1f}, {100 * c['delta_bootstrap95'][1]:+.1f}]",
        f"  A Wilson95 {ci(c['a']['wilson95'])}   B Wilson95 {ci(c['b']['wilson95'])}",
    ]
    for key, rows in c["groups"].items():
        lines.append(f"by {key}:")
        for g, v in rows.items():
            if v["delta"] is None:
                lines.append(f"  {g:<14} A {v['a'] if v['a'] is None else pct(v['a'])}  B {v['b'] if v['b'] is None else pct(v['b'])}")
                continue
            d = v["delta_bootstrap95"]
            lines.append(f"  {g:<14} {pct(v['a']):>6} → {pct(v['b']):>6}  Δ {100 * v['delta']:+5.1f}"
                         + (f"  [{100 * d[0]:+.1f}, {100 * d[1]:+.1f}]" if d else "") + f"  ({v['n_tasks']} tasks)")
    lines.append(f"median seconds: {c['seconds']['a']} → {c['seconds']['b']}")
    ta, tb = c["tokens"]["a"], c["tokens"]["b"]
    lines.append(f"median tokens in/out: {ta['median_input']}/{ta['median_output']} → {tb['median_input']}/{tb['median_output']}")
    for label in ("fixed", "broken", "moved"):
        if c[label]:
            lines.append(f"{label} ({len(c[label])}):")
            lines += [f"  {x}" for x in c[label]]
    return "\n".join(lines)


def results_md_line(result, path):
    s = result["summary"]
    o = s["overall"]
    split = s.get("by_split", {})
    dev, hold = split.get("dev"), split.get("holdout")
    return ("| {date} | {name} | {model} | {variant} | {relay} | {runs} | {tasks} | {rate} {ci} | {dev} | {hold} | {sec} | {file} |"
            .format(date=result.get("created", "")[:10], name=result.get("name"), model=result.get("model", {}).get("file"),
                    variant=result.get("variant", {}).get("name"), relay=result.get("relay", {}).get("commit", "?")[:10],
                    runs=result.get("runs"), tasks=s["tasks"], rate=pct(o["rate"]), ci=ci(o["wilson95"]),
                    dev=pct(dev["rate"]) if dev else "–", hold=pct(hold["rate"]) if hold else "–",
                    sec=s["seconds"]["median"], file=path))


RESULTS_MD_HEAD = (
    "# Threads bench results\n\n"
    "One line per committed run (`bench.py run --append`, or `bench.py report --append <file>`). Pass is over all\n"
    "task-runs with its Wilson 95% interval; dev and holdout are the two splits (README, \"Splits\"). Seconds are the\n"
    "median per task-run.\n\n"
    "| date | name | model | variant | relay | runs | tasks | pass [95% CI] | dev | holdout | median s | file |\n"
    "|---|---|---|---|---|---|---|---|---|---|---|---|\n"
)
