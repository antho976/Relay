# Design canvas

Relay's answer to Claude Design: an infinite canvas per project where everything — frames, text,
shapes, stickies, pen strokes, pictures — can be clicked, edited and moved, and where the agents in
the project's terminals draw and revise designs alongside you.

## Where it lives

- **Design** sits beside **Agents** in the project's view switch (`editor.rs`). It opens on the
  project's **gallery**: a card per design with a picture of its first page, the terminal it belongs
  to, when it last changed and by whom. Chips filter to **All**, one **terminal**, **Project only**
  or **Deleted**; search and sort (last edited, name, created) sit in the header.
- A design belongs to its project and, optionally, to one **terminal** (session): the agent session
  that made it, or the one it was tied to (the terminal key on the board, or the card's menu). A
  terminal's menu (the sliders key on its strip) has **Designs**, which opens the gallery filtered
  to that terminal; a design started there is tied to it.
- **Delete** moves a design to Deleted with an Undo; Deleted designs come back with **Restore**, or
  go for good with **Delete forever** (the only step that asks first). The engine purges designs
  left in Deleted past the notes' retention.
- **Open in a new window** (the board's window key, or a card's menu) puts a design in a window of
  its own, under the Notes window's compact chrome, to keep beside the terminals.

## The board

- **Dock** (top left): pages and layers, the page menu, then Move (V), Hand (H), Frame (F) with
  its sizes (phone, tablet, laptop, desktop, slide, social post, watch: placed beside the page's
  content), shapes (R rectangle, O ellipse, Shift R triangle, diamond, hexagon, Shift S star, L line,
  Shift L arrow), Text (T), Sticky (S), Pen (P) / Highlighter (Shift P) / Eraser (E), Icon (K: a
  searchable set of 70 interface icons) and Place image.
- **Connectors**: a line or arrow drawn from one shape to another attaches to both and follows
  them; its route is straight, elbow or curved (the toolbar key steps through them), and the
  inspector can detach its ends.
- **Rotation**: drag just outside a corner of the selection to turn it (Shift: 15° steps), or type
  the angle; a turned node resizes along its own sides.
- **Labels**: double-click a rectangle, ellipse or polygon to type text centred inside it.
- **Click selects what is under the pointer**, however deep: a label inside a button inside a card.
  The selection toolbar's first key (or Esc) selects the container; Enter selects inside; Tab walks
  siblings. A press inside the selection drags the selection. On an artboard's empty area a drag
  draws a marquee and a click selects the artboard; its name above it selects and moves it.
- **Selection toolbar** (floats over the selection, as Claude Design's does): for text, the style
  ramp (Display to Code), size, colour, bold, italic, underline, strikethrough and alignment; for
  shapes, fill, border, corner radius and auto layout; for strokes, colour, weight and arrowhead; for
  several layers, align and distribute, group and frame; always **Ask** and More.
- **Inspector** (Ctrl+.): position, size and rotation with Fixed / Hug / Fill rules, auto layout
  (direction, gap, padding, along and across), polygon corners and star, icon and its colour, line
  route and attachments, type (any installed family, weight, size, line height, letter spacing),
  solid or linear/radial gradient fills with their stops, border with solid/dashed/dotted styles,
  corners (one radius or each its own), picture fit, shadow, opacity, layer blur, and PNG export. Number
  fields take arithmetic (`120+16`), step with ↑/↓ (Shift: 10) and scrub by dragging their caption.
- **Pages and layers** (Alt+1): pages to add, rename, duplicate and delete, and the layer tree, top
  layer first, opened along the way to the selection, with hide and lock on each row.
- **Text** is typed in place: double-click (or Enter, or the Text tool, or double-click on empty
  canvas) and a native text view sits exactly over the words, in their font, size, line height and
  colour at the current zoom.
- **Moving** snaps to neighbours' edges and centres with red guides; Shift locks the axis; Alt drag
  duplicates. Dropping onto a frame puts the layer in it without it jumping; in an auto-layout frame
  a drag reorders, with an insertion bar.
- **Resizing** snaps the dragged edges; Shift keeps the ratio (pictures always do), Alt resizes from
  the centre. A text given a width wraps from then on.
- **View**: Ctrl scroll or pinch zooms at the pointer, scroll pans, Space or the middle button
  drags; Shift 1 fits the page, Shift 2 the selection, Shift 0 is 100%.
- **Undo** (Ctrl Z / Ctrl Shift Z) is the engine's: each gesture is one `design.apply`, whose answer
  carries the edits that undo it. An agent's change can be undone the same way from the audit log
  (`audit.undo`), unless its undo is over 256 KiB (deleting a page of thousands of layers), which
  the audit does not keep.
- **Clipboard**: copy and paste layers between designs and windows; a picture or text on the system
  clipboard pastes as an image or a text layer; image files drop in from the file manager.

## Agents

Agents reach designs through the `design.*` ops (BUS.md §10.18), which every builder may call; their
MCP tools are generated from the same registry, with each node field documented in the schema.

- `design.create` starts a design (an agent's is tied to its own terminal), `design.get` reads one,
  `design.apply` changes it: a list of `add` / `update` / `move` / `delete` edits run in order, all or
  none. Nodes are named by ids the agent chooses (`"hero"`, `"hero-title"`), so one call can build a
  frame and its children.
- Auto layout (`layout: "column"`, `gap`, `padding`, `justify`, `items`, and `w_sizing` /
  `h_sizing` of `hug` or `fill`) lets an agent build a screen without measuring text.
- `design.asset.add` stores a picture (PNG, JPEG, GIF, WebP, SVG) and returns the `src` an image
  node names.
- The bundled **Design** plugin (`plugins/design/`, switched on per project from Plugins) gives
  every agent there the `design-canvas` skill — the node model, auto layout, worked examples, how
  to revise exactly what was selected, and the craft that makes a mockup look real — and standing
  rules in its brief. The tools work without it; the plugin makes the results good.
- **Ask** on the board hands the selection to the design's terminal: it types a one-line prompt into
  that terminal naming the design, the page and the selected layers by id, with a PNG of them, and
  Enter. The agent's edits arrive as `design.changed`; an open board reads the design again, glows
  what changed in the live colour and says who changed it.

## Implementation

| Part | Where |
| --- | --- |
| Node model, edits, validation (shared by engine and client) | `crates/relay-bus/src/design.rs` |
| Ops and their payloads | `crates/relay-bus/src/ops/design.rs` |
| Storage and handlers (migration v28: `designs`, `design_nodes`) | `crates/relay-core/src/handlers/design.rs` |
| Scene tree, auto layout, hit testing, snapping, edit builders (GTK-free, tested in CI) | `crates/relay-client/src/design/` |
| Native view: gallery, board, canvas widget, tools, inspector, layers, text editing, Ask, export | `apps/relay-native/src/design.rs`, `apps/relay-native/src/design/` |
| Styles | `apps/relay-native/src/css/design.css` |

The canvas is one custom widget whose snapshot is GSK render nodes: the page's content is built once
into a cached node (text layouts, stroke paths and pictures are cached per node) and each frame only
transforms it by the camera and draws the selection, guides and labels over it, so panning and
zooming cost a transform on the GPU. Pictures decode off the main thread. Edits run on the local
scene at once and go out to the engine one at a time in order; a design another actor changed is
read again once nothing of ours is in flight, and never in the middle of a gesture or typing.
Relay has no webview (PRODUCT.md), so designs are native vector nodes rather than rendered HTML.

## Checking it

`scripts/design-smoke.py` (inside a headless compositor, never on a desktop) starts a disposable
engine, writes fixture designs through `design.apply` as an agent would, and drives the real client
(`design/drive.rs`, `RELAY_SMOKE_DESIGN_SCRIPT`): it captures the gallery, the board, a selection,
the layers panel and typing in place; runs gestures and commands (nudge, a snapped drag, editing a
text, drawing a rectangle, undo and redo, group and ungroup, duplicate, delete) and asserts that the
client's scene equals the engine's; types an Ask into a fixture terminal whose fake agent edits the
layer, and asserts the open board shows it; places a picture and pastes between frames; and times a
1,721-node design (`RELAY_DESIGN_TRACE`). `scripts/native-smoke.py` captures the Design view among
its pages. The engine side is `crates/relay-core/tests/design.rs`; the scene, layout, hit testing,
snapping and edit builders are tested in `crates/relay-client`.

