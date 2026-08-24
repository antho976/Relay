# Forge — screen map

Satellite of `.claude/DESIGN.md`. Read this when you need to know **what already exists**: which
screen owns a feature, what a section already renders, and why a thing sits where it does. You do
not need it to know *how* to build — that is the core doctrine.

`§` cross-references point at the core `.claude/DESIGN.md`, whose numbering is unchanged for §1 and §3–§13.

---

## App map

Hub = swipeable 5-tab pager + `ForgeBottomBar`: **Cardio · Stats · Home · Coach · Academy**
(Academy took Profile's slot 2026-07-27 — it had been a link buried inside Coach, and it is half
the coach, not a footnote to it. **Profile** moved to the Home top bar and remains a pushed route
with its own back arrow. **Settings** returned beside it on Home on 2026-08-22, replacing the extra
hop through Profile). Top bar everywhere = `←` (sub-screens) + ≤1 action, **never the screen's own
name** (no `TopAppBar` title); Home alone carries the bell + Profile + Settings;
**one back affordance per page — the top-bar `←` alone, never a second in-page back arrow**. The
notifications bell is **Home only** (2026-07-27): it sat in all ~20 top bars, which made an unread
badge follow you into every screen you had already navigated away from it to reach. A screen names
itself with a serif content hero (Stats "Stats", Profile "Athlete") or not at all (Home "Pull B").
On **Home** the outer bell and Settings glyph are pulled out by `GUTTER_SLACK` (12dp, half the gap between a
44dp target and its 20dp glyph) so their edges land on the 24dp page gutter, level with the serif
hero below; centred in their targets they sat 12dp inboard of everything else. Profile sits directly
beside Settings inside the right action group. This is Home's own
Row, not a `TopAppBar`, so no Material inset fights it — and with the bell now Home-only there is no
longer a second alignment to disagree with.
The bell replaced the `• Avex` wordmark on 2026-07-27; "Avex" now appears only in the cold-launch
`AvexIntro` beat and on exported artifacts (rank / before-after cards, the PDF footer).

### Notifications — `ui/notifications`

