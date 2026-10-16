---
name: Relay native
description: Warm near-black chrome in Geist around the unchanged screen-black terminal wall.
colors:
  ground: "#131211"
  pane-idle: "#161513"
  pane: "#181715"
  surface: "#1A1917"
  raised: "#1C1B19"
  selected: "#1F1D1B"
  track: "#2A2826"
  line-subtle: "#211F1D"
  line: "#242220"
  line-strong: "#262422"
  line-emphasis: "#33302D"
  line-focus: "#3A3733"
  ink-dim: "#6E6A64"
  ink-3: "#8C877F"
  ink-2: "#B5B0A8"
  ink: "#EDE9E2"
  screen: "#08090a"
  live: "#2ec469"
  held: "#e5382e"
  waiting: "#f0a828"
typography:
  ui:
    fontFamily: "Geist, sans-serif"
    fontSize: "13px"
  body:
    fontFamily: "Geist, sans-serif"
    fontSize: "14px"
  caption:
    fontFamily: "Geist, sans-serif"
    fontSize: "12px"
  mono:
    fontFamily: "Geist Mono"
    fontSize: "11.5px"
  brand:
    fontFamily: "Sora"
    fontSize: "17px"
    fontWeight: 600
    letterSpacing: "-0.03em"
  terminal:
    fontFamily: "Fira Mono"
    fontSize: "10pt"
rounded:
  sm: "7px"
  md: "10px"
  lg: "14px"
  pill: "999px"
  plate: "0px"
spacing:
  gutter: "2px"
  small: "4px"
  medium: "8px"
  record: "12px"
  page: "20px"
components:
  button:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.md}"
    padding: "0 10px"
  button-primary:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.ground}"
    rounded: "{rounded.md}"
    padding: "0 14px"
  button-quiet:
    backgroundColor: "transparent"
    textColor: "{colors.ink-2}"
    rounded: "{rounded.md}"
    padding: "0 10px"
  icon-key:
    backgroundColor: "transparent"
    textColor: "{colors.ink-3}"
    rounded: "{rounded.sm}"
  field:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.ink}"
    rounded: "{rounded.md}"
  navigation:
    backgroundColor: "transparent"
    textColor: "{colors.ink-2}"
    rounded: "{rounded.md}"
  segmented:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink-3}"
    rounded: "{rounded.md}"
  terminal-plate:
    backgroundColor: "{colors.screen}"
    textColor: "{colors.ink}"
    rounded: "{rounded.plate}"
---

# Design System: Relay native

## Overview

**Creative North Star: "The Multiviewer"**

Relay is a compact console surrounding a wall of agent terminals. Screen-black
plates, narrow gutters and persistent top identity strips make sessions
readable at a glance. In Operate mode, terminal work and attention states lead.

Visual authority is Relay-2 at `1745dd3f68b7bc786f48142ad7da591537aeb057`, especially
its `theme.ts`, `TerminalPane.svelte`, `AgentsWall.svelte`, `Icon.svelte` and
shell components. Rendered component anatomy takes precedence over stale prose. The user
explicitly requested this identity and layout; V3's UI is not the reference.
The native theme lives in `apps/relay-native/src/theme.css`, with runtime palette
and wallpaper application in `shell.rs`. This records the implemented GTK4
surfaces; it does not certify complete feature parity, visual fidelity or device
runtime behavior. Validation and remaining coverage belong in
`docs/VERIFICATION.md` and `docs/PARITY.md`.

