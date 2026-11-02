# Fundamentals

The general craft every screen needs, whoever it is for. The owner's rules (`../SKILL.md`) sit on
top of these and win where they differ. Each rule is worded so a screen can fail it.

## 1. Hierarchy: one answer per screen

- **Name the screen's one question** before drawing anything ("how much is left this month", "which
  agent needs me"). The biggest, highest-contrast thing on the screen answers it. If you cannot name
  the question, the screen is two screens or none.
- **One hero.** One element per screen carries the largest type or figure. Everything else steps
  down at least one rung of the type ramp. Two heroes cancel each other.
- **Rank by placement.** Top-left (top-start) is read first, then down. Live data and decisions go
  first, history and settings last. What people use daily never sits below what they use monthly.
- **Wayfinding.** Every screen answers: where am I, where can I go, what's here, how do I get out.
  Never trap anyone.
- **Map controls to what they change.** Put a control beside the thing it affects and arrange
  controls the way their effects are arranged. If a control needs a label to explain what it does, the
  mapping is weak.
- **Familiar beats novel.** Things that look the same behave the same and live in the same place.
  Break a familiar pattern only when you can show the new one is better.
- **Simple is not minimal.** Hiding everything in one place looks minimal but isn't simple. Show the
  common path first and keep advanced options one level deeper. Sometimes added context is what
  simplifies (a scrubber that shows the time left).
- **Squint test.** Blur the screen (or look at a 25% thumbnail): you should still see the hero, the
  primary action and the groups. If everything is the same grey texture, hierarchy is missing.
- **Five-second test.** Someone seeing it for the first time can say what the screen is for and what
  to do next within five seconds.

## 2. Type

- **One family for the interface, one mono for figures and code, at most one display face.** A
  fourth face is noise.
- **A ramp of five to seven sizes, used everywhere.** Never a one-off size at a call site. Typical
  desktop ramp: 11.5 / 12 / 13 / 14 / 17 / 22 / 28+. Typical phone ramp (sp): 10 / 12 / 14 / 16 /
  18 / 22 / 28 / 36.
- **Weight ranks before colour, size ranks before weight.** Use 400 for reading, 500 for controls,
  600 for names and the primary key. 700+ only for display.
- **Line length 45 to 75 characters for prose,** 720 px (or dp) is a good column cap. Line height
  about 1.4 to 1.5 for body text, 1.1 to 1.25 for headings, and 1.02 to 1.08 for display sizes.
- **Tracking depends on size.** 0 under 16 px (+0.01em is fine for small muted text), -0.02em from
  40 px, -0.03em from 56 px. Never negative on small text: a small size is made more legible by being
  bigger, a little heavier or slightly looser. Over translucent surfaces, text gets more contrast and a
  touch more weight.
- **No italic for emphasis.** Emphasis is weight or colour.
- **Respect the person's text size.** Size web type in `rem`, and size spacing that should grow with
  text in `em`.
- **Figures that change or line up use tabular numerals.** Amounts, timers, counts in a column.
- **Sentence case everywhere** (titles, buttons, menu items). Uppercase only for tiny letter-spaced
  labels, and only if the project already uses them.
- **Truncate chrome, never content.** A tab label may ellipsize. A person's name, note or amount
  wraps instead. Middle-ellipsize paths and session names (the end is what tells them apart).

## 3. Spacing and layout

- **One spacing scale on a 4-unit grid** (4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 80, 96, 128).
  Off-grid values are either named exceptions or bugs: a 2-unit nudge inside a small control and an
  odd value that exactly centres something are fine, while `mt-[13px]` is not.
- **Padding follows density:** about 16 inside compact panels and sidebars, 20 to 24 in cards, 24 to
  32 at page edges. Gaps: 4 to 12 inside a control, 2 to 8 between rows, 12 to 24 between groups, 32
  to 64 between sections.
- **Navigation is the densest part of an app:** rows 32 to 36 tall with 2 to 4 between them, a
  section label 24 above its items and 8 below. Row heights: 32 compact, 36 default, 40 to 44 for
  touch. Measure the sidebar after any change, because that's where spacing breaks first.
- **Proximity groups, air separates.** Related things sit closer together than unrelated ones.
  Space between groups is at least twice the space inside a group. Reach for a divider only when the
  space is already right and something still needs separating.
- **One row height and one row padding per list.** Siblings never mix 4, 5 and 6.
- **Align to a few edges.** Every left edge on the screen should line up with one of two or three
  columns. Count them. Each stray edge reads as sloppiness.
- **Containers hug their content.** No fixed heights on anything holding text: text grows (font
  scale, translations, long names) and the box must grow with it.
- **Design the emptiest and the fullest realistic state,** not the demo state. Longest real name,
  zero items, 500 items.
- **Density matches the job.** A tool used all day (terminal wall, ledger, board) is dense. A page
  read once a week (a summary, onboarding) breathes. Do not make a dense tool airy to look modern.

## 4. Colour

- **Neutrals carry the interface, colour carries meaning.** Build it in greys first. Add colour
  only where it says something: state (live, waiting, error), the one accent for what asks for you,
  data series.
