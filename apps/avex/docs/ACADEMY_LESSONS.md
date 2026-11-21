# Avex Academy — Lesson Curriculum (authoring order)

> Derived from `COACH_V3_PLAN.md` (AcademyRegistry, Mechanics M5, per-phase Academy
> notes) and `ENGINE_PLAN.md` (Academy track "The Engine"). This is the authoring guide:
> every lesson the plans call for, in the order the machinery that unlocks it ships.
> 33 lessons total — inside the plan's 30–40 budget.
> **Rev 3** (2026-07-24): re-keyed to the split phases (A2 / B1 / B2 / B3), C3 moved to
> Phase A2 where its machinery ships, C5 corrected to rate-based demotion, F6's trigger
> reconciled with the sick signal that already exists.

## Rules that shape this list (recap of plan invariants)

- **Unlock copy is a two-kind model, in code.** `Lesson.unlock` is a `LessonUnlock(label,
  detail, byYou)`, not a string. `byYou = true` is something the reader can go and do today and
  reads as an imperative ("Log a set"); `byYou = false` is the coach's move and names the moment
  ("When a block changes phase"), with `detail` saying what has to accumulate first. This mirrors
  the two trigger kinds below — app-usage moments vs coach-ledger moments — and `AcademyRegistryTest`
  enforces the grammar, so a coach-side unlock can never be written as a task the reader can't do.
  (2026-07-27: replaced the old single `unlockedBy` string, which named internal moments — "Your
  first placement-driven prescription" — and told a reader nothing actionable.)
- **Just-in-time, not curriculum-first.** Only Fundamentals is sequential for the user
  (cold-start). Everything else unlocks the first time its coach moment fires.
  "In order" below therefore means *authoring order* (which phase writes it), not a
  course index the user marches through. Unlock triggers are of two kinds — coach-ledger
  moments AND app-usage moments (first rest-timer use, first readiness tap). Both persist
  as `LessonEvent` rows; same idempotent-recompute rule.
- **Teach exactly what the coach does — no more.** Each lesson exists because a coach
  reason links to it (`lessonId`, per plan M3). If a concept below ever gets cut from the
  coach, cut its lesson too. Conversely: a phase that ships a coach concept ships its
  lesson in the same phase — which is why C3 sits in A2, not B3.
- **1–3 min read, plain language, offline.** Lessons are structured blocks
  (`Heading | Paragraph | Bullets | Callout | Example`), not markdown — there is no
  markdown renderer in the app and plan M5 rules out adding one. `Example` blocks may
  interpolate the user's own live numbers. The "Read more" pointers below are for the
  *author* (you) to verify the science — lessons themselves cite nothing and link
  nowhere (no internet permission).
- **Write per phase, never ahead.** Content is the real cost; a lesson written before
  its machinery exists describes a coach that doesn't.

## Tracks

| Track | Lessons | Ships with |
|---|---|---|
| Fundamentals | F1–F10 | Phase B3 (cold-start curriculum) |
| Coach Concepts | C1–C6 | A2 (C3), B3 (C1–C2), D (C4), E (C5–C6) |
| Programming | P1–P8 | Phase C (P1–P4), Phase D (P5–P8) |
| Signals | S1–S3 | Phase F (one per slot activation) |
| The Engine | E1–E6 | Engine E-B (E1–E3), E-C (E4–E5), E-D (E6) |

---

## Batch 0 — Phase A2: the first lesson (ships with `WeightPhase`)

A2 turns on phase-aware stall interpretation, which is a live coach concept the moment it
lands — so it ships its lesson with it, and with the miniature version of the M5 block
renderer that B3 later completes. One lesson, one component.

**C3 · `coach.strength_on_a_cut` — Holding strength while cutting is winning**
- Unlock: first time stall-suppression triggers in a cut phase (`WeightPhase = CUT`).
- Teaches: in a deficit the default outcome is *losing* strength; a held e1RM while
  bodyweight drops means you kept muscle and got relatively stronger. That's why the
  coach celebrates a flat line here and refuses to escalate "stalls" it expected.
- Read more: Murphy & Koehler 2022 (deficits blunt hypertrophy, strength is largely
  defended); strongerbyscience.com on training in a deficit.

---

## Batch 1 — Phase B3: Fundamentals (the cold-start track)

The only sequential track. During the data-starved window these ARE the Today
Directive, one per step, paired with the prepped template session.

**F1 · `fundamentals.what_a_program_is` — What a program is**
- Unlock: cold-start day 1.
- Teaches: a program is a repeatable structure (split, days, exercise slots) that makes
  progress measurable — you can't tell if something works if it changes every session.
  Why Avex generated *this* split from your equipment and days.
- Read more: Helms, Morgan & Valdez, *The Muscle & Strength Pyramid: Training* (ch. on
  adherence/structure); strongerbyscience.com — program design fundamentals.

**F2 · `fundamentals.sets_reps_rpe` — Sets, reps, and how hard is hard**
- Unlock: cold-start step 2 / first logged set.
- Teaches: what a set/rep prescription means; RPE and reps-in-reserve (RIR) as the
  effort dial — "RPE 8" = 2 clean reps left. Why effort, not exhaustion, drives the plan.
- Read more: Zourdos et al. 2016, *Novel resistance training–specific RPE scale
  measuring repetitions in reserve* (JSCR); Helms et al. 2016, *Application of the
  RIR-based RPE scale* (Strength & Conditioning Journal).

**F3 · `fundamentals.form_vs_load` — Form first, load second**
- Unlock: cold-start step 3 / first session with a technique tag.
- Teaches: load counts only for the muscles doing the work; range of motion and control
  beat added plates. Why the coach never rewards ugly PRs and why `technique` sessions
  are filtered out of stall detection.
- Read more: NSCA, *Essentials of Strength Training and Conditioning* (technique
  chapters); strongerbyscience.com on lengthened partials / ROM research.

**F4 · `fundamentals.progressive_overload` — Progressive overload**
- Unlock: first auto weight-progression suggestion.
- Teaches: the one non-negotiable principle — doing slightly more over time (load, reps,
  sets) is the signal that forces adaptation. The coach's job is choosing *which* "more"
  and *when*; your job is showing up and logging honestly.
- Read more: ACSM 2009 position stand, *Progression models in resistance training for
  healthy adults* (MSSE); Schoenfeld, *Science and Development of Muscle Hypertrophy*.

**F5 · `fundamentals.rest_and_recovery` — Rest is where you grow**
- Unlock: first rest-timer use / first rest-day directive.
- Teaches: two clocks — minutes between sets (2–3 min on compounds outperforms 1 min)
  and days between sessions (muscle protein synthesis runs ~48–72 h). Training is the
  stimulus; sleep and rest days are the adaptation.
- Read more: Schoenfeld et al. 2016, *Longer interset rest periods enhance muscle
  strength and hypertrophy* (JSCR); Vitale et al. 2019, *Sleep hygiene for optimizing
  recovery in athletes* (Int J Sports Med).

**F6 · `fundamentals.soreness_vs_injury` — Soreness vs injury**
- Unlock: first soreness or illness signal of any kind. Note the signal predates the
  lesson: a "sick" rest-day reason already exists and already moves readiness
  (`ReadinessAdvisor.kt:79`), and Coach v3 B1's sick flag subsumes it (plan M6). The
  trigger therefore reads the unified flag, not the check-in specifically — a user who
  has only ever logged a sick rest day still unlocks this.
- Teaches: DOMS (dull, symmetric, peaks 24–72 h, fades with a warm-up) vs injury (sharp,
  local, joint-y, worsens under load). Soreness is not the goal and not proof of a good
  session — the repeated-bout effect makes it fade even as progress continues. When the
  coach gates a muscle, this is what it's judging.
- Read more: Cheung, Hume & Maxwell 2003, *Delayed onset muscle soreness* (Sports
  Medicine); McHugh 2003 on the repeated-bout effect (Scand J Med Sci Sports).

**F7 · `fundamentals.warmups` — Why warm-ups are in your session**
- Unlock: first warm-up routine shown.
- Teaches: warm-ups raise tissue temperature and rehearse the movement pattern —
  measurable performance benefit, modest injury-risk story. Ramp sets: same exercise,
  ascending weight, low reps — never to fatigue.
- Read more: Fradkin, Zazryn & Smoliga 2010, *Effects of warming-up on physical
  performance: a systematic review with meta-analysis* (JSCR).

**F8 · `fundamentals.how_the_coach_works` — How your coach works**
- Unlock: first Week Brief.
- Teaches: the loop — snapshot of your data → advisors propose → you approve → the
  watcher judges every applied change in a window → wins earn trust, failures fold into
  bias. Nothing is written unwatched; everything is one-tap undoable. The coach is a
  system you can inspect (Coach Lab), not an oracle.
- Read more: internal — `domain/coach/` (`AutoCoachPlanner`, `OutcomeWatcher`,
  `TrustLedger`, `CoachGenBias`). This lesson is a mirror of the code, not of a paper.

**F9 · `fundamentals.what_readiness_means` — What readiness means**
- Unlock: first readiness score shown.
- Teaches: readiness = "how much should today ask of you," built from sleep, resting HR
  vs your own baseline, your check-in, soreness, and recent load. It *shapes* targets
  within bounds; it never cancels your plan. A low score is information, not a verdict.
- Read more: Saw, Main & Gastin 2016, *Monitoring the athlete training response:
  subjective self-reported measures trump commonly used objective measures* (BJSM).

**F10 · `fundamentals.log_honestly` — Log honestly**
- Unlock: end of cold-start track / first data-gate handoff.
- Teaches: every decision upstream is a function of your log — inflated RPEs, ghost
  reps, and skipped soreness flags don't cheat the coach, they mis-aim it. Honest logging
  is the entire price of Decision Zero. Also: your RPE accuracy itself improves with
  practice.
- Read more: Zourdos et al. 2016 (RIR accuracy in trained vs novice lifters); internal —
  the data-gate thresholds in `AdaptThresholds`.

## Batch 1b — Phase B3: first Coach Concepts (hooks built in A2)

**C1 · `coach.readiness_built_from` — What your score is built from**
- Unlock: tapping the readiness score (first live coach-moment link in the app).
- Teaches: the named parts of ReadinessV2 — last night's sleep, resting HR vs baseline,
  check-in answers, per-muscle soreness, acute load, cardio interference — each shown
  with *today's* value as the live example. Why subjective inputs are weighted seriously
  (they outperform gadgets in the research).
- Read more: Saw et al. 2016 (BJSM); Buchheit 2014, *Monitoring training status with HR
  measures: do all roads lead to Rome?* (Frontiers in Physiology).

**C2 · `coach.why_goals_fight` — Why some goals fight each other**
- Unlock: first goal-conflict flag in the Goal Portfolio.
- Teaches: physiology has budgets — an aggressive calorie deficit and maximal strength
  gain draw on the same recovery and energy budget, so the coach sequences ("cut, then
  strength block") instead of pretending both fit. Compatible goals (a 1RM + consistency
  + zone-2 base) genuinely run in parallel.
- Read more: Murphy & Koehler 2022, *Energy deficiency impairs resistance training gains
  in lean mass but not strength* (Scand J Med Sci Sports); Helms, *The Muscle & Strength
  Pyramid: Nutrition* (goal-sequencing chapters).

---

## Batch 2 — Phase C: Programming I (periodization)

**P1 · `programming.what_a_block_is` — What a training block is**
- Unlock: first block start.
- Teaches: a block is a few weeks with one intent, because the body adapts to trends,
  not single sessions. Named phases beat permanent "go hard" — organized stress, then
  planned backing-off, repeats better than a flat line of effort.
- Read more: Issurin 2010, *New horizons for the methodology and physiology of training
  periodization* (Sports Medicine); Helms, *M&S Pyramid: Training* (periodization ch.).

**P2 · `programming.four_phases` — Accumulate, intensify, peak, deload**
- Unlock: first phase transition (one lesson, phase-specific intro per transition).
- Teaches: ACCUMULATE = build volume at moderate effort; INTENSIFY = trade volume for
  load; PEAK = express the strength you built; DELOAD = planned recovery week. What the
  coach changes in each (volume ramps, progression aggressiveness, readiness bounds).
- Read more: Issurin 2010; Bosquet et al. 2007, *Effects of tapering on performance: a
  meta-analysis* (MSSE) — the peak/taper evidence.

**P3 · `programming.deloads_are_earned` — Deloads are earned, not failures**
- Unlock: first *scheduled* deload announcement (v2 users only saw reactive ones).
- Teaches: fatigue accumulates faster than fitness fades — a deload cashes in recovery
  while keeping the skill. Scheduled beats emergency: the coach plans one per block and
  the fatigue tripwire can only pull it *earlier*. Honest framing: direct research on
  deloading is thin; this is consistent practice + taper research, and the watcher
  verifies it on *your* data.
- Read more: Bell et al. 2023 deloading-practices survey (Sports Medicine – Open);
  Bosquet et al. 2007 (taper); strongerbyscience.com on fatigue management.

**P4 · `programming.reading_your_block_card` — Reading your block card**
- Unlock: first open of the mesocycle UI on the coach screen.
- Teaches: week-in-block, phase intent, next deload date, focus lifts — and what you're
  allowed to do to it at your trust tier (veto, postpone, cap).
- Read more: internal — `BlockPlanner` state machine; this is a product lesson.

## Batch 3 — Phase D: Programming II ("your numbers") + Coach Concepts

The plan calls this batch the single best teaching moment in the product: the app
explains MEV/MRV *using the user's own data*.

**P5 · `programming.your_volume_landmarks` — MEV/MRV: your volume landmarks**
- Unlock: first time a personal volume cap (promoted `volumeResponse` estimator)
  changes an allocation.
- Teaches: MEV = least weekly sets that still produce progress; MRV = most you can
  recover from; the productive zone is between. Population defaults started your caps;
  the coach has now *measured* yours — shown against the default. More volume helps
  until it doesn't; the dose-response curve bends.
- Read more: Israetel (Renaissance Periodization), *Training Volume Landmarks for Muscle
  Growth*; Schoenfeld, Ogborn & Krieger 2017, *Dose-response relationship between weekly
  resistance training volume and muscle mass* (J Sports Sciences).

**P6 · `programming.your_recovery_curve` — Your recovery curve**
- Unlock: first time `restResponse` spacing shapes the directive or block schedule.
- Teaches: how many days *you* need between hitting the same muscle before performance
  returns — estimated from your own bout-to-bout data. Why frequency is a lever, not a
  commandment: total weekly volume matters most; frequency is how you package it.
- Read more: Schoenfeld, Ogborn & Krieger 2016, *Effects of resistance training
  frequency on muscle hypertrophy* (Sports Medicine).

**P7 · `programming.sweet_spot_reps` — Your sweet-spot rep ranges**
- Unlock: first rep prescription biased by `sweetSpotRepRange`.
- Teaches: muscle grows across a wide load range (~5–30 reps taken close to failure);
  strength is more load-specific. Within that freedom, your log shows where you actually
  progress and tolerate work — so prescriptions lean there.
- Read more: Schoenfeld et al. 2017, *Strength and hypertrophy adaptations between low-
  vs. high-load resistance training* (JSCR meta-analysis); Schoenfeld et al. 2021,
  *Loading recommendations for muscle strength, hypertrophy, and local endurance*
  (Sports).

**P8 · `programming.imbalances` — Imbalances (and why the coach hunts them)**
- Unlock: first imbalance-type finding (push/pull, quad/ham) — from the ProjectScanner.
- Teaches: what an imbalance is, how it's measured in your data (volume and strength
  ratios), why it matters (plateau risk, joint-health rationale — honestly labeled as
  weaker evidence than the volume research), and how a catch-up block fixes it.
