# Forge — UI & Design Doctrine

**Read before any UI work.** Wins over existing screens for new work; older screens migrate when
touched. When silent, Home (`ui/overview`) is the feel reference but not gospel (§14 lists its
defects). `ui/theme/` + `ui/common/` own *values*, this file owns *usage*; a changed decision is
written here the same turn.

## 1. The language

**"Open editorial"**: content sits directly on the near-black page — no grey boxes around passive
content. Hierarchy = three type voices (big serif figures, quiet sans prose, tiny mono small-caps
labels) + whitespace, never container backgrounds and **never hairline strips** (a line exists only
as data: chart threshold/floor/baseline, table rule). Sections separate by air + their mono header
alone. Color is scarce — one user-chosen accent at fixed intensities, so its presence means
something. Surfaces/borders are **earned by interactivity**: can't tap it, no box. Modals keep surfaces.

## 2. App map

Hub = swipeable 5-tab pager + `ForgeBottomBar`: **Cardio · Stats · Home · Coach · Profile**. Top bar
everywhere = `←` (sub-screens) + `• Avex` wordmark (`ForgeWordmark`, taps→Home) + ≤1 action,
**never the screen's own name** (no `TopAppBar` title); **one back affordance per page — the top-bar `←` alone, never a second in-page back arrow**. A screen names itself with a serif content
hero (Stats "Stats", Profile "Athlete") or not at all (Home "Pull B").

