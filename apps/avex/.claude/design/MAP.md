# Forge — screen map

Satellite of `.claude/DESIGN.md`. Read this when you need to know **what already exists**: which
screen owns a feature, what a section already renders, and why a thing sits where it does. You do
not need it to know *how* to build — that is the core doctrine.

`§` cross-references point at the core `.claude/DESIGN.md`, whose numbering is unchanged for §1 and §3–§13.

---

## App map

Hub = swipeable 5-tab pager + `ForgeBottomBar`: **Cardio · Stats · Home · Coach · Profile**. Top bar
everywhere = `←` (sub-screens) + `• Avex` wordmark (`ForgeWordmark`, taps→Home) + ≤1 action,
**never the screen's own name** (no `TopAppBar` title); **one back affordance per page — the top-bar
`←` alone, never a second in-page back arrow**. A screen names itself with a serif content
hero (Stats "Stats", Profile "Athlete") or not at all (Home "Pull B").

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

### Cardio — `ui/cardio`

THIS WEEK figures hero (days · min · dist · streak) + a quiet `TODAY · N steps` line under the
figures when a watch is connected (GYMAP-64: today's Health Connect step total, loaded eagerly into
state on init/resume, honest zero when connected, hidden otherwise) + Mon–Sun accent bars + goal
meter (falling back to the **WHO 150-min/week** reference meter when no personal minutes target is
set — GYMAP-42, the same `MinutesMeter` bar with a `WHO 150 MIN` caption instead of `GOAL`, on the
hero and the week overlay's current week; `WHO_WEEKLY_ACTIVITY_MIN`), GOALS trim (cardio-metric
custom goals, shared `GoalProgressLine`, hidden at zero), **RECORDS** block (GYMAP-34: per-activity
all-time bests — the longest distance as the accent headline + the fastest pace as meta, one row per
type the user has distance sessions for, most-logged first, a row → its longest session;
`cardioActivityRecords`, hidden entirely until a distance session lands since the hero carries the
zero state), week-pager stats overlay (its current-week page alone carries a **PACE TREND** chart —
GYMAP-35: a per-activity pace-over-time `LineChart` + a type `SegmentPill` selector, only types with
≥2 paced sessions; pace is lower-is-faster so a downward line is improvement, said plainly in a dry
caption; `cardioPaceSeries`, cross-week so it rides the current page alone and never repeats per
week, §4.3), a **FROM YOUR WATCH** section above the recent rows (W5: watch-recorded HC sessions
with no matching entry, ≤3 rows, whole-row tap = import via a prefilled log sheet, header `hide`
dismisses the batch for good; hidden when empty), recent rows (header carries a small filled `+`
circle = log) → session detail (stat rows carry best-pace/longest compare meta + previous-session
read; a **HEART RATE** section (W5) draws the matched watch session's HR line — open chart, avg/max
as the header reading — plus a "Watch measured …" line with an explicit `use watch stats →` adopt,
never a silent overwrite). The activity picker lists the built-in `CardioType`s + the user's
**custom activities** (GYMAP-37: name + a glyph from the shared cardio set), with an inline "+ add
custom activity" row → `CustomActivityDialog` (a modal reused by the settings manager); custom defs
live as a DataStore JSON list (NOT the schema-locked DB — a logged session stores only the `custom_`
code), resolved to name/glyph at every cardio surface via `LocalCardioTypes` (a `CompositionLocal`
fed once at the nav root, like `LocalGoHome`); a deleted def falls back to "Other". Calories inherit
"Other"'s baseline (unknown code → `CardioType.OTHER`; kcal unsurfaced anyway, `SETTLED.md`).
Managed in Settings → **Cardio activities** (rename/glyph/delete). The log sheet keeps the common
case short — a date + **start-time** capsule pair at the head (GYMAP-33: the time capsule sets the
time-of-day of the entry's existing `date` timestamp via a Material3 `TimePicker` — no separate
start-time column, `combineTime` mirrors `combineDay` — and hides on rest days), then activity +
duration/distance up top, everything else (effort · HR zone · HIIT intervals · **conditions** · the
**per-type** fields) tucked behind a "More" expander; per-type = one field surfaced only for the
activities it fits (GYMAP-38: incline % on treadmill/elliptical, laps on swim, elevation gain on
run/walk/hike/cycle — the same idea as intervals showing for HIIT alone), gated on
`CardioActivity.optionalFields` so the form never carries an irrelevant field and a value typed then
switched away from is never saved; elevation rides the distance-unit toggle (ft with miles, m with
km) via `ElevationFormatter`. **Conditions** (GYMAP-39): the weather a session was done in
(hot/cold/rain/wind), a multi-select `PillChip` `FlowRow` in "More" (any non-rest activity, never
gated by type), stored comma-joined on `cardio_entry.conditions` (DB v28) via
`CardioCondition.encode`/`decode`, shown read-only as a ` · `-joined `StatRow` in session detail + a
words column in the cardio CSV; descriptive only, never touches a total/pace. A **new** entry seeds
its activity to the **last-logged** one (GYMAP-40: `last_cardio_type` in DataStore, written only on
a new non-rest save), not always Run.

