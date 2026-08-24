# Forge — settled decisions

Satellite of `.claude/DESIGN.md`. Read this **before re-adding anything that feels missing** — a
card, a hero, a summary, a gamification surface. Most "obvious improvements" to this app have
already been tried and deliberately removed; this is the record of which, and why.

Also lists what may not be touched, and the known defects to fix opportunistically.

`§` cross-references point at the core `.claude/DESIGN.md`, whose numbering is unchanged for §1 and §3–§13.

---

## Settled — do not reintroduce / do not touch

**The warm-up stepper** (removed 2026-08-23, same day it landed): the rebuild replaced a fixed
per-day checklist with a step-at-a-time flow under a progress rail, which turned a two-minute task
into a seven-tap marathon. **It does not come back.** The warmup is one screen and one button
(`design/MAP.md`). Specifically settled there, by the owner: do NOT re-add a second button beside
`Start lifting` (skip and start were the same action twice), do NOT re-add the time estimate, and do
NOT gate the button on the tick state. The ticks stay, as a place-keeper only. Also gone with the old
gate: its fixed per-day string list ("Jumping jacks — 20 reps"), which never knew what the session
trained, and the fixed 40/60/80% warmup suggester, which prescribed the same three sets for a heavy
triple and a set of fifteen.

**Removed on purpose**: boxed cards for passive content · **the Academy's card tiles, its lesson
sheet and its FOR YOU shelf** (2026-08-20: filled hairlined cards around 35 pieces of passive
content read as a wall whatever is inside them, which is the same finding as the first entry in this
list arrived at from the gallery end — pieces are plates with their captions under them now; the
`ModalBottomSheet` reader capped a lesson's height, could not carry a cover and counted a dismissal
as a completed read; the FOR YOU shelf became the page's one opening pointer, which is lifted out of
its own chapter so nothing is printed twice. Do not re-add a card, a scrim-over-photo caption, or a
second reader) · full-screen PR takeover (PR = confetti +
gold row) · accent-tinted "important" prose · session-summary extras (share card/tags/ghost/vs-last/
what's-next — the SESSION-summary card specifically; the Profile before/after share card `BeforeAfterCardRenderer` (GYMAP-55) is a separate, deliberate shareable artifact and stays) · cardio big-number hero · **the cardio week-stats overlay** (see the 2026-08-23 section below) · cardio kcal estimates (return only with real watch burn data) · new gamification surfaces (wait-listed) · Profile
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

## Cardio, rebuilt around one week and two lenses (2026-08-23)

**Removed and not to be re-added**: the **swipeable week-stats overlay** (`CardioWeekDetailSheet` +
`CardioWeekStatsPage`) — it was reached by tapping the hero, which made the page's richest content
depend on a gesture nothing announced, and it redrew the hero's OWN marks (Mon–Sun bars, goal meter,
days/minutes/distance figures) one screen away, which is §4.3 twice over. It also could only be
walked one week per swipe, so "how do my weeks compare" — the question it existed for — was the one
thing it could not answer. Week browsing is `CardioWeeksScreen`: **one bar per week, taller the more you
did, tap a bar to open it, arrows to page further back** (Antho, 2026-08-23 — a first pass drew the
weeks as a list of rows and it read as a table, not as a comparison). Do not put a week pager back on
the cardio tab: the hub is itself a `HorizontalPager`, so a nested week-swipe would eat the tab
gesture · the **LOAD chart on the overview's PROGRESS lens** (it was the weeks page's own mark drawn
a second time; `weeks →` is the way to it) · **BY ACTIVITY**, on both the WEEK lens and the week page
(Antho, 2026-08-23: at the counts a real week has, one or two types, it was a full-width bar
restating the minutes figure and the session row beside it) · the **`stats →`** link on a week's
session rows (the whole row already opens the session, so it was a second affordance for one tap) · the **hero as one page-wide tap target** (it is passive; `weeks →` is a
named action) · the **white `+` disc** on the sessions header (the primary action is a filled
`Log cardio` capsule above the fold) · the hero's **`TODAY · N STEPS` text line** (a data section
leads with its mark — the hourly `StepsByHourSection` carries today's steps, and the line was a
sentence standing in for a visual) · the **all-time "recent sessions" list** under a THIS WEEK hero
(two different scopes claiming one page; the list follows the hero's week, `view all →` goes to
History) · the session detail's **ten-row `label — value` stat table** and its hairlines (three
figures, one tag line, and the compare data drawn as ranked bars — the old page drew no mark at all,
so without a watch connected it was pure text) · **accent-coloured record distances** (they failed AA
on four of the five accents, §14; the accent moved to the bar, where colour carries meaning without
being read).

