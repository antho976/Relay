"""The fixed world every task runs in (contract: "The fixed world").

- Tally: `money.sample`, Tally's sample household, built for the anchor's today and the default
  seed, plus the Wealthsimple holdings report of the shared tests (docs/INVESTMENTS.md, W1), as of
  2026-05-08: more than a month before today, so the investments read as stale on purpose.
- Avex: `crates/relay-gym/tests/avex_export.json` with every date moved back exactly three weeks
  (weekdays kept), so it was exported on 2026-09-17 21:14, the evening before today.

The v2 world (`worlds/v2/`, its WORLD.md) is loaded by `load(engine, "v2")` through the engine's own
ops and checked against the facts its generator computed.

`load()` fills a fresh engine and returns a manifest (counts, digests, a fingerprint of what the
agent would read). `snapshot()` / `restore()` put the ledger back after a task that changed it.
"""

import datetime as dt
import hashlib
import json
from pathlib import Path

from . import BENCH_DIR, REPO, TODAY

AVEX_EXPORT = REPO / "crates/relay-gym/tests/avex_export.json"
#: Every date in the Avex export moves back this far.
AVEX_SHIFT = dt.timedelta(weeks=3)
#: Keys of the export that hold epoch milliseconds.
AVEX_MS_KEYS = {"startedAt", "finishedAt", "completedAt", "date", "createdAt"}

#: Wealthsimple's holdings report as the shared tests have it (crates/relay-core/tests/money.rs,
#: HOLDINGS_REPORT; docs/INVESTMENTS.md W1): a TFSA holding Apple in US dollars, XEQT and ARKK.
HOLDINGS_REPORT = (
    "Account Name,Account Type,Account Classification,Account Number,Symbol,Exchange,MIC,Name,Security Type,"
    "Quantity,Position Direction,Market Price,Market Price Currency,Book Value (CAD),Book Value Currency (CAD),"
    "Book Value (Market),Book Value Currency (Market),Market Value,Market Value Currency,Market Unrealized Returns,"
    "Market Unrealized Returns Currency\n"
    '"Demo TFSA","TFSA","Trade","DEMO0001CAD","AAPL","NASDAQ","XNAS","Apple Inc","EQUITY","10","LONG","100","USD",'
    '"1000","CAD","750","USD","1000","USD","0","USD"\n'
    '"Demo TFSA","TFSA","Trade","DEMO0001CAD","XEQT","TSX","XTSE","iShares Core Equity ETF Portfolio",'
    '"EXCHANGE_TRADED_FUND","10","LONG","25","CAD","250","CAD","250","CAD","250","CAD","0","CAD"\n'
    '"Demo TFSA","TFSA","Trade","DEMO0001CAD","ARKK","BATS","BATS","ARK Innovation ETF","EXCHANGE_TRADED_FUND",'
    '"1","LONG","50","USD","50","CAD","50","USD","50","USD","0","USD"\n'
    "\n"
    '"As of 2026-05-08 12:00 GMT-04:00"\n'
)
HOLDINGS_NAME = "holdings-report-2026-05-08.csv"

#: Reads whose answers make up the world's fingerprint: what an agent would see, as integers.
FINGERPRINT_READS = [
    ("money.lists", {}),
    ("money.summary", {}),
    ("money.tx.list", {"period_offset": 0, "limit": 1000}),
    ("money.tx.list", {"period_offset": -1, "limit": 1000}),
    ("money.tx.list", {"period_offset": -2, "limit": 1000}),
    ("money.invest.summary", {}),
    ("money.invest.list", {}),
    ("gym.summary", {}),
    ("gym.lifts", {}),
]
#: Fields that differ between two loads of the same world (random uids, the time of the load).
VOLATILE = {"uid", "imported_at", "file", "updated_at", "created_at"}


def shifted_avex(src=AVEX_EXPORT, shift=AVEX_SHIFT):
    """The export with every timestamp moved back by `shift`."""
    data = json.loads(Path(src).read_text())
    ms = int(shift.total_seconds() * 1000)

    def walk(node, key=None):
        if isinstance(node, dict):
            return {k: walk(v, k) for k, v in node.items()}
        if isinstance(node, list):
            return [walk(v, key) for v in node]
        if key in AVEX_MS_KEYS and isinstance(node, (int, float)) and not isinstance(node, bool) and node > 10**11:
            return type(node)(node - ms)
        return node

    out = walk(data)
    at = dt.datetime.strptime(data["exportedAt"], "%Y-%m-%d %H:%M") - shift
    out["exportedAt"] = at.strftime("%Y-%m-%d %H:%M")
    return out


