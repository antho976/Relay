"""Expected answers for the coverage suite (`coverage.py`), asked in the v2 world.

The same contract as the other truths, `fn(q, ctx)`, with the question's own `ctx["params"]`: the
kind of question, its days (`first` and `last`, or `since_pay`), and the category, shop, account,
lift or cardio it names. Each adds up raw rows itself. The ledger and the workouts are read once
per run and kept: no coverage question writes.
"""
import datetime as dt

from .truths import _check, major, truth
from .truths_v2 import _all_txs, _note_has
from .truths_v3 import _cardio, _counted_sessions

_KEPT = {}


def _kept(q, name, read):
    """`read(q)`, once per runner: every coverage question reads the same ledger."""
    key = (id(getattr(q, "__self__", q)), name)
    if key not in _KEPT:
        _KEPT[key] = read(q)
    return _KEPT[key]


def _txs(q):
    return _kept(q, "txs", _all_txs)


def _days(q, p):
    """The question's first and last day."""
    if p["days"] == "since_pay":
        pays = [t["date"] for t in _txs(q) if t["type"] == "INCOME" and (t.get("category") or "").lower() == "salary"]
        _check(bool(pays), "no pay in the ledger")
        return max(pays), p.get("today", "2026-09-18")
    return p["first"], p["last"]


def _between(rows, first, last, key="date"):
    return [r for r in rows if first <= r[key][:10] <= last]


def _expenses(q, p):
    first, last = _days(q, p)
    rows = [t for t in _between(_txs(q), first, last) if t["type"] == "EXPENSE"]
    if p.get("category"):
        rows = [t for t in rows if t.get("category") == p["category"]]
    if p.get("shop"):
        rows = [t for t in rows if _note_has(t, p["shop"])]
    return rows


def _sum(rows):
    return major(sum(t["amount"] for t in rows))


@truth
def cov_spent(q, ctx):
    return {"value": _sum(_expenses(q, ctx["params"]))}


@truth
def cov_category(q, ctx):
    rows = _expenses(q, ctx["params"])
    return {"value": _sum(rows), "entries": len(rows)}


@truth
def cov_shop(q, ctx):
    rows = _expenses(q, ctx["params"])
    return {"value": _sum(rows), "entries": len(rows)}


@truth
def cov_visits(q, ctx):
    rows = _expenses(q, ctx["params"])
    return {"value": len(rows), "spent": _sum(rows)}


@truth
def cov_biggest(q, ctx):
    rows = _expenses(q, ctx["params"])
    top = max(rows, key=lambda t: t["amount"], default=None)
    return {"value": major(top["amount"]) if top else 0, "note": top.get("note") if top else None}


@truth
def cov_income(q, ctx):
    first, last = _days(q, ctx["params"])
    return {"value": _sum([t for t in _between(_txs(q), first, last) if t["type"] == "INCOME"])}


#: How a category may be named in an answer, English and French.
NAMES = {
    "Dining": ["dining", "eating out", "restaurant", "resto"], "Groceries": ["grocer", "épicerie", "epicerie"],
    "Transport": ["transport"], "Subscriptions": ["subscription", "abonnement"], "Shopping": ["shopping", "magasinage"],
    "Health": ["health", "santé", "sante", "pharmac"], "Entertainment": ["entertainment", "sortie", "divertissement"],
    "Pet care": ["pet", "miso", "chat", "animal"], "Housing": ["housing", "rent", "loyer", "logement"], "Utilities": ["utilit", "hydro"],
    "Phone & internet": ["phone", "internet", "téléphone"], "Travel": ["travel", "voyage", "trip"], "Other": ["other", "autre"],
    "Gifts": ["gift", "cadeau"],
}


@truth
def cov_top(q, ctx):
    p = ctx["params"]
    first, last = _days(q, p)
    sums = {}
    for t in _between(_txs(q), first, last):
        if t["type"] == "EXPENSE":
            name = t.get("category") or "No category"
            sums[name] = sums.get(name, 0) + t["amount"]
    _check(bool(sums), f"nothing spent {p['days']}")
    top = max(sums, key=sums.get)
    return {"category": top, "spent": major(sums[top]), "names": NAMES.get(top, [top.lower()])}


@truth
def cov_balance(q, ctx):
    name = ctx["params"]["account"]
    hits = [a for a in q("money.summary")["accounts"] if a["name"] == name]
    _check(len(hits) == 1, f"no single account {name}")
    return {"value": major(abs(hits[0]["balance"]))}


def _sessions(q, p):
    first, last = _days(q, p)
    return _between(_kept(q, "sessions", _counted_sessions), first, last, key="day")


@truth
def cov_workouts(q, ctx):
    return {"value": len(_sessions(q, ctx["params"]))}


@truth
def cov_volume(q, ctx):
    return {"value": round(sum(s["volume"] for s in _sessions(q, ctx["params"])))}


def _cardio_rows(q, p):
    first, last = _days(q, p)
    return [c for c in _between(_kept(q, "cardio", _cardio), first, last, key="day") if c["kind"] == p["cardio"]]


@truth
def cov_km(q, ctx):
    return {"value": round(sum(c.get("distance_km") or 0 for c in _cardio_rows(q, ctx["params"])), 2)}


@truth
def cov_cardio_times(q, ctx):
    return {"value": len(_cardio_rows(q, ctx["params"]))}


def _lift_row(q, name):
    hits = [x for x in _kept(q, "lifts", lambda q: q("gym.lifts")["lifts"]) if x["name"] == name]
    _check(len(hits) == 1, f"no single lift {name}")
    return hits[0]


@truth
def cov_lift_best(q, ctx):
    row = _lift_row(q, ctx["params"]["lift"])
    best = row.get("best") or {}
    heaviest = row.get("heaviest") or best
    return {"e1rm": best.get("e1rm"), "heaviest": heaviest.get("weight"), "best_set": f"{best.get('weight')} x {best.get('reps')}"}


@truth
def cov_lift_last(q, ctx):
    day = dt.date.fromisoformat(_lift_row(q, ctx["params"]["lift"])["last_day"])
    month, mois = day.strftime("%B"), ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"][day.month - 1]
    said = [f"{month} {day.day}", f"{month[:3]} {day.day}", f"{day.day} {mois}", f"{day.day} {mois[:4]}", day.isoformat()]
    return {"day": day.isoformat(), "said": said}


@truth
def cov_both(q, ctx):
    p = ctx["params"]
    return {"spent": _sum(_expenses(q, p)), "workouts": len(_sessions(q, p))}
