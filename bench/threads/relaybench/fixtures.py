"""The fixed world every task runs in (contract: "The fixed world").

- Tally: `money.sample`, Tally's sample household, built for the anchor's today and the default
  seed, plus the Wealthsimple holdings report of the shared tests (docs/INVESTMENTS.md, W1), as of
  2026-05-08: more than a month before today, so the investments read as stale on purpose.
- Avex: `crates/relay-gym/tests/avex_export.json` with every date moved back exactly three weeks
  (weekdays kept), so it was exported on 2026-09-17 21:14, the evening before today.

`load()` fills a fresh engine and returns a manifest (counts, digests, a fingerprint of what the
agent would read). `snapshot()` / `restore()` put the ledger back after a task that changed it.
"""

import datetime as dt
import hashlib
import json
from pathlib import Path

from . import REPO, TODAY

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


def load(engine):
    """Fill a fresh engine with the fixed world. Returns the manifest recorded with the results."""
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
