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

They interlock: watch HR (W3) powers Engine's live coaching (E-C) and ReadinessV2 (Coach B);
Engine's load feeds readiness interference (Coach B) and the Today Directive's rest-day
prescriptions; the Today tile (W4) renders the directive (Coach B). No hard blockers
anywhere — every phase has a defined degraded mode.

## Suggested sequence

1. **Wear W0** — HC exercise-session write-back. Tiny, instant "Samsung Health" win.
2. **Coach v3 A** — eat everything + Goal Portfolio (foundation both later plans lean on).
3. **Wear W1–W2** — timer + set logging on the wrist (the headline watch release).
4. **Coach v3 B** — Today Directive + ReadinessV2 + check-in + Academy foundation.
5. **Engine E-A → E-B** — zones/load, then the coached conditioning week.
6. **Wear W3** — live HR (unlocks the next two).
7. **Engine E-C** — live zone coaching on the wrist.
8. **Coach v3 C–D** — blocks, then the learning loop + projects.
9. **Wear W4** — tiles/complications (directive now exists to render).
10. **Engine E-D** — the base loop; conditioning joins the weekly pass.
11. **Coach v3 E–F** — initiative/trust ladder, then future signal slots.

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
