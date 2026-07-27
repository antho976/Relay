# Forge — Wear addendum

Satellite of `.claude/DESIGN.md`. Read this **only when touching `:wear`**. The core doctrine still
applies; this records how it translates to a 1.4" round AMOLED.

`§` cross-references point at the core `.claude/DESIGN.md`, whose numbering is unchanged for §1 and §3–§13.

---

## Wear addendum (`:wear` — Galaxy Watch 6 Classic reference; Wear OS 3+, round)

The doctrine translated to a 1.4" round AMOLED, not reinvented. The watch answers "what now,
this instant?" only (§Principle 2 of `docs/WEAR_OS_PLAN.md`) — no stats, no history, no settings.

- **Ground**: pure black (`#000000`) always — AMOLED battery + bezel blend; no gradient plate.
  Surfaces exist only on interactive capsules (`#15161B`); passive content sits bare (§1 holds).
- **Accent**: the phone's accent arrives via `/config` (hex + enabled) and obeys the §5 ladder
  (1.0 strokes/actions · 0.6 secondary · 0.15 washes). Monochrome mode maps to onBg like the phone.
  Disconnected/stale state renders mono only — color is earned by live data. PR gold `#E3B341`
  reserved exactly as on the phone (PR flash only).
- **Type**: ONE serif figure per screen (timer countdown / target weight) — `headlineL`-scale,
  tnum; everything else mono uppercase micro-labels (10–13sp) or `bodyM` sans. Never two serif
  voices on one round screen. Curved text only for the top eyebrow label (time/exercise name),
  never prose.
- **Layout**: center-weighted single column; safe inset 8% of diameter; one decision per screen;
  swipe-dismiss preserved (no custom back). Screens: idle home · session (figure big) · rest
  timer · RPE picker (post-log, reached from the transient undo/`rate →` row on timer + set
  views) · one 20s "Finished" beat when the session ends (points at the phone for notes/details).
  Bezel/rotary = the primary adjust input; touch ± steppers always present (44dp touch, 34dp
  visual, flanking the figure). **The serif figure IS the adjust target**: weight big by default,
  tap the small reps line to bring reps up (and back) — bezel and steppers always change the big
  number; bodyweight slots pin reps big. Guard rails answer with a **Confirm capsule** (one more
  tap), never "do it on the phone". Status/error lines sit ABOVE the capsule (below clips on
  round). Nothing fake-tappable (§4 holds).
- **Ambient**: dimmed mono only — onBg text at 0.6, no accent, no surfaces, burn-in-safe (no
  large filled areas); timer keeps the serif countdown, everything else drops.
- **Haptics**: timer-done = the one strong buzz (with phone-suppression ack); set-logged = short
  tick; PR = double-tick + gold flash. Nothing else vibrates.
- **Copy**: §11 voice verbatim — dry imperative, no exclamation marks, mono labels uppercase.
  Disconnect state says "Reconnecting" + last-known data with its age, never an error banner.
- **Tiles/complications**: data-age stamped (mono labelS muted, "2H AGO"); degrade to
  next-day + readiness, never blank (§12 spirit: honest zero over hidden).
