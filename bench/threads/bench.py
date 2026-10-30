#!/usr/bin/env python3
"""Relay's Threads model bench (bench/threads/README.md).

    bench.py run     [--gguf F] [--template T] [--name N] [--runs R] [--suite S] [--tasks IDS]
                     [--split dev|holdout|all] [--variant V] [--parallel N] [--relay-bin B] [--out F]
                     [--append] [--capture DIR] [--check]
    bench.py report  RESULT.json [--append] [--json]
    bench.py compare A.json B.json [--json]
    bench.py seen    CAPTURE_DIR
"""

import argparse
import datetime as dt
import hashlib
import json
import os
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from relaybench import (ANCHOR, BENCH_DIR, MODEL_ID, REPO, RESULTS_DIR, TODAY, ZONE, default_relay_bin, log)  # noqa: E402
from relaybench import engine as eng  # noqa: E402
from relaybench import fixtures, grade, model, report, runner, seen, truths  # noqa: E402
from relaybench import truths_v2  # noqa: E402,F401  (registers the v2 truths in truths.TRUTHS)
from relaybench import truths_v3  # noqa: E402,F401  (and v3's)

DEFAULT_GGUF = Path.home() / "models" / "Qwen3.5-4B-Q8_0.gguf"
VARIANTS = BENCH_DIR / "variants.json"
RESULTS_MD = RESULTS_DIR / "RESULTS.md"


# ── provenance ───────────────────────────────────────────────────────────


def relay_provenance(relay_bin):
    """The binary under test: its path, a digest, and the commit it was built from when that can
    be known (a COMMIT file beside a frozen copy, else the checkout it sits in)."""
    relay_bin = Path(relay_bin).resolve()
    h = hashlib.sha256()
    with open(relay_bin, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 22), b""):
            h.update(chunk)
    info = {"bin": str(relay_bin), "sha256": h.hexdigest(), "mtime": dt.datetime.fromtimestamp(relay_bin.stat().st_mtime).isoformat(timespec="seconds")}
    commit_file = relay_bin.parent / "COMMIT"
    if commit_file.is_file():
        info["commit"] = commit_file.read_text().strip()
        info["commit_source"] = str(commit_file)
        return info
    for parent in relay_bin.parents:
        if (parent / ".git").exists():
            try:
                head = subprocess.run(["git", "-C", str(parent), "rev-parse", "HEAD"], capture_output=True, text=True, timeout=20).stdout.strip()
                dirty = subprocess.run(["git", "-C", str(parent), "status", "--porcelain", "--", "crates"], capture_output=True, text=True, timeout=20).stdout.strip()
                info["commit"] = head + ("-dirty" if dirty else "")
                info["commit_source"] = f"git checkout {parent} (the binary may predate it)"
            except (OSError, subprocess.TimeoutExpired):
                pass
            break
    info.setdefault("commit", "unknown")
    return info


def claude_version(env):
    try:
        r = subprocess.run(["claude", "--version"], capture_output=True, text=True, timeout=30, env=env)
        return r.stdout.strip() or r.stderr.strip()
    except (OSError, subprocess.TimeoutExpired):
        return "?"


def load_variant(name):
    variants = json.loads(VARIANTS.read_text())
    if name not in variants:
        raise SystemExit(f"no variant {name!r} in {VARIANTS} (have: {', '.join(variants)})")
    return {"name": name, **variants[name]}


# ── run ──────────────────────────────────────────────────────────────────


def setup_engine(args, variant, spec=None):
    relay_bin = Path(variant.get("relay_bin") or args.relay_bin or default_relay_bin())
    e = eng.Engine(relay_bin, extra_env=variant.get("env"), keep=args.keep)
    e.start()
    if spec:
        models_dir, wrapper = spec.install(e.root)
        e.setting("threads.local.dir", str(models_dir))
        e.setting("threads.local.server", str(wrapper))
    e.setting("threads.local.idle_minutes", 0)
    claude = os.environ.get("RELAYBENCH_CLAUDE") or _which("claude")
    if claude:
        e.setting("providers.claude.path", claude)
    for path, value in (variant.get("settings") or {}).items():
        e.setting(path, value)
    return e


