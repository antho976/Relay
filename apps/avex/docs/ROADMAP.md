# Avex Roadmap — the long arc

> Light sequencing doc, not a plan. Each item links to its real plan (or is a parked
> sketch). Solo-dev pace assumed; order optimizes for shippable releases and for each
> plan feeding the next. Revisit after every shipped phase.

## The three committed plans

| Plan | Doc | One-liner |
|---|---|---|
| Coach v3 + Academy | `COACH_V3_PLAN.md` | The coach that makes itself optional — goals, blocks, directive, trust, knowledge layer |
| Wear OS | `WEAR_OS_PLAN.md` | Companion watch app (Google + Samsung), live HR, HC write-back |
| Engine | `ENGINE_PLAN.md` | Conditioning coached as a lifter — zones, placement, base loop |

They interlock: watch HR (W3, **shipped**) powers Engine's live coaching (E-C) and
ReadinessV2 (Coach B1); Engine's `ConditioningLoad` is the single interference formula
feeding readiness (Coach B1) and the Today Directive's rest-day prescriptions; the Today
tile (W4, **shipped**) renders the directive once Coach B2 exists. No hard blockers
anywhere — every phase has a defined degraded mode, and every surface now declares its
freestyle / coach-off behavior too.

## Suggested sequence

> Rev 3 (2026-07-24): **the entire Wear plan (W0–W6) has shipped**. Both remaining plans
> re-verified at 0.8.8.3 / schema v31 and amended to rev 3; the audit behind those
> amendments is `COACH_ENGINE_PLAN_AUDIT.md`. Coach Phase A and Phase B are each split
> into shippable units — the sequence below uses the split names.

1. **Coach v3 A1** — eat everything: bodyweight series, moods/failure-tag/HRV/steps
   consumption, session-type tagging. No new surfaces. (Cheaper than rev 1 planned —
   HRV/steps/sleep-stage plumbing landed with Wear, and the session-type write path
   already exists.)
2. **Coach v3 A2** — Goal Portfolio + SignalRegistry/AcademyRegistry contracts + the
   first lesson. Ends the v3.0 series.
3. **Coach v3 B1** — ReadinessV2 + check-in + life events + three-valued verdicts.
4. **Coach v3 B2** — Today Directive + PreSessionBrief; the widget and wear tile become
   directive surfaces.
5. **Coach v3 B3** — Academy foundation + cold-start directive. Ends the v3.1 series.
6. **Engine E-A → E-B** — zones/load (includes age/max-HR capture), then the coached
   conditioning week.
7. **Engine E-C** — live zone coaching on the wrist (W3 HR is live; needs the
   `/cardio/live` protocol rev + its own `cardio_hr_sample` storage).
8. **Coach v3 C–D** — blocks, then the learning loop + projects.
9. **Engine E-D** — the base loop; conditioning joins the weekly pass.
10. **Coach v3 E–F** — initiative/trust ladder, then future signal slots.

Engine E-B now depends on Coach A2 (`CoachGoal` + catalogue) rather than carrying a
standalone-goal fallback — the fallback was dead by construction at this ordering and has
been removed from the Engine plan. E-A remains standalone and can fill any slot.

Rule of thumb kept throughout: alternate a platform release (watch) with a brain release
(coach/engine) so every few releases have a visible headline.

## Parked (sketched, not planned)

- **Anywhere** — gym/equipment profiles + loadability: suggestions only prescribe weights
  the current room can load; travel mode re-plans sessions preserving intent. Rated: strong
  major, not a flagship. Natural slot: after Coach v3 E (SessionAdaptor exists) — much of
  it becomes a constraint layer on machinery built there.
- **Hands-free** — on-device voice logging (tiny grammar, not open dictation), earbud/watch
  push-to-talk. Interaction moonshot; pairs with the watch. Revisit once W2 real-world
  usage shows where tapping still hurts.
- **Fuel** — protein-first offline nutrition. Judged good-but-bloat-risk for now; it is the
  declared unlock for Coach v3 Phase F's `protein_nutrition` slot, so the decision point
  is when Phase F approaches — build Fuel, or leave the slot COMING_SOON indefinitely.
- **Passed on**: Mirror (physique ledger), Form Lab (video form analysis), social/sharing,
  yearbook/narrative, outcome simulators — reconsider only if the app's audience shifts.
