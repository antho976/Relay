"""Expected answers for the v3 suite (tasks/v3/*.json), asked in the v2 world (worlds/v2/WORLD.md).

v3 is the harder suite: questions no single reading answers. Sums over a year or the whole ledger,
a shop or a note searched across months, what-ifs worked out from a goal or a bill, ratios, days
named the way people name them ("since Labour Day", "Q2"), false premises to correct, things the
apps do not hold, both apps over the same days, and writes corrected in a second turn.

The same contract as `truths.py` and `truths_v2.py`: `fn(q, ctx)`, raw integers from the
throwaway engine, added up here from entries and sessions and checked against the engine's own
figure where it has one. Every name starts with `v3_` and is registered in `truths.TRUTHS`.
Where a plausible misreading gives another figure, the truth returns it under a `*_wrong` key,
never checked for, so a failed run reads at a glance.
"""

import datetime as dt
import re
import statistics

from .truths import TruthError, _check, _fmt, _register_effect, major, thread_tool_inputs, truth
from .truths_v2 import TODAY, _acct, _all_txs, _category_id, _new, _note_has, _page, _spent

YEAR = "2026-01-01"


def _expenses(txs, since=None, until=None):
    return [t for t in txs if t["type"] == "EXPENSE" and (since is None or t["date"] >= since) and (until is None or t["date"] <= until)]


# Every truth reads only what the oldest relay a v3 run is compared on serves (b413157, before
# the ranges and the page totals): the same keys for both sides of a comparison. Where a newer
# engine has its own figure, `_newer` checks against it.


def _newer(q, op, payload):
    """`op`'s answer, or None from an engine without that payload."""
    try:
        return q(op, payload)
    except Exception as e:  # noqa: BLE001 (an OpError for an unknown field)
        if "unknown field" in str(e):
            return None
        raise


def _counted_sessions(q):
    page = q("gym.sessions", {"limit": 1000})
    rows = [s for s in page["sessions"] if not s.get("untracked")]
    if "untracked" in page:
        _check(len(rows) == page["total"] - page["untracked"], f"counted {len(rows)} vs total {page['total']} - untracked {page['untracked']}")
    return rows


def _cardio(q):
    return q("gym.cardio", {"limit": 1000})["entries"]


def _lift(q, name):
    return q("gym.lift.get", {"name": name})


def _workouts_between(q, start, end):
    """Counted workouts from `start` to `end`, both days in, checked against gym.sessions' range."""
    mine = len([s for s in _counted_sessions(q) if start <= s["day"] <= end])
    page = _newer(q, "gym.sessions", {"from": start, "to": end, "limit": 1})
    if page is not None:
        _check(page["total"] - page["untracked"] == mine, f"{start}..{end}: engine {page['total']} - {page['untracked']} vs sessions {mine}")
    return mine


# ══ Tally: sums over a year or the whole ledger ═══════════════════════════════════════════════


@truth
def v3_loc_interest(q, ctx):
    rows = [t for t in _expenses(_all_txs(q), since=YEAR) if _note_has(t, "intérêts") and t["account"] == "Line of credit"]
    _check(len(rows) == 8, f"eight interest charges Jan-Aug: {[(t['date'], t['amount']) for t in rows]}")
    every = [t for t in _expenses(_all_txs(q)) if _note_has(t, "intérêts")]
    return {"interest": major(_spent(rows)), "whole_ledger_wrong": major(_spent(every))}


def _income(txs, category, since=None, until=None):
    return sum(t["amount"] for t in txs if t["type"] == "INCOME" and t.get("category") == category
               and (since is None or t["date"] >= since) and (until is None or t["date"] <= until))


@truth
def v3_freelance_years(q, ctx):
    txs = _all_txs(q)
    this, last = _income(txs, "Freelance", since=YEAR), _income(txs, "Freelance", until="2025-12-31")
    _check(txs[0]["date"] == "2025-08-15", f"the ledger starts {txs[0]['date']}: last year is only from Aug 15")
    return {"this_year": major(this), "last_year": major(last)}


@truth
def v3_gst_total(q, ctx):
    rows = [t for t in _all_txs(q) if t["type"] == "INCOME" and _note_has(t, "TPS")]
    _check(len(rows) == 4 and len({t["amount"] for t in rows}) == 1, f"four equal GST credits: {[(t['date'], t['amount']) for t in rows]}")
    return {"total": major(sum(t["amount"] for t in rows)), "each": major(rows[0]["amount"])}