**The 2026-10 rework (Antho's mockup and token sheet).** Everything around the terminals is
warm near-black chrome in Geist: rounded keys, fields and rows (10px), pills for small toggles,
a search-field palette key with a Ctrl K keycap, the mark (three lanes staggered like a relay's start, the lime signal broken away ahead of the front one) and Sora wordmark, a segmented
view switch and a mono status bar. **The terminal panes are not part of it.** Their plates,
identity strips, lamps, metadata, gutters, Fira faces and screen colour stay exactly as below;
Antho asked for them to be left alone. The Dashboard page was removed at the same time.

**Key Characteristics:**
- Screen-black square plates with top session identity strips (unchanged).
- Warm near-black chrome, off-white primary keys and state color only where it means state.
- Geist for the interface, Geist Mono for figures, keycaps and code, Sora for the wordmark,
  Fira Mono (and Fira in the pane strips) for the terminals.
- Native GTK controls, VTE terminals and GtkSourceView source editing.
- Sidebar navigation, worktree tools and persistent note tabs/satellites.
- Matte, Dark and OLED palettes with optional wallpaper controls.

## Colors

Frontmatter records the default Matte palette; names below describe its roles.
Dark and OLED modes replace the neutral palette through the same semantic roles.
Live and held colors retain their meanings in every mode.

### Primary
- **Lit Ink** (`ink`): primary text and primary action fill.
  Primary keys use `wall` text and brighten to white on hover.

### Secondary
- **Live Green** (`live`): running session lamp.
- **Attention Red** (`held`): held session lamp and session name;
  also the notice underline.
- **Waiting Amber** (`waiting`): waiting states. Starting and restorable session
  lamps, the mirror's waiting lamp, review and pending markers, unsaved notes and
  the guardrail prompt's top rule use it. All terminal frames remain neutral.

### Neutral
The sheet's names, and the CSS token each one is (`theme.css`; the first ten change per mode
through `fonts::PALETTES`, Dark being the sheet itself, Matte a step lighter, OLED on black):
- **Ground** `@wall`/`@console`: app background, bars, sidebar. **Surface** `@slab`: keys,
  cards, pills, popovers. **Selected** `@wash`: hover and selected rows. **Raised** `@raised`:
  fields and the search key. **Track** `@track`: bar tracks, the lit segment, the avatar.
  On a page these give way to glass (Elevation & Depth, The Glass Rule).
- **Lines**: `@line_subtle`, `@edge` (line), `@strong` (line-strong), `@line_emphasis` (keycaps),
  `@line_focus` (focused fields).
- **Inks**: `@ink`, `@secondary` (ink-2), `@faint` (ink-3), `@ink_dim`.
- **Screen** `@screen`: the terminals and the code editor only, at each mode's earlier value.
  The terminal strips draw with the mode tokens as before.

**The Tally Rule.** Chrome color communicates state. Primary actions stay off-white; ANSI and syntax colors remain inside their content surfaces.

## Typography

Geist carries the interface (13px controls and rows, 14px nav and body, 12px captions; 400,
500 for buttons and toggles, 600 for names and the primary key). Geist Mono carries figures,
keycaps, paths and the status bar (11.5px). Sora 600 is the wordmark only: "relay" at 17px in
the title bar, 46px on the start screen. All four are bundled (`resources/fonts`, SIL OFL) and
registered for Relay alone. VTE requests Fira Mono at 10pt by default, adjustable from 8pt to
24pt in Settings, and the terminal strips keep Fira Sans and Fira Sans Condensed. These are
Pango point sizes, not CSS pixels. GTK owns scaling and metrics.

Section labels in the sidebar read in sentence case ("Workspaces"). Session names are 13px
semibold. The Code editor's GtkSourceView sets Geist Mono at 12.5px
(`.code-source`) and the bundled Relay scheme (`resources/relay-editor.xml`,
Relay-2's syntax colours over Adwaita-dark), installed as `relay-matte`,
`relay-dark` and `relay-oled` with each mode's background.

## Layout

The sidebar requests 248px. The top bar has a 52px minimum height, the status
line 30px and toolbars 38px; native theme metrics can increase these minimums.
Task and module details occupy the content page. The command palette is centered
inside the app and window presets open below the title bar. Only Notes and the
emulator use separate app windows.

The default window is 1440 by 900. Pages and the right-side launch sheet use
20px insets; the sheet overlays the wall at 780px including its insets. It does not shrink
the terminal wall. A scrim blocks background actions and Escape dismisses an idle sheet.

Navigation lives in the sidebar (Board, Skills, Plugins, Notes), followed by projects grouped
under workspaces. The footer is you: an initial in a round avatar, your first name and the gear
that opens Settings. The top bar carries identity and utility controls,
without a second row of page tabs. The bottom status bar exposes resource and
device tools.

Grid mode uses square terminal plates with 2px gutters and one, two or three
columns. The two-column wall has a draggable divider. Focus shows one session
with session tabs; Review places the focused session beside the remaining
stack. Plates request at least 280 by 280 and the wall scrolls vertically.
Sessions can be reordered or focused within the wall. Layout selection, order
and split position can be saved. The wall has no automatic breakpoints.

The Threads space's pages follow their own width instead, so they never hold the window wider
than the person made it: their scrollers keep their content's width to themselves, and the page's
measured width places the content. On the Threads page the conversation, its message box and a new
thread's greeting share one centered column of at most 720px between 28px gutters (16px under a
640px conversation); under 640 and 420px the message box's "Edits with Undo" line and its effort
picker step aside in turn, and under 600px the starter questions stack in one column. The Tally panel opens at 340px (300px at the least) and steps aside on a page narrower than
1000px; once the person shows or hides it with its key, that choice holds whatever the width.
Tally's pages center their content in at most 1200px: under a 980px page their side-by-side cards
stack, under 900px a table's less needed columns (Holdings' price and weight) hide, and under 760px
a strip's figures stack with their rules hidden, as do rows of keys and filters.

