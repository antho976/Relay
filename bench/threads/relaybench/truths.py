"""Expected answers, computed at run time from the throwaway engine (contract: "Truth function").

    @truth
    def spent_this_month(q, ctx):
        ...
        return {"spent": 1234.56}

`q(op, payload=None)` calls the op as the person through the CLI and returns its parsed result with
the bus's own integers (minor units, e8, bps): never the MCP door's decimals, so a change to what
the model is shown cannot move the answer key. `ctx` holds 'today' ('2026-09-18'), 'currency', and
for an `effect` check after the turn, 'before' (what this same function returned before the turn)
and 'thread' (the collected turns, as grade.py describes). A truth returns named values, money as
floats in major units, a list for several acceptable values; an effect truth returns
{'ok': bool, 'why': str} when ctx has 'before', and whatever it needs to compare otherwise.
Compute independently where it is cheap (add the entries up yourself) rather than quoting the
reading the model will quote.
"""

TRUTHS = {}


def truth(fn):
    """Register a truth function under its own name."""
    if fn.__name__ in TRUTHS:
        raise ValueError(f"truth {fn.__name__} is defined twice")
    TRUTHS[fn.__name__] = fn
    return fn


# ── shared helpers ───────────────────────────────────────────────────────


def major(minor, digits=2):
    """Minor units → a float in major units: 15799 → 157.99."""
    return round(minor / (10 ** digits), digits)


def e8(value):
    """An integer at 1e-8 → a float: 150000000 → 1.5."""
    return value / 1e8


def entries(q, offset=0, **filters):
    """Every entry of one budget period (0 this one, -1 the one before), newest first."""
    payload = {"period_offset": offset, "limit": 5000, **filters}
    return q("money.tx.list", payload)["transactions"]


def expenses(txs, category=None):
    return [t for t in txs if t["type"] == "EXPENSE" and (category is None or (t.get("category") or "").lower() == category.lower())]


def total(txs):
    return sum(t["amount"] for t in txs)


def category(q, name):
    """A category row by name, any case."""
    for c in q("money.lists")["categories"]:
        if c["name"].lower() == name.lower():
            return c
    raise KeyError(f"no category {name!r} in the ledger")


def budget_row(q, name):
    for b in q("money.summary")["budgets"]:
        if b["name"].lower() == name.lower():
            return b
    raise KeyError(f"no budget for {name!r}")


def all_entry_ids(q, periods=range(-24, 1)):
    ids = set()
    for off in periods:
        ids |= {t["id"] for t in entries(q, off)}
    return ids


def thread_tool_inputs(ctx, op):
    """The inputs of every call the thread made to `op` (e.g. "money.tx.add")."""
    want = op.replace(".", "_")
    calls = [c for t in (ctx.get("thread") or {}).get("turns", []) for c in t.get("tool_calls", [])]
    return [c.get("input") or {} for c in calls if (c.get("op") or "").replace(".", "_") == want]


# ══ smoke (bench/threads/tasks/smoke.json) ═══════════════════════════════
# A starter set that exercises every kind of check end to end. The full suite's truths follow it.


@truth
def smoke_spent_this_month(q, ctx):
    summary = q("money.summary")
    mine = total(expenses(entries(q, 0)))
    values = sorted({major(mine), major(summary["spent"])})
    return {"spent": values}


@truth
def smoke_groceries_last_month(q, ctx):
    return {"groceries": major(total(expenses(entries(q, -1), "Groceries")))}


@truth
def smoke_biggest_expense(q, ctx):
    exp = expenses(entries(q, 0))
    top = max(exp, key=lambda t: t["amount"])
    ties = [t for t in exp if t["amount"] == top["amount"]]
    names = sorted({x for t in ties for x in (t.get("note"), t.get("category")) if x})
    return {"amount": major(top["amount"]), "names": names}


@truth
def smoke_groceries_budget_left(q, ctx):
    b = budget_row(q, "Groceries")
    spent = total(expenses(entries(q, 0), "Groceries"))
    return {"left": major(b["budget"] - spent), "budget": major(b["budget"])}


@truth
def smoke_net_worth(q, ctx):
    lists = q("money.lists")
    return {"net_worth": major(sum(a["balance"] for a in lists["accounts"] if not a["archived"]))}


@truth
def smoke_invest_value(q, ctx):
    p = q("money.invest.summary")
    return {"value": major(p["value"]), "as_of": p["as_of"]}


@truth
def smoke_coffee_added(q, ctx):
    """Effect: one new EXPENSE of 4.75 dated today on the Visa, nothing removed."""
    now = {t["id"]: t for t in entries(q, 0)}
    if "before" not in ctx:
        return {"ids": sorted(now), "all": len(all_entry_ids(q))}
    before = set(ctx["before"]["ids"])
    gone = before - set(now)
    new = [now[i] for i in set(now) - before]
    if gone:
        return {"ok": False, "why": f"entries removed: {sorted(gone)}"}
    if len(new) != 1:
        return {"ok": False, "why": f"{len(new)} new entries: {[(t['amount'], t['note']) for t in new]}"}
    t = new[0]
    if t["type"] != "EXPENSE" or t["amount"] != 475 or t["date"] != ctx["today"] or "visa" not in t["account"].lower():
        return {"ok": False, "why": f"added {t['type']} {t['amount']} on {t['date']} to {t['account']} ({t.get('category')}, {t.get('note')!r})"}
    return {"ok": True, "why": f"added 4.75 on {t['date']} to {t['account']} in {t.get('category')} ({t.get('note')!r})"}


@truth
def smoke_workouts_last_week(q, ctx):
    s = q("gym.summary")
    start = s["last_week"]["start"]
    import datetime as dt
    end = (dt.date.fromisoformat(start) + dt.timedelta(days=7)).isoformat()
    rows = q("gym.sessions", {"limit": 500})["sessions"]
    mine = [r for r in rows if start <= r["day"] < end and not r["untracked"]]
    return {"sessions": sorted({len(mine), s["last_week"]["sessions"]}), "start": start}


@truth
def smoke_heaviest_bench(q, ctx):
    lift = q("gym.lift.get", {"name": "Bench Press"})
    top = max(h["top_weight"] for h in lift["history"] if h["top_weight"] is not None)
    return {"weight": top, "unit": lift["unit"]}


@truth
def smoke_nothing(q, ctx):
    """For tasks graded on the reply's shape alone (a chart, a refusal)."""
    return {}