def _which(name):
    import shutil
    p = shutil.which(name)
    return str(Path(p).resolve()) if p else None


def check_tasks(tasks):
    problems = {}
    for t in tasks:
        p = grade.validate_task(t, truths.TRUTHS)
        if p:
            problems[t["id"]] = p
    return problems


def cmd_check(args, variant, tasks, world="v1"):
    """Start the engine with the fixed world, run every task's truth, show what it expects. No model."""
    e = setup_engine(args, variant)
    try:
        manifest = fixtures.load(e, world)
        shown = ('sample', 'holdings', 'period', 'gym_exported_at') if world == "v1" else ('world', 'backup', 'holdings', 'activities', 'avex', 'period', 'gym_exported_at')
        log(f"world loaded: {json.dumps({k: manifest[k] for k in shown})}")
        log(f"fingerprint {manifest['fingerprint']}")
        ctx = {"today": TODAY, "currency": manifest["currency"]}
        bad = 0
        for t in tasks:
            line = {"id": t["id"], "split": t["split"]}
            try:
                exp = truths.TRUTHS[t["truth"]](e.q, dict(ctx)) if t.get("truth") else {}
                line["expected"] = exp
                missing = []
                for c in t.get("checks") or []:
                    keys = [k for k in grade._as_list(c.get("of")) if isinstance(k, str)]
                    keys += [k for k in grade._as_list(c.get("any_of")) if isinstance(k, str) and c.get("type") == "number"]
                    missing += [k for k in keys if k not in exp]
                    if c.get("type") == "effect":
                        line.setdefault("effect_before", {})[c["truth"]] = _short(truths.TRUTHS[c["truth"]](e.q, dict(ctx)))
                if missing:
                    line["missing_keys"] = missing
                    bad += 1
            except Exception as ex:
                line["error"] = f"{type(ex).__name__}: {ex}"
                bad += 1
            print(json.dumps(line, default=str))
        log(f"{len(tasks)} tasks, {bad} with a problem")
        return 1 if bad else 0
    finally:
        e.stop()


def _short(v):
    s = json.dumps(v, default=str)
    return v if len(s) < 300 else s[:300] + "…"


def wait_ready(e, timeout=300):
    e.q("thread.pick", {"model": MODEL_ID})
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        m = e.q("thread.models")
        if m.get("running") == MODEL_ID and m.get("ready"):
            return m
        if m.get("server_error"):
            raise RuntimeError(m["server_error"])
        if m.get("running") is None and time.monotonic() > deadline - timeout + 20:
            raise RuntimeError(f"the model server did not start; see {e.store_dir / 'local-model.log'}:\n" + _tail(e.store_dir / "local-model.log"))
        time.sleep(1)
    raise RuntimeError(f"the model did not load within {timeout}s:\n" + _tail(e.store_dir / "local-model.log"))


def _tail(path, n=15):
    try:
        return "\n".join(Path(path).read_text(errors="replace").splitlines()[-n:])
    except OSError:
        return ""