@truth
def v3_tax_refunds(q, ctx):
    rows = [t for t in _all_txs(q) if t["type"] == "INCOME" and t["date"] >= YEAR and re.search(r"ARC|Revenu Qu", t.get("note") or "")]
    _check(len(rows) == 2, f"a federal and a Quebec refund: {[(t['date'], t['note']) for t in rows]}")
    by = {("federal" if "ARC" in t["note"] else "quebec"): major(t["amount"]) for t in rows}
    return {"total": major(sum(t["amount"] for t in rows)), **by}


@truth
def v3_costco_year(q, ctx):
    rows = [t for t in _expenses(_all_txs(q), since=YEAR) if _note_has(t, "costco")]
    found = _newer(q, "money.tx.list", {"from": YEAR, "to": TODAY, "query": "costco", "limit": 5000})
    _check(found is None or found["spent"] == _spent(rows), f"engine's search {found and found['spent']} vs entries {_spent(rows)}")
    return {"total": major(_spent(rows)), "trips": len(rows), "average": round(_spent(rows) / len(rows) / 100, 2)}


@truth
def v3_dining_weekday(q, ctx):
    by = {}
    for t in _expenses(_all_txs(q)):
        if t.get("category") == "Dining":
            day = dt.date.fromisoformat(t["date"]).strftime("%A")
            by[day] = by.get(day, 0) + t["amount"]
    ranked = sorted(by.items(), key=lambda kv: -kv[1])
    _check(ranked[0][1] > ranked[1][1] * 1.2, f"one weekday clearly first: {ranked[:2]}")
    return {"day": ranked[0][0], "amount": major(ranked[0][1]), "second": ranked[1][0]}


@truth
def v3_quarters(q, ctx):
    txs = _all_txs(q)
    q1 = _spent(_expenses(txs, since="2026-01-01", until="2026-03-31"))
    q2 = _spent(_expenses(txs, since="2026-04-01", until="2026-06-30"))
    _check(q2 > q1, "Q2 is the larger")
    return {"q1": major(q1), "q2": major(q2), "difference": major(q2 - q1)}


@truth
def v3_ytd_spent(q, ctx):
    rows = _expenses(_all_txs(q), since=YEAR)
    found = _newer(q, "money.tx.list", {"from": YEAR, "to": TODAY, "limit": 5000})
    _check(found is None or found["spent"] == _spent(rows), f"engine {found and found['spent']} vs entries {_spent(rows)}")
    return {"spent": major(_spent(rows))}


@truth
def v3_since_labour_day(q, ctx):
    # Labour Day is the first Monday of September: Sep 7 in 2026.
    day = dt.date(2026, 9, 1)
    day += dt.timedelta(days=(7 - day.weekday()) % 7)
    _check(day.isoformat() == "2026-09-07", f"Labour Day 2026 is {day}")
    rows = _expenses(_all_txs(q), since=day.isoformat())
    return {"spent": major(_spent(rows)), "since": day.isoformat(), "entries": len(rows)}


@truth
def v3_pet_since_vet(q, ctx):
    pets = [t for t in _expenses(_all_txs(q)) if t.get("category") == "Pet care"]
    vet = max(pets, key=lambda t: t["amount"])
    _check(vet["date"] == "2026-02-11", f"the big vet bill is {vet['date']} {vet['amount']}")
    since = [t for t in pets if t["date"] >= vet["date"]]
    return {"since_vet": major(_spent(since)), "vet": major(vet["amount"]), "after_vet_wrong": major(_spent(since) - vet["amount"])}


def _bill(q, name):
    hits = [b for b in q("money.summary")["bills"] if (b.get("name") or "").lower() == name.lower()]
    _check(len(hits) == 1, f"no single bill {name}")
    return hits[0]


@truth
def v3_cancel_subs(q, ctx):
    # Both are monthly bills (WORLD.md); the summary's bills carry no frequency.
    netflix, spotify = _bill(q, "Netflix"), _bill(q, "Spotify")
    monthly = netflix["amount"] + spotify["amount"]
    return {"yearly": major(monthly * 12), "monthly": major(monthly)}