# ─────────────────────────────────────────────────────────────────────────────────────────────
# The suite's answer keys (tasks/tally.json, tasks/avex.json, tasks/both.json).
#
# Every value is read from the throwaway engine through q() as raw integers (minor units, e8) and
# turned into the person's units here, so a change in how the MCP door formats results never moves
# the key. Where it is cheap, a figure is computed from the entries themselves and then asserted
# equal to the engine's own total, so a key that drifts from the engine fails `bench.py run --check`
# instead of grading answers against a wrong number.
#
# Effect truths (`eff_*`) are called before a turn (no ctx['before']: they return a snapshot of the
# ledger under 'state') and after it (ctx['before'] is that first result): {'ok', 'why'}.
# ─────────────────────────────────────────────────────────────────────────────────────────────

import datetime as _dt  # noqa: E402

_WRITE_OPS = ("money.tx.add", "money.tx.update", "money.tx.restore", "money.invest.add")


class TruthError(AssertionError):
    """The answer key disagrees with the engine, or the fixed world is not the one the task was
    written for."""


def _check(cond, msg):
    if not cond:
        raise TruthError(msg)


def _day(s):
    return _dt.date.fromisoformat(s[:10])


def _today(ctx):
    return _day(ctx.get("today") or "2026-09-18")


def _iso(d):
    return d.isoformat()


def _last_weekday(today, weekday):
    """The most recent `weekday` (Monday 0) strictly before today."""
    back = (today.weekday() - weekday) % 7 or 7
    return today - _dt.timedelta(days=back)


def _monday(d):
    return d - _dt.timedelta(days=d.weekday())


# ── money ─────────────────────────────────────────────────────────────────────────────────────

def _fd(obj):
    return int(obj.get("fraction_digits", 2))


def _major(minor, fd=2):
    return round(minor / 10 ** fd, fd)


def _summary(q, ctx):
    return q("money.summary", {"today": _iso(_today(ctx))})


def _lists(q):
    return q("money.lists", {})


def _page(q, ctx, offset=0, **kw):
    p = {"period_offset": offset, "limit": 2000, "today": _iso(_today(ctx))}
    p.update(kw)
    return q("money.tx.list", p)


def _expenses(page):
    return [t for t in page["transactions"] if t["type"] == "EXPENSE"]


def _sum(txs):
    return sum(t["amount"] for t in txs)


def _category(q, name):
    hits = [c for c in _lists(q)["categories"] if c["name"].lower() == name.lower()]
    _check(len(hits) == 1, f"no single category named {name}")
    return hits[0]


def _account(q, kind=None, name=None):
    accts = _lists(q)["accounts"]
    hits = [a for a in accts if (kind is None or a["type"] == kind) and (name is None or a["name"].lower() == name.lower())]
    if kind is not None and name is None:
        # The sample household's own account of that type, not one an import created.
        hits = [a for a in hits if a["name"].startswith("Sample")] or hits
    _check(len(hits) == 1, f"no single account of type {kind} named {name}: {[a['name'] for a in hits]}")
    return hits[0]


def _period_spent(q, ctx, offset, category=None):
    """A period's spending, summed from its entries and checked against the engine's `spent`."""
    page = _page(q, ctx, offset)
    txs = _expenses(page)
    if category is not None:
        txs = [t for t in txs if (t.get("category") or "").lower() == category.lower()]
        cid = _category(q, category)["id"]
        filtered = _page(q, ctx, offset, category_id=cid)
        _check(filtered["spent"] == _sum(txs), f"{category} {offset}: engine {filtered['spent']} vs entries {_sum(txs)}")
    else:
        _check(page["spent"] == _sum(txs), f"period {offset}: engine spent {page['spent']} vs entries {_sum(txs)}")
    return _sum(txs), txs, page


def _envelope(q, ctx, name):
    s = _summary(q, ctx)
    hits = [b for b in s["budgets"] if b["name"].lower() == name.lower()]
    _check(len(hits) == 1, f"no budget for {name}")
    b = hits[0]
    spent, _, _ = _period_spent(q, ctx, 0, name)
    _check(b["spent"] == spent, f"{name} budget spent {b['spent']} vs entries {spent}")
    _check(b["left"] == b["budget"] - b["spent"], f"{name} left")
    fd = _fd(s)
    return {
        "budget": _major(b["budget"], fd),
        "spent": _major(b["spent"], fd),
        "left": _major(b["left"], fd),
        "over": _major(max(0, -b["left"]), fd),
        "over_pace": _major(b["pace_delta"], fd),
        "status": b["status"],
    }


def _bill(q, ctx, name):
    s = _summary(q, ctx)
    hits = [b for b in s["bills"] if b["name"].lower() == name.lower()]
    _check(len(hits) == 1, f"no bill named {name} in the next 30 days")
    return hits[0], _fd(s)


@truth
def spent_this_month(q, ctx):
    spent, _, _ = _period_spent(q, ctx, 0)
    s = _summary(q, ctx)
    _check(s["spent"] == spent, f"summary spent {s['spent']} vs entries {spent}")
    return {"spent": _major(spent, _fd(s))}


@truth
def spent_last_month(q, ctx):
    spent, _, _ = _period_spent(q, ctx, -1)
    return {"spent": _major(spent)}


@truth
def groceries_this_month(q, ctx):
    spent, _, _ = _period_spent(q, ctx, 0, "Groceries")
    return {"spent": _major(spent)}


@truth
def groceries_two_months(q, ctx):
    this, _, _ = _period_spent(q, ctx, 0, "Groceries")
    last, _, _ = _period_spent(q, ctx, -1, "Groceries")
    _check(last > this, "the task expects last month's groceries to be the larger")
    return {"this": _major(this), "last": _major(last)}


@truth
def transport_last_month(q, ctx):
    spent, _, page = _period_spent(q, ctx, -1, "Transport")
    _check(page["period"]["start"].endswith("-01"), "budget month starts on the 1st")
    return {"spent": _major(spent)}


@truth
def dining_july_august(q, ctx):
    aug, _, _ = _period_spent(q, ctx, -1, "Dining")
    jul, _, _ = _period_spent(q, ctx, -2, "Dining")
    _check(jul > aug, "the task expects July's dining to be the larger")
    return {"jul": _major(jul), "aug": _major(aug)}


@truth
def dining_since_july(q, ctx):
    parts = [_period_spent(q, ctx, off, "Dining")[0] for off in (-2, -1, 0)]
    series = q("money.series", {"measure": "spending", "by": "period", "periods": 3, "category": "Dining", "today": _iso(_today(ctx))})
    _check(series["series"][0]["values"] == parts, f"series {series['series'][0]['values']} vs entries {parts}")
    first = _page(q, ctx, -2)["period"]["start"]
    _check(first.endswith("-07-01"), f"period -2 starts {first}, the task says since July")
    return {"total": _major(sum(parts))}