- Read more: NSCA *Essentials* (needs-analysis and agonist/antagonist balance);
  strongerbyscience.com on weak-point training.

**C4 · `coach.what_a_project_is` — What a project is**
- Unlock: first Proactive Project proposed.
- Teaches: the coach permanently hunts your single biggest lever and runs ONE named
  project at a time — a why, a plan, a finish line. Roadmap shows now / next / done.
  This is "what should I improve?" answered before you ask.
- Read more: internal — `ProjectScanner`, `CoachProject`; product lesson. Each project
  *type* gets its explanatory sibling (P8 covers the imbalance type; future types ship
  with theirs, per the plan's per-project-type hook).

## Batch 4 — Phase E: Coach Concepts (trust & autonomy)

The "tool, not strangle" lessons — ship exactly when autonomy ships.

**C5 · `coach.trust_tiers` — What each trust tier means**
- Unlock: first tier change (and re-surfaced at every change, both directions).
- Teaches: T0 observe → T1 propose → T2 auto-apply earned types → T3 proactive (plans
  blocks, starts projects, sends directive nudges) → T4 full autonomy (owns the program,
  acts first, informs after — and only ever after you say yes to it). Trust is earned
  from *outcomes* (accepted proposals × watcher win-rate). Demotion is by failure *rate*,
  not a single miss — a coach making many calls will get some wrong, and one bad week
  shouldn't reset months of record; skipping something the coach suggested ("not
  followed") is not a failure at all and never costs it trust. You can cap the tier in
  Settings at any time. Key framing: appropriate reliance — trust the automation exactly
  as much as its track record earns, no more.
- Read more: Lee & See 2004, *Trust in automation: designing for appropriate reliance*
  (Human Factors) — the classic on calibrated trust; internal — `TrustLedger`.

**C6 · `coach.taking_decisions_back` — How to take any decision back**
- Unlock: first T3+ autonomous act (paired with its announcement).
- Teaches: every autonomous act is watcher-judged and one-tap revertible; the Settings
  tier cap; how to read the watcher's ok/failed verdicts in Coach Lab; and the Academy
  promise itself — everything the coach decides, you can learn to decide yourself.
- Read more: internal — `OutcomeWatcher` verdicts, undo ledger; product lesson.

## Batch 5 — Phase F: Signals (one per slot, as each activates)

**S1 · `signals.protein` — Why protein gates growth**
- Unlock: `protein_nutrition` slot flips ACTIVE (requires nutrition logging to exist).
- Teaches: muscle is built from what you eat; ~1.6 g/kg/day (≈0.7 g/lb) is where the
  measured benefit plateaus in the meta-analysis — more is fine, less leaves gains
  unbought. Why the coach correlates under-fueling with stalls before blaming your
  training ("this is a kitchen problem").
- Read more: Morton et al. 2018, *Protein supplementation meta-analysis and
  meta-regression* (BJSM); Jäger et al. 2017, ISSN position stand: protein and exercise
  (JISSN); examine.com protein intake guide.

**S2 · `signals.stress_hrv` — What HRV tells us (and doesn't)**
- Unlock: `stress_hrv` slot gets HC HRV data (subjective stress from check-in is
  already covered by C1).
- Teaches: HRV reflects autonomic recovery state — useful as *your* trend vs *your*
  baseline, noisy as a single number. Why it only nudges readiness/deload drivers and
  never overrides your check-in.
- Read more: Plews et al. 2013, *Training adaptation and heart rate variability in
  elite endurance athletes* (Sports Medicine); Marco Altini's HRV4Training blog
  (best practical treatment).

**S3 · `signals.hydration_supplements` — Creatine, consistency, and the boring truth**
- Unlock: `hydration_supplements` slot flips ACTIVE.
- Teaches: creatine monohydrate is the one supplement with mountain-of-evidence status —
  and it only works taken *daily* (saturation, not timing). Hydration's effect on
  performance. Everything else is rounding error next to sleep, protein, and showing up.
- Read more: Kreider et al. 2017, ISSN position stand: safety and efficacy of creatine
  supplementation (JISSN); examine.com creatine page.

## Batch 6 — Engine plan: The Engine track (per Engine phase)

Written per Engine phase, keyed to first live coach moments (first prescription, first
drift alert, first base-trend readout).

**E1 · `engine.why_aerobic_base` — Why lifters need an aerobic base** *(ships E-B)*
- Unlock: first cardio prescription.
- Teaches: a bigger aerobic base = faster recovery between sets and sessions, lower
  resting HR, better work capacity for high-volume blocks — conditioning *serving*
  lifting, never colonizing the week (floors AND ceilings on the prescription).
- Read more: San Millán & Brooks 2018 on metabolic flexibility/mitochondrial function
  (Frontiers in Physiology); Helms *M&S Pyramid: Training* (cardio-for-lifters section).

**E2 · `engine.what_zone2_is` — What zone 2 actually is (and the talk test)** *(E-B)*
- Unlock: first zone-2 prescription tap-through.
- Teaches: zone 2 ≈ the highest intensity where you can still speak in full sentences —
  that's the talk test, and it's validated well enough to prescribe by. Why "embarrassingly
  easy" is the point (mitochondrial adaptation without recovery cost). Explained, never
  enforced — the plan's explicit anti-dogma rule.
- Read more: Persinger, Foster et al. 2004, *Consistency of the talk test for exercise
  prescription* (MSSE); Seiler 2010, *What is best practice for training intensity and
  duration distribution in endurance athletes?* (IJSPP).

**E3 · `engine.interference` — Interference: why cardio placement matters** *(E-B)*
- Unlock: first placement-driven prescription ("Tue · 35 min zone 2 — serving: Base
  Build") or first interference deduction in readiness.
- Teaches: hard cardio near hard lifting competes for the same recovery; the classic
  meta showed interference concentrates in power/explosiveness and long-duration running.
  Modern reading: managed well (modality, dose, spacing) the cost is small — which is
  exactly the management the Engine does for you.
- Read more: Wilson et al. 2012, *Concurrent training: a meta-analysis* (JSCR);
  Schumann et al. 2022, *Compatibility of concurrent aerobic and strength training*
  (Sports Medicine).

**E4 · `engine.reading_hr` — Reading HR: zones, drift, resting HR** *(ships E-C)*
- Unlock: first live-HR session / first drift alert.
- Teaches: zones are anchored to *your* max/thresholds, not a poster; cardiovascular
  drift (HR creeping at constant pace) signals heat/dehydration/duration — why the coach
  may call a session before the timer does; resting HR trend vs your baseline as the
  cheapest recovery signal there is.
- Read more: Coyle & González-Alonso 2001, *Cardiovascular drift during prolonged
  exercise* (Exercise & Sport Sciences Reviews); Buchheit 2014 (Frontiers in Physiology).

**E5 · `engine.intervals` — Intervals: dose and recovery** *(E-C)*
- Unlock: first interval prescription.
- Teaches: intervals buy top-end adaptation at a steep recovery price, which is why the
  Engine *rations* them (the plan's rule) — a small weekly dose on top of the base, never
  the default answer, and placed where they don't rob a lifting day.
- Read more: Buchheit & Laursen 2013, *High-intensity interval training, solutions to
  the programming puzzle*, Parts I–II (Sports Medicine).

**E6 · `engine.base_without_a_lab` — How Engine measures your base without a lab** *(E-D)*
- Unlock: first base-trend readout on a goal card / Week Brief.
- Teaches: no lactate lab needed — pace-at-HR (same effort, faster pace = fitter),
  HR drift within sessions (aerobic decoupling), and resting-HR trend triangulate your
  aerobic base; that trend is what ramps, holds, or deloads your conditioning volume.
- Read more: Buchheit 2014 (the monitoring-metrics review); TrainingPeaks' aerobic
  decoupling (Pa:HR) articles — the practical version of the same idea.

---

## Authoring order at a glance

1. **Phase A2 batch (1):** C3 — ships with `WeightPhase`, and forces the first version of
   the lesson-block renderer.
2. **Phase B3 batch (12):** F1–F10, C1, C2 — the big one; F1–F10 double as the cold-start
   directive.
3. **Phase C batch (4):** P1–P4.
4. **Phase D batch (5):** P5–P8, C4.
5. **Phase E batch (2):** C5–C6.
6. **Phase F batch (3):** S1–S3 — written only as each slot activates.
7. **Engine batches (6):** E1–E3 with E-B, E4–E5 with E-C, E6 with E-D.

Total 33. Phases B1 and B2 write no lessons — their concepts (readiness parts, the
directive) are taught by C1 and the Fundamentals track in B3, which is why B3 follows
them directly rather than trailing the phase.

**When the 1:1 audit is enforced.** At the end of each *release series* — v3.0 (A1+A2),
v3.1 (B1+B2+B3), then each later phase on its own. Within a series an intermediate phase
may ship a concept whose lesson lands a phase later (ReadinessV2 in B1, taught by C1/F9
in B3); no series ever ends with an unlessoned concept or an orphan lesson. This is why
C3 cannot wait: A2 ends the v3.0 series.

Audit rule per batch (from the plan): every shipped coach concept has a lesson, every
lesson is reachable from a live coach moment, cold-start never renders blank.