def cmd_run(args):
    variant = load_variant(args.variant)
    tasks = runner.load_tasks(args.suite, args.tasks, args.split)
    if not tasks:
        raise SystemExit("no tasks match")
    problems = check_tasks(tasks)
    if problems:
        for tid, p in problems.items():
            log(f"task {tid}: {'; '.join(p)}")
        raise SystemExit("fix the tasks above first")
    try:
        world = runner.world_of(tasks)
    except ValueError as ex:
        raise SystemExit(str(ex))
    if args.check:
        return cmd_check(args, variant, tasks, world)

    spec = model.ModelSpec(args.gguf, name=args.name, template=args.template, keep_template=args.keep_template,
                           server=args.server, parallel=args.parallel, capture=args.capture)
    model_info = spec.describe()
    relay_bin = Path(variant.get("relay_bin") or args.relay_bin or default_relay_bin())
    relay_info = relay_provenance(relay_bin)
    name = args.name or f"{spec.gguf.stem}-{variant['name']}"
    stamp = dt.datetime.now().strftime("%Y%m%d-%H%M%S")
    out = Path(args.out) if args.out else RESULTS_DIR / f"{stamp}-{_slug(name)}.json"
    raw = out.with_suffix(".jsonl.gz")
    out.parent.mkdir(parents=True, exist_ok=True)

    with model.GpuLock(what=f"bench {name}"):
        # The run before may have let go of the lock while its server still leaves the card.
        model.wait_card_clear(60)
        others = model.llama_servers(gpu_only=True) or model.llama_on_card()
        if others:
            raise SystemExit(f"a llama-server is already running (pid {', '.join(map(str, others))}): "
                             "the GPU is shared, one at a time. Stop it (in Relay, pick another model) and run again.")
        others_before = model.llama_servers()
        gpu = model.gpu_info()
        if gpu:
            n = model.pick_parallel(args.parallel, spec.gguf.stat().st_size, gpu["free_mib"])
            if n < args.parallel:
                log(f"{gpu['free_mib']} MiB free on the GPU: {n} slot(s) instead of {args.parallel}")
            spec.parallel = n
        log(f"model {spec.gguf.name}, template {spec.template_mode()}, {spec.parallel} slot(s) × {model.SLOT_CONTEXT}")
        e = setup_engine(args, variant, spec)
        result = None
        try:
            manifest = fixtures.load(e, world)
            log(f"world {world} loaded, fingerprint {manifest['fingerprint'][:16]}; loading the model…")
            t0 = time.monotonic()
            wait_ready(e)
            load_s = round(time.monotonic() - t0, 1)
            llama = {"version": model_info["server_version"], "load_seconds": load_s,
                     **model.read_server_log(e.store_dir / "local-model.log")}
            try:
                llama["argv"] = json.loads((e.root / "llama-argv.json").read_text())["server"]
            except (OSError, json.JSONDecodeError):
                pass
            log(f"model ready in {load_s}s: {json.dumps({k: v for k, v in llama.items() if k != 'argv'})}")
            r = runner.Runner(e, tasks, truths=truths.TRUTHS, runs=args.runs, parallel=spec.parallel,
                              currency=manifest["currency"], fingerprint=manifest["fingerprint"],
                              turn_timeout=args.turn_timeout, raw_path=raw)
            started = time.time()
            # A copy: after an interruption, task-runs still in flight end when the engine stops,
            # with errors that are the bench's, not the model's; they are left out.
            records = list(r.run())
            result = {
                "schema": 1, "name": name, "created": dt.datetime.now().isoformat(timespec="seconds"),
                "wall_seconds": round(time.time() - started, 1),
                "anchor": ANCHOR, "zone": ZONE, "model": model_info, "variant": variant, "relay": relay_info,
                "claude_code": claude_version(e.cli_env()), "llama": llama, "gpu": gpu, "parallel": spec.parallel,
                "runs": args.runs, "suite": args.suite, "split": args.split, "task_filter": args.tasks,
                "turn_timeout": args.turn_timeout, "world": manifest, "gave_up": r.broken,
                "world_intact_after": fixtures.fingerprint(e) == manifest["fingerprint"],
                "records": records,
            }
            result["summary"] = report.summarize(records)
            out.write_text(json.dumps(result, indent=1, default=str))
            log(f"results: {out}")
            if args.capture:
                try:
                    endpoint = json.loads((Path(args.capture) / "endpoint.json").read_text())
                    s = seen.measure(args.capture, seen.Server(endpoint["port"], endpoint.get("key")))
                    print(seen.format_seen(s))
                except Exception as ex:
                    log(f"could not measure what the model sees: {type(ex).__name__}: {ex}")
        finally:
            e.stop()
            # Hold the lock until the card is free again, so the next run reads true free memory.
            if not model.wait_card_clear(30, quiet=True):
                log(f"warning: a llama-server is still on the GPU after teardown: {model.llama_on_card()}")
            left = [p for p in model.llama_servers(gpu_only=True) if p not in others_before]
            if left:
                log(f"warning: llama-server still running after teardown: {left}")
    print(report.format_summary(result))
    if args.append:
        append_results_md(result, out)
    return 0