### Coach — `ui/coach`

lens pills Now/Signals/Journey (Now = call + watch + one road-ahead section: milestone rail +
brief/verdict/autopilot bars; Signals = lifts + recovery + inputs + learned; Journey = record +
trust; old Brief/Lab/Timeline routes = lens deep-links). Coach content renders ONLY here —
Settings→Coach is config alone (on/off switch + mode chips + a feeds on/off glance whose silent HC
rows tap to Recovery), never a second brief/trust/history home.

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
entry, mirrored both ways via a Recovery `Body fat sync` row, `BodyFatSync` mirrors
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
- `GOALS`/`GOAL_EDITOR`
- `TROPHIES` (frozen)
- `NUTRITION`
- `SETTINGS?page=`
- `RECAP`
- `NOTES_SEARCH`
- `PROGRAM_BUILDER?blank&view` (ONE program screen, GYMAP-28: colour-dot + mono `labelLarge` day
  anchor + "N SETS" meta, hang-indented name / sets×reps rows (meta at muted@0.7) — the onboarding
  week preview renders this SAME section formula (GYMAP-21), accent hexes via the shared
  `parseAccentHex`; `view` = same layout read-only, the top-bar pencil unlocks editing; editor adds
  tap-into-day + long-press reorder + Save/Add at page end; day detail = rename/type/colour + exercise
  rows → SetsReps sheet (set stepper + rep-preset pills + in-place swap), duplicate/remove day at page
  end; removes undo via snackbar, never confirm)