def _strip(node):
    if isinstance(node, dict):
        return {k: _strip(v) for k, v in sorted(node.items()) if k not in VOLATILE}
    if isinstance(node, list):
        return [_strip(v) for v in node]
    return node


def fingerprint(engine):
    """sha256 over what the agent could read, with load-time noise left out. Two engines loaded
    with the same world at the same anchor have the same fingerprint."""
    h = hashlib.sha256()
    for op, payload in FINGERPRINT_READS:
        h.update(op.encode())
        h.update(json.dumps(_strip(engine.q(op, payload)), sort_keys=True).encode())
    return h.hexdigest()


def load(engine, world="v1"):
    """Fill a fresh engine with the fixed world (`v1`, the sample household, or `v2`). Returns the
    manifest recorded with the results."""
    if world == "v2":
        return load_v2(engine)
    if world != "v1":
        raise ValueError(f"no world {world!r} (have: {', '.join(WORLDS)})")
    fx = engine.root / "fixtures"
    fx.mkdir(exist_ok=True)
    manifest = {}
    summary = engine.q("money.summary")
    if not summary.get("empty", True):
        raise RuntimeError("the throwaway engine's ledger is not empty")
    manifest["sample"] = engine.q("money.sample")
    holdings = fx / HOLDINGS_NAME
    holdings.write_text(HOLDINGS_REPORT)
    imported = engine.q("money.invest.import", {"path": str(holdings), "accounts": []})
    manifest["holdings"] = {k: imported.get(k) for k in ("kind", "accounts_created", "holdings", "prices", "values")}
    avex = fx / "avex_export.json"
    avex_text = json.dumps(shifted_avex(), indent=1)
    avex.write_text(avex_text)
    manifest["avex"] = engine.q("gym.import", {"path": str(avex)})
    manifest["avex_sha256"] = hashlib.sha256(avex_text.encode()).hexdigest()

    # The fake clock reached the engine: today is the anchor's.
    summary = engine.q("money.summary")
    period = summary["period"]
    if not (period["start"] <= TODAY < period["end_exclusive"]):
        raise RuntimeError(f"the engine's budget period {period} does not hold {TODAY}: is libfaketime working?")
    manifest["period"] = period
    manifest["currency"] = summary["currency"]
    manifest["gym_exported_at"] = engine.q("gym.summary")["imported"]["exported_at"]
    manifest["fingerprint"] = fingerprint(engine)
    snapshot(engine)
    return manifest


# ── v2: a messier household, its budget month starting on the 15th (worlds/v2/WORLD.md) ──────

WORLDS = ("v1", "v2")
V2_DIR = BENCH_DIR / "worlds" / "v2"


def load_v2(engine, world_dir=V2_DIR):
    """The v2 world, through the engine's own ops: `money.import` of the Tally backup (which carries
    `month_start_day` 15), `money.invest.import` of Wealthsimple's holdings report and activities
    export into the TFSA the backup already holds, `gym.import` of the Avex export. Then the engine
    is read back and compared with `facts.json`, which the generator computed from the rows: a
    world that reads differently from WORLD.md stops the run."""
    world_dir = Path(world_dir)
    facts = json.loads((world_dir / "facts.json").read_text())
    if not engine.q("money.summary").get("empty", True):
        raise RuntimeError("the throwaway engine's ledger is not empty")
    manifest = {"world": "v2", "files": {}}
    for name in ("tally-backup.json", "holdings-report.csv", "activities.csv", "avex_export.json"):
        manifest["files"][name] = hashlib.sha256((world_dir / name).read_bytes()).hexdigest()
    manifest["backup"] = engine.q("money.import", {"path": str(world_dir / "tally-backup.json")})
    number = facts["invest"]["ws_number"]
    ws = [a for a in engine.q("money.lists")["accounts"] if a["institution"].lower() == "wealthsimple"]
    if len(ws) != 1:
        raise RuntimeError(f"the backup should hold one Wealthsimple account, has {[a['name'] for a in ws]}")
    for key, name in (("holdings", "holdings-report.csv"), ("activities", "activities.csv")):
        got = engine.q("money.invest.import", {"path": str(world_dir / name), "accounts": [{"number": number, "account_id": ws[0]["id"]}]})
        manifest[key] = {k: got.get(k) for k in ("kind", "accounts_created", "securities", "holdings", "activities", "prices", "values", "skipped")}
    manifest["avex"] = engine.q("gym.import", {"path": str(world_dir / "avex_export.json")})

    summary = engine.q("money.summary")
    period = summary["period"]
    if not (period["start"] <= TODAY < period["end_exclusive"]):
        raise RuntimeError(f"the engine's budget period {period} does not hold {TODAY}: is libfaketime working?")
    problems = check_v2(engine, facts)
    if problems:
        raise RuntimeError("the v2 world reads differently from worlds/v2/facts.json:\n  " + "\n  ".join(problems))
    manifest["period"] = period
    manifest["currency"] = summary["currency"]
    manifest["gym_exported_at"] = engine.q("gym.summary")["imported"]["exported_at"]
    manifest["fingerprint"] = fingerprint(engine)
    snapshot(engine)
    return manifest


