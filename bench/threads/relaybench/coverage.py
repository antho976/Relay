"""The coverage suite: many plain questions, generated, one run each (`--suite cov`).

v1, v2 and v3 are hand-written, and a suite Relay is tuned on stops showing what a new question
does: v3's dev split reached 93% while its holdout stayed at 61.5% (2026-10-30). This one is drawn
from templates instead: a kind of question (spent on a category, at a shop, workouts, km run, a
lift's best...) crossed with the days people name ("last week", "in March", "since my last pay",
"this year") and the categories, shops, accounts and lifts of the v2 world, in English and French,
some typed carelessly. A seed picks the sample, so a new seed is a set of questions no change was
made for; the same seed on two builds compares them.

`--suite cov` is seed 1, 300 questions; `cov:<seed>` and `cov:<seed>:<count>` pick others. Each
truth (`truths_cov.py`) reads the question's own `params`.
"""
import random

#: The v2 world's today (truths_v2.TODAY), a Friday; its budget months start on the 15th.
TODAY = "2026-09-18"

# Days as people name them: (key, English, French, first day, last day). Budget months are Tally's
# (the 15th to the 14th); "since_pay" is resolved by its truth from the last pay.
DAYS = [
    ("today", "today", "aujourd'hui", "2026-09-18", "2026-09-18"),
    ("yesterday", "yesterday", "hier", "2026-09-17", "2026-09-17"),
    ("this_week", "this week", "cette semaine", "2026-09-14", "2026-09-18"),
    ("last_week", "last week", "la semaine dernière", "2026-09-07", "2026-09-13"),
    ("last_weekend", "last weekend", "le weekend passé", "2026-09-12", "2026-09-13"),
    ("this_month", "this month", "ce mois-ci", "2026-09-15", "2026-09-18"),
    ("last_month", "last month", "le mois passé", "2026-08-15", "2026-09-14"),
    ("month_before", "the month before last", "le mois d'avant", "2026-07-15", "2026-08-14"),
    ("aug", "in August", "en août", "2026-08-01", "2026-08-31"),
    ("jul", "in July", "en juillet", "2026-07-01", "2026-07-31"),
    ("jun", "in June", "en juin", "2026-06-01", "2026-06-30"),
    ("may", "in May", "en mai", "2026-05-01", "2026-05-31"),
    ("apr", "in April", "en avril", "2026-04-01", "2026-04-30"),
    ("mar", "in March", "en mars", "2026-03-01", "2026-03-31"),
    ("feb", "in February", "en février", "2026-02-01", "2026-02-28"),
    ("jan", "in January", "en janvier", "2026-01-01", "2026-01-31"),
    ("this_year", "this year", "cette année", "2026-01-01", "2026-09-18"),
    ("last_30", "in the last 30 days", "dans les 30 derniers jours", "2026-08-20", "2026-09-18"),
    ("q1", "in Q1", "au premier trimestre", "2026-01-01", "2026-03-31"),
    ("q2", "in Q2", "au deuxième trimestre", "2026-04-01", "2026-06-30"),
    ("since_aug1", "since August 1", "depuis le 1er août", "2026-08-01", "2026-09-18"),
    ("since_pay", "since my last paycheque", "depuis ma dernière paie", None, None),
]
#: Tally's own months: an Avex question "last month" is a calendar month to most people.
BUDGET_DAYS = {"this_month", "last_month", "month_before", "since_pay"}
#: One day is too short for a shop or a kind of cardio: most such questions would be zero.
ONE_DAY = {"today", "yesterday"}
SHORT_NOT = {"shop", "visits", "km", "cardio_times", "volume"}

CATEGORIES = [
    ("Dining", "eating out", "restos"), ("Groceries", "groceries", "l'épicerie"), ("Transport", "transport", "le transport"),
    ("Subscriptions", "subscriptions", "les abonnements"), ("Shopping", "shopping", "le magasinage"), ("Health", "health", "la santé"),
    ("Entertainment", "entertainment", "les sorties"), ("Pet care", "the cat", "le chat"),
]
SHOPS = ["Metro", "Tim Hortons", "Uber Eats", "IGA", "Bixi", "Communauto", "Café Olimpico", "St-Viateur Bagel", "Costco", "Amazon",
         "Pho Lien", "Maxi", "Jean Coutu", "Provigo", "Dollarama", "Pharmaprix", "Canadian Tire", "Super C", "STM", "Netflix", "Spotify"]