- **One accent, spent in few places at large size.** Many small accent dots read as dead pixels.
- **State colours keep one meaning across the whole product.** If green means live, it never also
  means "selected".
- **A tonal ladder, not ad-hoc greys.** Ground → card → control → hover → selected, each a fixed step.
  Use the project's named tokens; never a raw hex at a call site.
- **Dark themes:** near-black (#070707 to #171614), not pure black, unless it's an explicit OLED
  mode. Off-white ink (#EDEDED range), not pure white. Pick warm or neutral and keep it. Never
  blue-grey by accident.
- **Data may be colourful.** Charts and categories get a real palette. Chrome stays neutral. Do not
  force charts into black and white to match the chrome.
- **Three text roles** (primary, secondary, muted), not ten slightly different greys. Muted is for
  meta and captions only.
- **The accent covers a few percent of the screen:** the primary action, the selection, one key
  figure. Text on it reaches 4.5:1, so darken it or put dark ink on it. On a dark theme the primary
  button can be an off-white from the ramp with dark text, leaving the accent for a smaller place.
- **Semantic colours are toned to sit inside the palette:** positive `#4cc38a` and negative `#e5484d`
  on dark, `#16a34a` and `#dc2626` on light.
- **No hue gradients.** A gradient is at most a small tone shift inside one neutral (an active row
  from `#2b2b2b` to `#1f1f1f`). No gradient text, coloured glows or coloured shadows.
- **Depth:** on dark, surface steps do it (an active row may carry
  `inset 0 1px 0 rgba(255,255,255,0.12)`). On light, small neutral shadows: `0 1px 2px rgba(0,0,0,.05)`,
  or `0 8px 24px rgba(0,0,0,.06)` for something floating. When space alone can't separate, a hairline
  is the primary ink at 6 to 10%.
- **Translucency only where the project uses it** (Relay's Glass Rule). Heavier material separates
  structure and lighter material marks what's interactive. Never put light translucency on light
  translucency. Bigger surfaces read thicker. A modal task dims what's behind it, while a parallel
  panel doesn't. Where content scrolls under floating chrome, fade or blur the edge instead of
  drawing a divider. Respect reduced transparency.

**A neutral ramp for a project without its own tokens** (apps use graphite or something as grey; a
landing page may tint toward its world):

| Role | Dark graphite | Dark warm | Dark ink | Light graphite | Light warm | Light ink |
|---|---|---|---|---|---|---|
| Deepest (sidebar) | `#0f0f10` | `#12110f` | `#07080d` | | | |
| Page | `#141415` | `#161513` | `#0b0d14` | `#fafafa` | `#f7f4ee` | `#f5f7fa` |
| Card | `#19191b` | `#1c1a18` | `#11141c` | `#ffffff` | `#fdfbf7` | `#ffffff` |
| Raised (active row, input) | `#232325` | `#262421` | `#1a1e28` | `#f4f4f5` | `#efe9df` | `#eceff4` |
| Primary text | `#ededed` | `#efe9de` | `#eef0f5` | `#171717` | `#1f1b16` | `#111827` |
| Secondary text | `#a1a1a6` | `#aaa398` | `#a3a9b8` | `#525252` | `#5e554a` | `#4b5565` |
| Muted text, icons | `#85858b` | `#8a847a` | `#7a8296` | `#737373` | `#7b7266` | `#6b7487` |

## 5. Contrast (measured, not felt)

- Body text ≥ 4.5:1 against what is behind it. Large text (≥ 18.66 px bold or 24 px) ≥ 3:1.
- Meaningful marks (chart strokes, lamps, focus rings, control borders) ≥ 3:1.
- Measure on the rendered pixels, not the token values: translucency, wallpapers and glass change
  the real background. A downscaled screenshot flatters low contrast.
- Distinguish states by shape as well as colour (filled vs ring, solid vs outline). About one man
  in twelve does not see red and green apart.

## 6. Controls and actions

- **One primary action per surface.** Filled, highest contrast. A second filled button is a demotion
  of the first.
- **Three levels only:** primary (filled), secondary (outlined or tonal), quiet (text or icon).
- **Destructive acts are undoable when they can be.** Undo beats a confirm dialog. Confirm only what
  cannot be undone, and name the consequence on the button ("Delete 3 entries", not "OK").
- **Nothing looks tappable that isn't, and everything tappable looks it.** Fills and borders are a
  promise of interaction.
- **One tap target per row.** A row action inside a tappable row is drawn, not separately clickable.
- **Targets:** 44 to 48 dp on touch, 24 to 28 px minimum on desktop with keyboard access to everything.
  Web buttons are 32 to 36 px in navigation and 44 to 48 in a hero, with 16 to 20 of side padding.
- **Feedback on press-down, commit on release.** A control that only responds on release feels dead.
- **Radius by size:** keycaps, tags and small buttons 4 to 6, rows, inputs and buttons 8 to 10, cards
  12 to 16, panels and windows 14 to 20. **Nested radius:** inner = outer − the padding between
  them, never below 4 (a 16 px card with 8 px padding holds 8 px items).
- **Counts and meta are quiet text** (often mono and muted), not glowing pills. Shortcuts are small
  keycaps.
- **Toggling views uses a segmented control or tabs, never a button.**
- **Back is always where the platform puts it** (top-left arrow on a phone page, Escape on a
  desktop sheet). Never two back affordances.

## 7. The states every surface draws

| State | Draw it as |
|---|---|
| Zero | The section's own shape at zero (empty track, unlit grid, honest "0"), plus one line on how to fill it |
| One | A row, not a chart of one point |
| Many | The mark (chart, grid), or rows if each carries its own reading |
| Overflow | A trimmed list and "View all" beside the trim |
| Loading | Nothing for local data (it's instant), a skeleton only where real latency exists and only after ~300 ms |
| Error | One quiet inline line saying the consequence and the fix, raw error in a tooltip or detail |
| Stale / offline | The last reading and its age ("2 h ago"), never a blank |

A state nobody drew is the most common bad UI. Check every one before calling a screen done.

## 8. Copy

- **Say the specific thing.** "3 sessions need you", not "You have notifications". Numbers beat adjectives.
- **Verbs on buttons,** naming the act: "Launch 3 agents", "Save entry".
- **Errors say what happened, what it means and what to do,** in the person's words. Machine
  identifiers (enum names, week ids, slot keys, raw paths) are translated before they render.
- **No exclamation marks, no hype, no praise the data doesn't support.** Dry and plain.
- **One fact, one home.** A fact appears once per screen. A caption repeating the heading is cut.
- **Fewer words.** Every sentence on a screen is read by someone in a hurry. Cut the explainer and
  let the state change explain itself. At most one short caption per section.
- **Titles name the context** (the page, the period: "October 2026"), never a greeting. Navigation
  items are named for what they hold ("Progress", "Library"), not for a vague umbrella ("Home").
- **A change says what it compares to:** "+12% vs last month".
- **Short strings end without a period** (titles, labels, buttons, captions). Placeholders are short:
  "Search", not "Search anything...".
- **Mock data looks real:** plausible names and numbers, never `[X]` or `123,456`.

## 9. Motion

Motion explains a change (where something came from, where it went, what just changed). It is never
decoration, it never moves what someone is reading or typing into, and meaning never depends on it.
Whether something animates, and every curve, duration, spring and stagger, is in `motion.md`.

## 10. Data display

- **Every section with numbers leads with a mark** (bar, sparkline, meter, grid) and uses words as
  its caption. A section that can only be text folds into a caption.
- **Show the reading, not just the verdict.** "Over pace" travels with the amount and the days.
- **Honest zeros.** "0", never a dash or a hidden figure.
- **Charts sit open on the page** (no frame box), with a light grid, labelled axes in the small mono
  voice and tabular values. One idea per chart.
- **A mark needs visual mass at the data's real size.** A row of tiny cells at real volumes reads as
  dirt. Use rows at small counts.
- **A mark and its caption state one unit.** "4/7" above a strip with one lit cell is a bug.
- **Charts have a hierarchy too.** One main chart is clearly the biggest thing, drawn with care (at
  least two thirds of the content width on a dashboard). Other data beside it is smaller and different
  in kind, like a ranked list with thin bars or a short table, never a second chart of the same size.
- **One accent per chart:** the current month, the tracked line or the plan that grew gets it, and the
  rest sit in neutral steps. Several hues only when series must be told apart, and even then one
  leads. Never rainbow bars within one series.
- **The chart type follows the data:** a trend is a line, a comparison is bars, a share is one stacked
  bar or a list with bars, and a single goal is a progress bar.

## 11. Icons

- **One family, one grid, one stroke weight** (for example a 16-unit grid, 1.5 to 1.75 stroke, round
  caps and joins). A stock icon dropped beside custom ones shows. A web project with no set of its own
  uses lucide at 16 or 18 px with a 1.5 stroke.
- **Icons take their text role's colour** (usually secondary or muted), never colours of their own.
  A category that needs colour gets a small status dot.
- **One icon box width in every row** and a fixed 8 to 10 gap to the label, so labels align. Drop
  icons that add nothing, like one on every card title.
- **Optically centred** in their key: check the render. A play triangle or an asymmetric glyph
  needs a nudge.
- **Icons label with text where meaning isn't universal.** Toolbar labels go under the icon, not
  beside it, when there is room for labels at all.
- **No emoji or unicode glyphs as icons** in product UI.
- **Pro quality means consistent:** same stroke, same corner treatment, same visual weight across the
  set, drawn for the size it renders at.

## 12. Accessibility and resilience

- Survive 200% text scale (phone) and 1.5× (desktop) without clipping or lost content.
- Every control has a label for screen readers; every chart describes its value ("4 of 5 sessions
  this week"), not its shape.
- Keyboard: everything reachable, focus visible (a 2 px ring at ≥ 3:1), Escape closes, Enter commits.
- Layout uses start/end, not left/right, so it mirrors for RTL.
- Design against the longest realistic string, and against no data, no network and no permission.
