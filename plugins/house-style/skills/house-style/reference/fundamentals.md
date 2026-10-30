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
  about 1.4 to 1.5 for body text and 1.1 to 1.25 for headings.
- **Figures that change or line up use tabular numerals.** Amounts, timers, counts in a column.
- **Sentence case everywhere** (titles, buttons, menu items). Uppercase only for tiny letter-spaced
  labels, and only if the project already uses them.
- **Truncate chrome, never content.** A tab label may ellipsize. A person's name, note or amount
  wraps instead. Middle-ellipsize paths and session names (the end is what tells them apart).

## 3. Spacing and layout

- **One spacing scale on a 4-unit grid** (4, 8, 12, 16, 20, 24, 32, 40, 48). Off-grid values are
  either named exceptions or bugs.
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

## 9. Motion

- **Motion explains change** (where something came from, went to, what just changed). It is not
  decoration, and nothing idles or loops in an Operate surface.
- **Durations:** 100 to 150 ms for small feedback (press, toggle), 200 to 300 ms for moves and
  reveals, 300 to 400 ms for large surfaces. Celebrations and intros can be longer, once.
- **Ease out when entering, ease in when leaving,** springs for direct manipulation.
- **Stagger related items** (30 to 60 ms apart) so a group arrives as one gesture.
- **Animate transform and opacity,** not layout. Never animate something the person is reading or
  typing into.
- **Respect reduce-motion:** everything settles to its final state instantly. Meaning never depends
  on motion.
- **Intros and showcases** (a startup beat, a release video) may be expressive: short, layered,
  staggered, with one idea, then out of the way.

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

## 11. Icons

- **One family, one grid, one stroke weight** (for example a 16-unit grid, 1.5 to 1.75 stroke, round
  caps and joins). A stock icon dropped beside custom ones shows.
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