ACCOUNTS = [("Desjardins chequing", "chequing account", "compte chèques"), ("Tangerine savings", "Tangerine savings", "épargne Tangerine"),
            ("Desjardins Visa", "Visa", "Visa"), ("Triangle Mastercard", "Triangle Mastercard", "Mastercard Triangle"),
            ("Cash", "cash", "argent comptant"), ("Line of credit", "line of credit", "marge de crédit")]
LIFTS = [("Back Squat", "squat", "squat"), ("Deadlift", "deadlift", "soulevé de terre"), ("Dumbbell Bench Press", "dumbbell bench", "développé haltères"),
         ("Overhead Press", "overhead press", "développé militaire"), ("Barbell Row", "barbell row", "rowing barre"), ("Romanian Deadlift", "RDL", "soulevé roumain")]
CARDIO = [("run", "run", "couru", "go running", "suis allé courir"), ("cycle", "cycle", "fait de vélo", "go cycling", "suis allé à vélo")]

WRITES = ["money.tx.add", "money.tx.update", "money.tx.delete", "money.tx.restore", "money.invest.add", "thread.propose"]

# Each kind: (name, source, weight, templates (English, French), what it needs, its checks).
# {d} is the days, {c} a category, {s} a shop, {a} an account, {l} a lift, {k}/{g} a cardio verb.
KINDS = [
    ("spent", "tally", 4, ["how much did I spend {d}?", "what did I spend {d}, all in?", "total spending {d}?"],
     ["combien j'ai dépensé {d}?", "j'ai dépensé combien {d}?"], ["days"]),
    ("category", "tally", 5, ["how much did I spend on {c} {d}?", "what did {c} cost me {d}?"],
     ["combien j'ai dépensé pour {c} {d}?"], ["days", "category"]),
    ("shop", "tally", 5, ["how much did I spend at {s} {d}?", "what have I spent at {s} {d}?"],
     ["combien j'ai dépensé chez {s} {d}?"], ["days", "shop"]),
    ("visits", "tally", 3, ["how many times did I buy something at {s} {d}?", "how many {s} purchases did I make {d}?"],
     ["combien de fois j'ai acheté chez {s} {d}?"], ["days", "shop"]),
    ("biggest", "tally", 3, ["what was my biggest expense {d}?", "what's the most I spent on one thing {d}?"],
     ["c'était quoi ma plus grosse dépense {d}?"], ["days"]),
    ("income", "tally", 2, ["how much money came in {d}?", "how much income did I get {d}?"],
     ["combien d'argent est rentré {d}?"], ["days"]),
    ("top", "tally", 2, ["what did I spend the most on {d}?", "which category cost me the most {d}?"],
     ["sur quoi j'ai le plus dépensé {d}?"], ["days"]),
    ("balance", "tally", 2, ["what's my {a} balance?", "how much is in my {a} right now?"],
     ["combien il y a dans mon {a}?"], ["account"]),
    ("workouts", "avex", 4, ["how many workouts did I do {d}?", "how many times did I train {d}?"],
     ["combien d'entraînements j'ai fait {d}?"], ["gym_days"]),
    ("km", "avex", 3, ["how many km did I {k} {d}?"], ["combien de km j'ai {k} {d}?"], ["gym_days", "cardio"]),
    ("cardio_times", "avex", 2, ["how many times did I {g} {d}?"], ["combien de fois je {g} {d}?"], ["gym_days", "cardio"]),
    ("volume", "avex", 2, ["how much weight did I lift in total {d}?", "what was my total volume {d}?"],
     ["combien de poids j'ai soulevé au total {d}?"], ["gym_days"]),
    ("lift_best", "avex", 3, ["what's my best {l}?", "what's my estimated max on {l}?"], ["c'est quoi mon meilleur {l}?"], ["lift"]),
    ("lift_last", "avex", 2, ["when did I last do {l}?", "when was my last {l} session?"], ["c'était quand la dernière fois que j'ai fait du {l}?"], ["lift"]),
    ("both", "both", 3, ["how much did I spend and how many workouts did I do {d}?", "{d}: what did I spend, and how often did I train?"],
     ["combien j'ai dépensé et combien d'entraînements j'ai fait {d}?"], ["days"]),
]


