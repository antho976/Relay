# Forge — settled decisions

Satellite of `.claude/DESIGN.md`. Read this **before re-adding anything that feels missing** — a
card, a hero, a summary, a gamification surface. Most "obvious improvements" to this app have
already been tried and deliberately removed; this is the record of which, and why.

Also lists what may not be touched, and the known defects to fix opportunistically.

`§` cross-references point at the core `.claude/DESIGN.md`, whose numbering is unchanged for §1 and §3–§13.

---

## Settled — do not reintroduce / do not touch

**Removed on purpose**: boxed cards for passive content · **the Academy's card tiles, its lesson
sheet and its FOR YOU shelf** (2026-08-20: filled hairlined cards around 35 pieces of passive
content read as a wall whatever is inside them, which is the same finding as the first entry in this
list arrived at from the gallery end — pieces are plates with their captions under them now; the
`ModalBottomSheet` reader capped a lesson's height, could not carry a cover and counted a dismissal
as a completed read; the FOR YOU shelf became the page's one opening pointer, which is lifted out of
its own chapter so nothing is printed twice. Do not re-add a card, a scrim-over-photo caption, or a
second reader) · full-screen PR takeover (PR = confetti +
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
with it. Home's contextual lesson link was the one entry point that stayed; it was **removed
2026-08-22**, see the Home entry below, so no sibling tab links to the Academy at all now) · the Profile's ON THIS DAY (Home's `OnThisDayMemoryLine` owns that throwback — a mark is cut, not copied, §4.3; it was also the page's one prose-only section, hung off a decorative accent rule) · Coach status serif verdicts AND status/anticipation asides (status states
= eyebrow + figures) · Coach pre-baseline signal dot-checklist in the hero (→ one labeled Baseline
bar in the "Coming up" idiom; the effort/HC inputs it spelled out live in Signals only, §4.3,
GYMAP-24) · hairline section separators (§1) · the 9-row milestone ladder (→ rail + next, §4.10).

**Removed by the Coach ledger redesign (2026-08-20, Antho: "I hate everything in the coach app").**
The page was doctrine-compliant and still unusable: eight sections across three lenses, all of them
rendering engine state, none of them leading. Do not re-add any of these:

- **The Now / Signals / Journey lens pills.** Splitting "what changed", "why", and "did it work"
  across three taps meant no view answered how training is going, and it forced the same facts to be
  policed for duplication across lenses instead of simply sitting together. One column now, ordered
  by time. `CoachLens` and `initialLens` are gone; `CoachEntryPoint` replaces them and the old
  `COACH_LAB` / `COACH_TIMELINE` routes resolve to a scroll position, not a tab.