- `FREESTYLE_LOG`
- `MIRROR_TEST` (the photo **Gallery**, revamped GYMAP-gallery; visual pass 2026-07-13:
  `statsEntrance` cascade + sparkline draw-in, real `EditorialHeader` anchors (BODYWEIGHT · SAME
  WEIGHT, DIFFERENT BODY · TIMELINE w/ `Albums →` as the header action), stock-Material content
  icons and per-cell pose chips removed: overview-first — serif "Gallery" hero + mono count/span
  eyebrow, a first↔latest **progress band** (corner-16 frames; center = serif span figure +
  direction-only weight-Δ + `compare →`; prefers a same-pose pair, tap → slider compare) as the §12
  mark at zero (ghost frames + add prompt), a bodyweight-through-time sparkline (only ≥2 weigh-ins),
  an auto-paired **same weight, different body** strip (GYMAP-60: same-pose shots within ~2lb of
  each other ≥30d apart, longest hold first, tap → compare; hidden when none, excludes the band pair
  so it never echoes it), pose lens pills (Front/Back/Side/Legs/Arms, only those present) +
  search/filters/compare **text pills** (one `GalleryChip` vocabulary with the range/sort/density
  chips) over the month-grouped grid (cells corner-12, date-only — the pose lens carries grouping);
  compare = select-2 → `CompareSheet` with a draggable **Slider** ⇄ **Split** toggle +
  time/weight/pose readout (a top-bar **Share** renders a 4:5 before/after card via
  `BeforeAfterCardRenderer` — GYMAP-55: the two shots + span + pose + **delta-only** weight (never
  the absolute bodyweight) to `ACTION_SEND`, a sibling of `RankCardRenderer` in the same
  Pearl-gradient/serif-hero/tinted-wordmark card language; band/same-weight/manual-compare all
  funnel through this sheet so every before/after path shares for free; adding it retired the
  Gallery's "never leave your phone" reassurance copy); the full-screen pager viewer doubles as a
  **metadata editor** (tap-date → DatePicker · pose chips · bodyweight · note · album · delete);
  albums behind "Albums →"; add via a chooser sheet → import OR the guided camera; photos carry EXIF
  capture date + a bodyweight snapshot nearest that date and are stored app-private off the DB —
  `ProgressPhotoRepository` with a reactive `revision` so teaser/gallery/camera stay in sync. **A
  photo LIBRARY first, a compare tool second (2026-07-25, Antho: "it was supposed to be a gallery of
  photos like phones, with metadata, classed per day, in order, with filters and a search bar").**
  The grid groups by **DAY**, not month — you shoot a set of angles in one sitting, so the day is
  the unit, and a month header hid the very thing that makes the library useful. Each day header
  names itself the way you'd say it (`TODAY` · `YESTERDAY` · `THU 16 JUL`, plus the year outside
  this one) over a meta line carrying the day's own reading: the shot count once there's more than
  one, then the titles those shots carry or their poses when untitled. **Photos carry a `title`**
  (short label, ≤60 chars, edited in the viewer above the note, stored in the photo index JSON —
  photos are deliberately off the schema, so no migration). **Search matches what the photo IS**:
  title · note · **pose** · album · date in several spellings. Pose and title were both missing
  until this pass, so the two most natural queries in a physique gallery — "arms", or whatever you
  named the shot — returned nothing. Pose matches on the enum LABEL, not the stored key. The field
  stands **always open**, never behind a chip: a gallery that makes you find its search doesn't read
  as a gallery, and its placeholder names the dimensions it matches so they need no caption.
  **Import is MULTI-select** (`PickMultipleVisualMedia`) — several photos per day was always allowed
  by storage (UUID filenames, no dedup anywhere) and was blocked purely by the picker asking for
  one. **The screen is whole at every count** — `OverviewLevel` used to `return` right after the
  band when there were no photos, so a new user got eyebrow + hero + two ghost frames + one italic
  line and then a BLANK page, with `Albums →` unreachable because it hangs off the TIMELINE header
  inside the returned-early region. The zero page now runs: honest `0 PHOTOS` eyebrow → serif hero →
  ghost frames **keeping their FIRST/NOW tags** (the mark states its own structure, it doesn't lean
  on the prose beneath it) → one hint → one filled `Add a photo` capsule → and BODYWEIGHT with its
  real figure and spark, which is no longer photo-gated (it is the live sibling the ghosts read
  against, §12). At ONE photo the shot takes **FIRST**, not NOW — a lone shot in NOW beside a ghost
  FIRST reads as a missing past rather than a start. **Browse controls are count-gated, never
  rendered over nothing**: compare ≥2 · search/filters ≥4 · pose pills only once ≥2 poses exist;
  dropping below a threshold CLEARS the state that control set, so a filter can't outlive its
  control, and a filtered-to-empty grid carries its own `Clear search`/`Clear filters` chip rather
  than being a dead end. The grid widens to 2-across at ≤2 photos (3-across put a single shot on
  screen as a ~100dp speck, §12 debris). **Loading is its own state**: `index.json` is read off disk
  async, so before this every entry — including a 200-photo library — rendered the zero state for
  the first frames and flashed "add your first shot" before snapping to content; the band now
  shimmers while `loading` and everything below it waits.)
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

### Sheets

- SessionSummarySheet (minimal)
- CardioSessionDetailSheet
- heatmap "That day"
- ExerciseLibraryPicker (`singleSelect` = radio-style choose-one for swaps — accent-wash pick, no
  checkbox)
- the program SetsReps sheet
- AvatarPickerSheet (profile cover — "select your own" + provided default covers by category,
  `DefaultAvatars`; picked default is baked into `avatar.jpg`)

Check this map + `ui/common/` before inventing; update it when screens change.

---

## Detail relocated from the core doctrine

Verbatim from the pre-split `DESIGN.md`. These are *inventory* — how a shipped feature works — rather
than rules, so they moved here to keep the always-loaded core small. Nothing was reworded.

### Onboarding — plan-mode vignettes, wearable pick, signal probes, equipment steps

all three plan-mode cards carry short pre-rendered vignette videos (alpha WebP authored in
`remotion-vignettes/`, rendered to res/raw: generated builds a full week, custom hand-builds a day
one exercise at a time, freestyle re-logs a scattered no-frame log in no order — its missing
structure IS the message) that play ~twice then FREEZE on the built plan / caught log — a shared
`PlanModeSync` starts the videos together so they replay/freeze in lockstep; the live Canvas
vignette (`PlanModeVignettes`, its own final frame, one draw per mode) is the decode / pre-28 /
reduce-motion fallback; equipment/preset/goal tiles use the `OnboardingIcons` matched glyph family.
About-you closes with the wearable pick (Galaxy · Pixel · no watch, keys + labels + source app from
`domain/health/WearableBrand`): cards carry the one honest per-brand difference as right-meta
("Routes sync"/"Routes vary"), the pick answers with a mono what-syncs readout + a version caveat
caption (feature sets differ by watch generation and companion-app version), and the same enum
drives Settings → Recovery's WEARABLE `PillChip` row + brand-aware source-app/routes explainers —
the two surfaces may not drift, and the brand is advisory only (HC reads stay vendor-neutral). Each
granted Recovery read-signal row carries a post-connect reading (§9, `probeSignalFlow`): `RECEIVING`
(onBg) when a record arrived in the last 30d, `NOTHING YET` (muted — quiet nudge, never alarm, since
absence is ambiguous) when granted-but-silent, plain `ON` while probing or for the write-only
calorie row. HC exposes data PRESENCE, never capability — so the UI never says "unsupported", and
grey-out-by-brand is banned (both watches can do every signal on recent versions). Gym setup = TWO
steps (GYMAP-20): preset grid (big-app lineup, `equipmentPresets`) + a live "in this setup" gear
readout, then a fine-tune page grouped by the shared `equipmentGroups` (Settings → Program →
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