def _slug(s):
    return "".join(c if c.isalnum() or c in "-_." else "-" for c in s)[:80]


def append_results_md(result, path):
    if not RESULTS_MD.exists():
        RESULTS_MD.write_text(report.RESULTS_MD_HEAD)
    try:
        rel = Path(path).resolve().relative_to(RESULTS_DIR)
    except ValueError:
        rel = Path(path)
    with open(RESULTS_MD, "a") as f:
        f.write(report.results_md_line(result, str(rel)) + "\n")
    log(f"appended to {RESULTS_MD}")


def cmd_regrade(args):
    """Grade a result's replies again with the checks as they are now: for a fixed grader, or a
    task whose check was wrong. An effect check keeps the verdict it had (it needs the engine)."""
    result = json.loads(Path(args.result).read_text())
    tasks = {t["id"]: t for suite in ("all", "v2", "v3") for t in runner.load_tasks(suite)}
    flipped = []
    for rec in result["records"]:
        task = tasks.get(rec["id"])
        if task is None:
            continue
        old = rec.get("checks") or []
        effects = {i: {"ok": old[i]["pass"], "why": old[i].get("why", "")}
                   for i, c in enumerate(task.get("checks") or []) if c.get("type") == "effect" and i < len(old)}
        thread = {"turns": rec.get("turns") or [], "error": rec.get("error"), "timeout": rec.get("timeout")}
        was = rec["pass"]
        rec["pass"], rec["checks"] = grade.grade(task, rec.get("expected"), thread, effects)
        if rec["pass"] != was:
            flipped.append((rec["id"], rec.get("run"), was, rec["pass"]))
    result["regraded"] = dt.datetime.now().isoformat(timespec="seconds")
    result["summary"] = report.summarize(result["records"])
    for id_, run, was, now in flipped:
        log(f"{id_} #{run}: {'pass' if was else 'fail'} -> {'PASS' if now else 'fail'}")
    log(f"{len(flipped)} task-runs changed")
    print(report.format_summary(result))
    if args.out:
        Path(args.out).write_text(json.dumps(result, indent=1, ensure_ascii=False))
        log(f"written to {args.out}")


def cmd_report(args):
    result = json.loads(Path(args.result).read_text())
    result["summary"] = report.summarize(result["records"])
    if args.json:
        print(json.dumps(result["summary"], indent=1))
    else:
        print(report.format_summary(result))
    if args.append:
        append_results_md(result, args.result)
    return 0


def cmd_compare(args):
    a = json.loads(Path(args.a).read_text())
    b = json.loads(Path(args.b).read_text())
    c = report.compare(a, b)
    print(json.dumps(c, indent=1) if args.json else report.format_compare(a, b, c))
    return 0


def cmd_seen(args):
    """Print what a capture run measured; with --measure, count its requests again on a CPU-only
    llama-server (no GPU, no lock), e.g. with another GGUF's tokenizer and template."""
    if args.measure:
        template = args.template or (None if args.no_template else REPO / "crates/relay-core/src/local_models/qwen3.5.jinja")
        server = args.server or _which("llama-server") or str(model.DEFAULT_SERVER)
        with seen.OfflineServer(Path(args.gguf).expanduser(), template, server) as srv:
            s = seen.measure(args.capture, srv)
    else:
        s = json.loads((Path(args.capture) / "seen.json").read_text())
    print(json.dumps(s, indent=1) if args.json else seen.format_seen(s))
    return 0