**Kept deliberately**: the "More" expander on the log form (progressive disclosure on a FORM is not
§4.2's banned "content behind a tap" — GYMAP-38/39 put those fields there on purpose) · the WHO
150-minute fallback meter · the watch-import section's dismiss-for-good.

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
CTA. MOVEMENT is listed as REMOVED from Home by the 2026-08-16 cut-down above and is rendering
again; that is a content call, not a craft one. The weekly volume line was the other half of this
note and is **closed now** — the 2026-08-16 removal was tested against its alternative and held.
See below.
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

---

## Swap picker, rebuilt as an arm-then-confirm list (2026-08-23)

`SwapPickerSheet` was rebuilt from scratch. It had been drawing a filled "Make default" plus an
outlined "Just today" capsule inside EVERY candidate row, and firing the write on the first touch.

The rebuild keeps the shape Antho liked — **every row carries its own `Today` / `Every week`
choice** — and fixes what was actually wrong with it. Selection is radio-style and OPENS on the lead
candidate's `Today`; the commit is accent-filled (`DECISIONS.md`, 2026-08-24). Both per-row controls are `SegmentPill`s, not
buttons, because arming a row SELECTS rather than acts; the commit is a single `ForgePrimaryCapsule`
at the END naming the move and the scope ("Swap to DB Fly every week"). So the *button wall* in
`FAILURES.md` is answered on its own terms (it names a FILLED capsule per row, and there is exactly
one filled capsule in the sheet, at the end per §8), the row is no longer a third tap target
wrapping the two inside it (§2③), and a mis-tap now costs one more tap instead of a committed write.

An intermediate build moved scope to ONE `SegmentPill` pair at the top of the sheet and made each row
a single whole-row tap. It was tighter, and it was wrong for this surface: the scope belongs to the
exercise you are choosing, not to the sheet, and reading a pill at the top to know what a tap 12 rows
down will do is worse than reading the two words under the row itself. Do not re-hoist scope to the
header.

**Removed, and why none of it is missing:**

- **Committing on first touch.** A swap mid-session is a write against the logged exercise; the
  picker now arms and waits. This is the one place §12's undo-over-confirm does not reach, because
  the act is not reversible from inside the sheet.
- **The hairlines between variants.** §1: a line exists only as data. Two `HorizontalDivider`s per
  row is the hairline habit, and the real fix was air plus the equipment glyph giving each row its
  own left edge.
- **The per-row `why` paragraph and `WHEN` line.** Twelve rationales stacked in a sheet opened
  one-handed mid-set, against §4.3's one-caption budget. The library is ordered best-first, so the
  LEAD entry keeps its `whenToUse` as the sheet's single caption, drawn inside the row it belongs
  to. `why` no longer renders anywhere; do not re-add a "why this?" disclosure, §4.2 bans prose
  behind a tap.
- **The standing two-sentence explainer** under the header. Replaced by one line naming what each
  scope costs, which is the only thing the two pill labels do not already say.
- **The `· CURRENT` badge.** §8 keeps state words out of a row's right meta — and it could never
  fire anyway: `DayScreen` filters the day's own effective names out of the candidate pool, so the
  active swap is never in this list. The move being replaced is named in the anchor instead, and the
  accent wash now means ARMED.
- **The serif "A different way in." headline.** §4.6 / `ModalRecipe`: a sheet does not repeat a
  title the row you tapped already said, and a modal has no serif hero in its toolkit.

**Deliberately absent — do not add:**

- **A search field.** §3 says a picker is search-first, and this one is the exception: the pool is
  already filtered to one muscle AND the user's own equipment (4 to 18 moves before dislikes and
  same-day exclusions), and the sheet opens one-handed between sets. A keyboard over six rows costs
  more than it finds.