- **Home** `ui/overview` — TODAY hero + Start session, week strip, goals, recent. Feel reference; defects (§14) fixed when touched.
- **Stats** `ui/gym/stats` — one page: hero figures + muscle map → lens pills Strength/Volume/Effort/Days → drill rows → heatmap → records → Banister.
- **Cardio** `ui/cardio` — THIS WEEK figures hero (days · min · dist · streak) + Mon–Sun accent bars + goal meter, GOALS trim (cardio-metric custom goals, shared `GoalProgressLine`, hidden at zero), week-pager stats overlay, recent rows (header carries a small filled `+` circle = log) → session detail (stat rows carry best-pace/longest compare meta + previous-session read).
- **Coach** `ui/coach` — lens pills Now/Signals/Journey (Now = call + watch + one road-ahead section: milestone rail + brief/verdict/autopilot bars; Signals = lifts + recovery + inputs + learned; Journey = record + trust; old Brief/Lab/Timeline routes = lens deep-links). Coach content renders ONLY here — Settings→Coach is config alone (on/off switch + mode chips + a feeds on/off glance whose silent HC rows tap to Recovery), never a second brief/trust/history home.
- **Profile** `ui/profile` — blending cover (**untouchable** compositing approach — its edge-fade + text-scrim STOPS were retuned 2026-07-09 (Antho) to remove a hard seam where the cover met the page; the masking technique itself stays frozen; a random default is seeded on first run so it's never empty, tap → `AvatarPickerSheet`), bodyweight-led, ALL-TIME 2×2, filmstrip.
- **Routed** (`Routes.kt`): `GYM_DAY` (`ui/gym/train`, **untouchable**) · `SESSION_HISTORY` (gym+cardio) · `SESSION_DETAIL` · `CARDIO_SESSION` · `GOALS`/`GOAL_EDITOR` · `TROPHIES` (frozen) · `NUTRITION` · `SETTINGS?page=` · `RECAP` · `NOTES_SEARCH` · `PROGRAM_VIEWER/EDITOR/BUILDER` · `FREESTYLE_LOG` · `MIRROR_TEST`.
- **Sheets**: SessionSummarySheet (minimal), CardioSessionDetailSheet, heatmap "That day", ExerciseLibraryPicker, AvatarPickerSheet (profile cover — "select your own" + provided default covers by category, `DefaultAvatars`; picked default is baked into `avatar.jpg`).

Check this map + `ui/common/` before inventing; update it when screens change.

## 3. Screen archetypes

Global rules (§4–§13) apply everywhere; beyond them a screen draws ONLY from its archetype's
toolkit (a settings page with a serif hero + chart cascade is as wrong as a boxed stat card).

| Archetype | Screens | Toolkit | Not here |
|---|---|---|---|
| **Overview / dashboard** | Home, Stats, Coach, Cardio, Profile | Serif hero = mono eyebrow (identity + human date) over ONE serif line ONLY when it carries a decision/result (Coach "Deload week"), else the bare name; status/anticipation is never a verdict (drop the serif line, figures/mark become the hero); never a name over a verdict, never a filler headline. Aside line only for the screen's unique read (a cue no section repeats). 2–4 `EditorialFigure`s + exceptions as quiet lines + primary action above fold; lens pills; open charts; **≥1 mark that works at zero**; `statsEntrance` cascade; scroll ≤2–3 viewports | an all-text screen — every section leads with a mark (§12) |
| **Detail page** | session/cardio detail, lift drill-down | Serif title + context, metric `SegmentPill`s, charts w/ draw-in, set tables; scoped to ONE item | dashboard figure walls, lens pills for unrelated views |
| **List / browser** | History, Goals, Trophies, pickers | Search-first, trim rows, light stagger; tiny hero (title + ≤1–2 figures) | charts, big hero, draw-in theatrics |
| **Settings / form / editor** | settings, goal/program editors, onboarding | Mono `SettingsSectionHeader` anchors + air, **no dividers**; each control gets a ≤1-line explainer; navigation = `action →` links, one-shot (do-it-now) actions = capsule buttons (filled · outlined sidekick) **grouped at the END of the page, never mid-scroll**; never toggle-chips; 44dp capsules. Keep each drill-in light — split a dense multi-block area into focused sub-pages, each menu row showing its live value. **Onboarding** (`ui/onboarding`): one decision per step; each page = mono chapter eyebrow → serif `headlineSmall` question (page-title voice, not a hero) → ≤1 caption → content; top chrome = `←` + 4dp accent progress rail + mono `skip →` (no wordmark pre-app); full-width filled capsule CTA; every selectable shares one tile formula (outline@0.35 border → accent border + accent@0.15 wash); plan-mode cards carry live looping Canvas vignettes (frozen at final frame under reduce-motion); equipment/preset/goal tiles use the `OnboardingIcons` matched glyph family | serif heroes, figures, lens pills, chart motion/stagger; section dividers; action buttons floating mid-page; actions as pill toggles; one long multi-block scroll |
| **Live / flow** | live session, freestyle log | Function-first: big targets, timers, set-log haptics + `bouncy()`, confetti (PR only), keep-screen-on | presentational motion, figures, padding at the cost of reach |
| **Modal** | sheets, dialogs | Surface fill, `large` top corners; dialogs = confirmations/tiny inputs only | open-editorial bare background |

## 4. Composition & interactivity (global)

1. **Overview-first**: every feature leads with one at-a-glance page answering "where I stand + what to know" at zero interaction; deep dives live in sibling lenses/sub-screens.
2. **Interaction earns its content**: no prose behind a tap ("Why this?/show more" banned — surviving content stays visible, the rest is cut not folded); no bare hyperlinks (a link exists only as `view all →` beside a trim of its destination, Home GOALS; nothing to preview → no link). Data drill-downs (Stats rows, set tables) are fine — navigation-in-place.
3. **Prose budget + one home.** A section's primary content is data-shaped (figure/bar/chart/tag/row); a sentences-only section is redesigned to data or cut. Max ONE muted caption (~12 words) per section beyond the hero's context line; mechanics narration is cut not trimmed (the state changing IS the explanation; explainers only beside a non-obvious control, §13). **One home**: a fact appears once per screen (a caption on its element, never a floating footer or repeated across lenses); likewise a MARK — a visual that only repeats another screen's/lens's answer (Home's week strip, the heatmap, a summary of another lens) is cut, not copied. (Empty section: hint vs caption per §12.)
4. **Sub-paging = lens pills** (`SegmentPill` row, labels ONE short word). Routed sub-screen = real sub-feature w/ own hero; sheet = transient detail of one tapped item.
5. **Data is explorable**: aggregate visuals (heatmap cell, chart point, row) answer a tap with detail; every interaction passes "you'd miss it" or "wow"; nothing looks tappable while doing nothing.
6. **Chrome earns its tap**: top bar = wordmark + `←` + ≤1 action, never the screen name (§2).
7. **Build flow = two-shot**: overview first → Antho device-checks → then subs/lenses.
8. **Lead with the live — placement is rank.** Within a screen/lens: real data/decisions sit under the pills, mixed reached/ahead ladders next, pure countdown/unlock meters last; a section of all-zero marks never opens a lens with a live sibling; a ladder and its countdowns sit adjacent.
9. **Show the reading, not just the conclusion.** Surface the engine's underlying readings, not only its verdict — both beside a conclusion (the deciding reading as row meta: "38% hard · +2", "7.2h avg") and BEFORE one exists (per-item readings render as soon as they're computable, not when a score/verdict unlocks — the recovery panel's ~8 checks read from session one). Below a gate the reading is progress toward it ("3 of 12 rated sets"), never "n/a". A panel of named checks each with its own reading is data, not §12 repetition; each lens stands on its OWN data without copying another's marks (§4.3). "Not enough data yet" is a last resort; when a reading panel arrives, any row elsewhere that only restated it goes. A dense panel may CLOSE with one muted line naming what the readings feed (recovery checks → "a deload is called when enough cross their line") so the numbers aren't unexplained.
10. **A checklist is not a section.** A run of >~4 uniform dot-text rows is the "AI look" — redraw as ONE mark + a single focused detail (the 9-row milestone ladder → a segmented rail "2 OF 9" + only the NEXT milestone). Rows earn a list only when each carries a distinct reading/action. Adjacent road-ahead content shares one visual language (all bars).

## 5. Color (`Color.kt`; via `MaterialTheme.colorScheme`, never raw vals)

Pearl (dark default): bg `#0E0E11` · gradient `#131318→#090909` (behind every screen) · surface
`#15161B` (sheets) · surfaceVariant `#1C1D24` (interactive tile fill) · outline `#2E2E38` · onBg
`#EEEEF2` · muted `#B4B4C2` (AA-safe to 0.7). AMOLED: bg black, surface `#080808`, surfaceVar
`#111111`, gradient `#000000→#050507`.

Accent = user-picked (Navy `#3D4F73` default · Red `#8B3535` · Olive `#4D6040` · Gold `#7A6435`);
`primary`=accent, `primaryContainer`=@0.15, `secondary`=@0.6. Design against muted navy — needing
a vivid accent means too much accent. Accent can be **disabled** (Appearance → monochrome): `primary`
falls back to a near-white neutral (onBg) so highlights stay legible and distinct without colour, and
`onPrimary` flips to bg for any light/neutral accent (so a filled-primary control never goes same-on-same).

**Intensity ladder — the only allowed alphas** (snap strays when editing):

| Rung | Use |
|---|---|
| accent 1.0 (`primary`) | chart strokes/dots, selected-pill border, `action →`, ↑ delta, legend dots, active nav |
| accent 0.6 (`secondary`) | wordmark dot, secondary chart series |
| accent 0.15 (`primaryContainer`) | tonal fills: selected pill bg, active-row wash |
| onBg 1.0 | primary text, serif figures |
| muted 1.0 | secondary text, mono labels, ↓ delta |
| muted 0.65–0.7 | captions, deselected pill text (floor — never dimmer) |
| outline 0.35 | borders on unselected controls |
| outline 0.25 | hairlines (`EditorialHairline` applies it) |

Bars: fill `primary` 1.0 on track outline 0.25–0.35. Error mirrors accent (full text/stroke,
container 0.15). **Reserved**: PR gold `#E3B341` (PR star + gold set row only) · `△ LAST` green
`#5BC873` · success `#4CAF7D` / warning `#CFAB47` / error `#BF4040` for true states, never decoration.

## 6. Type (`Type.kt`) — three voices; choosing the voice is choosing the meaning

| Voice | Styles | For | Never |
|---|---|---|---|
| **Serif** (tnum) | display 52 · headlineL 36 · headlineM 28 · headlineS 22 | THE one big thing per section: page titles, hero figures | prose, buttons, labels |
| **Sans** | titleL 18M · titleM 16M · titleS 14M · bodyL 16 · bodyM 14 · bodyS 12 | row titles, prose, button text | section anchors, big numbers |
| **Mono** (letter-spaced) | labelL 13 · labelM 11 · labelS 10 | UPPERCASE micro-labels: section headers = labelL 13 (`EditorialHeader`), figure captions + meta = labelM/S | sentences, titles |

- Mono labels `.uppercase()` in code; only off-scale sizes: 8–9sp figure captions.
- *Italic* = the aside voice (wordmark, coach one-liners, taglines): bodyM/S italic, muted.
- One serif hero per screen, everything else steps down; animating numbers use tnum styles.

## 7. Spacing & shape

4dp grid (steps 2–28); page gutter **24dp**. Section rhythm — air, no rules:
`28 → mono header → 10 → content` (whitespace + header ARE the separator, §1). Within a section:
`header → 2 → caption → 10 → content`; **no two text lines of different roles butt flush** — ≥8dp
between a header/caption/aside/row (put a Spacer(≥8) before the first row; don't lean on the row's
own bottom padding). Figure rows: gap 20, label 2 under number. List/data rows: **ONE vertical
padding for ALL of a lens's rows** (coach shares `COACH_ROW_PAD` = 6, i.e. 12 total) — sibling
sections never mix 4/5/6, the page reads as one rhythm; a text-link inside a row stays at vertical
2 so it doesn't inflate that row above its neighbours. Interactive tile inner padding 14×12. Shapes (`Shape.kt`): 4/8/12/16/24, pills
`RoundedCornerShape(50)`, tiles 12, sheet top 16 — no custom radii. Photos: rounded 16 clips,
captions on bottom scrims, full-bleed strips (Profile cover/filmstrip = reference).

## 8. Components & controls (`ui/common/` — reuse, never re-implement)

`EditorialHeader` (section anchor, mono labelLarge 13sp; onSurfaceVariant + primary) ·
`EditorialHairline` (**data lines only** — chart threshold/table rule, never a section separator,
§1) · `EditorialFigure` (serif number + mono caption + ↑/↓ delta) · `EditorialLegend` · `SegmentPill`
(all filter/lens toggles) · `ForgeSwitch` · `InlineEmptyHint` (§12) · `ForgeWordmark` · `bounceClick` / `bounceCombinedClick`
(latter adds a long-press hook — Home CTA tap = start, hold = skip warmup) ·
`clickableLabeled` (plain tappable + TalkBack label) · `ForgePrimaryCapsule`/`ForgeOutlineCapsule`
(the ①/② capsules below — sheets/editors use these; settings/onboarding wrappers match them) ·
`ForgeShimmer` · `ConfettiOverlay` (only celebration) · `statsEntrance`. A pattern used on a 3rd
screen gets promoted here the same turn.

**Buttons — three levels only**: ① filled light capsule = do-it-now, ≤1/section; ② outlined capsule =
its sidekick; ③ mono accent `action →` = navigation. No M3 default/floating-text/icon buttons in content.
Settings reuse `SettingsPrimaryAction` (do-it-now) / `SettingsOutlineAction` (sidekick) / `SettingsActionLink` (`action →` nav) from `SettingsPrimitives.kt`; group the page-level action buttons at the END of the page, never mid-scroll.
**Per-row action = compact OUTLINED pill, never filled.** A do-it-now action scoped to a single list row/integration (Recovery's Connect) renders as a right-aligned compact OUTLINED pill (`SettingsOutlineAction` weight — border only, onBg text, sentence case) with the WHOLE row as its tap target (the pill is drawn, not independently clickable — no nested tap). NEVER a filled-white capsule per row — five of those stack into a button wall (Recovery failed exactly this way); the ONE filled capsule stays page-level, grouped at the END (e.g. Get/Update Health Connect). A bare mono accent `connect →` link is too dim against a muted accent — prefer the pill. A connected row shows a passive `• ON` (accent disc + mono) on the right, and a list of connectables leads with its filled-disc/muted-ring dot rail (§12; ring at 1.5dp muted@0.55 so the empty state reads on near-black). Rows without a usable action render passive — no affordance that can't run. Coach + Recovery draw this dot and pill through the shared `StatusDot` / `ConnectPill` (`SettingsPrimitives.kt`) — reuse them, don't redraw.
**Sizing — trim, never chunky** (48dp touch from padding, not visual size): hero CTA ~60dp (Home
Start session only); standard capsules **44dp** (14sp); `ForgeSwitch` **40×24** track (thumb 14→17,
press ~20); `SegmentPill` 12×5, 10sp.
**Icons**: chrome (nav/gear/back/share) + a muted leading glyph on settings/list nav rows for
wayfinding. Row/content glyphs come from the matched custom families (`SettingsIcons`/`NavIcons`/
`ExerciseIcons`), never Material stock, never accent-tinted; TOP-BAR chrome may use Material stock
until a custom chrome set lands (content never). Exercise rows in browsers/pickers lead with their
`ExerciseIcons.forEquipment` equipment-class glyph (one glyph per implement class, custom moves =
pencil); elsewhere content is text-first, glyphs `→ ↑ ↓ △ • ·` carry meaning — no decorative
icons/emoji. (Known gap: `CardioType.icon` still mixes Material stock into its custom family.) The
families share only their builder plumbing via `VectorBuilders.kt` (icon/fillPath/strokePath/circle/
roundRect); glyph shapes stay per-family so each still evolves on its own.
**Don't render state twice, and flag only exceptions.** A leading `•` dot is earned only when its
COLOR flags something the eye should catch — a failure (error), a win/active (accent) — never the
neutral/default/inactive majority (a column of identical grey dots is noise). Paint the dot only
for the exception; the common rows reserve the gutter (`CoachFlagDot(null)`) so they still align,
dotless. Likewise a row's right meta is a count or reading only, never a state word the dot or
reason line already carries.

## 9. Motion & feel (`Motion.kt` — never literal durations/easings at call sites)

Durations: Fast 150 · Standard 240 · Emphasized 320 · Draw 900 · Celebration 2200. Easings:
Decelerate-in / Accelerate-out / Standard / DrawDecelerate (chart reveals). Springs: `bouncy()`
(set-log, PR pop) · `snappy()` (pill slide).

- Overview/detail: `statsEntrance(index)` cascade + one-shot `drawTween()` reveals (never re-trigger on scroll); lists light stagger; settings/live no presentational motion.
- **Press = bounce everywhere** (scale 0.97, no ripple; auto-ripple under TalkBack); migrate rippling M3 buttons when touched.
- Feel (don't retune casually): bounceClick MediumBouncy/MediumLow; ForgeSwitch position 0.68/900, thumb resize no-bounce/1400 + press-stretch.
- **Haptics rare** (`forgeHaptic`): set logged, PR/finish, timer ticks — nothing else.
- **Launch**: `AvexIntro` settles the serif "Avex" wordmark once per cold launch over the first screen, then the plate fades to reveal it. The system splash is background-only (`splash_blank`, no icon) so the wordmark is the brand beat; honors reduce-motion (short still hold, no fade). **The intro themes to the chosen app icon through the WORDMARK itself** (`AvexWordmark`): the name has a narrative arc in the icon's palette (`AppIcon.launchPalette`, deep→mid→bright) inside the stock envelope — it ENTERS with the family's verb (Metal sheen-sweep · Gem glint + jagged crystal chunks GROWING out of the letterforms, stroke-scale, staggered (probe shader, 33+) · Aurora northern lights RISING out of the glyph tops, waving + hue-shifting (probe shader, 33+; pre-33 drifting fill) · Nebula weightless float · Molten white-hot heat-shimmer (AGSL `RenderEffect`, 33+) · Solid plate-colour wipe · Stealth slow HUD flicker-in (~14Hz over ~1s) · Default plain), holds legible, then DIES the family's death overlapping the plate fade (Molten MELTS decelerating over 650ms, smooth slump + thin drip streams · Nebula is dragged into a BLACK HOLE vortex, modest twist (33+; pre-33 spin-shrink) · Solid wipes back out · Metal/Gem/Aurora/Stealth fade). Reduce-motion = settled still, plain fade. Full-screen launch SCENES (`ui/common/LaunchScenes.kt`: family→AGSL registry, per-pixel `RuntimeShader`, Aurora/Nebula/Molten/Gem device-approved + Stealth radar) are deliberately UNWIRED (2026-07-10 — every-launch spectacle wears out) but kept intact; re-wire via `IconLaunchScene` behind the wordmark + the 950ms themed hold. This deliberately spends colour off the one-accent rule (§1/§5) for a pre-app moment only. The launcher/adaptive icon **defaults** to the emblem (`ic_launcher_foreground`) but is user-selectable (Appearance → App icon, GYMAP-icons): one `.icon.*` `activity-alias` per icon, exactly one enabled at a time via `AppIconManager` (`PackageManager` component toggle). A settings row shows the current icon + name and opens a `ModalBottomSheet` grid (mirrors `AvatarPickerSheet`: family `EditorialHeader`s, ring the current pick, scroll edge-fade); warns it "updates after a moment" (OEM-launcher lag).

## 10. Charts (overview/detail archetypes)

Stroke + dots `primary`; secondary series `secondary`; area fades to transparent (≤0.15 top); grid =
hairline. Open on the page — no plot frames. Legends `EditorialLegend`; axis labels mono labelS muted.
Values tnum; draw in once with `drawTween()`. Comparison bars thin (3–4dp), rounded.

## 11. Copy, voice & glyphs

**Voice — dry, specific, earned.** Every generated line (coach/milestones/recaps/opinions/
notifications/hints) states a fact or instruction grounded in the user's data, and VARIES with
those numbers rather than repeating one generic cue (a quiet-week line reads the week's PRs / volume
trend / session pace, never the same "keep doing what works" every time); an understated nod only
when earned ("Clean sweep. You beat every comparable set."). Coach speaks imperative + "you",
never "I", never names itself. **Banned in any rendered string**: exclamation marks (confetti is the
exclamation mark); em dashes (split, or join with a comma / " · "); hype/bro-speak ("beast",
"crush"); poster clichés ("moves the needle"); praise ungrounded in data. Offenders rewritten when touched.

**Headlines & verdicts.** Serif hero/title takes NO terminal period ("Coach", "Pull B" — periods
only in body/italic-aside prose). A verdict states what it means FOR THE USER, not internal state
("Ready to coach", not "Baseline set"); if it needs system vocab, reword and let the subline carry the why.

**Translate the machine.** Machine identifiers never render ("2026-W27"→"Week of Jun 29", a status
enum→its word, a slot key→its exercise name). Machine PROSE too: paren-plurals ("3 session(s)"), em
dashes and jargon get rewritten at render, and a stored paragraph → a short derived line ("Baseline
still forming · 3 of 4 sessions"), since pass rows are immutable — the translation lives in the UI.

**Naming state & the future.** A status never stands alone — attach the referent the screen knows (a
held week shows its reason; a pending verdict names its change; a collapsed group names members only
when they fit ONE line, else count alone; a countdown names its landing day). An upcoming item is
worded forward under a `NEXT` eyebrow, never a past-tense achievement label. (Right-meta = count or
reading, never a duplicated state word — §8.)

**Glyphs & numbers.** Mono small-caps section headers, else sentence case (buttons included). Actions
end ` →` / `+ log` (accent mono); meta joins with `·`; deltas `↑`(accent)/`↓`(muted), `△ LAST`;
wordmark = "• Avex" via `ForgeWordmark()`. Numbers k-abbrev ≥10,000 ("4.5k"); RPE/RIR via
`Format.kt` (no ".0"); weights honor the unit setting (never hardcode lb/kg; unit suffix uppercase
only in mono captions). Coach lines = quiet italic asides, never banners.

## 12. Empty & first-run states

**Empty is data at zero — drawn, not written.** A zero-state reuses the SAME vocabulary it shows with
data (bars/meters/sparklines/dot rows) at zero/ghost, never a list of status words ("forming" /
"not connected" repeated per row = barebones and texty at once).

- **Every data section leads with a MARK** (words are its caption, never its content); no section is a bare text row. A section that can only ever be text isn't a section — fold it into a caption.
- **Zero-state = the section's own visual at zero**: forming lift = flat ghost sparkline or "n/2" pip; unconnected inputs = filled/hollow dot row; threshold-gated = progress-to-unlock meter. Plumb the count into the ViewModel — a visual earns the wiring.
- **Collapse repetition**: N rows sharing one empty status → ONE line naming the concrete unlock ("3 lifts building history · first read after two sessions"), never N identical rows. Its ghost mark shows ONLY beside a live sibling mark (contrast reads as "still forming"); an all-ghost group drops the mark (a lone flat line reads as broken). Ghost VISUALS good, ghost DATA (fake numbers) banned.
- **n-of-m meters** measure only a real unlock threshold, and m is the REAL gate (a bar filling to the wrong gate unlocks nothing); stacked gates → show the unfilled one. Never "data availability" jargon ("lifts with a trend: 3 of 8" promises a chart it doesn't show) — render items that HAVE data with their real trend, ghost-collapse the rest.
- **A mark needs visual mass at the data's REAL size**: a strip of tiny cells below ~a row reads as debris; at small counts use list rows. Design each section at its emptiest realistic state, not its fullest.
- **Figures show honest zeros** ("0 WORKOUTS"), never a dash, never hidden.
- **`InlineEmptyHint` is the last resort**, ≤1 per lens, only where there's no zero-shape; it REPLACES the caption (never both); terse, no em dashes. Boxed `EmptyState`/`FirstTouchTip` deprecated.

## 13. Inputs, loading & feedback

- **Text inputs** (interactive → bordered): `OutlinedTextField`, unfocused = outline rung, focused = accent, muted placeholder (placeholder text is a ghost affordance — it may dim below the §5 muted floor); search = leading magnifier + trailing clear, either bordered (History `SearchField`) or a filled rounded field (surfaceVariant — the standard phone-search look, Settings + timezone picker). Hot-path numbers = steppers + inline edit, never keyboard-first.
- **Loading**: local DB is instant — no spinners/blocking loads ever; screens appear with the entrance cascade; `ForgeShimmer` only for real latency (photos, Health Connect).
- **Feedback**: undo over confirm — reversible acts get a short Undo snackbar ("Set logged · Undo"); dialogs only for destructive/irreversible acts (wording the consequence); no success toasts for what the UI already shows; errors = quiet inline line in error color, never banners.

## 14. Settled — do not reintroduce / do not touch

**Removed on purpose**: boxed cards for passive content · full-screen PR takeover (PR = confetti +
gold row) · accent-tinted "important" prose · session-summary extras (share card/tags/ghost/vs-last/
what's-next) · cardio big-number hero · cardio kcal estimates (return only with real watch burn data) · new gamification surfaces (wait-listed) · Profile
identity-first restructure · mood/subjective coach drivers · Coach hero week-dot calendar, "Pulse",
pass-square record strip · Coach status serif verdicts AND status/anticipation asides (status states
= eyebrow + figures) · Coach pre-baseline signal dot-checklist in the hero (→ one labeled Baseline
bar in the "Coming up" idiom; the effort/HC inputs it spelled out live in Signals only, §4.3,
GYMAP-24) · hairline section separators (§1) · the 9-row milestone ladder (→ rail + next, §4.10).
**Facts**: dark-only (Indigo light scheme in `Color.kt` unshipped — never build light variants);
portrait phone only (no adaptive/tablet/landscape).
**Untouchable**: live-session screen · Profile blending cover (compositing approach; edge-fade/scrim
stops retuned 2026-07-09) · statsEntrance/draw tuning · `BodyAnatomy.kt` (generated).
**Known defects, fix when touched**: Home's GOALS section placement · Stats' 16dp gutter (§7 says 24 —
left because re-flowing the polished screen needs Antho's eyes) · any screen still drawing section
hairlines → migrate to air rhythm (§7). (Home's accent eyebrows + the Home/Stats section hairlines
were fixed 2026-07-08, GYMAP-4.)

## 15. Checklist before calling UI work done

- [ ] Right archetype toolkit (§3); no borrowed patterns.
- [ ] Overview answers state + conclusions + exceptions at zero interaction, ≤2–3 viewports.
- [ ] No prose behind taps, no bare links; aggregate visuals tap to detail; nothing fake-tappable. Top bar = wordmark + `←` + ≤1 action, never the screen name, one back arrow (no in-page duplicate). Serif titles/verdicts no terminal period.
- [ ] Passive content bare; only interactive elements + modals get fills/borders.
- [ ] One serif hero; mono headers + air rhythm, **no section hairlines** (lines = data only); alphas only from the §5 ladder; colors via `colorScheme`; ≥8dp between text lines of different roles.
- [ ] Controls trim (§8); bounce press no ripple; touch ≥48dp + TalkBack labels.
- [ ] Motion per archetype via shared modifiers only (§9).
- [ ] Empty = data at zero, drawn (§12): every section leads with a mark at zero/ghost, repeats collapse to one, ≤1 hint/lens, never a bare text row or caption+hint.
- [ ] Prose budget + one home (§4.3): data-led sections, ≤1 caption, no mechanics narration, no fact/mark repeated across lenses.
- [ ] Generated text: dry + earned, imperative + "you", no exclamations/em-dashes/hype (§11).
- [ ] Inputs/loading/feedback (§13); shared primitives reused, 3rd-use promoted; screens ~300 / VMs ~150 lines as a guide (split at seams, don't contort to hit it); §2 map updated; overview shown before subs.