def _interrupt(signum, frame):
    raise KeyboardInterrupt(f"signal {signum}")


def main(argv=None):
    eng.become_subreaper()
    # A `timeout` or a closed terminal tears down like Ctrl-C: the engine, its model server and the
    # GPU lock are let go of in the `finally`s.
    import signal
    signal.signal(signal.SIGTERM, _interrupt)
    signal.signal(signal.SIGHUP, _interrupt)
    p = argparse.ArgumentParser(prog="bench.py", description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run", help="run tasks against a model (or --check the truths without one)")
    r.add_argument("--gguf", default=str(DEFAULT_GGUF), help="the model file (default: %(default)s)")
    r.add_argument("--template", help="a chat template (.jinja) in place of the product's / the GGUF's")
    r.add_argument("--keep-template", action=argparse.BooleanOptionalAction, default=None,
                   help="use the product's Qwen3.5 template (default: only for a Qwen3.5 GGUF)")
    r.add_argument("--server", help="llama-server (default: on PATH, else ~/.local/bin/llama-server)")
    r.add_argument("--name", help="a name for this run (default: <gguf>-<variant>)")
    r.add_argument("--runs", type=int, default=1, help="runs of each task (default 1)")
    r.add_argument("--suite", default="all", help="tasks/<suite>.json, a comma list, or all (default); v2 for tasks/v2/ in the v2 world; v3 for tasks/v3/, the harder suite, in the v2 world")
    r.add_argument("--tasks", action="append", help="task ids or globs, comma separated (repeatable)")
    r.add_argument("--split", choices=["dev", "holdout", "all"], default="all")
    r.add_argument("--variant", default="baseline", help="a product variant from variants.json")
    r.add_argument("--parallel", type=int, default=3, help="llama-server slots, so tasks side by side (default 3)")
    r.add_argument("--relay-bin", help=f"the relay binary under test (default {default_relay_bin()})")
    r.add_argument("--turn-timeout", type=float, default=runner.TURN_TIMEOUT)
    r.add_argument("--out", help="the result file (default results/<time>-<name>.json)")
    r.add_argument("--append", action="store_true", help="add a line to results/RESULTS.md")
    r.add_argument("--capture", help="record every model request into this folder and measure what the model sees")
    r.add_argument("--check", action="store_true", help="no model: load the world and print every task's expected values")
    r.add_argument("--keep", action="store_true", help="keep /tmp/relay-bench/<n>/ afterwards, for a look")
    r.set_defaults(fn=cmd_run)
    rp = sub.add_parser("report", help="summarize a result file")
    rp.add_argument("result")
    rp.add_argument("--append", action="store_true")
    rp.add_argument("--json", action="store_true")
    rp.set_defaults(fn=cmd_report)
    g = sub.add_parser("regrade", help="grade a result's replies again with today's checks (effects keep their verdict)")
    g.add_argument("result")
    g.add_argument("--out", help="write the regraded result here (default: only print it)")
    g.set_defaults(fn=cmd_regrade)
    c = sub.add_parser("compare", help="B against A: deltas with intervals, flipped tasks")
    c.add_argument("a")
    c.add_argument("b")
    c.add_argument("--json", action="store_true")
    c.set_defaults(fn=cmd_compare)
    s = sub.add_parser("seen", help="what the model sees, from a capture run (README)")
    s.add_argument("capture", help="the --capture folder of a run")
    s.add_argument("--measure", action="store_true", help="count again on a CPU-only llama-server")
    s.add_argument("--gguf", default=str(DEFAULT_GGUF))
    s.add_argument("--template", help="chat template (default: the product's Qwen3.5 one)")
    s.add_argument("--no-template", action="store_true", help="the GGUF's own template")
    s.add_argument("--server")
    s.add_argument("--json", action="store_true")
    s.set_defaults(fn=cmd_seen)
    args = p.parse_args(argv)
    return args.fn(args)


if __name__ == "__main__":
    sys.exit(main())