Code uses resizable file-tree, editor and Git regions with a worktree selector.
Its tree split starts at 240px; file and Git regions can be hidden. Notes uses a
list beside a scrollable native tab strip and editor. Tabs identify project and
note, can reorder, and can open in separate windows.

## Elevation & Depth

Depth comes from tonal surface steps and thin dividers. Custom buttons and the
titlebar suppress shadows. Compact in-app panels use a thin edge and a restrained
shadow; OS file choosers keep their native decoration. Launch reveal duration is zero and VTE cursor blinking is off.

Optional PNG/JPEG wallpaper fills the window with cover sizing, beneath a black
dim layer. Settings exposes a wallpaper library, panel opacity, wallpaper dim
and content contrast. Increasing content contrast raises effective panel
opacity. Chrome (the top bar, the status bar and the sidebars: the app's, Notes'
library and title bar, Settings' categories) shows the wallpaper through one
console layer at panel opacity; the top bar sits inside the wallpaper backdrop
like the status bar, in Notes too. Everything else is a page, about 5 points
denser, and every page is exactly as see-through as a terminal: a terminal
plate tops up the agents view's wall layer with `@plate`, and every other page
(Board, Skills, Plugins, Files, Git, the Notes paper, full-page panels) paints
one `@page_fill` layer of the same coverage, with nothing nested repainting.
VTE's own ground is clear, and a parked session's slate hides its stale screen.
The code editor stays a step denser for legibility. The Matte default uses
fully opaque panels, which keeps the screen-black plates.

**The Glass Rule.** What sits on a page is glass: a veil of `@ink` over the
page's one layer, never a colour token (`@slab`, `@wash` and `@raised` are near
or fully opaque, and over `@page_fill` read as solid blocks). One scale by role
(`theme.css`, the Skills page's Instructions | Projects tabs its reference):
`@glass_well` (3%) groups controls — a segmented trough, a board lane;
`@glass_card` (4%) is a card — stat tiles, facts, a list of rows, a field;
`@glass_control` (5.5%) is a control at rest — a key, a dropdown, a task card;
`@glass_hover` (7.5%) and `@glass_selected` (9%) answer the pointer and mark the
chosen segment, row, count or track. Edges go `@glass_edge` (7%) for wells and
cards, `@glass_line` (11%) for controls, `@glass_line_hover` (18%). Only what
floats over content — menus, popovers, the board's quick-create sheet and drag
rail — is opaque, in the `_solid` twins. Board, Modules, task pages, Skills,
Plugins, the agents view's crumb and tabs, the Dev | Threads switch and Tally's pages (with
Avex's and Arbiter's, which share their cards) follow it.

The status bar's usage meters show, per enabled provider, the 5-hour, weekly
and Fable windows chosen in the limits popup, with amber at 70% and red at 90%,
and faint once a window has reset since its report. Beside them sit when the
limits were last read and a refresh key.

## Shapes

Keys, entries, navigation and project rows use the 10px radius; icon keys 7px; popovers 12px;
panels, sheets and cards 14px; small toggles, switches and badges are pills. Terminal plates and
their strips stay square. Terminal boundaries and strip separators are neutral 1px strokes
in every session state. Live and held lamps carry the tally color; held session
names also turn red.

## Components

### Buttons
Keys have a 28px minimum height (the title bar's 34px). Ordinary keys use surface with a
line-strong stroke, the primary key ("New agent") uses ink with ground text, and quiet keys are
transparent with ink-2 text. Hover uses wash except for
primary keys, which brighten to white. Keyboard focus draws a 2px outline of ink
at 35% alpha just outside the key (`outline-offset: 0`). Disabled keys use 40%
opacity; a few compact controls dim further or less (0.32 to 0.55).

### Inputs / Fields
Entries use raised fill, a line-strong stroke, a 10px radius and a 30px minimum height. Focus
moves the stroke to line-focus with a faint ink ring. The palette key is drawn as a search
field: glass, "Search or run a command", and a Ctrl K keycap. Native text views have 8px padding and ink carets.
GTK supplies editing, selection and accessibility semantics.

### New agents sheet
The launch sheet (`launch.rs`, `css/launch.css`) reads top to bottom: a heading naming the project
and base branch; one segmented choice between separate worktrees and a review group; then a crew
rail beside the chosen agent's settings; and a footer stating how much work goes out, with a Ctrl
Enter keycap and the primary key ("Launch agent", "Launch 3 agents", "Launch review group"). The
crew is built with "Add an agent" and each row's remove key, not a count: six at most, a review
group two or three, its last member the reviewer. Rail rows carry a number badge (lit ink when
chosen), a name and provider facts. Settings sit in a caption column: provider tiles with a radio,
segmented role and effort (lit with track, like the view switch), the model field, then one folded
line for instructions, worktree and permissions that summarizes them. Tasks are single rows; a
ticked row's badge shows its place in the queue, since the first becomes the agent's current task.

### Navigation
Sidebar navigation (36px rows) selects with the selected fill and ink. Project rows have a 34px
minimum height and select with the selected fill, no rail; counts are plain Geist Mono. Above
the wall, the project crumb is a surface pill and Agents, Editor, Files and Git are one
segmented control whose showing view is lit with track. Focus-session tabs use
neutral wash for selection. The main window draws its own minimize, maximize
and close keys in the top bar (`.window-control`: transparent, wash on hover,
held red when hovering close). The Notes window styles its own titlebar
controls, menu bar and popover menus (`css/notes.css`), and the sidebar row,
board, Git and branch menus carry their own popover styling.

### Icons
`apps/relay-native/src/icons.rs` carries the pinned Relay-2 SVG path geometry.
Shared shell/action icons use a 16-unit view box, 1.5-unit strokes and rounded
stroke caps and joins, rendered by GTK's SVG paintable. Preserve that geometry
when adding native controls. File-type glyphs in the explorer and Git panel are the
one exception to neutral chrome: like syntax colour, they describe content, so each
carries a muted per-language tint (`css/git_files.css`), and Git status letters use
VS Code's status colours. Native widget fallback icons remain GTK-owned;
matching path geometry alone is not proof of complete rendering fidelity.

### Records / Containers
Records are flat square slabs with 12px padding. Board lanes use console fill
and 8px padding. Notices use slab, ink and an attention-colored bottom rule.

### Terminal plate
VTE owns rendering and scrolling. Terminal margins are 10px left, 2px right,
8px top and 4px bottom. The top identity strip uses console fill, 8px side
padding and a 26px minimum height, with a neutral bottom separator. A colored
lamp leads the session name, provider/role metadata, state and compact actions.
Session names ellipsize in the middle. Identity remains visible while terminals are
busy. VTE supports Ctrl+Shift+C/V, preserves scroll position on output and
scrolls on keystrokes.

### Code and notes
GtkSourceView provides the Code gutter, highlighting, find/replace and native
text editing. Worktree selection scopes the file tree, content search, file
operations and Git controls. Read-only comparisons sit beside editable source;
staging, commits, history and branch tools stay in the Git region.

Notes is a desktop text editor in the KWrite mould, in its own window. Its compact
titlebar carries a File/Edit/Search/View menu bar (menus show their shortcuts)
and the open note's name; below it sit a toolbar, a resizable library (search,
pinned and dated rows, sort, a context menu) and tabbed GtkSourceView documents
with Markdown highlighting and list continuation. A find/replace bar (match case,
whole words, regular expressions, "3 of 12"), go-to-line and a status bar (save
state, line and column, counts, Markdown/plain, encoding, line endings, zoom,
autosave) complete each document. Open drafts remain mounted in tabs, and each
project's open tabs are restored. Saving is explicit by default: unsaved tabs show
a dot and closing one asks Save / Don't Save / Cancel. Autosave is an opt-in
preference that saves through the same conflict-checked update after a pause and
when the window hides. A change made elsewhere reloads a clean tab and raises a
notice on a dirty one. Plan uses the same editor with its title fixed. Preserve
project identity when a tab outlives navigation.

### Native adaptations
Mail composition remains mounted while messages refresh. Message/error text is
selectable. Approval controls distinguish task approval from permission to
execute a held action. Task details, skills, settings, launch profiles and
device tooling use native controls. Imported roadmap documents are future
requirements, not evidence that every corresponding surface is complete.

## Do's and Don'ts

### Do:
- Do preserve the square terminal plate anatomy; the warm rework stops at its edge.
- Do keep session identity visible above terminal content in its top strip.
- Do use off-white primary keys and state color only where it has meaning.
- Do preserve native keyboard focus, selection and clipboard behavior.

### Don't:
- Don't add decorative accent colors or idle animation; pills are for small toggles and
  badges only. The
  guardrail prompt is the deliberate exception: as the one floating surface it is
  rounded (10px card, 6px keys, round icon and "more" keys), shadowed and fades
  in and out (`css/guardrails.css`). The adb lease card (8px) and the mirror's
  device screen (12px) are rounded too.
- Don't copy web layout rules into GTK without checking native behavior.
- Don't treat imported roadmap surfaces as implemented UI.
- Don't claim full parity, sustained performance or complete accessibility from screenshots alone.