@truth
def income_august(q, ctx):
    page = _page(q, ctx, -1)
    _check(page["period"]["start"].endswith("-08-01"), "last month is August")
    inc = [t for t in page["transactions"] if t["type"] == "INCOME"]
    _check(page["income"] == _sum(inc), "income total")
    _check(len(inc) == 3, f"August should hold three paycheques, has {len(inc)}")
    return {"income": _major(_sum(inc)), "two_cheques": _major(2 * inc[0]["amount"])}


@truth
def versus_last_month(q, ctx):
    s = _summary(q, ctx)
    elapsed = s["pace"]["elapsed_days"]
    last = _page(q, ctx, -1)
    start = _day(last["period"]["start"])
    cut = start + _dt.timedelta(days=elapsed)
    last_same = _sum([t for t in _expenses(last) if _day(t["date"]) < cut])
    diff = s["spent"] - last_same
    _check(diff < 0, "the task expects less than last month by this day")
    _check(f"${-diff // 100:,}" in s["lines"]["versus_last"] or str(round(-diff / 100)) in s["lines"]["versus_last"].replace(",", ""),
           f"line {s['lines']['versus_last']!r} vs {diff}")
    return {"less_by": [_major(-diff), float(round(-diff / 100))], "last_same_day": _major(last_same), "this": _major(s["spent"])}


@truth
def balance_chequing(q, ctx):
    return {"balance": _major(_account(q, "CHEQUING")["balance"])}


@truth
def balance_visa(q, ctx):
    b = _account(q, "CREDIT")["balance"]
    _check(b < 0, "the Visa should carry a balance owed")
    return {"owed": [_major(-b), _major(b)]}


@truth
def balance_cash(q, ctx):
    b = _account(q, "CASH")["balance"]
    _check(b < 0, "the task expects the cash account below zero")
    return {"balance": [_major(b), _major(-b)]}


@truth
def net_worth(q, ctx):
    lists = _lists(q)
    s = _summary(q, ctx)
    total = sum(a["balance"] for a in lists["accounts"] if not a["archived"])
    _check(total == s["net_worth"] == lists["net_worth"], f"net worth {s['net_worth']} vs accounts {total}")
    return {"net_worth": _major(s["net_worth"])}


@truth
def goal_lisbon(q, ctx):
    g = [g for g in _summary(q, ctx)["goals"] if "lisbon" in g["name"].lower()]
    _check(len(g) == 1, "one Lisbon goal")
    return {"saved": _major(g[0]["saved"]), "target": _major(g[0]["target"]), "to_go": _major(g[0]["target"] - g[0]["saved"])}


@truth
def bills_next_10_days(q, ctx):
    s = _summary(q, ctx)
    soon = [b for b in s["bills"] if b["days_until"] <= 10]
    pay = [b for b in soon if b["type"] == "EXPENSE"]
    _check([b["name"] for b in pay] == ["Internet"], f"expense bills within 10 days: {[b['name'] for b in pay]}")
    _check({b["type"] for b in soon} >= {"INCOME", "TRANSFER"}, "the trap needs income and a transfer within 10 days")
    return {"internet": _major(pay[0]["amount"]), "due": pay[0]["next_date"]}


@truth
def rent_next(q, ctx):
    b, fd = _bill(q, ctx, "Rent")
    _check(b["next_date"] == "2026-10-01", f"rent next {b['next_date']}: the task's text check names October 1")
    return {"date": b["next_date"], "amount": _major(b["amount"], fd)}


@truth
def rent_amount(q, ctx):
    b, fd = _bill(q, ctx, "Rent")
    _, txs, _ = _period_spent(q, ctx, 0, "Housing")
    _check([t["amount"] for t in txs] == [b["amount"]], "this month's rent entry matches the bill")
    return {"rent": _major(b["amount"], fd)}


@truth
def salary_next(q, ctx):
    b, fd = _bill(q, ctx, "Salary")
    _check(b["type"] == "INCOME", "salary is income")
    _check(b["next_date"] == "2026-09-27", f"salary next {b['next_date']}: the task's text check names September 27")
    return {"date": b["next_date"], "amount": _major(b["amount"], fd)}


@truth
def biggest_this_month(q, ctx):
    _, txs, _ = _period_spent(q, ctx, 0)
    top = max(txs, key=lambda t: t["amount"])
    _check(top["note"] == "Rent", f"biggest is {top['note']}")
    return {"amount": _major(top["amount"]), "note": top["note"]}


def _biggest_not_rent(q, ctx, offset):
    _, txs, _ = _period_spent(q, ctx, offset)
    rest = sorted((t for t in txs if (t.get("category") or "") != "Housing"), key=lambda t: -t["amount"])
    _check(rest[0]["amount"] > rest[1]["amount"], "a unique biggest entry")
    return {"amount": _major(rest[0]["amount"]), "note": rest[0]["note"], "date": rest[0]["date"]}


@truth
def biggest_not_rent_last_month(q, ctx):
    r = _biggest_not_rent(q, ctx, -1)
    _check(r["note"] == "Books" and r["date"].endswith("-08-14"), f"{r}")
    return r


@truth
def smallest_this_month(q, ctx):
    _, txs, _ = _period_spent(q, ctx, 0)
    low = sorted(txs, key=lambda t: t["amount"])
    _check(low[0]["amount"] < low[1]["amount"], "a unique smallest entry")
    _check("olimpico" in low[0]["note"].lower(), f"smallest is {low[0]['note']}: the task's text check names Olimpico")
    return {"amount": _major(low[0]["amount"]), "note": low[0]["note"]}


@truth
def top_category_not_housing_last_month(q, ctx):
    _, txs, _ = _period_spent(q, ctx, -1)
    by = {}
    for t in txs:
        by[t["category"]] = by.get(t["category"], 0) + t["amount"]
    ranked = sorted(((v, k) for k, v in by.items() if k != "Housing"), reverse=True)
    _check(ranked[0][1] == "Groceries" and ranked[0][0] > ranked[1][0], f"{ranked[:2]}")
    return {"amount": _major(ranked[0][0]), "name": ranked[0][1]}


@truth
def spent_yesterday(q, ctx):
    y = _iso(_today(ctx) - _dt.timedelta(days=1))
    _, txs, _ = _period_spent(q, ctx, 0)
    day = [t for t in txs if t["date"] == y]
    _check(len(day) >= 1, "something was spent yesterday")
    return {"total": _major(_sum(day)), "count": len(day)}


@truth
def spent_today(q, ctx):
    t0 = _iso(_today(ctx))
    _, txs, _ = _period_spent(q, ctx, 0)
    day = sorted(t["amount"] for t in txs if t["date"] == t0)
    _check(day == [5500, 7840], f"today's entries {day}: the task's text check names 78.40 and 55.00")
    return {"total": _major(sum(day)), "items": [_major(a) for a in day]}


