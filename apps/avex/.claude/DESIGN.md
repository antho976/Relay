# Forge — UI & Design Doctrine

**Binding for all UI work.** Wins over existing screens for new work; older screens migrate when
touched. When silent, Home (`ui/overview`) is the feel reference but not gospel (`design/SETTLED.md`
lists its defects). `ui/theme/` + `ui/common/` own *values*, this file owns *usage*. A decision
changed during a task is written here the same turn (§16).

## 0. Use this file

1. **Pick the archetype** (§3). Everything downstream follows from it.
2. **Open its recipe** — `app/src/debug/java/com/forge/app/ui/recipes/<Archetype>Recipe.kt`. Copy the
   scaffold; don't compose a screen from prose. The recipe already encodes rhythm, marks and zero-states.
3. **Build**, resolving each decision with a ladder (§2) rather than by taste.
4. **Run the checklist** (§15); `gradle -p forge-android testDebugUnitTest` checks the mechanical half.

**Load always:** this file + the one recipe you need. Everything else is on demand — see §16.

## 1. The language — and why

**"Open editorial"**: content sits directly on the near-black page — no grey boxes around passive
content. Hierarchy = three type voices (big serif figures, quiet sans prose, tiny mono small-caps
labels) + whitespace, never container backgrounds and **never hairline strips** (a line exists only
as data: chart/table rule, Coach's ledger spine). Sections separate by air + their mono header
alone. Color is scarce — one user-chosen accent at fixed intensities, so its presence means
something. Surfaces/borders are **earned by interactivity**: can't tap it, no box. Modals keep surfaces.

*Why it holds together:* a box is a promise of a tap, a line is a claim about data, and colour is a
claim about importance. Spend any of the three on decoration and it stops meaning anything — which is
why the rules below are mostly about **not spending**. When a case isn't covered here, ask what the
element is *promising* and whether that promise is true.

## 2. Decision ladders

Where doctrine says "use judgment", drift starts. These are lookups, not judgment.

**① I have a new number.**

| Test | Treatment |
|---|---|
| The screen's single most important answer | serif hero figure — **max one per screen** |
| One of 2–4 headline readings for this screen | `EditorialFigure` in the figure row |
| Qualifies a row that already exists | row right-meta — a count or reading, never a state word |
| Explains a mark | mono caption, ≤12 words, ≤1 per section |
| None of the above | **cut it.** A number with no decision attached is noise. |

**② I have a new section — which mark carries it?** (Words are the caption, never the content.)

| Data shape | Mark | At zero |
|---|---|---|
| one value vs a target | meter bar | empty track, honest 0 |
| one value over time | sparkline | flat ghost line — *only beside a live sibling* |
| value per day-of-week | 7 accent bars | all-zero bars, never hidden |
| value per day over months | dot / heat grid | unlit grid |
| set of items, some present | filled / hollow dot rail | all hollow |
| ranked comparison | thin bars (3–4dp, rounded) | — |
| progress to an unlock | n-of-m meter, m = the **real** gate | 0-of-m |
| one categorical state | *not a section* — fold it into a caption | — |

**③ I have a new action.**

| Test | Treatment |
|---|---|
| Navigates somewhere | ③ mono accent `action →` |
| Does something now, primary | ① filled capsule, ≤1 per section; page-level grouped at the **end** |
| Its sidekick, or destructive | ② outlined capsule (destructive tinted `error` + Undo, never filled red) |
| Scoped to one list row | **whole row is the tap target** + a drawn compact outlined pill — never nested taps, never filled per row |
| Switches which view you see | `SegmentPill`, never a button |
| Can't actually run | render passive. Nothing looks tappable while doing nothing. |

**④ I have a new screen** → pick from the §3 table, then use only that archetype's toolkit.

**⑤ I have a new state** → §12. All seven are answered there; none may be left undrawn.

**⑥ I have a new component.** First use → local to the screen file. Second → extract within the
feature package. **Third → promote to `ui/common/` the same turn**, and add it to the recipe.

## 3. Screen archetypes

Global rules (§4–§14) apply everywhere; beyond them a screen draws ONLY from its archetype's toolkit
(a settings page with a serif hero + chart cascade is as wrong as a boxed stat card).

| Archetype | Screens | Toolkit | Not here |
|---|---|---|---|
| **Overview / dashboard** | Home, Stats, Coach, Cardio, Profile | Serif hero = mono eyebrow (identity + human date) over ONE serif line ONLY when it carries a decision/result (Coach "Deload week"), else the bare name; status/anticipation is never a verdict (drop the serif line, figures/mark become the hero); never a name over a verdict, never a filler headline. Aside line only for the screen's unique read. 2–4 `EditorialFigure`s + exceptions as quiet lines + primary action above fold; lens pills; open charts; **≥1 mark that works at zero**; `statsEntrance` cascade; scroll ≤2–3 viewports. **Account variant** (Coach, 2026-08-20): no pills, no serif hero — one time-ordered column of entries on a spine, the ONE surface fill spent on the single entry still asking for something, evidence attached to the decision it supports (`design/MAP.md`) | an all-text screen — every section leads with a mark (§12) |
| **Detail page** | session/cardio detail, lift drill-down | Serif title + context, metric `SegmentPill`s, charts w/ draw-in, set tables; scoped to ONE item | dashboard figure walls, lens pills for unrelated views |
| **List / browser** | History, Goals, Trophies, pickers | Search-first, trim rows, light stagger; tiny hero (title + ≤1–2 figures) | charts, big hero, draw-in theatrics |
| **Settings / form / editor** | settings, goal/program editors, onboarding | Mono `SettingsSectionHeader` anchors + air, **no dividers**; each control gets a ≤1-line explainer; navigation = `action →` links, one-shot actions = capsule buttons (filled · outlined sidekick) **grouped at the END of the page, never mid-scroll**; never toggle-chips; 44dp capsules. Keep each drill-in light — split a dense multi-block area into focused sub-pages, each menu row showing its live value. **Onboarding** (rebuilt 2026-08-22): the path asks ONLY what the plan needs and nothing that is a setting — settings go to one optional closing step, after the thing being set up exists. Each page = serif `headlineSmall` question (page-title voice, not a hero; NO chapter eyebrow, the rail already says where you are) → ≤1 caption → content; chrome = `←` + segmented step rail, one cell per step of the path actually taken + mono `skip →` (no wordmark pre-app); full-width filled capsule CTA; **the thing being built is drawn under the question and OUTSIDE the page slider**, so it holds still and animates its own values while questions slide past — real at every stage or absent, never a placeholder; every selectable shares ONE tile formula (outline@0.35 border → accent border + accent@0.15 wash) | serif heroes, figures, lens pills, chart motion/stagger; section dividers; action buttons mid-page; actions as pill toggles; one long multi-block scroll (only onboarding's optional closing step is exempt) |
| **Live / flow** | live session, freestyle log | Function-first: big targets, timers, set-log haptics + `bouncy()`, confetti (PR only), keep-screen-on | presentational motion, figures, padding at the cost of reach |
| **Modal** | sheets, dialogs | Surface fill, `large` top corners; dialogs = confirmations/tiny inputs only | open-editorial bare background |

Each archetype has a compiling recipe under `ui/recipes/` — start there, not from this table.

## 4. Composition & interactivity (global)

1. **Overview-first**: every feature leads with one at-a-glance page answering "where I stand + what to know" at zero interaction; deep dives live in sibling lenses/sub-screens.
2. **Interaction earns its content**: no prose behind a tap ("Why this?/show more" banned — surviving content stays visible, the rest is cut not folded); no bare hyperlinks (a link exists only as `view all →` beside a trim of its destination; nothing to preview → no link). Data drill-downs (Stats rows, set tables) are fine — navigation-in-place.
3. **Prose budget + one home.** A section's primary content is data-shaped (figure/bar/chart/tag/row); a sentences-only section is redesigned to data or cut. Max ONE muted caption (~12 words) per section beyond the hero's context line; mechanics narration is cut not trimmed (the state changing IS the explanation; explainers only beside a non-obvious control, §13). **One home**: a fact appears once per screen (a caption on its element, never a floating footer or repeated across lenses); likewise a MARK — a visual that only repeats another screen's/lens's answer is cut, not copied.
4. **Sub-paging = lens pills** (`SegmentPill` row, labels ONE short word). Routed sub-screen = real sub-feature w/ own hero; sheet = transient detail of one tapped item.
5. **Data is explorable**: aggregate visuals (heatmap cell, chart point, row) answer a tap with detail; every interaction passes "you'd miss it" or "wow"; nothing looks tappable while doing nothing.
6. **Chrome earns its tap**: top bar = `←` + ≤1 action, **never the screen's own name** (no `TopAppBar` title). Home alone carries its three destinations together: the bell on the left, then Profile + Settings on the right. The bell (`NotificationBell`, tap→the feed, hold→Home) is **HOME ONLY** and carries a `CountBadge`, not a dot; a *tab* may badge its own count (Academy) but no page repeats the bell's. A screen names itself with a serif content hero or not at all. **One back affordance per page** — the top-bar `←` alone, never a second in-page back arrow. **A notice belongs in the feed, not on the page**: anything dismissible, celebratory or "waiting on you" goes behind the bell — a page never opens with a resident strip above its own answer. The ONE thing that may cross a page is a **transient arrival receipt** (`ArrivalBannerHost`): an overlay that displaces nothing, is never dismissible, and flies into the bell on its own (`design/SETTLED.md`, 2026-07-27 / 2026-08-15).
7. **Build flow = two-shot**: overview first → Antho device-checks → then subs/lenses.
8. **Lead with the live — placement is rank.** Within a screen/lens: real data/decisions sit under the pills, mixed reached/ahead ladders next, pure countdown/unlock meters last; a section of all-zero marks never opens a lens with a live sibling; a ladder and its countdowns sit adjacent.
9. **Show the reading, not just the conclusion.** Surface the engine's underlying readings, not only its verdict — both beside a conclusion (the deciding reading as row meta: "38% hard · +2", "7.2h avg") and BEFORE one exists (per-item readings render as soon as they're computable, not when a score unlocks). Below a gate the reading is progress toward it ("3 of 12 rated sets"), never "n/a". A panel of named checks each with its own reading is data, not §12 repetition; each lens stands on its OWN data. "Not enough data yet" is a last resort; when a reading panel arrives, any row elsewhere that only restated it goes. A dense panel may CLOSE with one muted line naming what the readings feed.
10. **A checklist is not a section.** A run of >~4 uniform dot-text rows is the "AI look" — redraw as ONE mark + a single focused detail (a 9-row ladder → a segmented rail "2 OF 9" + only the NEXT item). Rows earn a list only when each carries a distinct reading/action. Adjacent road-ahead content shares one visual language (all bars).

## 5. Color (`Color.kt`; via `MaterialTheme.colorScheme`, never raw vals)

Pearl (dark default) is a **warm** near-black (2026-08-16; it was blue-leaning, and neutral-cool is a
temperature, not a style): bg `#110F0C` · gradient `#17120E→#0A0806` (behind every screen) · surface
`#1A1613` (sheets) · surfaceVariant `#221C16` (interactive tile fill) · outline `#38302A` · onBg
`#F2EFEA` · muted `#BFB6AA`. AMOLED: bg black, surface `#080808`, surfaceVar `#111111`, gradient
`#000000→#050507`.

**M3's container tones ARE themed** (2026-08-20): `ForgeTheme` sets the `surfaceContainer*` ladder,
`outlineVariant` and `surfaceTint` = surface, so sheets/menus/pickers land on Pearl rather than
Material's stock purple-grey. **A modal still says `containerColor = surface`**, its dividers the
outline 0.25 rung, its confirm/dismiss `onBackground`/muted.

Accent = user-picked (Red `#E23D3D` **default** · Ember `#D4761F` · Olive `#4D6040` · Gold `#7A6435` ·
Navy `#3D4F73`); `primary`=accent, `primaryContainer`=@0.15, `secondary`=@0.6. Spend it in FEW places
at LARGE size, never many at postage-stamp size — scattered tiny accent reads as a dead pixel, not as
energy. Accent can be **disabled** (Appearance → monochrome): `primary` falls back to a near-white
neutral (onBg) so highlights stay legible without colour, and `onPrimary` flips to bg above luminance
0.18 (so a filled-primary control never goes same-on-same, and mid-tone warm accents get dark text).

**Intensity ladder — the only allowed alphas** (snap strays when editing; enforced by §14's gate):

| Rung | Use |
|---|---|
| accent 1.0 (`primary`) | chart strokes/dots, selected-pill border, `action →`, ↑ delta, legend dots, active nav |
| accent 0.6 (`secondary`) | launch-wordmark dot, secondary chart series |
| accent 0.15 (`primaryContainer`) | tonal fills: selected pill bg, active-row wash |
| onBg 1.0 | primary text, serif figures |
| muted 1.0 | secondary text, mono labels, ↓ delta |
| muted 0.65 | captions, deselected pill text — **hard floor, measured 4.54:1** (§14) |
| outline 0.35 | borders on unselected controls |
| outline 0.25 | **data lines only** (`EditorialHairline`) — never a section separator, §1 |

Two named exceptions and no others: a **placeholder** in a text field may dim below the muted floor
(it is a ghost affordance, not content); a **gradient or scrim** interpolates freely between rungs.

Bars: fill `primary` 1.0 on track outline 0.25–0.35. Error mirrors accent (full text/stroke,
container 0.15). **Reserved**: PR gold `#E3B341` (PR star + gold set row only) · `△ LAST` green
`#5BC873` · success `#4CAF7D` / warning `#CFAB47` / error `#BF4040` for true states, never decoration.

## 6. Type (`Type.kt`) — three voices; choosing the voice is choosing the meaning

| Voice | Styles | For | Never |
|---|---|---|---|
| **Serif** (tnum) | display 52 · headlineL 36 · headlineM 28 · headlineS 22 | THE one big thing per section: page titles, hero figures | prose, buttons, labels |
| **Sans** | titleL 18M · titleM 16M · titleS 14M · bodyL 16 · bodyM 14 · bodyS 12 | row titles, prose, button text | section anchors, big numbers |
| **Mono** (letter-spaced) | anchor 15 · labelL 13 · labelM 11 · labelS 10 | UPPERCASE micro-labels: section anchors = **`MonoSectionAnchor` 15** (`EditorialHeader` only) · row/metric labels = labelL 13 · figure captions + meta = labelM/S | sentences, titles |

- **Always take a style from `MaterialTheme.typography`** — never `fontSize =` at a call site. The
  three voices ARE the meaning system; an inline size opts out of it. Only sanctioned off-scale use:
  8–9sp figure captions.
- **A sentence is never mono** — explainers take `bodySmall` (`SettingsExplainer`).
- **Rank two mono labels by SIZE, never tracking or colour** (`design/DECISIONS.md`, 2026-07-25).
- Mono labels `.uppercase()` in code. *Italic* = the aside voice (wordmark, coach one-liners,
  taglines): bodyM/S italic, muted.
- One serif hero per screen, everything else steps down; animating numbers use tnum styles.

## 7. Spacing & shape

4dp grid (steps 2–28); page gutter **24dp**. Section rhythm — air, no rules:
`28 → mono header → 10 → content` (whitespace + header ARE the separator, §1). Within a section:
`header → 2 → caption → 10 → content`; **no two text lines of different roles butt flush** — ≥8dp
between a header/caption/aside/row (put a Spacer(≥8) before the first row; don't lean on the row's
own bottom padding). Figure rows: gap 20, label 2 under number. List/data rows: **ONE vertical
padding for ALL of a lens's rows** (coach shares `COACH_ROW_PAD` = 6; settings shares `SETTINGS_ROW_PAD` = 12) — sibling
sections never mix 4/5/6, the page reads as one rhythm; a text-link inside a row stays at vertical 2
so it doesn't inflate that row above its neighbours.

**Sanctioned off-grid values** — deliberate, do not "fix": interactive tile inner padding **14×12**;
`SegmentPill` **12×5**. Shapes (`Shape.kt`): 4/8/12/16/24, pills `RoundedCornerShape(50)`, tiles 12,
sheet top 16 — no custom radii. Photos: rounded 16 clips, caption UNDER the plate or on a bottom scrim, full-bleed strips
(Profile cover/filmstrip = reference).

## 8. Components & controls (`ui/common/` — reuse, never re-implement)

**The kit** — check here before writing any component. `EditorialHeader` · `EditorialHairline` ·
`EditorialFigure` · `EditorialLegend` · `SegmentPill` · `ForgeSwitch` · `InlineEmptyHint` ·
`NotificationBell` / `CountBadge` · `ArrivalBannerHost` · `AvexWordmark` · `AvexIntro` · `IconLaunchScene` · `bounceClick` /
`bounceCombinedClick` · `clickableLabeled` · `GlyphButton` · `ForgeHeroAction` · `ForgePrimaryCapsule` /
`ForgeOutlineCapsule` · `ForgeRowPill` · `ForgeShimmer` · `ConfettiOverlay` · `statsEntrance` ·
`EntranceItem` · `rememberDrawProgress` · `CountUpText` · `ExerciseLibraryPicker` ·
`ProvideTouchExploration` · `SnackbarController` · `DayLogSheet`. **A pattern used on a 3rd
screen gets promoted here the same turn (§2⑥)** — `DoctrineParityTest` checks this list against the
package both ways, so it cannot drift.

What a signature won't tell you: `EditorialHairline` is **data lines only** (§1) · `SegmentPill` carries
every filter/lens toggle · `ConfettiOverlay` is celebration only · `bounceCombinedClick` adds the
long-press hook (Home CTA tap = start, hold = skip warmup) · `DayLogSheet` answers "what did I do that day" for
every consistency grid (Stats, Profile), over History's own rows · `GlyphButton` guarantees the ≥48dp target a
padded `Text` kept missing · `ProvideTouchExploration` feeds the TalkBack flag that restores ripple (§9) ·
`SnackbarController` is the app's ONE Undo snackbar, an injected `@Singleton` any VM reaches via `showUndo(msg){
restore }`, rendered once at the app root by `SnackbarControllerHost` beside `ProgramChangeGuardHost`; §12's
undo-over-confirm soft-delete uses it (delete now, re-insert the captured row on undo) so a reversible delete
never gets a confirm dialog. Don't re-roll a per-screen `SnackbarHostState`.

**Buttons — three levels only**: ① filled capsule = do-it-now, ≤1/section (light; `accent = true` only for a MODAL's one commit, which has to out-rank the selection controls above it); ② outlined capsule =
its sidekick (a destructive one-shot = level ② tinted `error` via `ForgeOutlineCapsule(contentColor)`,
paired with an Undo snackbar, never a filled red button); ③ mono accent `action →` = navigation. No M3
default/floating-text/icon buttons in content. Settings reuse `SettingsPrimaryAction` (do-it-now) /
`SettingsOutlineAction` (sidekick) / `SettingsActionLink` (`action →` nav) from `SettingsPrimitives.kt`;
both capsules are GUTTERLESS and go only inside `SettingsActionRow`, which owns the gutter and wraps
them at large scale (your own padded Row double-gutters them); group them at the page END, never mid-scroll.

**Per-row action = compact OUTLINED pill, never filled.** A do-it-now action scoped to a single list
row/integration (Wearable's Connect, the Profile BODY rows' Log/Sync/Open) renders as a right-aligned
compact OUTLINED pill — the shared **`ForgeRowPill`** (`ui/common/Capsules.kt`; promoted out of
settings on its third screen 2026-07-24, `ConnectPill` now delegates to it) — border only, onBg text,
sentence case, with the WHOLE row as its tap
target (the pill is drawn, not independently clickable — no nested tap). NEVER a filled capsule per
row: five of those stack into a *button wall* (`FAILURES.md`; Wearable failed exactly this way). The
ONE filled capsule stays page-level, grouped at the END. A bare mono accent `connect →` link is too dim
against a muted accent — prefer the pill. A connected row shows a passive `• ON` (accent disc + mono)
on the right, and a list of connectables leads with its filled-disc/muted-ring dot rail (§12; ring at
1.5dp muted@0.55 so the empty state reads on near-black). Rows without a usable action render passive —
no affordance that can't run. Coach + Wearable draw this dot and pill through the shared `StatusDot` /
`ConnectPill` (`SettingsPrimitives.kt`) — reuse them, don't redraw.

**Sizing — trim, never chunky** (48dp touch from padding, not visual size — and these are *minimums*
that grow with font scale, §14): hero CTA `ForgeHeroAction`, accent-filled ≥56dp, a HUB TAB's ONE
primary action and never a section's (Home · Cardio, 2026-08-23); standard capsules **44dp** (14sp); `ForgeSwitch` **40×24** track (thumb 14→17, press ~20); `SegmentPill` 12×5, 10sp.

**Icons**: chrome (nav/gear/back/share) + a muted leading glyph on settings/list nav rows for
wayfinding. Row/content glyphs come from the matched custom families (`SettingsIcons`/`NavIcons`/
`ExerciseIcons`/`NoticeIcons`), never Material stock, never accent-tinted; TOP-BAR chrome may use Material stock
until a custom chrome set lands (content never). Exercise rows in browsers/pickers lead with their
`ExerciseIcons.forEquipment` equipment-class glyph (one glyph per implement class, custom moves =
pencil); elsewhere content is text-first, glyphs `→ ↑ ↓ △ • ·` carry meaning — no decorative
icons/emoji. (Known gap: `CardioType.icon` still mixes Material stock into its custom family.) The
families share only their builder plumbing via `VectorBuilders.kt`; glyph shapes stay per-family.

**Don't render state twice, and flag only exceptions.** A leading `•` dot is earned only when its
COLOR flags something the eye should catch — a failure (error), a win/active (accent) — never the
neutral/default/inactive majority (a column of identical grey dots is noise → *grey dot column*,
`FAILURES.md`). Paint the dot only for the exception; the common rows reserve the gutter
(`CoachFlagDot(null)`) so they still align, dotless. Likewise a row's right meta is a count or reading
only, never a state word the dot or reason line already carries.

## 9. Motion & feel (`Motion.kt` — never literal durations/easings at call sites)

Durations: Fast 150 · Standard 240 · Emphasized 320 · Draw 900 · Celebration 2200. Easings:
Decelerate-in / Accelerate-out / Standard / DrawDecelerate (chart reveals). Springs: `bouncy()`
(set-log, PR pop) · `snappy()` (pill slide).

- Overview/detail: `statsEntrance(index)` cascade + one-shot `drawTween()` reveals (never re-trigger on scroll); lists light stagger; settings/live no presentational motion.
- **Press = bounce everywhere** (scale 0.97, no ripple; auto-ripple under TalkBack); migrate rippling M3 buttons when touched.
- Feel (don't retune casually): bounceClick MediumBouncy/MediumLow; ForgeSwitch position 0.68/900, thumb resize no-bounce/1400 + press-stretch.
- **Haptics rare** (`forgeHaptic`): set logged, PR/finish, timer ticks — nothing else.
- **Reduce-motion is global**, not per-feature: entrance cascade, chart draw-ins, confetti, launch intro and press bounce all degrade to a settled still state. Never gate meaning on motion.
- **Launch**: `AvexIntro` settles the serif "Avex" wordmark once per cold launch over the first screen, themed to the chosen app icon, then the plate fades to reveal it. The system splash is background-only so the wordmark is the brand beat. This deliberately spends colour off the one-accent rule (§1/§5) for a pre-app moment only. Per-family mechanics, the icon picker and the unwired launch scenes are recorded in `design/MAP.md`.

## 10. Charts (overview/detail archetypes)

Stroke + dots `primary`; secondary series `secondary`; area fades to transparent (≤0.15 top); grid =
hairline. Open on the page — no plot frames. Legends `EditorialLegend`; axis labels mono labelS muted.
Values tnum; draw in once with `drawTween()`. Comparison bars thin (3–4dp), rounded. Every chart is a
Canvas and therefore invisible to TalkBack until given semantics — §14.

## 11. Copy, voice & glyphs

**Voice — dry, specific, earned.** Every generated line (coach/milestones/recaps/opinions/
notifications/hints) states a fact or instruction grounded in the user's data, and VARIES with those
numbers rather than repeating one generic cue (a quiet-week line reads the week's PRs / volume trend /
session pace, never the same "keep doing what works" every time); an understated nod only when earned
("Clean sweep. You beat every comparable set."). Coach speaks imperative + "you", never "I", never
names itself. **Banned in any rendered string**: exclamation marks (confetti is the exclamation mark);
em dashes (split, or join with a comma / " · "); hype/bro-speak ("beast", "crush"); poster clichés
("moves the needle"); praise ungrounded in data. Offenders rewritten when touched. Enforced by §14.

**Headlines & verdicts.** Serif hero/title takes NO terminal period ("Coach", "Pull B" — periods only
in body/italic-aside prose). A verdict states what it means FOR THE USER, not internal state ("Ready
to coach", **not** "Baseline set"); if it needs system vocab, reword and let the subline carry the why.

**Translate the machine.** Machine identifiers never render ("2026-W27"→"Week of Jun 29", a status
enum→its word, a slot key→its exercise name). Machine PROSE too: paren-plurals ("3 session(s)"), em
dashes and jargon get rewritten at render, and a stored paragraph → a short derived line ("Baseline
still forming · 3 of 4 sessions"), since pass rows are immutable — the translation lives in the UI.

**Naming state & the future.** A status never stands alone — attach the referent the screen knows (a
held week shows its reason; a pending verdict names its change; a collapsed group names members only
when they fit ONE line, else count alone; a countdown names its landing day). An upcoming item is
worded forward under a `NEXT` eyebrow, never a past-tense achievement label.

**Glyphs & numbers.** Mono small-caps section headers, else sentence case (buttons included). Actions
end ` →` / `+ log` (accent mono); meta joins with `·`; deltas `↑`(accent)/`↓`(muted), `△ LAST`;
the wordmark "• Avex" is the LAUNCH beat only (`AvexWordmark`), never chrome. Numbers k-abbrev ≥10,000 ("4.5k"); RPE/RIR via `Format.kt`
(no ".0"); **weights always via `WeightFormatter`** reading `LocalForgeSettings.current.weightUnit` — a
tri-state `WeightUnit` (lb·kg·st), never a `useKg: Boolean` (a legacy bridge lingers on the formatter +
`ForgeUiSettings.useKg`; new code passes `weightUnit`). Never hardcode lb/kg anywhere. Stones render as a `stone + pounds` compound
("12 st 4 lb"), but AGGREGATES stay one decimal in every unit ("1.2k st"). Unit suffix uppercase only
in mono captions. Coach lines = quiet italic asides, never banners.

## 12. The seven states

**Every section answers all seven.** Most bad UI is not ugly — it is a state nobody drew.

| State | Treatment | Never |
|---|---|---|
| **zero** | the section's own visual at zero/ghost (§2②) | status words; hiding the section |
| **one** | a list row | a chart of one point; a strip of tiny cells |
| **many** | the mark; rows only if each carries a distinct reading | >4 uniform rows (§4.10) |
| **overflow** | trim + `view all →` beside the trim | a bare link; "show more" |
| **loading** | nothing — local DB is instant; screens appear with the entrance cascade. `ForgeShimmer` only for real latency (photos, Health Connect) | spinners, blocking loads |
| **error** | quiet inline line in error color, wording the consequence | banners, toasts, dialogs |
| **stale / denied** | last-known reading + its age ("2H AGO"); honest zero when connected; hidden when the grant was never given | "unsupported"; grey-out-by-capability; an error banner for an absent signal |

*Health Connect exposes data PRESENCE, never capability — so the UI never says "unsupported", and
disabling a signal by device brand is banned.*

**Empty is data at zero — drawn, not written.** A zero-state reuses the SAME vocabulary it shows with
data (bars/meters/sparklines/dot rows), never a list of status words.

- **Every data section leads with a MARK** (words are its caption, never its content); no section is a bare text row. A section that can only ever be text isn't a section — fold it into a caption. At *screen* scope this means ≥1 mark that works at zero (§3). Plumb the count into the ViewModel — a visual earns the wiring.
- **Collapse repetition**: N rows sharing one empty status → ONE line naming the concrete unlock ("3 lifts building history · first read after two sessions"), never N identical rows. A ghost mark shows ONLY beside a live sibling (contrast reads as "still forming"); an all-ghost group drops the mark — a lone flat line reads as broken. Ghost VISUALS good, ghost DATA (fake numbers) banned.
- **n-of-m meters** measure only a real unlock threshold, and m is the REAL gate (a bar filling to the wrong gate unlocks nothing); stacked gates → show the unfilled one. Never "data availability" jargon ("lifts with a trend: 3 of 8" promises a chart it doesn't show) — render items that HAVE data with their real trend, ghost-collapse the rest.
- **A mark needs visual mass at the data's REAL size**: a strip of tiny cells below ~a row reads as debris; at small counts use list rows. Design each section at its emptiest realistic state, not its fullest.
- **Figures show honest zeros** ("0 WORKOUTS"), never a dash, never hidden.
- **`InlineEmptyHint` is the last resort**, ≤1 per lens, only where there's no zero-shape; it REPLACES the caption (never both); terse, no em dashes. Boxed `EmptyState`/`FirstTouchTip` deprecated.
- **Feedback: undo over confirm** — reversible acts get a short Undo snackbar via `SnackbarController` ("Set logged · Undo"); dialogs only for destructive/irreversible acts (wording the consequence); no success toasts for what the UI already shows.

## 13. Inputs

- **Text inputs** (interactive → bordered): `OutlinedTextField`, unfocused = outline rung, focused = accent, muted placeholder (a ghost affordance — it may dim below the §5 muted floor); search = leading magnifier + trailing clear, either bordered (History `SearchField`) or a filled rounded field (surfaceVariant — the standard phone-search look, Settings + timezone picker).
- Hot-path numbers = steppers + inline edit, never keyboard-first.
- An explainer belongs beside a non-obvious control only (§4.3), ≤1 line.

## 14. Physics — scale, contrast, touch, semantics

The screen must survive the biggest font, the longest string, no data, no network and no permission.
These are measured, not felt. The mechanical half is enforced by `DesignDoctrineTest`.

**Font scale — the app must survive 200%.**
- Text sizes in `sp`, always from `MaterialTheme.typography` (§6). Never `fontSize =` at a call site.
- **A container holding text sizes to its content** — no fixed `.height()` on a row, tile, capsule or
  cell that contains text. Use padding, `heightIn(min = …)` or `wrapContentHeight`.
- **44dp / 48dp are minimums, not sizes.** Controls grow with scale; touch target comes from padding.
- No `maxLines = 1` on user content (names, notes, exercise titles). Chrome and mono labels may clamp.
- Figure rows wrap rather than clip. The serif *hero* may clamp its own scaling at ~1.3× so a 52sp
  display figure stays on-screen — clamping the hero is allowed, clamping content is not.
- Check every touched screen at 100% and 200%. `RecipeScreenshotTest` pins the six archetypes as
  golden images at both scales (plus AMOLED and monochrome); CI diffs them. A changed golden is a
  question, not a chore — look at the diff before re-recording.

**Contrast — measured on Pearl `#110F0C`.** Text ≥4.5:1. Data marks that carry meaning ≥3:1.

| Element | Ratio | |
|---|---|---|
| onBg `#F2EFEA` | 16.68:1 | ✓ |
| muted 1.0 | 9.56:1 | ✓ |
| muted @0.7 | 5.18:1 | ✓ |
| **muted @0.65** | **4.63:1** | ✓ the floor — 0.6 gives 4.08:1 and **fails** |
| PR gold · △ green · success · warning | 7.0–9.8:1 | ✓ |
| **accent as text** — default `#E23D3D` | **4.53:1** ✓ | pass: Red 4.53 · Ember 5.84 — fail: Olive 2.79 · Gold 3.37 · Navy 2.34 |
| **error `#BF4040` as text** | **3.67:1** | ✗ **fails AA** |

Accent-as-text failed for every accent until the warm repalette. **A default MUST clear 4.5:1** (margin today: 0.03); alternates need not.
The rule stands: **no new accent- or error-coloured body text** — use onBg text with an accent glyph
or mark, the only treatment correct under every accent choice. Open decisions in `SETTLED.md`. Structural hairlines and tonal washes (`outline` rungs, `primaryContainer`) are **exempt**:
they are decorative boundaries, not content or state, and raising them to 3:1 would destroy §1.

**Touch**: ≥48dp from padding, not visual size. ONE tap target per row — never nested (§2③).

**Semantics**: every Canvas mark (sparkline, meter, dot rail, heatmap, chart) carries a
`contentDescription` reading its *value*, not its shape ("This week, 4 of 5 sessions", not "bar
chart"). Purely decorative marks take `null`. `EditorialHeader` marks itself a heading. Interactive
non-Button elements use `clickableLabeled`. Bounce-press auto-restores ripple under TalkBack (§9).

**Resilience**: design against the longest realistic string, not the demo one; layouts stay
RTL-correct (`start`/`end`, never `left`/`right`).

## 15. Checklist before calling UI work done

**Every screen**

- [ ] Right archetype toolkit (§3), started from its recipe; no borrowed patterns.
- [ ] All seven states drawn (§12) — especially zero, overflow, and stale/denied.
- [ ] One serif hero; mono headers + air rhythm; **no section hairlines** (lines = data only); alphas only from the §5 ladder; colors via `colorScheme`; ≥8dp between text lines of different roles.
- [ ] Prose budget + one home (§4.3): data-led sections, ≤1 caption, no mechanics narration, no fact/mark repeated across lenses.
- [ ] Top bar = `←` + ≤1 action; Home = bell + Profile + Settings. Never the screen name, one back arrow; no page-level notice strips. Serif titles/verdicts no terminal period.
- [ ] Passive content bare; only interactive elements + modals get fills/borders. Nothing fake-tappable; no nested taps.
- [ ] Controls trim (§8); bounce press, no ripple; motion per archetype via shared modifiers only (§9).
- [ ] Generated text dry + earned, imperative + "you", no exclamations/em-dashes/hype (§11).
- [ ] **Renders at 200% font scale** without clipping or lost content; touch ≥48dp; Canvas marks carry value-reading `contentDescription`s (§14).
- [ ] Shared primitives reused, 3rd use promoted (§2⑥); screens ~300 / VMs ~150 lines as a guide (split at seams, don't contort to hit it); `design/MAP.md` updated; `gradle -p forge-android testDebugUnitTest` green.

**Overview** — answers state + conclusions + exceptions at zero interaction, ≤2–3 viewports; ≥1 mark
that works at zero; primary action above the fold; `statsEntrance` cascade present; overview shown to
Antho before subs (§4.7).
**Detail** — scoped to ONE item; no figure wall; charts draw in once, not on scroll.
**List** — search-first; trim rows; no charts, no big hero, no draw-in theatrics.
**Settings** — no dividers; every control has its ≤1-line explainer; one-shot actions grouped at the
END; navigation as `action →`; no serif hero, no figures, no lens pills.
**Live** — reach and target size beat padding; no presentational motion; keep-screen-on; haptics only
at set-log / PR / timer.

## 16. Satellites & change protocol

| File | Read it when |
|---|---|
| `design/MAP.md` | you need to know what already exists, or why a thing sits where it does |
| `design/SETTLED.md` | **before re-adding anything that feels missing** — and for open defects |
| `design/FAILURES.md` | a layout feels off and you want the named diagnosis |
| `design/AUDIT.md` | picking up doctrine debt — what is broken today, ranked |
| `design/DECISIONS.md` | why a rule is the way it is, or changed |
| `design/WEAR.md` | touching `:wear` only |
| `ui/recipes/*.kt` | starting any screen — copy the scaffold |

**Protocol.** A design decision made or changed during a task is written down the same turn. New
*rules* go in this file; new *screens/features* go in `MAP.md`; *removals* go in `SETTLED.md`; a newly
named mistake goes in `FAILURES.md`. If a rule is mechanically checkable, add it to
`DesignDoctrineTest` rather than relying on it being read.

**Budget: 420 lines.** Adding a rule means finding one that can leave, move to a satellite, or become
a test. The pre-split doctrine hit 2.5× its stated budget because nothing ever left. `DoctrineSelfCheckTest`
enforces this number and asserts that §16 and the test agree, so the cap can be raised deliberately
but never quietly exceeded. (Raised from 400 on 2026-07-24 — see `design/DECISIONS.md`.)