The one feed for everything that used to be a page-level banner, reached from any screen's bell.
List archetype **minus the search field** (it tops out at a handful of rows, all visible at once — a
search box that can never earn a tap). Tiny hero + `N WAITING` count, rows ranked live-first (§4.8):
in-progress session → coach brief → milestones → results (import / backup restored / a resolved
leftover session) → the two invites (turn on notifications, connect a watch). A row
that goes somewhere is bordered and bounces; a celebration or housekeeping note is bare and passive
(§2③). No per-row `×` — that would nest a tap inside the row's own target — so acting on a row
clears it, and the top bar's ONE action (a gear) opens `NotificationsOptionsSheet`, a bottom sheet
holding the two page-scoped actions: **Notification settings** and **Clear all notifications** (with
Undo, §12; both rows render passive when there's nothing to clear). The sheet is the modal archetype
— `containerColor = surface` — but takes NO divider between its rows: §5 permits a modal one and
every other sheet in the app took that permission, yet two rows don't need a line to be told apart.

Rows carry **no outline**. §1 earns a border with interactivity, but one bordered box per notice
stacks into a wall on a page whose whole job is to be scanned; the glyph chip IS the row's mark
(§12), and boxing a mark is not the same as boxing passive content. With the border gone the accent
` →` is the only thing separating a row that goes somewhere from one that just happened, so it is
load-bearing rather than decoration. The mono eyebrow went with the border — the glyph names the
kind, and says it aloud through its `contentDescription` (§4.3 one home, §14).

Every row leads with a glyph from **`NoticeIcons`** (`ui/notifications/NoticeIcons.kt`) — the fourth
matched custom family, same 24dp/one-weight house style as `NavIcons` and `SettingsIcons`, built on
the shared `VectorBuilders` plumbing. Eight glyphs: session, milestone, import, backup,
housekeeping, bell, watch, and the coach brief which reuses `NavIcons.Coach` outright so a coach row
speaks one glyph everywhere. Each carries its kind as its `contentDescription`, since the eyebrow
that used to say it aloud is gone (§14).

**Settings → Notifications** is split by where a thing ARRIVES, not by feature: `ON YOUR PHONE`
(training reminders · weekly recap · rest timer alerts · silence during quiet hours, with the
per-day windows) and `IN THE APP` (one `ToggleRow` per `NoticeKind` — unfinished workouts · coach
briefs · milestones · imports and backups · setup invites). Quiet hours sits INSIDE the phone group
because it suppresses exactly those three rows and nothing in the app group; trailing it after the
in-app switches read as though it silenced those too. The two headers are a parallel pair and carry
the split alone — **no group captions**, which is where the first attempt went wrong (§4.3). Stored
as the DISABLED set
(`DISABLED_NOTICE_KINDS`) so any kind added later is on by default, and applied as the LAST filter
on the feed — a notice stays queued while its kind is off, so switching it back on brings the row
back rather than having silently dropped it.

Both setup invites are dismissed FOR GOOD and neither has an un-dismiss control (the old cardio
banner's `×` was a one-way door too), so `SettingsSection.NOTIFICATIONS` now also clears
`DISABLED_NOTICE_KINDS`, `CARDIO_WEARABLE_HINT_DISMISSED` and `NOTIF_PERM_ASKED` — resetting the
section is the way back, and covers every switch on the page. `NotificationPrefsTest` pins that,
plus the round-trip of every write the Undo lambdas make.

`data/repo/NotificationFeed.kt` is the single `@Singleton` source, feeding both the page and every
bell's unread count through `LocalUnreadNotifications`. Most sources are already observable (active
session, prefs); the weekly coach pass and the Health Connect grants have no observable, so
`refresh()` re-polls them at app open and on resume. Milestones and one-shot results are now
PERSISTED (`UNREAD_MILESTONES`, `SYSTEM_NOTICES`) rather than held in memory — a feed that emptied
on process death would lose the thing it exists to hold.

Three `MainActivity` dialogs were folded in at the same time: the share-import result, the
backup-restored confirmation and the POST_NOTIFICATIONS rationale. The first two were pure "here's
what happened · OK" over whatever screen you were on; the third interrupted a cold launch to ask.
The permission row now opens the OS app-notification screen rather than re-requesting, so it keeps
working after any number of denials. **Still dialogs on purpose**: `ProgramChangeGuardHost` (a
destructive confirm — it must block), `CheckinSheet` and `DislikeSwapPromptDialog` (they ask for
input at the moment of relevance, not for attention), and Settings → Notifications'
`NotificationsBlockedBanner`, which is the denied-state of the controls directly beneath it (§12)
rather than a notice.

### Academy — `ui/academy`

The knowledge half of the coach, and a hub tab since 2026-07-27. **Rebuilt as one open gallery
2026-08-16** (`AcademyScreen` + `AcademyGallery`), replacing the gated hub → track-screen → lesson
structure. Antho: *"too crowded and behind 50 sub menus, and the worst thing is it feels like
achievement, not a hub to knowledge. You should be able to see everything."*

**Three causes, all removed.** (1) It was **87% locked** — 27 of 31 lessons gated on coach moments,
and a mostly-locked inventory can only read as an achievement tree. (2) It reported progress **twice
per track**, a `LessonDotRail` *and* an "n OF m", making score the loudest thing on a reading page.
(3) **Three levels** to reach a lesson.

**Rebuilt again 2026-08-20 — plates and chapters, and one reader.** The 08-16 gallery put every
piece in a filled, hairlined card from the Home experiment's `SurfaceKit`, with the title printed
over the picture under a scrim. Antho's three complaints: it still *"reads as blocks"*, there is
*"no sense of where to start"*, and *"the reading itself is plain"*.

**The blocks were the cards.** Thirty-five identical filled rectangles with a hairline round each is
a wall whatever is printed inside them — §1's central ban, arrived at from the other direction. A
piece is now a **plate and its caption, straight on the page**: the cover clipped to 16dp (§7),
greyscaled at render time so a colour asset cannot break the one-accent rule, and the words UNDER it
in the page's own type. Nothing is printed over a photograph, which fixes contrast by construction,
frees the art from being composed for its slot (one 3:4 master crops to every shape — see
`docs/ACADEMY_ART.md`), and turns a piece with no cover yet into an index line rather than a hole.
**Rhythm is a five-beat**: a full-width 3:2 lead, then two-up 3:4 posters until the next lead
(`isLeadSlot`). In a row where one piece has art and its neighbour does not, the unplated one holds
the plate's space open (empty air, never a filled placeholder) so both captions sit on one line and
the pair still reads left to right — without it, Fundamentals read 04 before 03.

**Where to start is answered three times.** (1) The page opens on ONE named piece (`startHere`): the
coach's poke if a moment fired, else the next unread Fundamentals lesson ("Start here" / "Continue"),
else anything unread, else "Read again". It is typographic — kicker, serif title, deck, `read →` —
and carries NO plate, because the piece it names also appears in its chapter with its own picture.
(2) It is then **lifted out of that chapter**, with the chapter's numerals computed BEFORE the lift,
so Fundamentals opens at 02 rather than renumbering itself. (3) Each chapter prints the blurb its
track was authored with (`LessonTrack.blurb`, written since B3 and never rendered until now), and
Fundamentals numbers its pieces 01-10 — the only track with an authored order, so the only one that
earns a numeral.

**Read is a tone, not a word.** An opened piece prints its title in `muted` instead of `onBg`, the
visited-link convention; the meta line dropped the word "read" with it (§4.3, one home).

**One reader** (`ReaderScreen`, shared by `LessonScreen` and `ArticleScreen`). The lesson
`ModalBottomSheet` is deleted: it capped a lesson at a sheet's height, could not carry a cover, and
recorded completion from a DISMISSAL, so a bounce counted as a read and corrupted the only signal
the ledger keeps. Both halves now record opened-on-resolve and completed-on-reaching-the-end, and
both open from `Routes.LESSON` / `Routes.ARTICLE`. The reader's cover bleeds full width under the
transparent top bar and **dissolves into the page** by masking its own alpha (`Plate(dissolve =
true)`), so it fades into whatever ground is actually behind it, AMOLED included. Prose is
`bodyLarge`; a `Heading` block is a real `EditorialHeader` (and therefore a TalkBack heading); a
`Callout` is a serif pull-quote instead of a `primaryContainer`-washed box (which was §1's ban
sitting in the middle of the reading page); an `Example`'s value is a serif figure (§2①). Every
piece ends with a named next piece where one honestly exists — "Next in Fundamentals" in the ordered
track, "More in Conditioning"/"More in Recovery" on a shelf, "Next chapter" at a track's end — and
reading on REPLACES the current piece on the back stack, so Back returns to the gallery rather than
walking the chain in reverse.

Now: masthead (`35 PIECES · 40 MIN` + serif name + aside) → the opening pointer → the five lesson
tracks as chapters → the Library's articles grouped by `ArticleTopic`. `AcademyTrackScreen`,
`Routes.academyTrack`, `academyLessonsPane`, `libraryPane`, `LessonDotRail`, `LessonRow`,
`LessonSheet`, `AcademyComponents.kt`, the separate FOR YOU shelf and the `academy?lesson=`
argument are all **deleted**.

**The gate became a poke, with zero domain change.** `LessonState.unlocked` already meant "a coach
moment fired for this reader" — a statement about RELEVANCE, not entitlement. The UI stopped
treating it as permission: every lesson is readable from install, and a fired moment now only marks
a piece FOR YOU (an accent dot, and first claim on the page's opening pointer). The ledger, `ArrivalController`, the notifications feed and the tab badge are
untouched and still count `isNew`. `LessonUnlock.label/detail` are no longer rendered anywhere; they
stay on the model as the authoring record `orphanLessons()` audits against.

**One page, labelled differently** (Antho's words) — the `AcademyLens` `SegmentPill` row is gone.
Lessons and articles share the gallery and are told apart by a word in each piece's meta line
(`ARTICLE · 6 MIN` on the minority; lessons say only their length), with reading time for both derived by the same
`List<LessonBlock>.readMinutes()` (lifted out of `Article` so two neighbouring tiles cannot state
their length by two different rules). The Library's search field and topic pills went with the
merge; tracks and topics are the browsing structure now.

**Still true:** no XP, no streaks, no percentage, no course index. `docs/ACADEMY_LESSONS.md`'s
"just-in-time, not curriculum-first" now holds more literally than before, since nothing is
sequenced at all — only Fundamentals is authored in a reading order, and the gallery preserves it.

**Library** — `domain/academy/Article.kt` + `ArticleRegistry`, listed in the gallery above and read
in `ArticleScreen`, ledgered in `article_event` (v36) via `LibraryRepository`. A deliberate sibling of
`Lesson`, not an extension: folding articles in would force `Lesson.unlock` to be a lie on every
row. They share the block renderer (`BlockBody`), so both halves read in one voice.

- **Topic groups, never difficulty.** "What is this about" is answerable before opening something;
  "how hard is it" is not, so difficulty never sorted or gated anything. Since the 2026-08-16 merge
  the topic pills are gone too — each topic is a gallery section, and **only topics holding an
  article appear** (§12: eight empty shelves against four articles would open the Library as a
  promise nothing keeps).
- **Read time is derived from word count**, never authored, so it cannot drift when a paragraph is
  edited. Ceiling ~30 minutes: past that it is a book, not a lesson.
- **Every article is sourced**, and `ArticleRegistryTest` fails the build on an empty source list —
  the only mechanical guard on the Library being research rather than opinion. Sources render as
  plain text at the end, never links: no INTERNET permission, so a tappable citation would be an
  affordance that cannot run (§2③).
- **`AcademyLink`** resolves an id to whichever half holds it. Articles are namespaced `library.*`,
  lessons are not, so the coach's existing nullable `lessonId` slots (`Recommendation`,
  `TodayDirective`, `CoachSignal`, `ConditioningPlanner`, `GoalPortfolio`) can point at an article
  with no schema change and no call-site churn. The test pins the two id spaces disjoint.
- **No XP, no streaks, no percentage.** The plan's ban on gamifying the Academy applies here at
  least as hard as to lessons: a library that scores you is a course wearing a disguise.

**The Academy never interrupts** (2026-08-15). A newly unlocked lesson is knowledge that became
relevant, not a decision waiting on you, so it goes to the feed as `NoticeKind.ACADEMY` and waits.
The row exists exactly while the lesson is unlocked-and-unopened, so opening one clears it with no
bookkeeping; only a DISMISSAL needs its own record, and it lives in prefs rather than the ledger
(writing a fake "opened" event to silence a row would corrupt the read history). `NotificationFeed`
now sorts by `NoticeKind` ordinal, so **declaration order in that enum IS feed rank** (§4.8) rather
than something emergent from builder order, and `refresh()` runs `syncCoachMoments()` first so a
lesson can unlock without a visit to the Academy tab.

Its arrival is announced by `ArrivalBannerHost` — a transient overlay that settles over whatever is
on screen, then flies into the bell and bumps its `CountBadge`. The queue lives in `ArrivalController`,
a plain singleton with no Compose types, so the ordering and de-dup rules are unit-tested without a
device. `announcedLessonNotices` (persisted, one-way) is separate from unread, or every app open
would re-announce everything still unread. Two doctrine rules were narrowed to allow this and both
carry a stated boundary in `design/SETTLED.md`, 2026-08-15 — read it before deleting either as a
violation.

### Home — `ui/overview`

TODAY hero + Start session, week strip, goals, recent. Feel reference; defects (`SETTLED.md`) fixed
when touched. THIS WEEK's stat row closes with a **MOVEMENT line** (W6): today's watch steps as one
mono reading + a thin today-vs-typical bar (14-day median; bar = the mark, honest at zero), rendered
only when the steps grant is held (GYMAP-64 rule: connected → honest zero, else hidden) and
refreshed on resume. Deliberately NOT the removed recovery-snapshot card (2026-07-04) — one line +
one bar, no card, and the hourly-bars mark stays cardio's (§4.3 one home; Home carries the total +
typical compare, a reading no other screen has).

### Stats — `ui/gym/stats`

one page: hero figures + muscle map → lens pills Strength/Volume/Effort/Days → drill rows → heatmap
→ records → Banister.

### Cardio — `ui/cardio` — **rebuilt 2026-08-23**

**Hero + one action + two lenses.** The page was an overview whose richest content sat behind a tap
on the hero (a full-screen week pager) that redrew the hero's own marks one screen away. The pager is
gone (`SETTLED.md`); week browsing is a ledger, and the page is:

1. **Hero** (`CardioComponents.CardioHero`) — `THIS WEEK · MMM D – MMM D` eyebrow with a `weeks →`
   action, the week's figures (days · minutes · distance, streak once ≥2), the Mon–Sun accent bars,
   and the minutes `MeterBar` (a personal target, else the **WHO 150-min/week** reference —
   GYMAP-42/`WHO_WEEKLY_ACTIVITY_MIN`). The hero is no longer ONE page-wide tap target; it carries
   two named ways out instead — `weeks →` to the weeks chart, and **the Mon–Sun strip, which opens
   THIS week's page** (`Routes.cardioWeeks(weekStartMs)`; Antho, 2026-08-23). The strip is a single
   tap target: seven day-wide ones would each fall under the 48dp minimum and all lead to the same
   place anyway (§2③). Everything else in the hero stays passive.
2. **`Log cardio`** — `ForgeHeroAction`, the same accent-filled button Home's Start session is
   (2026-08-23: a hub tab's primary action reads the same on every tab; Cardio's was the one white
   capsule in an accent app). Above the fold. Replaced a small white `+` disc that rode the sessions
   header and was none of §2③'s three levels.
3. **Lens pills** (`CardioLens`) — `Week` · `Progress`.
   - **WEEK**: FROM YOUR WATCH (W5 import suggestions, leads because it is the only section still
     asking for a decision) → SESSIONS (this week's rows; at zero, one line naming your last session
     and opening it) → `view all N →` (History) → STEPS (the hourly `StepsByHourSection` mark for
     today). No BY ACTIVITY split (Antho, 2026-08-23) — at the counts a real week has it was one
     full-width bar restating the minutes figure and the session row beside it.
   - **PROGRESS**: PACE (`CardioPaceTrendSection`, moved off the old overlay since it was always
     cross-week data) → RECORDS → GOALS. All three absent → one line naming the unlock, not three
     empty shells. Deliberately NO weekly-load chart: that is the weeks page's mark, and a visual
     that only repeats another screen's answer is cut, not copied (§4.3).

**THE WEEKS** (`CardioWeeksScreen` + `CardioWeeksViewModel`, route `Routes.CARDIO_WEEKS`) — the page
`weeks →` opens, and the one home for comparing weeks. Its route takes an optional `week` argument:
the cardio strip passes this week's Monday to land straight on that week, and backing out of a week
arrived at that way leaves the route entirely rather than stranding you on a chart you never asked
for (`CardioWeeksViewModel.arrivedOnWeek`). **One bar per week, taller the more you did**
(`CardioWeekBars`): `WEEKS_PER_PAGE` = 8 on screen, `←` / `→` paging a window back through history to
the first logged week (capped at 104), and **each bar is a tap target opening that week**. The whole
column is the target, not the drawn bar, so a quiet week is still reachable. The week in progress is a
dashed slot (a Monday must not read as a collapse), an untrained week keeps a ghost stub, and the
target/WHO reference is drawn across each track as a dashed rule — per column rather than as an
overlay, so nothing has to stay in sync with the rows above it. Bar labels are the day of the month
(eight "18 Aug"s do not fit the gutter); the nav row above carries the full range, and TalkBack gets
the full date. The tiny hero's two figures read the VISIBLE WINDOW, so paging says something.

The series is `cardioWeekSeries` (`domain/cardio/`): Mon–Sun weeks oldest→newest **with gaps kept at
zero**, because a chart that drops untrained weeks reads as an unbroken run. `cardioWeeksOnTarget`
and `cardioLoadDeltaPct` (a week against the median of the completed weeks, null under three of them)
ride alongside it.

A bar opens **`CardioWeekDetail`** (Detail archetype): serif week name, per-day bars, figures, the
meter (every week, not just the current one), BY ACTIVITY, and that week's sessions as
`SessionTimelineRow`s — no dividers between them.

**Session detail** (`CardioSessionDetailSheet`) — was ten `label — value` rows behind hairlines, which
drew no mark at all: with no watch connected the page was pure text. Now: eyebrow + serif activity
name → three figures (minutes · distance · pace, the pace figure absent rather than dashed when there
is no distance) → the descriptive tags (effort · HR zone · intervals · the one per-type field ·
weather) as ONE mono line, since §2② says a lone categorical state is a caption → **AGAINST YOUR
{ACTIVITY}**, the `CardioSessionCompare` data drawn as ranked bars (distance as a share of your
longest; pace inverted so lower-is-faster fills toward your best) with the previous outing as its
closing line → HEART RATE (W5, matched watch HR + the explicit `use watch stats →` adopt, never a
silent overwrite) → ROUTE → STEPS → note → Edit/Delete as two outlined capsules at the END, the
destructive one tinted `error`.

**Log form** (`CardioLogSheet`) — unchanged in shape (date + start-time capsules, activity dropdown,
duration/distance side by side, everything else behind "More", save actions at the end). Duration now
carries **quick-picks** (20 · 30 · 45 · 60) because §13 wants hot-path numbers on steppers, not a
keyboard; the field stays typeable and accepts H:MM (GYMAP-41). The number fields size to their
content rather than to a fixed 52/64dp box, which clipped at large font scales.

The activity picker lists the built-in `CardioType`s + the user's **custom activities** (GYMAP-37:
name + a glyph from the shared cardio set), with an inline "+ add custom activity" row →
`CustomActivityDialog` (a modal reused by the settings manager); custom defs live as a DataStore JSON
list (NOT the schema-locked DB — a logged session stores only the `custom_` code), resolved to
name/glyph at every cardio surface via `LocalCardioTypes` (a `CompositionLocal` fed once at the nav
root, like `LocalGoHome`); a deleted def falls back to "Other". Calories inherit "Other"'s baseline
(unknown code → `CardioType.OTHER`; kcal unsurfaced anyway, `SETTLED.md`). Managed in Settings →
**Cardio activities** (rename/glyph/delete). Per-type fields (GYMAP-38: incline on treadmill/
elliptical, laps on swim, elevation gain on run/walk/hike/cycle) are gated on
`CardioActivity.optionalFields`, so a value typed then switched away from is never saved; elevation
rides the distance-unit toggle via `ElevationFormatter`. **Conditions** (GYMAP-39) are the weather a
session was done in, a multi-select `PillChip` `FlowRow` in "More", stored comma-joined on
`cardio_entry.conditions` (DB v28) via `CardioCondition.encode`/`decode`, shown in session detail's
tag line and as a words column in the cardio CSV; descriptive only, never touching a total or pace. A
**new** entry seeds its activity to the **last-logged** one (GYMAP-40: `last_cardio_type` in
DataStore, written only on a new non-rest save), not always Run.

