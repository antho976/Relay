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
dismissible or "waiting on you", it is a notice, and notices live in the feed. **Narrowed
2026-08-15**, see "Two rules narrowed" below) · **the three
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

## Home, cut down to two questions (2026-08-16)

Antho's brief, verbatim: *"What do I do now, really small summary of important info that changed, and
that's it"* — plus keep RECENT, and keep GOALS but make them readable at a glance.

**Removed from Home, and where each lives now**: the WORKOUTS tile (its data is the week strip, which
says the same thing legibly — the old 6dp dash rail was the same information drawn as debris) ·
the CARDIO tile (Cardio tab) · VOLUME THIS WEEK (Stats; it also duplicated the figure the RECENT row
already showed) · TODAY'S TARGETS (folded into the hero's whisper line) · the Academy read strip
(Academy tab) · MOVEMENT / steps (Stats) · ON THIS DAY, the coach cards and the fatigue nudge (Coach
tab; their signal survives compressed into the single changed-line) · TROPHIES · the
"Nutrition · soon" footer.

**The test each survivor had to pass: is something at stake?** A count of finished workouts reports
something already settled, so there is nothing to feel about it — which is why the page could be read
as "bland" with nothing identifiably wrong in it. Do not re-add a past-tense tally to Home.

**The hero's button follows the COACH, not the plan mode.** It used to branch on freestyle / no-program
/ otherwise and never read the directive, so a day the coach had already called rendered "Done for
today" over a filled "Start session →" — the page contradicting itself in its two largest elements.
`TodayDirective.Kind` decides now (resume and no-program outrank it, being facts rather than opinions).
On REST the action survives but goes **outlined**: you may train on a rest day, but a filled ember
capsule is the app's one "do this now" signal and a rest day has none. "Train anyway" is the wording,
because it hands over the agency without the app pretending it recommended this.

**Also gone**: the accent glow behind the hero (tried and cut the same day it shipped — it bought
atmosphere without an asset, but a tinted wash across a page is decorative colour, and this design
spends accent on decisions only; **do not re-add a background glow**) · the goals card carousel (a horizontal strip that shows one and a half cards cannot
answer "at a glance"; three stacked lines fit in less height than one card) · its ghost cells (an
absent goal is an absent row, never an empty rectangle — three of those read as content that failed
to load) · RECENT's grey-box zero state with its ghost bar rail (a loading skeleton on a screen where
the local DB is instant can only read as broken).

## Two rules narrowed — the Academy arrival receipt (2026-08-15)

Phase 2 of the Academy rework needed two things §4.6 had ruled out. Both are **narrowings with a
stated boundary**, not exceptions, and both are here so the next reader does not simply delete them
as violations.

**1. A transient banner may cross a page. A resident strip still may not.**

The 2026-07-27 ban was written against strips that sit above a page's own answer and push its
content down: Home's milestone toast, the coach-brief strip, the connect-a-watch invite. Every one
of them was *resident* (present when you arrived, present when you came back) and *dismissible*
(a decision the page asked you to make before it would show you its content).

`ArrivalBannerHost` is a different object on all three counts. It is an overlay in a `fillMaxSize`
Box above the nav host, so **nothing on the page moves**; it has **no dismiss affordance** because
it leaves on its own after ~1.6s; and it **cannot be present when you next look at the page**,
because each arrival is announced exactly once and the announcement is persisted
(`announcedLessonNotices`). It carries no decision — the decision stays in the feed, where the ban
put it. The banner only says "that went behind the bell", which is the sentence the ban's own
logic requires somebody to say.

Test for whether a future banner is allowed: **if it can still be there when you come back, it is a
strip and it is banned.**

**2. A tab may badge its own count. The bell stays Home-only.**

§4.6 called a global unread badge on every page "a nag", and that judgement stands for the BELL:
it counts everything waiting on you, so putting it on every screen turns every screen into an
inbox. The Academy tab badge is a narrower claim — one count, for one destination, of something a
reader can ignore forever with no consequence — and it appears on a control that is already
permanently on screen, so it adds no chrome. A page still may not repeat the bell's count.

**3. The bell's dot became a count.** A dot answered "something happened" and stopped. That was
enough while notices were rare; once the Academy feeds the feed, two waiting things and five
waiting things are different decisions about whether to look now. `CountBadge` draws the numeral in
`onPrimary` **on** the accent fill rather than as accent-coloured text, because accent as text
fails AA at all four accents (§14) — the open contrast defect below is not to be widened.

## Open decisions — accessibility contrast (2026-07-24, **partly resolved 2026-08-16**)

Measured against Pearl bg, now the warm `#110F0C` (WCAG 2.1; AA normal text = 4.5:1). Until these are
resolved, **§14 forbids adding new accent-coloured or error-coloured body text** — new work uses onBg
text with an accent glyph or mark.

**1 — `action →` links are accent-coloured text. RESOLVED FOR THE DEFAULT ONLY.**

| Accent | vs Pearl | AA normal (4.5) | AA large (3.0) |
|---|---|---|---|
| **Ember `#D4761F` (default, 2026-08-16)** | **5.84:1** | **pass** | pass |
| Red `#8B3535` | 2.42:1 | fail | fail |
| Olive `#4D6040` | 2.79:1 | fail | fail |
| Gold `#7A6435` | 3.37:1 | fail | pass |
| Navy `#3D4F73` (former default) | 2.34:1 | fail | fail |

Option (c) — lighten the accent until it clears 4.5:1 — was taken for the DEFAULT, as a side effect of
the warm repalette rather than as an accessibility fix. Ember was chosen for heat (Antho: Home felt
"bland and lifeless"; navy-on-near-black is the deadest available pairing) and clearing AA came free.
§5's old "design against muted navy" line went with it: the rule is now **spend accent in few places
at large size**, which is the same restraint expressed as placement rather than as dimness.

Still open for the four alternates, so the ban stands as written. A user who picks Red still reads
`action →` at 2.42:1. The remaining options are unchanged: (a) accept and document; (b) render the
words in onBg with the `→` glyph alone in accent; (d) lift accent text to AA-large sizes only.
Monochrome mode (§5) resolves it incidentally, since `primary` falls back to onBg.

**2 — the inline error line is `#BF4040` at 3.69:1.** §12 mandates "quiet inline line in error color"
for every error, and error text is the text a user most needs to read. Lightening `error` toward
~`#D96565` would clear AA without touching the accent system, and `error` is reserved for true states
(§5) so the blast radius is small. Not applied — it changes a reserved colour.

Everything else measured clean: onBg 16.66:1 · muted 1.0 9.41:1 · muted@0.7 5.07:1 · muted@0.65
4.54:1 (the floor; 0.6 gives 4.05:1 and fails) · PR gold, △ green, success and warning all 7.1–9.9:1.
Structural hairlines and tonal washes are exempt as decorative boundaries (§14).

