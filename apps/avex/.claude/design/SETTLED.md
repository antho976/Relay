# Forge — settled decisions

Satellite of `.claude/DESIGN.md`. Read this **before re-adding anything that feels missing** — a
card, a hero, a summary, a gamification surface. Most "obvious improvements" to this app have
already been tried and deliberately removed; this is the record of which, and why.

Also lists what may not be touched, and the known defects to fix opportunistically.

`§` cross-references point at the core `.claude/DESIGN.md`, whose numbering is unchanged for §1 and §3–§13.

---

## Settled — do not reintroduce / do not touch

**Removed on purpose**: boxed cards for passive content · full-screen PR takeover (PR = confetti +
gold row) · accent-tinted "important" prose · session-summary extras (share card/tags/ghost/vs-last/
what's-next — the SESSION-summary card specifically; the Profile before/after share card `BeforeAfterCardRenderer` (GYMAP-55) is a separate, deliberate shareable artifact and stays) · cardio big-number hero · cardio kcal estimates (return only with real watch burn data) · new gamification surfaces (wait-listed) · Profile
identity-first restructure · mood/subjective coach drivers · Coach hero week-dot calendar, "Pulse",
pass-square record strip · **every page-level banner** (2026-07-27: Home's milestone toast, coach-brief
strip, orphan-session notice and resume reminder; Cardio's connect-a-watch invite — all moved to the
notifications feed behind the bell, §4.6. Do not re-add a strip above a page's own answer; if it is
dismissible or "waiting on you", it is a notice, and notices live in the feed) · **the three
launch/result dialogs** (share-import result, backup-restored confirmation, POST_NOTIFICATIONS
rationale — same date, same destination; an OK button is not a decision, and nothing should
interrupt a cold launch to ask for a permission) · the `• Avex` chrome
wordmark and `ForgeWordmark` itself (2026-07-27, replaced by `NotificationBell`; the name survives as
the launch beat and on exported artifacts only) · **the Coach→Academy link** (2026-07-27: the
knowledge half of the coach is its own bottom tab now, so a link from a sibling tab is redundant
navigation, §4.2. Removed from the Goals section; the `onOpenAcademy` wiring into `CoachScreen` went
with it. Home's lesson card is the one entry point that stays, because it is contextual — it names a
lesson, not the tab) · the Profile's ON THIS DAY (Home's `OnThisDayCard` owns that throwback — a mark is cut, not copied, §4.3; it was also the page's one prose-only section, hung off a decorative accent rule) · Coach status serif verdicts AND status/anticipation asides (status states
= eyebrow + figures) · Coach pre-baseline signal dot-checklist in the hero (→ one labeled Baseline
bar in the "Coming up" idiom; the effort/HC inputs it spelled out live in Signals only, §4.3,
GYMAP-24) · hairline section separators (§1) · the 9-row milestone ladder (→ rail + next, §4.10).
**Facts**: dark-only (Indigo light scheme in `Color.kt` unshipped — never build light variants);
portrait phone only (no adaptive/tablet/landscape).
**Untouchable**: live-session screen · Profile blending cover (the compositing approach only — its
mask stops are tuning, retuned 2026-07-09 and again 2026-07-24 when the extra text-scrim was deleted
for good; never reintroduce a second scrim over the fade) · statsEntrance/draw tuning ·
`BodyAnatomy.kt` (generated).
**Known defects, fix when touched**: Home's GOALS section placement · Stats' 16dp gutter (§7 says 24 —
left because re-flowing the polished screen needs Antho's eyes) · any screen still drawing section
hairlines → migrate to air rhythm (§7). (Home's accent eyebrows + the Home/Stats section hairlines
were fixed 2026-07-08, GYMAP-4.)

## Open decisions — accessibility contrast (2026-07-24)

Measured against Pearl bg `#0E0E11` (WCAG 2.1; AA normal text = 4.5:1). Two of these are failures the
doctrine itself currently *mandates*, so they need a product call, not a silent fix. Until they are
resolved, **§14 forbids adding new accent-coloured or error-coloured body text** — new work uses onBg
text with an accent glyph or mark.

**1 — `action →` links are accent-coloured text at 2.35–3.40:1.**

| Accent | vs Pearl | vs AMOLED | AA normal (4.5) | AA large (3.0) |
|---|---|---|---|---|
| Navy `#3D4F73` (default) | 2.35:1 | 2.56:1 | fail | fail |
| Red `#8B3535` | 2.44:1 | 2.66:1 | fail | fail |
| Olive `#4D6040` | 2.81:1 | 3.06:1 | fail | fail |
| Gold `#7A6435` | 3.40:1 | 3.70:1 | fail | pass |

This is load-bearing: `action →` is level ③ of the button system (§8) and the accent-as-navigation
signal runs through the whole app. It is also in direct tension with §5's "design against muted navy —
needing a vivid accent means too much accent", which is a deliberate aesthetic choice. Options, none
taken yet: (a) accept and document as a known limitation; (b) render `action →` in onBg with the `→`
glyph alone in accent — keeps the colour signal, moves the text above 4.5:1; (c) lighten every accent
until it clears 4.5:1, which changes the app's look; (d) lift accent text to AA-large sizes only.
Monochrome mode (§5) already resolves this incidentally, since `primary` falls back to onBg.

**2 — the inline error line is `#BF4040` at 3.69:1.** §12 mandates "quiet inline line in error color"
for every error, and error text is the text a user most needs to read. Lightening `error` toward
~`#D96565` would clear AA without touching the accent system, and `error` is reserved for true states
(§5) so the blast radius is small. Not applied — it changes a reserved colour.

Everything else measured clean: onBg 16.66:1 · muted 1.0 9.41:1 · muted@0.7 5.07:1 · muted@0.65
4.54:1 (the floor; 0.6 gives 4.05:1 and fails) · PR gold, △ green, success and warning all 7.1–9.9:1.
Structural hairlines and tonal washes are exempt as decorative boundaries (§14).