- **The serif verdict hero.** "2 proposals" at the 52sp display rung is an inbox badge blown up to
  poster size — the biggest thing on the screen was a count of pending admin. The change itself
  carries the serif rung now, inside the call it belongs to. (This extends the earlier "no status
  serif verdicts" removal above to the *count* verdicts too.)
- **The signal registry rail** (`SignalSlotRail`, "Also on the roadmap", "hollow is waiting on your
  data, dim is on the way"). A roadmap of unbuilt features is not a reading. `SignalRegistry` still
  exists in the domain layer; nothing renders it.
- **The dormant half of the fatigue instrument panel.** Every check the advisor tracks, listed with
  its quiet reading, was the machine showing its work. Only checks that FIRED render, and the full
  panel appears as evidence *inside* a deload call, where it is the argument for something.
- **The "Coming up" countdown section.** Its contents dispersed to where they are about: the next
  brief is the `THIS WEEK` anchor's right meta, a pending verdict is the watch bar on its own entry,
  the milestone rail is in `AHEAD`.
- **"Under watch" as its own section.** A change under watch is the same entry it always was; it
  just carries its window. Nothing moves between sections when it resolves — it is stamped in place.
- **Per-goal archive from Coach.** The wiring existed and no affordance ever reached it; archiving
  belongs to the Goals screen.

What the redesign deliberately KEPT, all four confirmed by Antho as non-negotiable: apply / skip /
undo on every change; the evidence behind a call, visible before approving; the week-by-week record;
and goals + block + project.

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

## Onboarding, cut from fifteen steps to eight (2026-08-22)

The rebuild (`design/DECISIONS.md`, same date) removed five whole screens and two readouts. None of
it is missing — every item is still asked or still reachable, just not in front of the plan.

**Moved out of the path, into one optional closing step** ("Anything else?", after the week exists):
your name · weight and distance units · bodyweight and sex · watch brand (removed outright a day
later, see below) · app lock · plate weight · auto-refresh cadence. (Sore spots went the other way — see below.) Every one also lives in Settings, and every one has a working
default, so walking past the whole page is a complete answer. Do not push any of them back in front
of the plan: they are settings, and a setting asked before the product has shown anything is a toll.

**Cut outright:**

- **The gym step's "in this setup" gear dump** — a mono list of every selected piece, under the
  preset grid. The week meter below it now answers what the preset DID, and the fine-tune page next
  lists every piece with its own on/off state. Two readouts of one answer, §4.3.
- **The day step's split readout** ("PUSH · PULL · LEGS") — the meter's bars are labelled with the
  same day names, so the line restated the mark directly beneath it.
- **The wearable step's per-brand version caveat** ("Older models send fewer") — kept in Settings →
  Recovery, where the user can act on it. On the closing step the brand pick answers with one line
  naming what that companion app feeds through, which is the fact that helps at that moment.
- **The mono chapter eyebrow on every step** ("ABOUT YOU", "YOUR GYM") — the step rail already says
  where you are. This is onboarding-only; every other §3 archetype keeps its eyebrow.
- **The coach `AlertDialog`** — a boolean with a sane default is a row, not an interruption (§12).
  It is now offered on all three plan modes rather than only the two that got the dialog.

**Promoted, not cut:** sore / injured spots. It started the rebuild as a chip row on the closing
step and ended it as its own page, before the week — it shapes exercise selection, so asking after
the preview shaped a plan the user had already approved. Do not fold it back into a settings list.

## Onboarding, the watch question and the week page (2026-08-23)

**The watch pick left onboarding entirely.** It survived the 2026-08-22 rebuild by moving to the
closing step; a day later it went. Picking Galaxy / Pixel / none changed nothing the user could ever
see — it only tailored the WORDING of Settings → Wearable's sync pointers — so it was a question
whose answer had no consequence, asked while the user was still in the flow. Wearable setup needs
the Health Connect grants anyway, which is a Settings job, and Settings → Wearable already asks the
same question with the same enum. Do not put a device question back into first run: nothing in the
flow can act on the answer. (`OnboardingDraft` dropped its `wearable` field with it; the schema
stayed at 4, because the path length and therefore the resume cursor did not change.)

**The week page's exercise dump.** "Here's your week" used to redraw the `PlanLedger` bars it had
already shown for three screens, then list every exercise of every day underneath — roughly 25
uniform rows across three viewports for a four-day week, with each day's volume stated three times
over (once by a bar, once by a day header, once by its rows). §3 bans a long multi-block scroll for
this archetype and exempts only the closing step. It is now ONE mark that navigates — the same bars,
accent on the day you are reading — over that one day's movements. Do not re-add a linear all-days
list: every day is on screen with its real volume from the moment the page opens, one tap from being
read in full, so nothing is hidden by it.

**Cut with that rewrite:** the per-day colour dot on each exercise section. Seven hues at 8dp is the
scattered-tiny-accent §5 forbids, and the mono day name already carries day identity. The Program
Editor keeps its dot — it lists every day at once, where the colour is doing work.

**Do not re-add:** a welcome screen. The cold-launch `AvexIntro` plays the wordmark moments earlier,
§3 bans a wordmark pre-app, and the plan-mode fork is a better opening beat than a greeting — it is
the most consequential question in the flow and the only one with motion in it.

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

## Home, put back on the app's own rails (2026-08-22)

Antho: *"I like the current design of the home page but something's wrong with it and I can't
point what."* Nothing in the page's structure was wrong. Home was speaking two design languages at
once and changing the ground under your feet, which is felt on every swipe and never looked at
directly. Six removals, no content cut:

- **The pure-black ground.** `OverviewScreen` copied the scheme to `background = surface =
  Color.Black` and painted it opaque through its own Scaffold. `ForgeTheme` paints the Pearl
  gradient behind every screen and `HubScreen`'s Scaffold is transparent precisely so each pager
  page sits on it, so Home was the one page in the pager whose ground changed: flat cold black on
  Home, warm gradient one swipe either side. It also undid the 2026-08-16 warm repalette on the
  exact page that repalette was for, and flattened AMOLED against Pearl. Its text and outline
  colours were read from the OUTER scheme anyway, so warm Pearl ink was already sitting on a cold
  black page. **Do not re-theme a pager page's background.**
- **The sans hero.** `headlineLarge` was copied and overridden to `FontFamily.SansSerif` with a
  hand-set `letterSpacing` at the call site, so the page's biggest element was out of the
  three-voice system (§6) while every other overview leads serif — and out of the 1.3× hero clamp,
  so at 200% font scale it pushed the CTA off the fold. It renders through `HeroHeadline` now,
  which took a `style` parameter to serve both the display and headline rungs.
- **The second section-header treatment.** THIS WEEK was a sans `titleMedium` in sentence case with
  its own right meta while GOALS and RECENT used `SectionAnchor`, so three sections carried three
  header treatments in three type voices. `SectionAnchor` gained an optional passive `meta`, which
  is the only reason THIS WEEK had forked in the first place.
- **The 14dp button radius.** A radius off `Shape.kt` entirely (§7), so it matched neither the app
  nor the page. Corrected to pills first, and that was **wrong — reverted the same day** (Antho:
  *"make the start a session and plan rounded like the this week section, that'd look more
  coherent"*). Home states a geometry of its own and states it twice, in `CellShape`'s rounded-square
  week cells and the RECENT rows' 10dp leading marks; the CTA row sits directly above the week strip,
  so a pill put the one disagreeing shape immediately above the element that defines the page. Both
  capsules bind to `CellShape` itself at 56dp, so they cannot drift from the cells. **Home's buttons
  are deliberately the only non-pill buttons in the app** — page coherence beat app coherence here
  because the two elements are adjacent. Do not "fix" them back to `RoundedCornerShape(50)`.
- **The half-bounce button pair.** The primary took `bounceCombinedClick` and the "Plan" capsule
  beside it took a bare `clickableLabeled`, so two adjacent controls answered the same thumb
  differently (§9). A control that does not answer the press reads as the disabled one.
- **The box around ON THIS DAY.** A 12dp radius and a hairline border around content its own KDoc
  called display-only — §1's central ban, and the only bordered element on a page whose every other
  passive line sits bare, so it read as the one pressable thing and was not. Renamed
  `OnThisDayMemoryLine`; the memory itself is unchanged.

Also paid down, same pass: the page gutter (20 → §7's 24; Stats' 16 is still the open defect), the
spacing scale (5dp and 3dp gaps between text of different roles → one 8/12/16/20/28 rhythm, §7),
the entrance (`tween(450)` → `ForgeMotion.enterTween`, so **Remove animations** is honoured again),
two `fontSize =` call sites and `CardLink`'s ~24dp touch target (§14). The workout count stopped
being stated three times inside THIS WEEK — anchor meta, week strip, and the facts line — and the
facts line now renders nothing rather than an empty line when it has nothing left (§4.3).

**The Academy cold-start strip, removed the same day (Antho: "remove the academy strip above the
button in home").** A `Read <lesson title>` link in accent mono, sitting between the hero's reason
lines and the CTA row, shown while `coldStartLesson` was non-null (a LEARN directive, or fewer than
`COLD_START_SESSIONS` = 6 sessions logged). It was the last thing between the page's answer and the
page's one filled capsule, and on a new account — the only time it appears — that is exactly where
the CTA needs to be. The Academy is a bottom tab and it badges its own count (§4.6 as narrowed
2026-08-15), so it is not unreachable; a LEARN directive still points the CTA itself at it.

Its data plumbing survives unused: `AcademyRepository.coldStartLesson()` →
`DirectiveRepository.TodayAnswer.coldStartLesson` → `OverviewUiState.coldStartLesson`. Nothing
renders it. Remove the chain or give it a home; do not re-add the strip.

**Still open on Home, deliberately not touched:** `HomeHero` has no call sites — the hero is still
assembled inline because the page's CTA row is `[action][Plan]` and `HomeHero` owns a full-width
CTA. MOVEMENT and the weekly volume/cardio facts line are both listed as REMOVED from Home by the
2026-08-16 cut-down above and are both rendering again; that is a content call, not a craft one.
`RecentRow` still passes `palette.mutedOnCard` (the 0.70 on-card floor) on a page that has no card.

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
| **Red `#E23D3D` (default, 2026-08-23)** | **4.53:1** | **pass** | pass |
| Ember `#D4761F` (former default, 2026-08-16) | 5.84:1 | pass | pass |
| Red `#8B3535` (dropped 2026-08-23 — could not be the default) | 2.42:1 | fail | fail |
| Olive `#4D6040` | 2.79:1 | fail | fail |
| Gold `#7A6435` | 3.37:1 | fail | pass |
| Navy `#3D4F73` (former default) | 2.34:1 | fail | fail |

Option (c) — lighten the accent until it clears 4.5:1 — was taken for the DEFAULT, as a side effect of
the warm repalette rather than as an accessibility fix. Ember was chosen for heat (Antho: Home felt
"bland and lifeless"; navy-on-near-black is the deadest available pairing) and clearing AA came free.

**2026-08-23 — the default is Red.** Asked for a red default, and the palette's own Red `#8B3535`
measured 2.42:1, i.e. the same failure band as the Navy it would have replaced. Rather than reopen
this issue, the default is a lighter `#E23D3D` at 4.53:1 and `#8B3535` was dropped from the presets.
Clearing 4.5:1 is now a stated requirement of any default accent, not a happy accident — and the
current value has only 0.03 of margin, so it cannot be darkened without re-measuring.
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