**Added, because it was unreachable before:** `onClearPersistent` had been a parameter the sheet
accepted and never drew, so a persistent swap could not be undone from the picker that made it. It
is now the `Back to <plan exercise>` outlined capsule at the END (§8, level ② beside the confirm),
shown only when a persistent swap is active.

Along the way the 21 em dashes in the library copy this sheet actually renders (`muscleTarget` on
every row, `whenToUse` on the lead) were rewritten (§11). The remaining 53 sit in `why`, which no
longer reaches a screen.

---

## Goals, one ranked ladder with a clock (2026-08-23)

The Goals screen and the shared `GoalProgressLine` it lends to Home, Cardio and the Profile were
rebuilt together. Antho's brief was to tear the section apart; everything below came out of it, and
none of it is missing from the product.

**Removed from the Goals screen, and why each was noise rather than content:**

- **The "Lift targets" / "Other goals" split.** That is the shape of the two tables behind the
  screen, not of any question a person brings to it, and it cost the ladder its ranking twice over:
  once by grouping, and again because the screen never re-sorted the rows it filtered. Rows are now
  one list ordered closest-first. Do not re-group goals by which table they live in.
- **The thirty-word opening explainer** ("Custom goals track themselves from what you log"). Prose
  budget, §4.3 — and it narrated mechanics, which is cut rather than trimmed: the rows filling in by
  themselves ARE that sentence. It also carried the screen's one `fontSize =` call (§6).
- **`REACHED` in the row meta.** §2① rules a state word out of a row's right-hand slot, and putting
  it there meant a finished goal stopped showing its own numbers at the moment they were worth
  seeing. The word survives as the meter's caption — one line, under the mark it explains — and the
  Reached lens now says it once for a whole list instead of once per row (§12, collapse repetition).