@truth
def spent_week_of_sep_7(q, ctx):
    lo, hi = _dt.date(2026, 9, 7), _dt.date(2026, 9, 13)
    _check(lo.weekday() == 0 and hi.weekday() == 6, "Monday to Sunday")
    _, txs, _ = _period_spent(q, ctx, 0)
    wk = [t for t in txs if lo <= _day(t["date"]) <= hi]
    return {"total": _major(_sum(wk)), "count": len(wk)}


@truth
def pace_overall(q, ctx):
    s = _summary(q, ctx)
    p = s["pace"]
    spent, _, _ = _period_spent(q, ctx, 0)
    _check(p["spent"] == spent, "pace spent")
    _check(p["status"] == "OVER_PACE" and p["pace_delta"] > 0, f"pace status {p['status']}")
    _check(p["pace_delta"] == p["spent"] - p["expected"], "pace delta")
    return {"over_pace": _major(p["pace_delta"]), "remaining": _major(p["remaining"]), "days_left": p["days_left"]}


@truth
def daily_allowance(q, ctx):
    p = _summary(q, ctx)["pace"]
    _check(p["daily_allowance"] == p["remaining"] // p["days_left"], "allowance")
    exact = _major(p["daily_allowance"])
    return {"per_day": [exact, float(round(p["remaining"] / p["days_left"] / 100))]}


@truth
def budget_transport(q, ctx):
    e = _envelope(q, ctx, "Transport")
    _check(e["status"] == "OVER_BUDGET", f"transport is {e['status']}")
    return e


@truth
def budget_groceries(q, ctx):
    e = _envelope(q, ctx, "Groceries")
    _check(e["status"] == "OVER_PACE" and e["left"] > 0, f"groceries is {e['status']}, the trap needs over pace but under budget")
    return e


@truth
def budget_dining(q, ctx):
    e = _envelope(q, ctx, "Dining")
    _check(e["status"] == "OVER_PACE" and e["left"] > 0, f"dining is {e['status']}")
    return e


@truth
def budget_entertainment(q, ctx):
    e = _envelope(q, ctx, "Entertainment")
    _check(0 < e["left"] < 40, f"entertainment left {e['left']}: the task asks about a $40 concert")
    e["short"] = round(40 - e["left"], 2)
    return e


@truth
def no_pet_category(q, ctx):
    cats = [c["name"] for c in _lists(q)["categories"] if "pet" in c["name"].lower()]
    _check(not cats, f"a pet category exists: {cats}")
    hits = _page(q, ctx, 0, query="pet")["transactions"]
    _check(not hits, "no entry mentions pets")
    return {"count": 0}


@truth
def june_is_empty(q, ctx):
    page = _page(q, ctx, -3)
    _check(page["period"]["start"].endswith("-06-01"), "period -3 is June")
    _check(not page["transactions"], "June has no entries")
    return {"count": 0}


@truth
def nothing(q, ctx):
    """Tasks whose checks need no figure (charts, refusals, asks)."""
    return {}


# ── investments ───────────────────────────────────────────────────────────────────────────────

def _portfolio(q, ctx):
    return q("money.invest.summary", {"today": _iso(_today(ctx))})


@truth
def invest_value(q, ctx):
    p = _portfolio(q, ctx)
    val = sum(h["value"] for h in p["holdings"]) + p["cash"]
    _check(val == p["value"], f"holdings {val} vs value {p['value']}")
    _check(p["as_of"] and (_today(ctx) - _day(p["as_of"])).days > 31, "the task expects values more than a month old")
    return {"value": _major(p["value"]), "as_of": p["as_of"], "gain": _major(p["gain"])}


@truth
def invest_gain(q, ctx):
    p = _portfolio(q, ctx)
    _check(p["gain"] == p["value"] - p["book"], "gain")
    _check(p["gain_bps"] == round(p["gain"] * 10000 / p["book"]), "gain bps")
    return {"gain": _major(p["gain"]), "pct": [p["gain_bps"] / 100, round(p["gain_bps"] / 100, 1)]}


@truth
def invest_top_holding(q, ctx):
    hs = sorted(_portfolio(q, ctx)["holdings"], key=lambda h: -h["value"])
    _check(hs[0]["value"] > hs[1]["value"] and hs[0]["symbol"] == "AAPL", f"top {hs[0]['symbol']}")
    return {"symbol": hs[0]["symbol"], "value": _major(hs[0]["value"]), "weight": hs[0]["weight_bps"] / 100}


@truth
def invest_xeqt(q, ctx):
    h = [h for h in _portfolio(q, ctx)["holdings"] if h["symbol"] == "XEQT"]
    _check(len(h) == 1, "one XEQT holding")
    units = h[0]["quantity"] / 100_000_000
    _check(units == 10, f"XEQT units {units}: the task's text check names 10")
    return {"units": units, "value": _major(h[0]["value"])}


@truth
def invest_aapl(q, ctx):
    h = [h for h in _portfolio(q, ctx)["holdings"] if h["symbol"] == "AAPL"]
    _check(len(h) == 1 and h[0]["fx_estimated"], "AAPL valued at an estimated rate")
    return {"value": _major(h[0]["value"]), "fx_estimated": True}


@truth
def invest_etf_share(q, ctx):
    p = _portfolio(q, ctx)
    etf = sum(h["value"] for h in p["holdings"] if h["kind"] == "ETF")
    kinds = {k["kind"]: k for k in p["kinds"]}
    _check(kinds["ETF"]["value"] == etf, "ETF value")
    bps = kinds["ETF"]["share_bps"]
    _check(abs(bps - etf * 10000 / p["value"]) <= 0.5, "ETF share")
    return {"pct": [bps / 100, round(bps / 100, 1)]}


@truth
def invest_room(q, ctx):
    rows = [r for r in _portfolio(q, ctx)["room"] if r["registration"] == "TFSA"]
    _check(len(rows) == 1 and rows[0]["room"] is None, "TFSA room is not known to Tally")
    return {"limit": _major(rows[0]["limit"]) if rows[0]["limit"] else None, "year": rows[0]["year"]}


@truth
def invest_income(q, ctx):
    p = _portfolio(q, ctx)
    _check(p["income_12m"] == 0 and not p["income"], "no dividends or interest recorded")
    divs = q("money.invest.list", {"type": "DIVIDEND"})["activities"]
    _check(not divs, "no dividend activities")
    return {"income": 0.0}


@truth
def sample_tfsa(q, ctx):
    a = _account(q, "INVESTMENT", "Sample TFSA")
    acct = [x for x in _portfolio(q, ctx)["accounts"] if x["id"] == a["id"]]
    _check(acct and acct[0]["holdings"] == 0 and acct[0]["value"] == 0, "Sample TFSA is valued by hand")
    _check(a["balance"] > 0, "it has a balance")
    return {"balance": _major(a["balance"])}


# ── effects: the ledger after a turn ─────────────────────────────────────────────────────────

_TX_FIELDS = ("type", "amount", "date", "account_id", "to_account_id", "category_id", "note")


def _state(q, ctx):
    txs = {}
    for off in range(-14, 4):
        for t in _page(q, ctx, off)["transactions"]:
            txs[str(t["id"])] = {k: t.get(k) for k in _TX_FIELDS}
    lists = _lists(q)
    s = _summary(q, ctx)
    return {
        "txs": txs,
        "balances": {str(a["id"]): a["balance"] for a in lists["accounts"]},
        "accounts": sorted([a["id"], a["name"], a["type"], a["archived"], a.get("registration"), a.get("institution")] for a in lists["accounts"]),
        "categories": sorted([c["id"], c["name"], c["archived"]] for c in lists["categories"]),
        "budgets": sorted([b["category_id"], b["budget"]] for b in s["budgets"]) + [["overall", s["pace"]["budget"]]],
        "month_start_day": s["month_start_day"],
        "currency": s["currency"],
        "activities": sorted(a["id"] for a in q("money.invest.list", {"limit": 1000})["activities"]),
        "room": [[r["registration"], r["year"], r["room"]] for r in _portfolio(q, ctx)["room"]],
    }


def _diff(before, after):
    b, a = before["txs"], after["txs"]
    new = [dict(a[k], id=int(k)) for k in a if k not in b]
    gone = [dict(b[k], id=int(k)) for k in b if k not in a]
    changed = [(dict(b[k], id=int(k)), dict(a[k], id=int(k))) for k in a if k in b and a[k] != b[k]]
    other = [k for k in ("accounts", "categories", "budgets", "month_start_day", "currency", "activities", "room") if before[k] != after[k]]
    return new, gone, changed, other


def _balances_follow(before, after, new, changed):
    """Every balance moved by exactly what the seen entries explain: nothing was written outside
    the periods read."""
    expect = dict(before["balances"])

    def apply(t, sign):
        def move(acct, d):
            if acct is not None and str(acct) in expect:
                expect[str(acct)] += sign * d
        if t["type"] == "INCOME":
            move(t["account_id"], t["amount"])
        elif t["type"] == "EXPENSE":
            move(t["account_id"], -t["amount"])
        else:
            move(t["account_id"], -t["amount"])
            move(t["to_account_id"], t["amount"])
    for t in new:
        apply(t, 1)
    for old, cur in changed:
        apply(old, -1)
        apply(cur, 1)
    # An investment account's balance starts from its newest recorded value and counts only the
    # entries after it, so an entry dated before that moves nothing: leave those accounts out.
    valued = {str(a[0]) for a in after["accounts"] if a[2] == "INVESTMENT"}
    return [k for k in expect if k not in valued and expect[k] != after["balances"].get(k)]


def _effect(q, ctx, judge):
    after = _state(q, ctx)
    if "before" not in ctx or not ctx["before"] or "state" not in ctx["before"]:
        return {"ok": True, "why": "the ledger before the turn", "state": after}
    before = ctx["before"]["state"]
    new, gone, changed, other = _diff(before, after)
    if gone:
        return {"ok": False, "why": f"entries gone: {gone}"}
    if other:
        return {"ok": False, "why": f"changed outside entries: {other}"}
    ok, why = judge(q, ctx, new, changed)
    if ok:
        bad = _balances_follow(before, after, new, changed)
        if bad:
            return {"ok": False, "why": f"balances moved beyond the entries seen (accounts {bad}): something was written outside the periods read"}
    return {"ok": ok, "why": why}


def _fmt(t):
    return f"{t['type']} {t['amount']} {t['date']} acct {t['account_id']}->{t.get('to_account_id')} cat {t.get('category_id')} {t.get('note')!r}"


def _one_new(spec):
    """A judge: exactly one new entry, no entry changed, and the new one matches `spec`, a function
    (q, ctx) -> dict of field: allowed value or set of allowed values."""
    def judge(q, ctx, new, changed):
        if changed:
            return False, f"entries changed: {[_fmt(c) for _, c in changed]}"
        if len(new) != 1:
            return False, f"{len(new)} new entries: {[_fmt(t) for t in new]}"
        t = new[0]
        for field, want in spec(q, ctx).items():
            allowed = want if isinstance(want, (set, frozenset)) else {want}
            if t.get(field) not in allowed:
                return False, f"{field} is {t.get(field)!r}, expected {sorted(allowed, key=str)}: {_fmt(t)}"
        return True, f"added {_fmt(t)}"
    return judge


def _one_changed(find, spec):
    """A judge: no new entry, exactly one entry changed, the one `find(before_tx)` picks, now
    matching `spec`, with every other field as it was."""
    def judge(q, ctx, new, changed):
        if new:
            return False, f"new entries instead of a change: {[_fmt(t) for t in new]}"
        if len(changed) != 1:
            return False, f"{len(changed)} entries changed: {[_fmt(c) for _, c in changed]}"
        old, cur = changed[0]
        if not find(old):
            return False, f"the wrong entry changed: {_fmt(old)}"
        want = spec(q, ctx)
        for field in _TX_FIELDS:
            expect = want.get(field, old.get(field))
            allowed = expect if isinstance(expect, (set, frozenset)) else {expect}
            if cur.get(field) not in allowed:
                return False, f"{field} is {cur.get(field)!r}, expected {sorted(allowed, key=str)}"
        return True, f"changed to {_fmt(cur)}"
    return judge


def _nothing_changed(q, ctx, new, changed):
    if new or changed:
        return False, f"the ledger changed: new {[_fmt(t) for t in new]}, changed {[_fmt(c) for _, c in changed]}"
    return True, "the ledger is as it was"


def _cid(q, *names):
    return {_category(q, n)["id"] for n in names}


def _aid(q, kind):
    return _account(q, kind)["id"]


def _register_effect(name, judge):
    def fn(q, ctx):
        return _effect(q, ctx, judge)
    fn.__name__ = name
    return truth(fn)


_register_effect("eff_coffee_today", _one_new(lambda q, ctx: {
    "type": "EXPENSE", "amount": 475, "date": _iso(_today(ctx)), "account_id": _aid(q, "CREDIT"),
    "category_id": _cid(q, "Dining"), "to_account_id": None}))

_register_effect("eff_lunch_wednesday", _one_new(lambda q, ctx: {
    "type": "EXPENSE", "amount": 1850, "date": _iso(_last_weekday(_today(ctx), 2)), "account_id": _aid(q, "CASH"),
    "category_id": _cid(q, "Dining"), "to_account_id": None}))

_register_effect("eff_freelance_income", _one_new(lambda q, ctx: {
    "type": "INCOME", "amount": 30000, "date": _iso(_today(ctx)), "account_id": _aid(q, "CHEQUING"),
    "category_id": _cid(q, "Side income", "Other income"), "to_account_id": None}))

_register_effect("eff_transfer_savings", _one_new(lambda q, ctx: {
    "type": "TRANSFER", "amount": 20000, "date": _iso(_today(ctx)), "account_id": _aid(q, "CHEQUING"),
    "to_account_id": _aid(q, "SAVINGS"), "category_id": None}))

_register_effect("eff_iga_fr", _one_new(lambda q, ctx: {
    "type": "EXPENSE", "amount": 4520, "date": _iso(_today(ctx)), "account_id": _aid(q, "CREDIT"),
    "category_id": _cid(q, "Groceries"), "to_account_id": None}))

_register_effect("eff_bixi_saturday", _one_new(lambda q, ctx: {
    "type": "EXPENSE", "amount": 650, "date": _iso(_last_weekday(_today(ctx), 5)), "account_id": _aid(q, "CREDIT"),
    "category_id": _cid(q, "Transport"), "to_account_id": None}))


_register_effect("eff_lunch_fixed", _one_new(lambda q, ctx: {
    "type": "EXPENSE", "amount": 1400, "date": _iso(_today(ctx)), "account_id": _aid(q, "CASH"),
    "category_id": _cid(q, "Dining"), "to_account_id": None}))

_register_effect("eff_recategorize_books", _one_changed(
    lambda t: t["note"] == "Books" and t["amount"] == 15899 and t["date"].endswith("-08-14"),
    lambda q, ctx: {"category_id": _cid(q, "Entertainment")}))

_register_effect("eff_fix_provigo", _one_changed(
    lambda t: t["note"] == "Provigo" and t["amount"] == 6549 and t["date"].endswith("-09-12"),
    lambda q, ctx: {"amount": 7549}))

_register_effect("eff_unchanged", _nothing_changed)


@truth
def provigo_sep_12(q, ctx):
    """The entry eff_fix_provigo expects to change, so --check shows the world still has it."""
    hits = [t for t in _page(q, ctx, 0, query="Provigo")["transactions"] if t["date"].endswith("-09-12")]
    _check(len(hits) == 1 and hits[0]["amount"] == 6549, f"Provigo on Sep 12: {hits}")
    return {"amount": _major(hits[0]["amount"]), "new_amount": 75.49}


@truth
def books_aug_14(q, ctx):
    hits = [t for t in _page(q, ctx, -1, query="Books")["transactions"] if t["date"].endswith("-08-14")]
    _check(len(hits) == 1 and hits[0]["amount"] == 15899 and hits[0]["category"] == "Shopping", f"Books on Aug 14: {hits}")
    return {"amount": _major(hits[0]["amount"])}


@truth
def board_game_today(q, ctx):
    hits = [t for t in _page(q, ctx, 0, query="Board game")["transactions"] if t["date"] == _iso(_today(ctx))]
    _check(len(hits) == 1, f"one board game entry today: {hits}")
    return {"amount": _major(hits[0]["amount"])}


@truth
def demo_tfsa_is_wealthsimple(q, ctx):
    a = [a for a in _lists(q)["accounts"] if a["name"] == "Demo TFSA"]
    _check(len(a) == 1 and a[0]["institution"] == "Wealthsimple", f"Demo TFSA: {a}")
    return {}


# ── Avex ──────────────────────────────────────────────────────────────────────────────────────

def _gym(q):
    s = q("gym.summary", {})
    _check(s.get("imported"), "an Avex export is imported")
    _check(s["unit"] == "lb", "the export is in pounds")
    return s


def _sessions(q):
    out = q("gym.sessions", {"limit": 500})
    _check(len(out["sessions"]) == out["total"], "every session in one page")
    return out["sessions"]


def _tracked(rows):
    return [r for r in rows if not r["untracked"]]


def _gym_today(ctx):
    return _today(ctx)


def _round1(v):
    """Rust's `(v * 10.0).round() / 10.0`: halves away from zero."""
    import math
    return math.copysign(math.floor(abs(v) * 10 + 0.5) / 10, v)


def _epley(w, reps):
    if w is None or not 1 <= reps <= 12:
        return None
    return w if reps == 1 else w * (1 + reps / 30)


def _lift(q, name):
    d = q("gym.lift.get", {"name": name, "limit": 500})
    row = d["row"]
    hist = d["history"]
    _check(len(hist) == row["sessions"], f"{name}: history {len(hist)} vs sessions {row['sessions']}")
    best = row["best"]
    if best and best.get("e1rm") is not None:
        _check(abs(_round1(_epley(best["weight"], best["reps"])) - best["e1rm"]) < 0.051, f"{name}: Epley of {best}")
        _check(max(p["e1rm"] or 0 for p in hist) == best["e1rm"], f"{name}: best e1rm vs history")
    return d


def _lift_reading(q, name):
    d = _lift(q, name)
    row, hist = d["row"], d["history"]
    top = max(p["top_weight"] or 0 for p in hist)
    _check(top == row["best"]["weight"], f"{name}: heaviest {top} vs best set {row['best']['weight']}")
    return {
        "weight": row["best"]["weight"], "reps": row["best"]["reps"],
        "e1rm": [row["best"]["e1rm"], float(round(row["best"]["e1rm"]))],
        "change_pct": row["change_pct"], "sessions": row["sessions"], "last_day": row["last_day"],
        "progress": [row["change_pct"], row["best"]["e1rm"], row["best"]["weight"]],
        # The best set, by its weight or by its estimate: either names the same set.
        "best_set": [row["best"]["weight"], row["best"]["e1rm"]],
    }


@truth
def lift_bench(q, ctx):
    return _lift_reading(q, "Bench Press")


@truth
def lift_squat(q, ctx):
    return _lift_reading(q, "Back Squat")


@truth
def lift_deadlift(q, ctx):
    r = _lift_reading(q, "Deadlift")
    _check(r["last_day"] == "2026-09-15", f"last deadlift {r['last_day']}: the task's text check names Sep 15")
    return r


@truth
def squat_vs_deadlift(q, ctx):
    sq, dl = _lift_reading(q, "Back Squat"), _lift_reading(q, "Deadlift")
    _check(dl["e1rm"][0] > sq["e1rm"][0] and dl["weight"] > sq["weight"], "deadlift is the stronger")
    return {"deadlift": [dl["weight"], dl["e1rm"][0]], "squat": [sq["weight"], sq["e1rm"][0]]}


@truth
def bench_goal(q, ctx):
    s = _gym(q)
    g = [g for g in s["goals"] if g["kind"] == "lift_target" and g["target_key"] == "bench"]
    _check(len(g) == 1, "one open bench goal")
    b = _lift_reading(q, "Bench Press")
    _check(b["weight"] < g[0]["target_value"], "the goal is not met yet")
    return {"target": g[0]["target_value"], "best": b["weight"], "gap": g[0]["target_value"] - b["weight"]}


@truth
def bench_kg(q, ctx):
    b = _lift_reading(q, "Bench Press")
    lb_per_kg = 2.2046226218
    return {"kg": [round(b["weight"] / lb_per_kg, 1), round(b["e1rm"][0] / lb_per_kg, 1)]}


@truth
def last_workout(q, ctx):
    s = _gym(q)
    rows = _tracked(_sessions(q))
    last = rows[0]
    _check(last["day"] == s["totals"]["last_day"], "newest tracked session")
    since = (_gym_today(ctx) - _day(last["day"])).days
    _check(since == s["days_since_last"], f"days since {since} vs {s['days_since_last']}")
    _check(last["day"] == "2026-09-17", f"last workout {last['day']}: the task's text check names Sep 17")
    return {"day": last["day"], "title": last["title"], "volume": last["volume"], "days_since": since}


def _week(rows, start, end=None):
    end = end or start + _dt.timedelta(days=7)
    return [r for r in rows if start <= _day(r["day"]) < end]


@truth
def week_counts(q, ctx):
    s = _gym(q)
    rows = _tracked(_sessions(q))
    mon = _monday(_gym_today(ctx))
    _check(s["this_week"]["start"] == _iso(mon), f"Avex weeks start {s['this_week']['start']}, expected Monday {mon}")
    this, last = _week(rows, mon), _week(rows, mon - _dt.timedelta(days=7))
    for got, want in ((this, s["this_week"]), (last, s["last_week"])):
        _check(len(got) == want["sessions"], "week sessions")
        _check(sum(r["sets"] for r in got) == want["sets"], "week sets")
        _check(abs(sum(r["volume"] for r in got) - want["volume"]) <= 1, "week volume")
    _check(len(this) == 3 and len(last) == 4, "the tasks' text checks name 3 this week and 4 last week")
    return {
        "this": len(this), "last": len(last),
        "this_sets": sum(r["sets"] for r in this), "last_sets": sum(r["sets"] for r in last),
        "this_volume": s["this_week"]["volume"], "last_volume": s["last_week"]["volume"],
        "this_titles": [r["title"] for r in reversed(this)],
    }


@truth
def squat_yesterday(q, ctx):
    y = _iso(_gym_today(ctx) - _dt.timedelta(days=1))
    rows = [r for r in _tracked(_sessions(q)) if r["day"] == y and "Back Squat" in r["lifts"]]
    _check(len(rows) == 1, "one squat session yesterday")
    d = q("gym.session.get", {"id": rows[0]["id"]})
    ex = [e for e in d["exercises"] if e["name"] == "Back Squat"][0]
    work = [s for s in ex["sets"] if s["kind"] != "warmup" and s["reps"] > 0]
    warm = [s for s in ex["sets"] if s["kind"] == "warmup"]
    vol = sum((s["weight"] or 0) * s["reps"] for s in work)
    with_warm = vol + sum((s["weight"] or 0) * s["reps"] for s in warm)
    _check(abs(vol - ex["volume"]) <= 1, f"squat volume {vol} vs {ex['volume']}")
    _check(len(work) == 3 and len(warm) == 1, "the tasks' checks name 3 working sets and a warm-up")
    return {"sets": len(work), "warmups": len(warm), "volume": float(round(vol)), "with_warmups": float(round(with_warm))}


@truth
def gym_totals(q, ctx):
    s = _gym(q)
    rows = _sessions(q)
    tracked = _tracked(rows)
    _check(len(tracked) == s["totals"]["sessions"], "tracked sessions")
    _check(len(rows) > len(tracked), "the trap needs an untracked session")
    _check(len(tracked) == 48 and len(rows) == 49, "the task's text checks name 48, not 49")
    return {"sessions": len(tracked), "with_untracked": len(rows)}


def _month_sessions(q, ym):
    return [r for r in _tracked(_sessions(q)) if r["day"].startswith(ym)]


def _series_month(q, measure, ym):
    sr = q("gym.series", {"measure": measure, "by": "month", "periods": 12})
    i = sr["days"].index(ym + "-01")
    return sr["series"][0]["values"][i]


@truth
def june_sessions(q, ctx):
    rows = _month_sessions(q, "2026-06")
    _check(len(rows) == _series_month(q, "sessions", "2026-06"), "June sessions vs series")
    all_june = [r for r in _sessions(q) if r["day"].startswith("2026-06")]
    _check(len(all_june) == len(rows) + 1, "June holds the untracked session")
    _check(len(rows) == 5, "the task's text checks name 5")
    return {"count": len(rows), "with_untracked": len(all_june)}


@truth
def month_sessions(q, ctx):
    ym = _iso(_gym_today(ctx))[:7]
    rows = _month_sessions(q, ym)
    _check(len(rows) == _series_month(q, "sessions", ym), "month sessions vs series")
    _check(len(rows) == 10, "the task's text check names 10")
    return {"count": len(rows)}


@truth
def august_sessions(q, ctx):
    rows = _month_sessions(q, "2026-08")
    _check(len(rows) == _series_month(q, "sessions", "2026-08"), "August sessions vs series")
    _check(len(rows) == 15, "the task's text check names 15")
    return {"count": len(rows)}


@truth
def volume_july_august(q, ctx):
    jul = sum(r["volume"] for r in _month_sessions(q, "2026-07"))
    aug = sum(r["volume"] for r in _month_sessions(q, "2026-08"))
    _check(abs(jul - _series_month(q, "volume", "2026-07")) <= 1 and abs(aug - _series_month(q, "volume", "2026-08")) <= 1, "volume vs series")
    _check(jul > aug, "the task expects more in July")
    return {"jul": float(jul), "aug": float(aug)}


def _cardio(q):
    return q("gym.cardio", {"limit": 500})["entries"]


@truth
def cardio_totals(q, ctx):
    s = _gym(q)
    c = _cardio(q)
    _check(len(c) == s["totals"]["cardio"], "cardio count")
    _check(sum(e["minutes"] for e in c) == s["totals"]["cardio_minutes"], "cardio minutes")
    runs = [e for e in c if e["kind"] == "run"]
    _check(len(runs) == len(c), "every cardio entry is a run")
    return {"km": round(sum(e["distance_km"] or 0 for e in runs), 2), "minutes": sum(e["minutes"] for e in runs), "runs": len(runs)}


@truth
def cardio_august(q, ctx):
    c = [e for e in _cardio(q) if e["day"].startswith("2026-08")]
    _check(sum(e["minutes"] for e in c) == _series_month(q, "cardio_minutes", "2026-08"), "August cardio vs series")
    return {"minutes": sum(e["minutes"] for e in c), "runs": len([e for e in c if e["kind"] == "run"])}


@truth
def cardio_this_week(q, ctx):
    s = _gym(q)
    mon = _monday(_gym_today(ctx))
    c = [e for e in _cardio(q) if _day(e["day"]) >= mon]
    _check(not c and s["this_week"]["cardio_minutes"] == 0, "no cardio this week")
    last = _cardio(q)[0]
    _check((_gym_today(ctx) - _day(last["day"])).days <= 7, "the trap needs a run within the last 7 days")
    return {"this_week": 0, "last_day": last["day"]}


@truth
def deadlift_count(q, ctx):
    d = _lift(q, "Deadlift")
    rows = _sessions(q)
    tracked = [r for r in _tracked(rows) if "Deadlift" in r["lifts"]]
    every = [r for r in rows if "Deadlift" in r["lifts"]]
    _check(len(tracked) == d["row"]["sessions"], f"deadlift sessions {len(tracked)} vs {d['row']['sessions']}")
    _check(len(every) == len(tracked) + 1, "the trap needs an untracked deadlift session")
    _check(len(tracked) == 11, "the task's text checks name 11")
    return {"sessions": len(tracked), "with_untracked": len(every)}


@truth
def most_done_lift(q, ctx):
    lifts = q("gym.lifts", {})["lifts"]
    _check(lifts[0]["sessions"] > lifts[1]["sessions"] and lifts[0]["name"] == "Bench Press", f"{lifts[:2]}")
    return {"name": lifts[0]["name"], "sessions": lifts[0]["sessions"]}


@truth
def most_improved(q, ctx):
    lifts = [l for l in q("gym.lifts", {})["lifts"] if l["change_pct"] is not None]
    lifts.sort(key=lambda l: -l["change_pct"])
    _check(lifts[0]["change_pct"] > lifts[1]["change_pct"] and lifts[0]["name"] == "Triceps Pushdown", f"{[(l['name'], l['change_pct']) for l in lifts[:2]]}")
    return {"name": lifts[0]["name"], "pct": lifts[0]["change_pct"]}


@truth
def lateral_raise(q, ctx):
    d = _lift(q, "Lateral Raise")
    _check(d["row"]["best"]["e1rm"] is None and d["row"]["best"]["reps"] > 12, "lateral raises have no estimate")
    w, r = d["row"]["best"]["weight"], d["row"]["best"]["reps"]
    # What a model that applied Epley past 12 reps, or capped the reps at 12, would invent.
    return {"weight": w, "reps": r, "invented": [_round1(w * (1 + r / 30)), _round1(w * (1 + 12 / 30))]}


@truth
def no_front_squat(q, ctx):
    _check(not q("gym.lifts", {"query": "front"})["lifts"], "no front squat in the history")
    return {}


@truth
def streak(q, ctx):
    s = _gym(q)
    weeks = {_monday(_day(r["day"])) for r in _tracked(_sessions(q))}
    at = _monday(_gym_today(ctx))
    if at not in weeks:
        at -= _dt.timedelta(days=7)
    n = 0
    while at in weeks:
        n += 1
        at -= _dt.timedelta(days=7)
    _check(n == s["streak_weeks"], f"streak {n} vs {s['streak_weeks']}")
    _check(n == 13, "the task's text check names 13")
    return {"weeks": n}


@truth
def gym_export(q, ctx):
    s = _gym(q)
    exported = s["imported"]["exported_at"]
    _check(exported.startswith("2026-09-17"), f"exported {exported}: the tasks' text checks name Sep 17")
    today = [r for r in _sessions(q) if r["day"] == _iso(_gym_today(ctx))]
    _check(not today, "nothing logged today in the copy")
    return {"exported_at": exported, "today_sessions": 0}


# ── both ──────────────────────────────────────────────────────────────────────────────────────

@truth
def gym_cost_august(q, ctx):
    b, fd = _bill(q, ctx, "Gym")
    _check(b["type"] == "EXPENSE", "the gym is an expense")
    n = august_sessions(q, ctx)["count"]
    return {"cost": _major(b["amount"], fd), "sessions": n}


@truth
def gym_bill_and_last_workout(q, ctx):
    b, _ = _bill(q, ctx, "Gym")
    _check(b["next_date"] == "2026-10-14", f"gym bill next {b['next_date']}: the task's text check names October 14")
    return {"bill": b["next_date"], "last_workout": last_workout(q, ctx)["day"]}


@truth
def week_recap(q, ctx):
    mon = _monday(_today(ctx))
    _, txs, _ = _period_spent(q, ctx, 0)
    since_monday = _sum([t for t in txs if _day(t["date"]) >= mon])
    # Tally's own weeks are the budget month's seven-day blocks: the week holding today.
    page = _page(q, ctx, 0)
    start = _day(page["period"]["start"])
    block = (_today(ctx) - start).days // 7
    lo = start + _dt.timedelta(days=7 * block)
    in_block = _sum([t for t in txs if lo <= _day(t["date"]) <= _today(ctx)])
    sr = q("money.series", {"measure": "spending", "by": "week", "periods": 1, "today": _iso(_today(ctx))})
    _check(sr["series"][-1]["values"][block] == in_block, f"series week {sr['series'][-1]['values'][block]} vs entries {in_block}")
    w = week_counts(q, ctx)
    return {"spent": [_major(since_monday), _major(in_block)], "sessions": w["this"]}


@truth
def yesterday_both(q, ctx):
    y = _iso(_today(ctx) - _dt.timedelta(days=1))
    rows = [r for r in _tracked(_sessions(q)) if r["day"] == y]
    _check(len(rows) == 1, "one workout yesterday")
    return {"spent": spent_yesterday(q, ctx)["total"], "title": rows[0]["title"]}


@truth
def runs_transport_august(q, ctx):
    runs = cardio_august(q, ctx)["runs"]
    _check(runs == 5, "the task's text check names 5 runs")
    return {"runs": runs, "transport": transport_last_month(q, ctx)["spent"]}