Cardio-local shared marks live in `components/CardioBars.kt`: `VerticalBarRow`/`BarGeom`/`BarGeomBox`
(the bar geometry behind every cardio chart), `MeterBar` (value vs target + mono caption) and
`RankedBarRow` (name · thin bar · reading). Kept in the feature package rather than promoted, matching how
`VerticalBarRow` was already shared across four cardio surfaces.

### Coach — `ui/coach` — **THE LEDGER** (2026-08-20 redesign)

**One running account, no lenses.** The Now/Signals/Journey pills are gone (`SETTLED.md`). The page
is one column read in this order, and every region is a file:

1. **The account** (`CoachAccount.kt`) — `THIS WEEK` with the open calls, then every week before it,
   newest first, capped at 6 with an "and N more weeks" line. Each decision is one **entry**: a node
   on the spine, a title (`subject · change`), a **stamp** (open / applied / skipped / undone /
   absorbed / its outcome word), a watch bar while its 14-day window runs, and Undo when it carries
   `undoData`. Closes on the week's three figures. A pre-baseline account gets `BaselineEntry`
   instead of a quiet line: the serif count, the meter, and what fills it.
2. **`WHERE YOU STAND`** (`CoachStand.kt`) — recovery load + only the checks that FIRED, lifts on
   watch with real trends, and the inputs with their charts and Connect pills.