@truth
def v3_japan_300(q, ctx):
    goals = [g for g in q("money.summary")["goals"] if "japon" in g["name"].lower()]
    _check(len(goals) == 1, "one Japon goal")
    g = goals[0]
    left = g["target"] - g["saved"]
    months = -(-left // 30000)
    _check(months == 10, f"{left} left at 300.00 a month is {months} months")
    return {"months": months, "left": major(left)}


@truth
def v3_afford_laptop(q, ctx):
    s = q("money.summary")
    left = s["pace"]["remaining"]
    end = s["period"]["end_exclusive"]
    bills = sum(b["amount"] for b in s["bills"] if b["type"] == "EXPENSE" and b["next_date"] < end)
    after = left - bills
    _check(after == 79131, f"left after bills {after}")
    return {"after_bills": major(after), "short": major(120000 - after), "left_before_bills_wrong": major(left)}


@truth
def v3_rent_share(q, ctx):
    page = _page(q, -1)
    rent = _spent([t for t in page["transactions"] if t["type"] == "EXPENSE" and _note_has(t, "loyer")])
    _check(rent == 142500 and page["income"] == 588640, f"rent {rent}, income {page['income']}")
    return {"pct": round(rent / page["income"] * 100, 2), "rent": major(rent), "income": major(page["income"])}


@truth
def v3_cash_withdrawn(q, ctx):
    cash = _acct(q, "Cash")
    rows = [t for t in _all_txs(q) if t["type"] == "TRANSFER" and t.get("to_account_id") == cash and t["date"] >= YEAR]
    return {"total": major(sum(t["amount"] for t in rows)), "times": len(rows)}


@truth
def v3_salary_raise(q, ctx):
    pays = [t for t in _all_txs(q) if t["type"] == "INCOME" and t.get("category") == "Salary"]
    steps = [(a, b) for a, b in zip(pays, pays[1:]) if b["amount"] != a["amount"]]
    _check(len(steps) == 1, f"one raise: {[(a['date'], a['amount'], b['date'], b['amount']) for a, b in steps]}")
    before, after = steps[0]
    gaps = {(dt.date.fromisoformat(b["date"]) - dt.date.fromisoformat(a["date"])).days for a, b in zip(pays, pays[1:])}
    _check(gaps == {14}, f"paid every 14 days: {gaps}")
    more = after["amount"] - before["amount"]
    return {"first_raised": after["date"], "per_cheque": major(more), "yearly": major(more * 26)}


@truth
def v3_priciest_month(q, ctx):
    series = q("money.series", {"by": "period", "periods": 14})
    values = series["series"][0]["values"]
    i = max(range(len(values)), key=lambda k: values[k])
    _check(series["labels"][i] == "Apr 15 2026", f"the priciest period is {series['labels'][i]}")
    page = _page(q, i - (len(values) - 1))
    _check(page["period"]["start"] == "2026-04-15" and page["spent"] == values[i], f"period {page['period']} spent {page['spent']}")
    big = sorted(_expenses(page["transactions"]), key=lambda t: -t["amount"])[:3]
    return {"spent": major(values[i]), "start": "2026-04-15", "biggest": [(t["note"], major(t["amount"])) for t in big]}


@truth
def v3_top_category_ytd(q, ctx):
    by = {}
    for t in _expenses(_all_txs(q), since=YEAR):
        by[t.get("category")] = by.get(t.get("category"), 0) + t["amount"]
    ranked = sorted(((k, v) for k, v in by.items() if k != "Housing"), key=lambda kv: -kv[1])
    _check(ranked[0][0] == "Groceries", f"after rent: {ranked[:2]}")
    return {"category": ranked[0][0], "amount": major(ranked[0][1]), "second": ranked[1][0]}


@truth
def v3_uncat_all(q, ctx):
    rows = [t for t in _expenses(_all_txs(q)) if not t.get("category")]
    _check(len(rows) == 7, f"seven with no category: {[t['note'] for t in rows]}")
    return {"total": major(_spent(rows)), "entries": len(rows)}


@truth
def v3_rent_unchanged(q, ctx):
    rents = [t for t in _expenses(_all_txs(q)) if _note_has(t, "loyer")]
    _check(len({t["amount"] for t in rents}) == 1, f"one rent amount: {sorted({t['amount'] for t in rents})}")
    return {"rent": major(rents[0]["amount"]), "months": len(rents)}


@truth
def v3_no_starbucks(q, ctx):
    _check(not [t for t in _all_txs(q) if _note_has(t, "starbucks")], "no Starbucks in the ledger")
    return {"spent": 0}


@truth
def v3_last_weekend(q, ctx):
    # Today is Friday Sep 18: last weekend is Sat Sep 12 and Sun Sep 13.
    rows = _expenses(_all_txs(q), since="2026-09-12", until="2026-09-13")
    return {"spent": major(_spent(rows)), "entries": [(t["date"], t["note"], major(t["amount"])) for t in rows]}


@truth
def v3_tim_chain(q, ctx):
    def tim(offset):
        return _spent([t for t in _page(q, offset)["transactions"] if t["type"] == "EXPENSE" and _note_has(t, "tim hortons")])
    last, before = tim(-1), tim(-2)
    _check(last and before, "Tim Hortons in both periods")
    return {"last": major(last), "before": major(before), "together": major(last + before)}


# ══ Tally: writes, some corrected in a second turn ═════════════════════════════════════════════


@truth
def v3_write_targets(q, ctx):
    out = {}
    for name in ("Desjardins chequing", "Desjardins Visa", "Triangle Mastercard", "Cash"):
        out[name] = _acct(q, name)
    for name in ("Groceries", "Dining", "Shopping", "Other"):
        out["cat " + name] = _category_id(q, name)
    return out


_register_effect("v3_eff_amount_fix", _new({
    "type": "EXPENSE", "amount": 1580, "date": TODAY, "account_id": lambda q: _acct(q, "Desjardins Visa"),
    "category_id": lambda q: _category_id(q, "Dining"), "to_account_id": None}))

_register_effect("v3_eff_card_fix", _new({
    "type": "EXPENSE", "amount": 4200, "date": "2026-09-17", "account_id": lambda q: _acct(q, "Triangle Mastercard"),
    "category_id": lambda q: _category_id(q, "Dining"), "to_account_id": None}))

_register_effect("v3_eff_atm", _new({
    "type": "TRANSFER", "amount": 10000, "date": TODAY, "account_id": lambda q: _acct(q, "Desjardins chequing"),
    "to_account_id": lambda q: _acct(q, "Cash"), "category_id": None}))


def _two_at_once(q, ctx, new, changed):
    """Two new expenses yesterday: 12.00 cash at the dépanneur (any everyday category, or none) and
    45.30 at Metro on the Visa in Groceries."""
    if changed:
        return False, f"entries changed: {[_fmt(c) for _, c in changed]}"
    if len(new) != 2:
        return False, f"{len(new)} new entries: {[_fmt(t) for t in new]}"
    by = {t["amount"]: t for t in new}
    dep, metro = by.get(1200), by.get(4530)
    if not dep or not metro:
        return False, f"want 12.00 and 45.30: {[_fmt(t) for t in new]}"
    everyday = {None} | {_category_id(q, n) for n in ("Groceries", "Other", "Dining", "Shopping")}
    checks = [
        (dep, {"type": "EXPENSE", "date": "2026-09-17", "account_id": _acct(q, "Cash")}),
        (metro, {"type": "EXPENSE", "date": "2026-09-17", "account_id": _acct(q, "Desjardins Visa"), "category_id": _category_id(q, "Groceries")}),
    ]
    for t, want in checks:
        bad = {k: t.get(k) for k, v in want.items() if t.get(k) != v}
        if bad:
            return False, f"{_fmt(t)}: {bad} differ from {want}"
    if dep.get("category_id") not in everyday:
        return False, f"the dépanneur went under category {dep.get('category_id')}"
    return True, f"added {_fmt(dep)} and {_fmt(metro)}"


_register_effect("v3_eff_two_at_once", _two_at_once)


#: The three uncategorized entries the recategorizing task names, by their notes, and where each goes.
RECAT = (("SAQ", "Dining"), ("AMZN Mktp", "Shopping"), ("PAYPAL *XYZSTORE", "Shopping"))


def _recat_ids(txs):
    """{id: category name} of RECAT's entries among `txs` (dicts with `note`), each found once."""
    out = {}
    for word, cat in RECAT:
        hits = [t for t in txs if (t.get("note") or "") == word or (word != "SAQ" and word.lower() in (t.get("note") or "").lower())]
        _check(len(hits) == 1, f"one entry noted {word}: {hits}")
        out[hits[0]["id"]] = cat
    return out


@truth
def v3_recat_three(q, ctx):
    ids = _recat_ids(_all_txs(q))
    _check(all(not t.get("category") for t in _all_txs(q) if t["id"] in ids), "all three have no category")
    return {"targets": {str(k): v for k, v in ids.items()}}


def _recat_three(q, ctx, new, changed):
    """The three entries now in their categories, each changed in nothing else; or one proposal
    that makes exactly those changes, for the person to approve."""
    if new:
        return False, f"new entries instead of changes: {[_fmt(t) for t in new]}"
    before = [dict(v, id=int(k)) for k, v in ctx["before"]["state"]["txs"].items()]
    want = {i: _category_id(q, cat) for i, cat in _recat_ids(before).items()}
    if changed:
        got = {cur["id"]: cur for _, cur in changed}
        if set(got) != set(want):
            return False, f"changed {sorted(got)}, want {sorted(want)}"
        for old, cur in changed:
            if cur["category_id"] != want[cur["id"]]:
                return False, f"#{cur['id']} went to category {cur['category_id']}, want {want[cur['id']]}"
            others = {k: (old.get(k), cur.get(k)) for k in ("type", "amount", "date", "account_id", "to_account_id", "note") if old.get(k) != cur.get(k)}
            if others:
                return False, f"#{cur['id']} changed beyond its category: {others}"
        return True, "the three are in their categories"
    proposed = {}
    for p in thread_tool_inputs(ctx, "thread.propose"):
        for item in (p.get("action") or {}).get("money.tx.update") or []:
            proposed[item.get("id")] = item
    if set(proposed) == set(want):
        return True, f"proposed the three changes: {sorted(proposed)}"
    return False, f"nothing changed, and the proposal holds {sorted(proposed)}"


_register_effect("v3_eff_recat_three", _recat_three)


# ══ Avex ═══════════════════════════════════════════════════════════════════════════════════════


def _monday(day):
    d = dt.date.fromisoformat(day)
    return d - dt.timedelta(days=d.weekday())


@truth
def v3_missed_plan(q, ctx):
    rows = _counted_sessions(q)
    first = _monday(min(s["day"] for s in rows))
    this = _monday(TODAY)
    weeks = (this - first).days // 7
    done = len([s for s in rows if _monday(s["day"]) < this])
    _check(first.isoformat() == "2025-12-15" and weeks == 39, f"from {first}, {weeks} whole weeks")
    return {"missed": 4 * weeks - done, "missed_with_this_week": 4 * (weeks + 1) - len(rows), "weeks": weeks, "done": done}


@truth
def v3_squat_since_january(q, ctx):
    lift = _lift(q, "Back Squat")
    now = lift["row"]["best"]["e1rm"]
    jan = [h for h in lift["history"] if h["day"].startswith("2026-01") and h.get("e1rm")]
    first = min(jan, key=lambda h: h["day"])["e1rm"]
    best_jan = max(h["e1rm"] for h in jan)
    return {"gain": [round(now - best_jan, 1), round(now - first, 1)], "now": now, "january_best": best_jan, "january_first": first}


@truth
def v3_km_since_race(q, ctx):
    runs = [c for c in _cardio(q) if c["kind"] == "run"]
    race = [c for c in runs if "10 km" in (c.get("note") or "")]
    _check(len(race) == 1, "one 10 km race")
    after = [c for c in runs if c["day"] > race[0]["day"]]
    return {"km": round(sum(c["distance_km"] for c in after), 2), "runs": len(after),
            "with_race_wrong": round(sum(c["distance_km"] for c in after) + race[0]["distance_km"], 2)}


@truth
def v3_upper_a_minutes(q, ctx):
    mins = [s["minutes"] for s in _counted_sessions(q) if s["title"] == "Upper A"]
    return {"minutes": [round(statistics.mean(mins), 1), statistics.median(mins)], "sessions": len(mins)}


@truth
def v3_deadlift_august_sets(q, ctx):
    august = [s for s in q("gym.sessions", {"limit": 1000})["sessions"] if s["day"].startswith("2026-08") and "Deadlift" in s["lifts"]]
    work = lambda s: sum(1 for e in q("gym.session.get", {"id": s["id"]})["exercises"] if e["name"] == "Deadlift" for x in e["sets"] if x["kind"] == "work")
    counted = sum(work(s) for s in august if not s.get("untracked"))
    with_untracked = sum(work(s) for s in august)
    return {"sets": sorted({counted, with_untracked}), "workouts": len([s for s in august if not s.get("untracked")])}


@truth
def v3_best_volume_week(q, ctx):
    series = q("gym.series", {"measure": "volume", "by": "week", "periods": 45})
    values = series["series"][0]["values"] if "series" in series else series["values"]
    i = max(range(len(values)), key=lambda k: values[k])
    _check(sorted(values)[-1] > sorted(values)[-2], "one week is first")
    return {"week": series["labels"][i], "volume": values[i]}


@truth
def v3_squat_goal_pct(q, ctx):
    goals = [g for g in q("gym.summary")["goals"] if g["kind"] == "lift_1rm" and g["target_key"] == "squat"]
    _check(len(goals) == 1, f"one squat goal: {goals}")
    target, best = goals[0]["target_value"], goals[0]["best"]["e1rm"]
    _check(best == _lift(q, "Back Squat")["row"]["best"]["e1rm"], "the goal's best is the lift's")
    _check(target == 140, f"the squat goal is {target}")
    return {"pct": round(best / target * 100, 2), "to_go": round(target - best, 1)}


@truth
def v3_cycle_months(q, ctx):
    rides = [c for c in _cardio(q) if c["kind"] == "cycle"]
    jul = round(sum(c["distance_km"] for c in rides if c["day"].startswith("2026-07")), 2)
    aug = round(sum(c["distance_km"] for c in rides if c["day"].startswith("2026-08")), 2)
    _check(jul > aug, "July is the longer")
    return {"july": jul, "august": aug}


@truth
def v3_ohp_attempts(q, ctx):
    lift = _lift(q, "Overhead Press")
    top = max(h["top_weight"] for h in lift["history"] if h.get("top_weight"))
    _check(top == 52.5, f"heaviest {top}")
    hist = [h for h in lift["history"] if h.get("top_weight") == top]
    _check("heaviest_sessions" not in lift["row"] or len(hist) == lift["row"]["heaviest_sessions"], f"{len(hist)} workouts at {top} vs the engine's {lift['row'].get('heaviest_sessions')}")
    return {"times": len(hist)}


@truth
def v3_dumbbell_since_switch(q, ctx):
    hist = sorted(_lift(q, "Dumbbell Bench Press")["history"], key=lambda h: h["day"])
    first, last = hist[0], hist[-1]
    _check(first["day"] == "2026-07-06", f"the switch was {first['day']}")
    return {"gain": last["top_weight"] - first["top_weight"], "from": first["top_weight"], "to": last["top_weight"]}


@truth
def v3_rest_day_last_week(q, ctx):
    days = {(dt.date(2026, 9, 7) + dt.timedelta(days=i)).isoformat() for i in range(7)}
    busy = {s["day"] for s in q("gym.sessions", {"limit": 1000})["sessions"] if s["day"] in days}
    busy |= {c["day"] for c in _cardio(q) if c["day"] in days}
    idle = sorted(days - busy)
    _check(idle == ["2026-09-11"], f"the idle days: {idle}")
    return {"day": "Friday", "date": idle[0]}


@truth
def v3_heaviest_deadlift_lb(q, ctx):
    lift = _lift(q, "Deadlift")
    row, kg = lift["row"], max(h["top_weight"] for h in lift["history"] if h.get("top_weight"))
    _check(kg == 165.0 and row.get("heaviest", {"weight": kg})["weight"] == kg, f"heaviest {kg}, the engine's {row.get('heaviest')}")
    return {"lb": round(kg / 0.45359237, 1), "kg": kg, "best_set_lb_wrong": round(row["best"]["weight"] / 0.45359237, 1)}


@truth
def v3_no_steps(q, ctx):
    return {"steps": None}


@truth
def v3_kept_training(q, ctx):
    august = _workouts_between(q, "2026-08-01", "2026-08-31")
    last_week = _workouts_between(q, "2026-09-07", "2026-09-13")
    _check(august >= 12 and last_week == 4, f"august {august}, last week {last_week}")
    return {"august": august, "last_week": last_week}


@truth
def v3_month_chain(q, ctx):
    rows = _counted_sessions(q)
    jul = [s for s in rows if s["day"].startswith("2026-07")]
    aug = [s for s in rows if s["day"].startswith("2026-08")]
    vol = lambda xs: round(sum(s["volume"] for s in xs))
    _check(vol(aug) > vol(jul), "August had more volume")
    return {"july": len(jul), "august": len(aug), "july_volume": vol(jul), "august_volume": vol(aug)}


@truth
def v3_before_trip(q, ctx):
    # The last workout before the trip was Thu Jun 18; the flight was Sat Jun 20.
    by_days = _workouts_between(q, "2026-05-23", "2026-06-19")
    by_weeks = _workouts_between(q, "2026-05-25", "2026-06-21")
    return {"workouts": sorted({by_days, by_weeks})}


# ══ Both apps ══════════════════════════════════════════════════════════════════════════════════


@truth
def v3_gym_cost_year(q, ctx):
    fees = [t for t in _expenses(_all_txs(q), since=YEAR) if _note_has(t, "bloc shop")]
    _check(len(fees) == 9, f"nine Bloc Shop charges Jan-Sep: {[t['date'] for t in fees]}")
    workouts = _workouts_between(q, YEAR, TODAY)
    return {"per_workout": round(_spent(fees) / workouts / 100, 2), "fees": major(_spent(fees)), "workouts": workouts}


@truth
def v3_trip_both(q, ctx):
    visa = _acct(q, "Desjardins Visa")
    rows = [t for t in _expenses(_all_txs(q), since="2026-06-20", until="2026-07-05") if t["account_id"] == visa]
    trip = [t for t in rows if not _note_has(t, "bloc shop") and not _note_has(t, "mondou")]
    workouts = _workouts_between(q, "2026-06-20", "2026-07-05")
    _check(workouts == 0, f"{workouts} counted workouts on the trip")
    hikes = [c for c in _cardio(q) if c["kind"] == "hike" and "2026-06-20" <= c["day"] <= "2026-07-05"]
    return {"visa": [major(_spent(rows)), major(_spent(trip))], "workouts": workouts, "hikes": len(hikes)}


@truth
def v3_race_month_health(q, ctx):
    race = [c for c in _cardio(q) if "10 km" in (c.get("note") or "")][0]
    _check(race["day"] == "2026-05-24", f"the race was {race['day']}")
    rows = [t for t in _expenses(_all_txs(q), since="2026-05-15", until="2026-06-14") if t.get("category") == "Health"]
    calendar = [t for t in _expenses(_all_txs(q), since="2026-05-01", until="2026-05-31") if t.get("category") == "Health"]
    return {"health": major(_spent(rows)), "calendar_may_wrong": major(_spent(calendar))}


@truth
def v3_bixi_and_km(q, ctx):
    bixi = [t for t in _expenses(_all_txs(q), since=YEAR) if _note_has(t, "bixi")]
    km = round(sum(c["distance_km"] for c in _cardio(q) if c["kind"] == "cycle" and c["day"] >= YEAR), 1)
    return {"bixi": major(_spent(bixi)), "km": km}


@truth
def v3_month_vs_month(q, ctx):
    last, before = _page(q, -1), _page(q, -2)
    return {"spent_last": major(last["spent"]), "spent_before": major(before["spent"]),
            "workouts_last": _workouts_between(q, "2026-08-15", "2026-09-14"),
            "workouts_before": _workouts_between(q, "2026-07-15", "2026-08-14")}


@truth
def v3_weekend_both(q, ctx):
    spent = _spent(_expenses(_all_txs(q), since="2026-09-12", until="2026-09-13"))
    workouts = [s["title"] for s in _counted_sessions(q) if s["day"] in ("2026-09-12", "2026-09-13")]
    runs = [c for c in _cardio(q) if c["day"] in ("2026-09-12", "2026-09-13")]
    _check(workouts == ["Lower B"] and len(runs) == 1, f"workouts {workouts}, cardio {runs}")
    return {"spent": major(spent), "workout": workouts[0], "run_km": runs[0]["distance_km"]}


__all__ = ["TruthError"]