def check_v2(engine, facts):
    """Where the engine's readings differ from `facts.json` (empty when they agree)."""
    out = []

    def same(what, got, want):
        if got != want:
            out.append(f"{what}: the engine reads {got!r}, facts.json says {want!r}")

    s = engine.q("money.summary")
    same("month_start_day", s["month_start_day"], facts["month_start_day"])
    p0 = facts["periods"]["0"]
    same("this period", (s["period"]["start"], s["period"]["end_exclusive"]), (p0["start"], p0["end_exclusive"]))
    accounts = {a["name"]: a["balance"] for a in engine.q("money.lists")["accounts"]}
    for name, balance in facts["accounts"].items():
        same(f"balance of {name}", accounts.get(name), balance)
    same("net worth", s["net_worth"], facts["net_worth"])
    for off, want in facts["periods"].items():
        page = engine.q("money.tx.list", {"period_offset": int(off), "limit": 5000})
        same(f"period {off}", (page["period"]["start"], len(page["transactions"]), page["spent"], page["income"]),
             (want["start"], want["entries"], want["spent"], want["income"]))
    total = sum(len(engine.q("money.tx.list", {"period_offset": off, "limit": 5000})["transactions"]) for off in range(-14, 1))
    same("entries in all (a bill the engine posted on loading would show here)", total, facts["transactions"])
    budgets = {b["name"]: b for b in s["budgets"]}
    for name, want in facts["budgets"].items():
        if name == "(overall)":
            same("overall pace", (s["pace"]["budget"], s["pace"]["spent"], s["pace"]["pace_delta"], s["pace"]["status"]),
                 (want["budget"], want["spent"], want["pace_delta"], want["status"]))
            continue
        b = budgets.get(name) or {}
        same(f"budget {name}", (b.get("budget"), b.get("spent"), b.get("left"), b.get("pace_delta"), b.get("status")),
             (want["budget"], want["spent"], want["left"], want["pace_delta"], want["status"]))
    g = engine.q("gym.summary")
    a = facts["avex"]
    same("Avex", (g["imported"]["exported_at"], g["unit"], g["totals"]["sessions"], g["totals"]["untracked"], g["totals"]["cardio"]),
         (a["exported_at"], a["unit"], a["sessions"], a["untracked"], a["cardio"]))
    inv = engine.q("money.invest.summary")
    tfsa = [x for x in inv["accounts"] if x["institution"].lower() == "wealthsimple"]
    i = facts["invest"]
    same("investments", (inv["as_of"], tfsa[0]["value"] if tfsa else None, tfsa[0]["holdings"] if tfsa else None),
         (i["as_of"], i["value"], i["holdings"]))
    same("investment activities", len(engine.q("money.invest.list", {"limit": 1000})["activities"]), i["activities"])
    return out


def snapshot_path(engine):
    return engine.root / "fixtures" / "ledger-snapshot.json"


def snapshot(engine):
    """Write the ledger as a Tally backup, to restore after a task that changes it."""
    return engine.q("money.export", {"path": str(snapshot_path(engine))})


def restore(engine, expect_fingerprint=None):
    """Put the ledger back as the snapshot has it. With `expect_fingerprint`, check that it is."""
    engine.q("money.import", {"path": str(snapshot_path(engine))})
    if expect_fingerprint:
        got = fingerprint(engine)
        if got != expect_fingerprint:
            raise RuntimeError(f"the ledger restored from the snapshot reads differently ({got[:12]} != {expect_fingerprint[:12]})")