3. **`AHEAD`** (`CoachAhead.kt`) — block phase rail, goals, the one project, the milestone rail.
4. **`WHAT IT HAS LEARNED`** (`CoachLearned.kt`) — the standing balance: autopilot trust per type,
   the biases, your numbers.

**The spine** (`Modifier.ledgerSpine`, `CoachUi.kt`) is the one line on the page and it is DATA: the
time axis, drawn at x=10dp inside the gutter so all four regions keep the one 24dp content column.
`EntryNode` carries the lifecycle (open = the larger accent node), the stamp carries the outcome —
never both saying the same thing.

**The one body on the page** is `CoachCallTile` (`CoachCall.kt`): an open call, and nothing else,
gets `surfaceVariant` at radius 16. Surface here is rank made visible. Inside it, `callCopy`
decomposes the stored imperative summary into subject + change so the CHANGE ("3 → 4 sets") can take
the serif rung (28sp under 18 chars, else 22) instead of arriving as one more line of body text; the
evidence draws at FULL width (88dp sparkline, or the fatigue meter + fired checks for a deload),
because the evidence for a call belongs to that call.

`humanizeMachineProse` (`CoachUi.kt`) is the ONE place em dashes and paren plurals are translated
out of stored planner prose — the allowlist counts that file's literals, so nothing else re-rolls
them. Deep links: `CoachEntryPoint`, with `accountItemCount` scrolling past the account (it mirrors
`coachAccount`'s emission — change one, change the other).

Coach content renders ONLY here — Settings→Coach is config alone (on/off switch + mode chips + a
feeds on/off glance whose silent HC rows tap to Wearable), never a second brief/trust/history home.

### Profile — `ui/profile`

blending cover (**untouchable** compositing approach — the offscreen `DstIn` edge-fade mask itself
stays frozen; its STOPS are not: the 2026-07-09 retune kept a second text-scrim that PEAKED at 0.85
black under the name then released, painting a band darker than the page and reading as the very
seam it was meant to hide, so 2026-07-24 (Antho) **deleted the scrim outright** and eased the mask's
bottom tail instead (62/72/82/91% ramp, not one linear slope — a straight alpha slope lands as a
visible edge). The dissolve can now only approach the page background, never overshoot it; name/meta
keep their own shadow haloes. A random default is seeded on first run so it's never empty, tap →
`AvatarPickerSheet`), bodyweight-led, **ALL-TIME** = ONE row of shared `EditorialFigure`s, COUNTS
only (workouts · sets · PRs + their week arrows) — lifted volume is deliberately NOT a figure here,
it is the **LIFETIME VOLUME** section's own serif reading sitting over its cumulative
session-by-session curve, so the number has one home (§4.3, 2026-07-24; the old 2×2 grid of 36sp
figures and its local `StatCell` copy of `EditorialFigure` are both retired) — then the merged
**BODY** section (2026-07-13, Antho — bodyweight, body fat and measurements folded from three
separate sections into ONE compact stack, `BodyMetricsSection` in `ProfileBody.kt`): one mono `BODY`
header over a row per metric built on exactly TWO RAILS and nothing between them (rebuilt
2026-07-24, Antho — the old row was a fixed 84dp label column, a figure floating loose in the
middle, and a bare accent `+ log` / `open →` text link stranded at the far edge): LEFT = the mono
metric name with its serif reading (`headlineMedium`, matching the ALL-TIME row) glued directly
beneath it, unit + ~30-day delta on the baseline; RIGHT = the trend mark, then the row's action as a
compact `ForgeRowPill` (Log · Log · Sync · Open). The WHOLE row is the tap target and the pill is
drawn only, so a stack of body metrics never becomes a column of accent text links (§8). **WEIGHT**
= serif figure + ~30-day delta off the 7-day average (a noisy final weigh-in can't flip the arrow) +
the smoothed spark keeping the dashed goal on-canvas + `+ log`. **BODY FAT** (GYMAP-62) = serif
figure + ~30-day delta in *points* (direction-only arrow, never a good/bad verdict) + a raw-reading
spark (logged sparsely, so no smoothing) + `+ log`. **MUSCLE** (W6) = the watch's BIA lean-mass
reading, import-only (`lean_mass` v30, `LeanMassRepository`, own `LeanMassViewModel` like
measurements): serif figure in the weight unit + ~30-day delta + raw-reading spark + a `sync →`
action (no manual log for a watch-authored metric); the row renders only when the HC read is granted
or data exists — an HC-only metric never shows an unconnected ghost row here. **SIZES** (GYMAP-52) =
ONE named circumference. Its label reads at the 15sp **anchor** rung, not the 11sp metric-name rung
its siblings use (2026-07-25, Antho: "SIZES is a separator like BODY"): "SIZES" is not the name of a
measurement the way WEIGHT and BODY FAT are — it is a GROUP, and the meta line under it names which
member is showing, so inside its row it does exactly what "BODY" does over the section. The site's
name therefore sits on the meta line at the metric-name rung, its value as the serif figure, its
unit and its total change since the FIRST reading, and its spark as the mark (2026-07-25, Antho).
Coverage pips and an `n OF 5` count were tried first and struck out on two counts: anonymous pips
cannot say WHICH site they stand for, and **coverage is not a reading** — you do not measure
yourself once, so `5 OF 5` saturates within a fortnight and then reports the same fact forever,
occupying the row where every sibling carries a live number. Featured site = WAIST by convention,
falling back to whichever site was logged most recently. Whole-row `Open` into `BODY_MEASUREMENTS`
(the full Measurements screen, unchanged, owns the values/trends/logging). Empty is DRAWN (§12) and
**the rows themselves ARE the zero-shape**: a metric with no readings still renders its name + its
action pill, so a brand-new profile gets an actionable section. The old all-three-empty collapse to
ONE `InlineEmptyHint` is GONE (2026-07-24, Antho — it replaced the whole redesigned section with a
single italic sentence, and a hint is the last resort only where there IS no zero-shape). A metric
with no readings draws **NO MARK AT ALL** — the slot keeps its width so the marks and pills still
share one right edge, but nothing is painted in it. Empty TRACKS were tried in between (2026-07-24)
and struck out a day later: a container with nothing in it is decoration, not an empty state, and on
device the whole section read as two flat grey dashes and a row of dots that said nothing. The
zero-shape is the ROW — a named metric with a real action pill — not a drawn stand-in for data that
doesn't exist. (2026-07-25, after a first-run profile rendered WEIGHT / BODY FAT / SIZES as label +
pill and nothing else: each mark independently and correctly chose to draw nothing, and with the
section wholly at zero every mark took that branch at once — locally sound, globally a settings
page. Any rule worded "only draw X when Y" needs a defined answer for "no Y", and that answer may
not be "nothing" for every mark in a section simultaneously.) Storage/sync unchanged and separate:
`bodyweight_entry`, the `body_fat` table (v29, sibling of bodyweight — NOT the cm-bounded
`body_measurement`, since a % is unitless — fed from a smart scale via Health Connect **or** manual
entry, mirrored both ways via a Wearable `Body fat sync` row, `BodyFatSync` mirrors
`BodyweightSync`), and `body_measurement` (v24, local-only, canonical cm); measurements keeps its
own `BodyMeasurementsViewModel`, read at the section level so ProfileViewModel is untouched. THIS
YEAR consistency grid (GYMAP-58 — whole calendar year, one ROW per month · day-of-month columns ·
dots lit by that day's training count across gym + cardio; a PASSIVE glance below the body cluster,
deliberately NOT the Stats week-column 26-week load heatmap (§4.3): different
range/layout/metric/interaction, so the two consistency views don't echo; hidden until the year has
any activity, `buildYearActivity` in `ProfileRepository`), filmstrip.

### Routed — `Routes.kt`

- `GYM_DAY` (`ui/gym/train`, **untouchable**)
- `SESSION_HISTORY` (gym+cardio)
- `SESSION_DETAIL` (one finished workout's breakdown; a page-end "Log again today" capsule (§8 ①)
  re-logs it verbatim as today's freestyle session — a full-fidelity data-layer copy incl. set
  type/RPE/holds, no editor, with an Undo — GYMAP-36. Lives here, NOT as a history-row button: a
  history row already owns its whole-surface tap for navigation, so a per-row action would be a banned
  nested tap (§8); an in-list long-press shortcut is a deliberate deferred follow-up)
- `CARDIO_SESSION`
- `GOALS`/`GOAL_EDITOR` (List archetype. ONE ranked ladder, closest-first, both goal kinds
  interleaved and each carrying its own glyph; `Live`/`Reached` lens pills instead of
  kind-sections; tiny hero = title + a `N LIVE · N REACHED` count. The row is the shared
  `GoalProgressLine` (`ui/goals/GoalsComponents.kt`) that Home, Cardio and the Profile also
  render — title + mono reading, meter, and ONE optional mono caption carrying whatever the
  bar cannot draw: days left in a period window, the weigh-in a cut is measured from, or
  `REACHED`. Adding is a trim filled capsule closing the list — §8 level ① at standard size; a corner
  cube and a full-width bar were both tried and reverted. Scaled one rung above the List archetype's trim default — see SETTLED before
  re-tightening. Rebuilt 2026-08-23, `design/SETTLED.md`)
- `TROPHIES` (frozen)
- `NUTRITION`
- `SETTINGS?page=`
- `RECAP`
- `NOTES_SEARCH`
- `PROGRAM_BUILDER?blank&view` (ONE program screen, GYMAP-28: colour-dot + mono `labelLarge` day
  anchor + "N SETS" meta, hang-indented name / sets×reps rows (meta at muted@0.7) — accent hexes via
  the shared `parseAccentHex`. Onboarding's week page shared this formula until 2026-08-23 and no
  longer does: it shows ONE day at a time under a selectable mark, so it has no need of a per-day
  colour dot and its rows lead with the `ExerciseIcons` equipment glyph instead; `view` = same layout read-only, the top-bar pencil unlocks editing; editor adds
  tap-into-day + long-press reorder + Save/Add at page end; day detail = rename/type/colour + exercise
  rows → SetsReps sheet (set stepper + rep-preset pills + in-place swap), duplicate/remove day at page
  end; removes undo via snackbar, never confirm)
- `FREESTYLE_LOG`
- `MIRROR_TEST` (the photo **Gallery**, `ui/profile/MirrorTest*` + `Gallery*`). **Rebuilt
  gallery-first 2026-08-22** (Antho: "a full revamp, it should be like a real gallery, with date,
  tags, compare, muscle tag"). The library leads; the instruments scroll away above it.
  - **One lazy list.** The whole screen is a single `LazyColumn`; `GalleryLibrary.kt`'s
    `LazyListScope.galleryLibrary()` emits masthead -> browse bar -> grid -> roll-end, and
    `galleryGrid()` emits one item per grid ROW under a `stickyHeader` per day. The old screen was a
    `Column(verticalScroll)` that composed and decoded every photo in the library on entry: fine at
    twenty shots, a felt stall at three hundred. The gallery is the app's one unbounded screen, so it
    is the one screen that pages. Day headers pin while their rows pass under them (a page-ground
    `Brush.verticalGradient` that fades at its foot, not a box, §1/§5).
  - **Four tag axes.** A photo carries its **date**, its **pose** (where the camera stood, one per
    photo, unchanged), its **muscle tags** (what the shot documents, MULTI, drawn from the program's
    own `MuscleGroup` codes, so a photo and a training week share one vocabulary) and its free
    **tags** (`domain/photo/PhotoTag.kt` normalizes "Week 12" / "#week 12" / "week-12" to one
    spelling, <=24 chars, <=8 per photo). Both new axes are `List<String>` on `ProgressPhoto`,
    written to the index JSON **only when non-empty**, so an untagged library's file is
    byte-identical to what the pre-revamp build wrote. Still off the DB, still no migration.
  - **Faceted, not modal.** `GalleryFilter` ANDs across axes and ORs within them; search composes
    with the facets instead of replacing them (it used to silently discard the window and lens you
    had set). `GalleryFilterBar` draws an always-open search field, the pose lens, then `Filters` +
    `Compare`; WHEN / MUSCLE / TAGS / VIEW rails live behind Filters, which carries the active-facet
    COUNT. With the panel shut, every active value stands above the grid as its own removable chip
    plus `Clear all`, so a filter set three scrolls ago can never quietly eat the library. Sort and
    density moved INTO the panel: four chips on the top row ran past the screen edge and made two
    presentation switches look as load-bearing as the filters beside them.
  - **One muscle is a LENS, not a filter.** With exactly one muscle chip active the progress band
    re-pairs inside that muscle and names it, so the mark at the top answers "how has my back
    changed", not only "how have I changed". Its cell badges suppress themselves at the same time,
    because a word repeated on every visible thumbnail is noise (§8).
  - **Metadata has one home per level.** The DAY header carries count, titles-or-poses, tags; the
    CELL carries muscle over date on the scrim it already draws. Muscles were briefly in both and
    produced headers reading "BACK, BACK" over a row of thumbnails already saying BACK (§4.3).
    **The per-cell muscle badge is a deliberate exception to the 2026-07-13 removal of per-cell POSE
    chips**: pose was on nearly every photo so the chip said the same word every time, whereas a
    muscle tag is optional, varies shot to shot, and is the thing you filter by. It wears no plate of
    its own (a second dark shape per thumbnail read as a sticker applied to the photo).
  - **Density follows the busiest DAY on screen**, not the library total: filter to one muscle and
    every day may hold a single shot, which three across is one speck beside two thirds of nothing
    (§12 debris). Never below two, because one across is a list.
  - **Compare** keeps its select-2 -> `CompareSheet` (Slider / Split + share card), and gains
    **long-press a cell to start a selection with that shot in it**, the phone-gallery idiom. The
    readout and the shared card now name the muscles the two shots AGREE on; a muscle tagged on one
    end alone says nothing about a change between them.
  - **The viewer edits every field the gallery filters on**: date, title, note, pose, muscles, tags,
    bodyweight, album. Collapsed it is one line stating what the shot is; `edit ->` opens the full
    form in its own scroll, so five chip rails at 200% font scale cannot push the photo off a photo
    viewer. Tag entry offers the library's existing tags first so the vocabulary converges instead of
    sprouting a third spelling of "cut"; a typed-but-unsubmitted tag flushes on swipe.
  - **Imports inherit the lens you are browsing under** (active pose + narrowed muscles), because
    that is what you were looking at when you decided to add more of it. Multi-select, as before.
  - **BODYWEIGHT and SAME WEIGHT close the roll** rather than sitting between you and the photos
    (§4.8, placement is rank). Bodyweight stays NOT photo-gated: at zero it is still the one live
    mark beside the band's ghost frames (§12), which the first gallery-first draft regressed.
  - Zero and one are unchanged and still whole: honest `0 PHOTOS` eyebrow, serif hero, ghost frames
    keeping their FIRST/NOW tags, one hint, one filled `Add a photo` capsule, the live bodyweight
    spark. `Albums ->` moved onto the eyebrow line as the library's index action; it was a stranded
    row under the band, and before that it hung off a TIMELINE header that no longer exists.
  - **Browse controls are NOT count-gated** (reversed same day, on device, Antho: "then it sucks, I
    want you to remove the have-photos-in-it limit"). They were: search at >=4 photos, the pose lens
    at >=2 distinct poses, the muscle and tag rails only once something carried them, on the §4.5
    reasoning that a control which can only return the same grid does nothing. On a photo library
    that produced a first-run screen with no search, no lens and no filters, which does not read as a
    restrained gallery, it reads as an unfinished one. A gallery states its own shape before it holds
    anything, the same way the band draws ghost frames rather than waiting for a photo to justify
    itself. **Pose and muscle rails now show their FULL fixed vocabularies**, not just values in use,
    so the rail doubles as the answer to "what can I even tag a shot with" — the question a new
    library actually raises. TAGS is the one user-invented vocabulary, so at zero it names where tags
    come from instead of drawing an empty row. `GALLERY_TOOLS_MIN` and `GalleryFacets` are gone.
    Only free tags can still go stale (untag the last `#cut` shot), so that is the one filter value
    the screen drops when its chip leaves the rail. An empty LIBRARY draws no "no results" line (the
    ghost frames and the capsule already say it); only a grid narrowed to nothing carries its own
    `Clear filters` chip. `Albums ->` rides the hero line at every count, so there is one door in one
    place rather than a conditional link inside the zero block.
  - The pass also PAID DOWN 13 frozen doctrine violations across these files (five off-ladder alphas,
    eleven inline `fontSize`s, an em dash and an exclamation mark in rendered copy); the one
    remaining allowlist entry is the search placeholder's `maxLines = 1`, which is chrome.
- `PROGRESS_CAMERA` (`ui/profile`, CameraX guided capture — live preview + a ~0.3-alpha ghost of
  your last same-pose shot for alignment, pose chips, rule-of-thirds grid, 3s self-timer, front/rear
  flip; writes straight to app-private storage; CAMERA permission is optional — deny falls back to
  import; no INTERNET, never the camera roll)
- `BODY_MEASUREMENTS` (`ui/profile`, GYMAP-52: the body-measurement tracker reached from the Profile
  BODY section's SIZES row. Reworked 2026-07-25 — mono `3 OF 5 TRACKED` eyebrow + serif
  "Measurements" hero whose MARK is a full-width **five-segment coverage rail** (accent where that
  site has a reading, empty track where it does not), captioned by the names of the sites still
  missing (`NOT TRACKED · THIGHS · HIPS`; at zero the names alone, at 5-of-5 no caption). Then a
  `TRACKED` anchor over one row per **tracked** site ONLY, on the same two rails as the Profile BODY
  row: LEFT = mono site name + open serif value + unit + total change **since the first reading,
  naming it** (`↓ 1.5 SINCE MAR 3` — a circumference is logged sparsely, so a bare "since last" is a
  delta over an unknown gap; direction-only arrow, never a verdict); RIGHT = the 140dp trend lane
  stamped underneath with the last reading's DATE, so a value logged in March never reads as current
  beside one from last week. One reading draws `SingleReadingMark` — the empty track carrying its
  single accent dot — because `ProfileSparkline` needs two points and the row otherwise went blank
  the moment you first used it. **Untracked sites get no rows at all**: they are the rail's hollow
  segments and its caption (§12 — N rows sharing one empty status collapse to ONE mark; the old
  screen drew four identical grey dashes for the untracked while the one LIVE row had no mark). At
  zero: hollow rail + the five names + one filled `Log measurements` capsule, no hint.
  `statsEntrance` cascade; top-bar `+` opens the same five-field log sheet (mirrors
  `BodyweightLogSheet`). Stored canonically in cm off a per-type Room table (`body_measurement`, one
  row per type per day), displayed cm/in via the independent `use_cm` Format toggle; local-only,
  `BodyMeasurementRepository`).

### Lock — `security`

+ `ui/security` (GYMAP-69) — an optional biometric / device-credential lock; no app PIN is stored
(`BiometricPrompt` with `BIOMETRIC_STRONG or DEVICE_CREDENTIAL`, failing OPEN when the phone has no
screen lock). Two independent Settings → **Security** toggles: **App lock** (an opaque
`AppLockScreen` gate over the nav host at cold start / after a configurable background timeout —
Immediately·1·5 min — wired in `MainActivity` through the `AppLockManager` singleton +
`LocalAppLock`, honouring the existing `userLeaving`/`onUserLeaveHint` guard so a
picker/camera/share return never re-locks) and **Photo gallery lock** (gates `MIRROR_TEST` in
`ForgeNavHost`). Both share ONE authenticated session — unlocking the app opens the gallery for
free, no second prompt — and app lock forces `FLAG_SECURE` app-wide (like Privacy mode, so it also
hides recents/screenshots). The unlock screen is the modal archetype: opaque theme-gradient scrim +
`• Avex` wordmark + serif "Locked" + one caption + one filled `Unlock` capsule (the OS sheet does
the credential entry). Offered as one onboarding opt-in step (shared "about you" block). Settings →
Appearance keeps the separate **Privacy mode** FLAG_SECURE toggle.

### Exercise likes — `ui/settings/SettingsSubPages.kt`

The preference list opens on **All exercises**: every public library movement plus the user's custom
moves, regardless of configured equipment. Its first scope selector adds **Your gear**, which alone
uses `ExerciseLibrary.availablePool` (including a frozen preset); muscle and Custom scopes remain
available beside it. Custom moves stay out of Your gear because they store no equipment metadata.

### Sheets

- SessionSummarySheet (minimal)
- CardioSessionDetailSheet
- heatmap "That day"
- ExerciseLibraryPicker (`singleSelect` = radio-style choose-one for swaps — accent-wash pick, no
  checkbox)
- SwapPickerSheet (**rebuilt 2026-08-23**) — the live session's swap picker, a List inside a Modal.
  Arm-then-confirm: every row carries its own `Today` / `Every week` `SegmentPill` pair (selection,
  not action), the armed row takes the accent tile wash, and ONE `ForgePrimaryCapsule` at the END
  commits, naming the move and the scope. Rows are the `ExerciseIcons` equipment glyph + name +
  `muscleTarget`; a timed hold flags `HOLD`; the lead entry (the library is ordered best-first)
  carries the sheet's one caption. `hasPersistentSwap` draws the `Back to <plan exercise>` outlined
  capsule beside the confirm, the only route out of a persistent swap. Body is `SwapPickerContent`
  (`initialArmed` is its preview/test seam), pinned by `SwapPickerScreenshotTest` at 100% / 200%
- the program SetsReps sheet
- AvatarPickerSheet (profile cover — "select your own" + provided default covers by category,
  `DefaultAvatars`; picked default is baked into `avatar.jpg`)

Check this map + `ui/common/` before inventing; update it when screens change.

---

## Detail relocated from the core doctrine

Verbatim from the pre-split `DESIGN.md`. These are *inventory* — how a shipped feature works — rather
than rules, so they moved here to keep the always-loaded core small. Nothing was reworded.

### Onboarding — the flow (rebuilt 2026-08-22)

`ui/onboarding/` — `OnboardingScreen.kt` holds the state and the page order;
`OnboardingScaffold.kt` the shell every step shares (chrome, question slot, ledger slot, one action); `OnboardingPrimitives.kt` the
shared tile / chip / rail / toggle-row formulas; `OnboardingSteps.kt` the fork and the goal /
experience / day-count questions; `OnboardingGymSteps.kt` the two gym steps plus the week page;
`OnboardingSoreSpots.kt` the sore / injured spots page; `OnboardingExtras.kt` the optional closing
step; `OnboardingWeekMeter.kt` the `PlanLedger` mark;
`OnboardingDraft.kt` the resume snapshot (schema 4, cursor = an index into the path, not a page id).

Path: **generated** = mode → goal → experience → days → gym → gear → sore spots → week → extras
(9 steps);
**custom / freestyle** = mode → goal → experience → extras (4) — they still answer goal and
experience because those steer the coach and Stats. `pathFor(planMode)` owns this; the rail counts
its cells from the list, so the short path visibly drops four.

`PlanLedger` is the persistent mark: one bar per training day carrying that day's SETS, scaled to
the heaviest day, sitting below the question and outside the page slider. Before any gear exists it
draws `ProgramGenerator.plannedSetsPerDay` (the split's own volume allocation, computable from a
day-count alone); once a gym is picked it draws the generated week, so a sparse setup visibly costs
sets. Its track is 148dp on the day-count step, where the week IS the answer, and animates down to
72dp on the gym / gear steps, which need their grids. Bar columns share the width evenly until a day
name stops fitting under one, past which the mono label clamps at two lines (§14) — an earlier build
scrolled the row instead and clipped the seventh day on any phone narrower than the dev device. The
week page shows the same mark at 104dp, where it also NAVIGATES (2026-08-23): pass `selectedIndex` /
`onSelect` and each bar becomes its own day's tap target, accent for the day being read and
muted@0.7 for the rest, with that one day's movements listed underneath. Without those two arguments
nothing is tappable and the mark draws exactly as it does under the questions — one implementation,
so the two cannot drift into reading as two different weeks. Bar tracks align at their TOPS, not
their bottoms, so a day name wrapping to two lines cannot float its own bar above its neighbours.

**The week page** (`StepWeek`, rebuilt 2026-08-23) is that selectable mark plus the open day: mono
day anchor with `N MOVES · N SETS` as right meta, then one row per movement — equipment-class glyph,
name, `sets × reps`. Both moves it can make crossfade: switching day, and a re-roll dealing fresh
movements into the day already open (the pick survives a re-roll, since the split is unchanged). It
replaced a page that redrew the same bars it had shown for three screens and then listed every
exercise of every day beneath them — ~25 uniform rows over three viewports, which §3 bans for this
archetype, restating each day's volume three times over. A one-day week gets no tap affordance, no
line telling the user to use one, and drops the day's set count (at one day it is the week total).

The sore-spot page counts, per joint, how many movements in the pool THIS gym supports load it
(`ExerciseLibrary.contraindicationsOf` over `availablePool`) — the reason to flag one, and stable
against a re-roll. The closing step reads down a label-left spine (`ValueRow`) wherever a control
fits beside its name; `OnboardingIcons` draws every line at `LIMB` = 1.8, the same weight as
`NavIcons` and `SettingsIcons` (fills are masses and may be thicker); half the family had been
rendering as solid silhouettes and half as wireframe, and a first correction to 2.2 fixed the
internal consistency by breaking the match with the rest of the app.

### Onboarding — plan-mode vignettes, signal probes, equipment steps

all three plan-mode cards carry short pre-rendered vignette videos (alpha WebP authored in
`remotion-vignettes/`, rendered to res/raw) that play twice then FREEZE on the built plan / caught
log, and REPLAY when you tap that card — the illustration is the answer to the question, so choosing
an option plays the answer back instead of leaving a frozen frame (`replays` counter, per card, so
picking one doesn't restart the two you didn't pick). All three are written in one vocabulary — a mono
UPPERCASE label, then that row's accent blocks, on the warm page — and differ only in what the TEXT
says, how the rows are ARRANGED and the RHYTHM they land in, which is the step's whole argument:
generated is `MON PUSH ▪▪▪▪▪` × 3, aligned into a table (a dated week, handed over) that snaps in
almost at once and then tallies its exercises straight across the week without pausing at row breaks;
custom is `BENCH ▪▪▪` — the exercises themselves, the level you work at when you build your own —
landed one per second by a `+ ADD` that is still blinking on the next open line when it freezes,
because a plan that is yours isn't done until you say so; freestyle drops alignment entirely and
lands day-stamped rows wherever, out of order and at uneven intervals, so no two share a row or a
left edge. Redesigned 2026-08-23: the set before it was a dense grid of unlabelled blocks (unreadable
at 72dp) and before that a wall of 8dp exercise names still keyed to the pre-2026-08-16 cool palette
and the Navy accent, which made every accent mark on the step a dead pixel. A shared `PlanModeSync` starts
the videos together so they loop and freeze in lockstep; the live Canvas vignette
(`PlanModeVignettes`) is the decode / pre-28 / reduce-motion fallback — reduce-motion being the one
that matters, since those users never see the video at all — and is a deliberate number-for-number
transcription of the compositions, text included, honouring the same two-loops-then-freeze and replay
contract; it sizes its type off the canvas width rather than the type scale so the drawing keeps the
videos' proportions and cannot overflow 72dp at 200% font scale (sanctioned in
`design-allowlist.txt`). Change one side and you must change the other; equipment/preset/goal tiles use the `OnboardingIcons` matched glyph family.
The wearable pick (Galaxy · Pixel · no watch, keys + labels + source app from
`domain/health/WearableBrand`) left onboarding entirely on 2026-08-23 (`design/SETTLED.md`); the enum
drives Settings → **Wearable**'s equal-width device row + brand-aware source-app/routes explainers;
Galaxy, Pixel, and no watch/other stay on one line, with no watch directly beside Pixel. The two
surfaces may not drift, and the brand is advisory only (HC reads stay vendor-neutral). Each granted
Wearable read-signal row carries a post-connect reading (§9, `probeSignalFlow`): `RECEIVING`
(onBg) when a record arrived in the last 30d, `NOTHING YET` (muted — quiet nudge, never alarm, since
absence is ambiguous) when granted-but-silent, plain `ON` while probing or for the write-only
calorie row. HC exposes data PRESENCE, never capability — so the UI never says "unsupported", and
grey-out-by-brand is banned (both watches can do every signal on recent versions). Gym setup = TWO
steps (GYMAP-20): preset grid (big-app lineup, `equipmentPresets`), then a fine-tune page grouped by
the shared `equipmentGroups` (Settings → Program →
Equipment groups its chips the same way — the two selectors may not drift.

### Launch intro — per-family mechanics

**The intro themes to the chosen app icon through the WORDMARK itself** (`AvexWordmark`): the name
has a narrative arc in the icon's palette (`AppIcon.launchPalette`, deep→mid→bright) inside the
stock envelope — it ENTERS with the family's verb (Metal sheen-sweep · Gem glint + jagged crystal
chunks GROWING out of the letterforms, stroke-scale, staggered (probe shader, 33+) · Aurora northern
lights RISING out of the glyph tops, waving + hue-shifting (probe shader, 33+; pre-33 drifting fill)
· Nebula weightless float · Molten white-hot heat-shimmer (AGSL `RenderEffect`, 33+) · Solid
plate-colour wipe · Stealth slow HUD flicker-in (~14Hz over ~1s) · Default plain), holds legible,
then DIES the family's death overlapping the plate fade (Molten MELTS decelerating over 650ms,
smooth slump + thin drip streams · Nebula is dragged into a BLACK HOLE vortex, modest twist (33+;
pre-33 spin-shrink) · Solid wipes back out · Metal/Gem/Aurora/Stealth fade). Reduce-motion = settled
still, plain fade.

**The theming is user-optional** (Appearance → **Custom startup animation**, `themedLaunchIntro`,
default on): off resolves the intro to `AppIcon.Default` so the plain black-and-white Avex settle
plays with no family effect/exit — MainActivity reads the flag in the same first-frame prefs pass as
the icon key so there's no themed flash. Full-screen launch SCENES (`ui/common/LaunchScenes.kt`:
family→AGSL registry, per-pixel `RuntimeShader`, Aurora/Nebula/Molten/Gem device-approved + Stealth
radar) are deliberately UNWIRED (2026-07-10 — every-launch spectacle wears out) but kept intact;
re-wire via `IconLaunchScene` behind the wordmark + the 950ms themed hold. This deliberately spends
colour off the one-accent rule (§1/§5) for a pre-app moment only. The launcher/adaptive icon
**defaults** to the emblem (`ic_launcher_foreground`) but is user-selectable (Appearance → App icon,
GYMAP-icons): one `.icon.*` `activity-alias` per icon, exactly one enabled at a time via
`AppIconManager` (`PackageManager` component toggle). A settings row shows the current icon + name
and opens a `ModalBottomSheet` grid (mirrors `AvatarPickerSheet`: family `EditorialHeader`s, ring
the current pick, scroll edge-fade); warns it "updates after a moment" (OEM-launcher lag). Family +
tile order = the `AppIcon` enum declaration order (the picker derives headers from
`entries…distinct()`), kept in the design-reference sequence Default → Solid → Metal → Stealth →
Molten → Nebula → Aurora → Gem; persistence is by enum `name`, so reordering never migrates a pick.

### Weight units — stones edge cases

editable single fields hold decimal stones and bodyweight logs stone+lb
as a two-field pair; the plate calculator has no stone denomination so it falls back to lb.

### SnackbarController — styling

Styled dark: `surface` plate, onBg text, accent Undo.

### Live archetype — timed holds

**Timed holds** (GYMAP-51: plank/dead-hang/wall-sit/side-plank, flagged `ExerciseDef.timed`) log a
held DURATION not reps — the set-log row's REPS column becomes HOLD and the input swaps in a `m:ss`
readout + a wall-clock-anchored start/stop stopwatch (live session, `SetInputRow`) or a manual
`m:ss` field (freestyle); a timed set stores `logged_set.duration_seconds`, carries reps 0, and is
excluded from every weight×reps stat (volume/e1RM/PR) — history/detail read it as "0:45" / "Best
0:45".

### Launch intro — splash + reduce-motion

The system splash is background-only (`splash_blank`, no icon) so the wordmark is the brand beat;
honors reduce-motion (short still hold, no fade).