- **The `+ add goal` line** (Antho, same day: *"I wanted this bigger, it's stuck in a corner, make it
  take more space, it looks bad"*). §11's "+ log" idiom is right where an action belongs to the list
  it closes and adds one more of the same thing. It is wrong here: adding a goal is the only thing a
  person can DO on this screen, and the idiom rendered it as the smallest, dimmest mark on the page.
  It is now a `ForgePrimaryCapsule` at its STANDARD trim size (~44dp, no `fillMaxWidth()`), closing
  the list on the same left rail as the rows. §8 level ① — the one filled do-it-now action, grouped
  at the END of the page. Do not put the mono line back.

  **Two overshoots got it here, and both are worth not repeating.** First a 56dp filled cube in the
  bottom corner: that makes the action *smaller* and pins it to an edge, which is the complaint
  restated rather than fixed, and a corner FAB has no home here anyway — §8 has three button levels
  and none of them float, and M3's `FloatingActionButton` would import its container colour,
  elevation and ripple, all three of which this app spends differently (§9 presses bounce). Then a
  full-width bar, which outweighed the ladder it was supposed to close (Antho: *"make add a goal
  smaller now, looks weird"*).

  The lesson is that the button was never the problem. It read as small because **everything around
  it was small**; once the rows grew (below), the standard trim capsule was already enough. Reach for
  the surrounding scale before reaching for the control.
- **Bare error-coloured text actions** ("Clear goal", "Delete goal"). §14 measures `#BF4040` as text
  at 3.67:1, which fails AA; §8's destructive treatment is the outlined capsule tinted `error`,
  which is what both are now. Never re-add error-coloured body text.

**Added, and each from data that already existed and was being thrown away:**

- **The meter caption.** A weekly target is the one goal shape whose reading means opposite things on
  different days — "3.2 of 5 km" is comfortable on Tuesday and lost on Sunday night — and the window
  was stored all along in `GoalPeriod` without any surface drawing it. A bodyweight goal likewise
  stored the weigh-in it is measured from (`stretch_value`) and showed a bar computed from a baseline
  it never named. Both now render as one mono line under the bar. Sparse by design: most rows have no
  caption and stay two lines tall.
- **Lens pills, Live / Reached.** Shown only when both sides have rows; a toggle with an always-empty
  half is a control that cannot run (§2③).
- **Row glyphs on the Goals screen.** They were deliberately off while the list was grouped by kind,
  because a column of identical marks says nothing. One mixed ladder inverts that premise: the marks
  now differ per row and are the fastest read on the screen.

**Not done, and deliberately.** The goals here carry no rate, no ETA and no on/off-track verdict.
`domain/coach/GoalPortfolio` computes all three, but for `CoachGoal` — a separate goal system with a
separate table. PRODUCT.md's "goals ... with a live reading and an ETA" describes that one, not this
screen. Wiring a trajectory into `ExerciseGoal` / `ExtendedGoal` is a real feature with a data gate
(`MIN_WEEKS_FOR_RATE` = 3), not a UI pass, and inventing one from two data points would be the kind
of verdict §4.9 exists to prevent. **The two parallel goal systems are the open question here**, and
worth resolving before either grows again.

**The whole section was scaled up one rung** (Antho, same day: *"not just the add a goal button,
everything"*). A ladder of six goals was leaving two thirds of the page empty while every element on
it sat at the smallest rung it had. Nothing left the type scale — each moved one step along it:
hero `headlineSmall` → `headlineMedium`, row title `bodyMedium` → `titleMedium`, the mono reading
`labelMedium` → `labelLarge`, the caption `labelSmall` → `labelMedium`. The meter went 6dp → 10dp,
which is also what lets the accent register on a reached goal instead of reading as a thread. Row
rhythm went 18 → 24. `CardMark` gained a `size`/`glyphSize` pair defaulted to the fixed 30/16 it
already drew — every other call site is unchanged and the goldens confirm it — and the goal row
passes 38/20, because the mark became the weakest thing on the row once the row grew around it.

This is a deliberate departure from §3's List archetype, which asks for "trim rows" and a "tiny
hero", and it applies to the SHARED line, so Home, Cardio and the Profile trims grew with it. That
is the point: the four surfaces render one component so goals read the same everywhere. If the
Goals screen and Home ever want different weights, split the component before re-tuning either.

**The bar's track moved from the outline 0.35 rung to 0.25.** §5 reserves 0.25 for data lines and a
meter track is one; 0.35 is the rung for borders on unselected controls, which had every empty goal
quietly looking like an interactive element it is not.

## Home's weekly volume, removed a second time — and this time tested (2026-08-24)

The 2026-08-16 cut-down removed VOLUME THIS WEEK from Home, and it went on rendering anyway as a
bare `424 kg` under the week strip; the open note above recorded that contradiction without
resolving it. Antho pointed at the line: *"redesign this that looks bad"*.

Three options were put to him — name it as the section's caption, cut it, or promote it to a real
readout. He took **promote**, so it was built and shipped to his device: `WeekReadout`, two
`EditorialFigure`s under the strip (`424 / KG LIFTED`, `4 / SESSIONS`), cardio appended only when
non-zero, honest zeros throughout, on an 88dp column floor so the figures did not collide into
"4244" at a squint. Verified at 100% and 200%.

He looked at it on the phone and cut it: *"remove kg and sessions ... it doesn't look as good as I
thought."* **The 2026-08-16 removal stands, and it now stands on evidence rather than on argument** —
the alternative was built, rendered on device, and rejected on sight.

**So the entry above is stronger than it was, not weaker.** The test it stated — *is something at
stake?* — is what a labelled, well-spaced, doctrine-clean readout still failed. Volume lifted and
sessions logged are both finished tallies; naming them properly makes them legible without making
them matter, and a section of legible things that do not matter is exactly the "bland with nothing
identifiably wrong in it" that the cut-down was written against. **Do not re-add the weekly volume
to Home a third time.** Neither as a bare line, nor as a caption, nor as figures — all three have
now been seen.

THIS WEEK ends on its mark: anchor, strip, and the MOVEMENT line when there are steps. The state
that fed the old line survives unused (`OverviewUiState.volumeThisWeekLb`, `.workoutsThisWeek`,
`.cardioMinutesThisWeek`) — same shape as the Academy cold-start chain above. Remove the plumbing or
give it a home; do not re-add the line.

**What DID survive from this pass, and was the real defect**: the anchor read `4 / 7 target` over a
strip with one cell lit, because it counted sessions while the strip drew days. See *Two units, one
section* in `design/FAILURES.md`. `OverviewUiState.weeklyWorkoutTarget` is `weeklyTrainingDays` now,
because the name was the bug.