def _typo(text, rng):
    """Typed carelessly: lowercase, no question mark, a letter dropped from one longer word."""
    words = text.lower().rstrip("?").split(" ")
    long = [i for i, w in enumerate(words) if len(w) > 4 and w.isalpha()]
    if long:
        i = rng.choice(long)
        j = rng.randrange(1, len(words[i]) - 1)
        words[i] = words[i][:j] + words[i][j + 1:]
    return " ".join(words)


def _checks(kind, numbers):
    out = [{"type": "no_tool", "ops": WRITES}, {"type": "no_error"}]
    if kind == "top":
        return [{"type": "text_any", "of": "names"}] + out
    if kind == "lift_last":
        return [{"type": "text_any", "of": "said"}] + out
    if kind == "lift_best":
        return [{"type": "number", "any_of": ["e1rm", "heaviest"], "tol": 0.15}] + out
    tol = {"km": 0.05, "volume": 1}.get(kind, 0.01)
    rel = {"volume": 0.3}.get(kind)
    return [dict({"type": "number", "of": key, "tol": tol, "zero_words": True}, **({"tol_pct": rel} if rel else {})) for key in numbers] + out


def parse(suite):
    """`cov`, `cov:<seed>` or `cov:<seed>:<count>` → (seed, count)."""
    parts = suite.split(":")
    seed = int(parts[1]) if len(parts) > 1 and parts[1] else 1
    count = int(parts[2]) if len(parts) > 2 and parts[2] else 300
    return seed, count


def generate(seed=1, count=300):
    """`count` questions drawn with `seed`: the same arguments give the same tasks."""
    rng = random.Random(seed)
    weights = [k[2] for k in KINDS]
    tasks, seen = [], set()
    while len(tasks) < count:
        name, source, _, english, french, needs = rng.choices(KINDS, weights=weights)[0]
        params, words = {"kind": name}, {}
        lang = "fr" if rng.random() < 0.25 else "en"
        fr = lang == "fr"
        if "days" in needs or "gym_days" in needs:
            pool = [d for d in DAYS if ("gym_days" not in needs or d[0] not in BUDGET_DAYS) and (name not in SHORT_NOT or d[0] not in ONE_DAY)]
            key, en_words, fr_words, first, last = rng.choice(pool)
            params.update({"days": key, "first": first, "last": last})
            words["d"] = fr_words if fr else en_words
        if "category" in needs:
            cat, en_words, fr_words = rng.choice(CATEGORIES)
            params["category"] = cat
            words["c"] = fr_words if fr else en_words
        if "shop" in needs:
            words["s"] = params["shop"] = rng.choice(SHOPS)
        if "account" in needs:
            acct, en_words, fr_words = rng.choice(ACCOUNTS)
            params["account"] = acct
            words["a"] = fr_words if fr else en_words
        if "lift" in needs:
            lift, en_words, fr_words = rng.choice(LIFTS)
            params["lift"] = lift
            words["l"] = fr_words if fr else en_words
        if "cardio" in needs:
            ck, en_k, fr_k, en_g, fr_g = rng.choice(CARDIO)
            params["cardio"] = ck
            words["k"], words["g"] = (fr_k, fr_g) if fr else (en_k, en_g)
        text = rng.choice(french if fr else english).format(**words)
        text = text[0].upper() + text[1:]
        typo = not fr and rng.random() < 0.12
        if typo:
            text = _typo(text, rng)
        if text in seen:
            continue
        seen.add(text)
        params.update({"lang": lang, "typo": typo})
        numbers = {"both": ["spent", "workouts"]}.get(name, ["value"])
        tasks.append({
            "id": f"cov.{name}.{len(tasks):03d}", "source": source, "category": name, "difficulty": 1, "prompt": text,
            "truth": f"cov_{name}", "params": params, "checks": _checks(name, numbers), "mutates": False,
        })
    return tasks
