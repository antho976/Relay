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
- **Videos** follow the designs in the gallery, under the same chips and search: videos agents write
  as code (Remotion) and Relay renders, each opening on a board of its own with its player, props,
  renders and versions. They come with the Video plugin; `docs/VIDEO.md` has the rest.

## The board

- **Dock** (top left): pages and layers, the page menu, then Move (V), Hand (H), Frame (F) with
  its sizes (phone, tablet, laptop, desktop, slide, social post, watch: placed beside the page's
  content), shapes (R rectangle, O ellipse, Shift R triangle, diamond, hexagon, Shift S star, L line,
  Shift L arrow), Text (T), Sticky (S), Pen (P) / Vector pen (B) / Highlighter (Shift P) /
  Eraser (E), Icon (K: a searchable set of 155 interface icons, then the ones saved to Relay's icon set) and the image key (a file, or a stock
  photo: see Pictures).
- **Vector pen** (B): click to place points (Shift: 45° steps), click the first point to close the
  shape, click the last again, press Enter or Esc to leave it open; Backspace takes the last point
  back. The result is a line of your own points: the inspector closes it, smooths it into a curve
  through its points, and fills it.
- **Arcs**: an ellipse's Arc section turns it into a pie, donut, ring or gauge (start, sweep and
  hole, with presets).
- **Flip** (Shift H / Shift V, or the keys beside the rotation): mirrors the selection about its
  centre; a frame mirrors what is in it.
- **Copy and paste style** (Ctrl Alt C / Ctrl Alt V): fill, stroke, corners, shadow, blend, opacity
  and type from one layer onto others, each taking what fits it.
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
- **Inspector** (Ctrl+.), grouped as design tools group it, for the selection. A section shows only
  where every selected layer takes its fields; values that differ across several layers show empty,
  and setting one sets it on all. From the top:
  - the layer's name (Esc puts back the one it had), with Lock and Hide; **Style** (below);
  - **Position and size**: X, Y, W, H, rotation and the flip keys, Fixed / Hug / Fill rules, min and
    max sizes, and **Absolute position** for a layer an auto layout would place; a group's size is
    its layers', and a connector's follows the shapes it joins;
  - **Constraints**: how a layer free in a frame follows the frame's size (left, right, both,
    centre, scale; top, bottom, both, centre, scale);
  - **Auto layout** (direction, gap, padding, along and across), **Type** (any installed family,
    weight, size, line height, letter spacing, case, alignment, colour), and the panels of polygon
    corners and star, icon and its colour, line route and attachments, path closing and smoothing,
    and arc;
  - **Fill** and **Border** (**Stroke** for a line): solid, or a linear, radial or conic gradient with
    its stops; solid, dashed or dotted; one weight all round (a border with sides shows its widest,
    and a weight set there is all round again) or **Each side on its own**; **Corners** (one radius
    or each its own);
  - **Picture**: fit, **Replace picture…**, and for a stock photo its **Credit** (below);
  - **Shadow** (drop or inner), and **Layer**: opacity, blend mode (multiply, screen, overlay and the
    rest), layer blur and, for a frame, rect or ellipse, **Frost** (`backdrop_blur`: what is under it
    blurred through a see-through fill);
  - **Motion**, and for a frame **Timeline** (below); **Export** as PNG (1× or 2×), SVG or PDF.

  Number fields take arithmetic (`120+16`) and units (`45°`), step with ↑/↓ (Shift: ten steps; a line
  height steps by 0.1), scrub by dragging their caption (Shift: finer), and Esc puts back what they
  showed.
- **Pages and layers** (Alt+1): pages to add, rename, duplicate and delete, and the layer tree, top
  layer first, opened along the way to the selection, with hide and lock on each row.
- **Text** is typed in place: double-click (or Enter, or the Text tool, or double-click on empty
  canvas) and a native text view sits exactly over the words, in their font, size, line height and
  colour at the current zoom.
- **Moving** snaps to neighbours' edges and centres with red guides; Shift locks the axis. Alt
  pressed during a drag turns snapping off. Alt held at the press duplicates, and that drag still
  snaps. Dropping onto a frame puts the layer in it without it jumping; in an auto-layout frame a
  drag reorders, with an insertion bar.
- **Resizing** snaps the dragged edges. Shift keeps the proportions (pictures always do; on a side
  handle it sizes the other dimension to match, about the middle), and Alt resizes from the centre,
  the opposite edge moving as far the other way, snap included. Alt does not turn snapping off here.
  A turned layer resizes along its own sides, without snapping. A text given a width wraps from then
  on.
- **A gradient's centre**: one selected layer with a radial or conic fill (or stroke) shows a ring
  at the gradient's centre; dragging it moves the colour inside the layer (`center`, Shift lands on
  tenths), so a glow is placed without its layer hanging past its frame. The inspector has the same
  centre as X and Y in % and a radial one's size (R, `radius`).
- **View**: a mouse wheel zooms at the pointer (Shift and the wheel pan sideways, Ctrl and the
  wheel up and down); on a touchpad two fingers pan, and a pinch or Ctrl zooms. Space or the middle
  button drags; Shift 1 fits the page, Shift 2 the selection, Shift 0 is 100%.
- **Alt+scroll** over a scrolling frame (`scroll: x`, `y` or `both`) scrolls it: the deepest one
  under the pointer, held within what its content overflows. The offset is the canvas's view only,
  never stored, and every still (an export, a render, the phone) shows the frame at its start. With
  no scrolling frame there, or with Ctrl, the wheel pans or zooms as before.
- **Undo** (Ctrl Z / Ctrl Shift Z) is yours: each gesture of yours is one `design.apply`, whose answer
  carries the edits that undo it, and the history is shared by every board and window showing the
  design. An undo meeting a layer changed since leaves that change and says what it kept. An agent's
  changes are not on that stack: after one the status names who edited, the layers glow in the live
  colour, and an **Undo** key beside it takes back that agent's last change (its tooltip names the
  agent), one more of its changes on each press, through `audit.undo`. If you edited the layer since,
  the key says which and keeps it; a second press within ten seconds undoes it anyway. A change whose
  undo is over 256 KiB (deleting a page of thousands of layers) is not kept by the audit: the key says
  so, and nothing older is undone in its place. **Show** goes to the page an agent changed when it
  was not the one open.
- **Clipboard**: copy and paste layers between designs and windows; a picture or text on the system
  clipboard pastes as an image or a text layer; image files drop in from the file manager.

### Pictures

The image key (Ctrl+Shift+K still opens the file chooser at once) is a popover. **From this
computer…** places files as image nodes at their natural size, and so do dropping and pasting: each
goes to the engine's picture store (`design.asset.add`, which copies it, so a design keeps its
pictures when the file moves). Below it, **Search free photos** looks for stock photos (Enter
searches), from **Openverse** (no key) or **Pexels** (with your API key, kept in the system keyring
through **Pexels key…**), in any shape, landscape, portrait or square, 18 at a time with **More
photos**. A click places the photo as one undo step (`design.picture.add`), with its credit kept
beside the file. Stock photos come with the **Pictures** plugin; where it is off the popover says so
and has **Turn on Pictures**.

A stock photo's **Credit** block in the inspector gives its title, creator and licence, **Copy
credit**, **Open photo page** and **Add caption**, which puts the credit under the photo as a text
layer: after it in a column, 6 px under it in a free frame, group or page, and in a row a hugging
column holding the photo and the caption takes its place. One undo step.

### Styles

A design names colours and styles so one change reaches every layer using them. A **named colour**
is taken by any colour field as `$name`; a **style** is a look (fill or gradient, border and its
sides, corners, shadow, opacity, blur, frosted glass, blend and type) a layer takes for each of those fields it does not set itself.
A design holds at most 256 of both together.

- In a colour popover the design's colours come first under **Named**. **Name…** names the colour of
  the selection's field; a right click on a named colour offers **Change…** (its value, for every
  layer naming it) and **Delete** (each layer keeps the colour).
- The inspector's **Style** row picks a style for the selection (**Mixed styles** when they differ),
  or **No style** (each layer keeps its look); on one layer **New style from this layer…** makes one
  from what it draws, and with a style on it **Update ‹name› from this layer**, **Reset to ‹name›**
  (its own settings go) and **Delete ‹name›** (the layers using it keep their look).

Each is one undo step. They are the `color`, `style` and `delete_style` edits of `design.apply`,
which agents send too (BUS.md §10.18); the canvas draws them with the engine's rules
(`Styles::resolve`).

## Motion

A layer moves when the design **plays**. The inspector's **Motion** section adds a preset (Fade in,
Slide up, Slide in, Pop, Pulse, Spin, Blink; Draw on for a line or path; Fill ring for an ellipse),
replaces or removes it, and edits its keys: each is a time and what it sets (move across or down,
turn, scale, opacity, blur, sweep, gradient angle, drawn part, fill, stroke, text colour), with an
ease, the point it turns about and whether its keys loop. A frame's **Timeline** sets the clock
its layers play on: its length (else as long as its latest key) and whether it loops. Motion is offsets
and looks over the layer's own, so it never changes layout, and the layer at rest is what is stored.

- **Play** is the top bar's Play key, shown while something on the page moves, or Ctrl+P. It starts
  every clock together from 0 on the page open; Esc or Ctrl+P stops it. While it plays the page shows
  without selection, guides or handles, the pointer only moves the view (Hand, Space, the middle
  button), and keys that edit or pick a tool do nothing; zoom, undo and redo, the panels and the
  shortcut sheet still work. Play edits nothing, an edit shows in the running motion, and changing
  page ends it. A motion that does not loop holds its last frame and pauses.
- The **time bar** under the canvas has start again, pause and play, the time, a scrubber (drag to
  a paused moment of the motion) and stop.
- **Exports**: the design title's menu has **Export page as animation (APNG)…** and **Export page
  as PNG frames…**, and **Export selection as…** pairs the same (More on the selection toolbar, the
  context menu), drawn at 30 fps and 1× (smaller where a frame would pass 8,192 px a side or 4
  megapixels), up to 600 frames (20 s). The
  APNG loops forever when the motion loops, else plays once and ends on its last frame; frames are
  `name-0000.png`, …; the folder is left with that sequence only (an earlier, longer export's frames
  past the last are removed, and a write that fails takes out what it wrote). They wait for
  something that moves. Every other export and picture (PNG,
  SVG, PDF, thumbnails, Ask's picture, a render) is drawn at rest, even during Play.

## Agents

Agents reach designs through the `design.*` ops (BUS.md §10.18), which every builder may call; their
MCP tools are generated from the same registry, with each node field documented in the schema.

- `design.create` starts a design (an agent's is tied to its own terminal), `design.get` reads one
  (by page, by node, or only what changed since a rev), `design.apply` changes it: a list of `add` /
  `update` / `move` / `delete` / `copy` edits, and `color` / `style` / `delete_style`, run in order,
  all or none. Nodes are named by ids the agent chooses (`"hero"`, `"hero-title"`), so one call can
  build a frame and its children. `design.find` finds nodes by kind, name, words, colour or font.
- Auto layout (`layout: "column"`, `gap`, `padding`, `justify`, `items`, and `w_sizing` /
  `h_sizing` of `hug` or `fill`) lets an agent build a screen without measuring text.
- `design.asset.add` stores a picture (PNG, JPEG, GIF, WebP, SVG) and returns the `src` an image
  node names. With the Pictures plugin on in a project, `design.picture.search`, `.thumb` and `.add`
  place a stock photo with its credit.
- The bundled **Design** plugin (`plugins/design/`, switched on per project from Plugins) gives
  every agent there the `design-canvas` skill — the node model, auto layout, worked examples, how
  to revise exactly what was selected, and the craft that makes a mockup look real — and standing
  rules in its brief. The tools work without it; the plugin makes the results good.
- **Ask** on the board (the selection toolbar's key, the top bar's, or Ctrl+Enter) opens a composer
  for the design's terminal and sends your request with the selection. The prompt is one line naming
  the design, the page and the selected layers by id (the first eight with their kind and name), with
  the request and the `design.get` that reads just those layers, and a PNG of them on their real
  ground (of the whole page when nothing is selected), which the engine stores so the agent can read
  it. What it does depends on the terminal: an agent at its prompt has the line typed in, with Enter;
  one at work gets it as priority mail (`mailbox.send`), which it reads with its next tool call, and
  the board says it is busy; one waiting on you (a permission prompt or a question) or still starting
  is never typed into, since a digit of the prompt would answer its dialog, and the board says so; a
  parked one can be woken, and a design with no live terminal asks which to use and ties the design
  to it. When what you ask about moves, the picture is a contact sheet of six frames of its motion and
  the prompt gives their times (`It moves: the picture is a contact sheet of 6 frames at …`). Your
  request stays in the composer until it has gone, and if the agent changed nothing 90 seconds later
  the status says "No change from <agent> yet".
- **The agent panel** (the terminal key beside Ask) puts the design's terminal on the canvas as a
  card, moved by its head and resized from its corner, kept where it was left for the next design
  opened. **Terminal** shows the terminal itself: the wall's own pane, lent while it is here (off the
  wall, and kept attached whatever page is open), so there is one PTY at one size; closing the panel
  or the board hands it back. **Chat** shows the agent's conversation as Threads draws one, read
  from its provider's log (`session.transcript`, as the phone's chat reads it) about every second
  while the agent works and every few while it waits; an Ask shows as the words asked, and each tool
  the agent ran as one line. Its box sends with Enter: as an Ask about the selection (checked as Ask
  checks the terminal, with a picture), or, with **About the selection** off, typed at the agent's
  prompt as written, to answer it.
- **Who undoes what.** Ctrl+Z undoes your own edits. The status's **Undo <agent>** key takes back an
  agent's last change, one more on each press (see Undo above). The agent's edits arrive as
  `design.changed`; an open board reads the design again, glows what changed in the live colour and
  says who changed it.
- **Render** (`design.render`) is how an agent sees. Relay's desktop app draws, so it must be open
  (`design.render.no_renderer` otherwise; BUS.md §6.2): the agent gets a PNG of a page, or of some of
  its nodes, at rest, at a moment of their motion (`at_ms`) or as a contact sheet of 2 to 24
  labelled frames (`frames`), shown to it as an image where its harness takes MCP images; and, with
  or without the picture, every visible node's laid-out box in page px with hugged text measured in
  the fonts of the machine that drew it, and the problems found: `overflow` (a child past its frame's
  padding), `overlap`, `text_overlap`, `clipped` and `contrast`. It reads the design afresh and draws
  with a painter of its own, never a board's. Every open board showing that design says **"<agent>
  looked at this design"** in a toast once it has drawn; your own renders carry no name and say
  nothing. `since_rev` pictures what changed after a rev: the outermost nodes written since, on
  the page most of them are on (a warning names the others).
- **Few calls.** An agent's turns are what a person waits on, so the agent's tools save them. A
  `design.apply` from the MCP tool may carry `render` (true, or render's options): the server
  applies the list, then renders `since_rev` its `prev_rev`, and answers both in one tool result
  (boxes only when asked; without the desktop app the edits stand and the answer says why there is
  no picture). `update_many` and `delete_many` change or delete every node their `ids` name and
  their `where` finds (design.find's filters, matched against the design before the list); the
  engine runs them as an edit a node, so each keeps every rule of `update` and `delete`, under the
  edit's own index, and the reply's `matched` counts them. A list the engine refuses is kept by
  the MCP server for that design, and the refusal's `resend` says how to send it again with only
  its corrections (`fix: {"<index>": <edit>}`, `drop: [index]`, or `fix: {}` after `design.stale`).

## On the phone

The phone shows designs read only: the gallery, and a page at a time, drawn with ports of the
canvas's layout, shape and paint rules, with a line under each page for what it leaves out (blur
and frosted glass, glows, SVG pictures, motion, newer kinds). It never edits. `docs/ANDROID.md` ("Design on the phone")
has the rest; the ops it may send are `design.list`, `design.get` and `design.asset.get`
(`PHONE_OPS`).

## Reading other builds' designs

Designs outlive the build that wrote them, and the engine and the clients read a newer one's rows:

- A field this build does not know, or a value of a known one it cannot read (a newer gradient
  kind), is read past, kept in the node's `extra` and written back as it was by an update, a delete
  and its undo. A strict list from an agent still refuses to write what this build cannot read.
- A row of a kind this build does not know, or a body that is not a node, is left out of
  `design.get` with what is inside it, and the design still opens. It is never edited: an edit
  naming it, or deleting what holds it, is refused `design.node_unreadable` ("open it in a newer
  Relay"), and an undo skips it as it skips a node that is gone (the canvas calls the reason "from a
  newer Relay"). Connectors attached to it stay attached.
- The phone counts both and says so under the design ("… from a newer Relay").

## Implementation

| Part | Where |
| --- | --- |
| Node model, edits, validation (shared by engine and client); the fields each kind uses, the rules on what a caller writes, reading a node whole, expectations, pins, motion, named colours and styles | `crates/relay-bus/src/design.rs`, `crates/relay-bus/src/design/` (`fields`, `read`, `expect`, `pins`, `motion`, `style`, `hints`) |
| Ops and their payloads | `crates/relay-bus/src/ops/design.rs`, `crates/relay-bus/src/ops/picture.rs` |
| Storage and handlers (migration v28: `designs`, `design_nodes`; v30: revs, styles, removals) | `crates/relay-core/src/handlers/design/` (`mod`, `apply`, `get`, `find`, `assets`, `expect`, `pins`, `revs`, `styles`, `render`) |
| `design.render`: the lease, the waits, the answer's checks and files | `crates/relay-core/src/design_render.rs`, the socket door in `socket.rs` |
| Stock photos: providers, licences, thumbnails, the Pexels key | `crates/relay-core/src/pictures/`, `picture.rs` (formats and sizes) |
| **Native view** | |
| The view: gallery or board in the main window, boards in windows, `design.*` events routed to open boards | `apps/relay-native/src/design.rs` |
| Gallery: cards, chips, thumbnails kept per instance | `apps/relay-native/src/design/gallery.rs` |
| Board: top bar, dock, panels, zoom, Play and the time bar, Undo agent, the commands they share | `apps/relay-native/src/design/board.rs` |
| The canvas widget, whose snapshot is the board's render nodes | `apps/relay-native/src/design/canvas.rs` |
| Input: pointer, wheel, pinch and keys for each tool | `apps/relay-native/src/design/tools.rs` |
| One open design: the local scene, edits queued to `design.apply` in order with their `expect`, the undo stack, rereads; shared by every board of a design | `apps/relay-native/src/design/doc.rs` |
| Drawing nodes as render nodes, cached per node, posed for Play | `apps/relay-native/src/design/paint.rs` |
| Text typed in place | `apps/relay-native/src/design/textedit.rs` |
| Selection toolbar | `apps/relay-native/src/design/context.rs` |
| Inspector, and its panels for shapes (corners, icon, line, path, arc, fills, radii, dashes) and for motion (keys, presets, timeline) | `apps/relay-native/src/design/inspector.rs`, `shape_panels.rs`, `motion_panel.rs` |
| Pages and layers (a recycling list) | `apps/relay-native/src/design/layers.rs` |
| Controls shared by the toolbar and the inspector: colour key, number field, segmented choice | `apps/relay-native/src/design/widgets.rs` |
| Tool and property glyphs; the icon picker | `apps/relay-native/src/design/glyphs.rs`, `icons.rs` |
| Pictures from disk, drop and paste | `apps/relay-native/src/design/assets.rs` |
| Stock photos: the image key's popover, the credit, Add caption | `apps/relay-native/src/design/pictures.rs` |
| Ask: composer, terminal picker, the prompt, delivery by the terminal's state | `apps/relay-native/src/design/ask.rs` |
| Export: PNG, SVG, PDF; a picture at a time, a contact sheet, APNG and frames | `apps/relay-native/src/design/export.rs` |
| `design.render` on the desktop: the lease on a connection of its own, the job | `apps/relay-native/src/design/render.rs` |
| A design in a window of its own | `apps/relay-native/src/design/window.rs` |
| The scripted run the smoke drives | `apps/relay-native/src/design/drive.rs` |
| Styles | `apps/relay-native/src/css/design.css` |
| **Client logic (GTK-free, tested in CI)** | |
| Module index | `crates/relay-client/src/design/mod.rs` |
| Scene tree: applies the engine's edits locally; a node as stored and as drawn | `crates/relay-client/src/design/scene.rs` |
| Layout: auto layout, hugging text, pins, scrolling frames | `crates/relay-client/src/design/layout.rs` |
| Hit testing; snapping | `crates/relay-client/src/design/hit.rs`, `snap.rs` |
| Edit builders: duplicate, paste, group, frame, ungroup, align, reorder, reparent, the `expect` an edit is sent with | `crates/relay-client/src/design/edit.rs` |
| Flip and copy/paste style | `crates/relay-client/src/design/flip.rs` |
| Styles and named colours: new, update, reset, detach, name | `crates/relay-client/src/design/styles.rs` |
| Polygons, arcs, smooth lines, rotation, connectors, shadows | `crates/relay-client/src/design/shapes.rs` |
| What a node paints: its bounds | `crates/relay-client/src/design/bounds.rs` |
| What `design.render` answers: boxes, the area and scale, the issues | `crates/relay-client/src/design/boxes.rs` |
| Colours as painted: defaults, the ground under text, contrast | `crates/relay-client/src/design/color.rs` |
| Motion: clocks, poses, bounds, presets and frame times | `crates/relay-client/src/design/motion.rs` |
| Animated PNG assembly | `crates/relay-client/src/design/apng.rs` |
| Rectangles in page px and the camera | `crates/relay-client/src/design/geom.rs` |
| The icon set (the phone holds a copy) | `crates/relay-client/src/design/icons.rs` |
| Relay's saved icons (`design.icon.*`), and an SVG fitted onto the grid | `crates/relay-core/src/handlers/design/icons.rs`, `crates/relay-bus/src/design/path_data.rs` |
| Default names, copies' names, captions, cut text and sort order | `crates/relay-client/src/design/names.rs` |
| Numbers as fields show and read them | `crates/relay-client/src/design/number.rs` |
| Runs and case over a text's characters | `crates/relay-client/src/design/text.rs` |

The canvas is one custom widget whose snapshot is GSK render nodes: the page's content is built once
into a cached node (text layouts, stroke paths and pictures are cached per node) and each frame only
transforms it by the camera and draws the selection, guides and labels over it, so panning and
zooming cost a transform on the GPU. During Play a frame repaints only the movers and their
ancestors. Pictures decode off the main thread. Edits run on the local scene at once and go out to
the engine one at a time in order; a design another actor changed is read again once nothing of ours
is in flight, and never in the middle of a gesture or typing. Relay has no webview (PRODUCT.md), so
designs are native vector nodes rather than rendered HTML.

On one connection, a request's `design.changed` can arrive before or after its reply, or interleave
with an earlier request's event; events and replies are not ordered with each other. A client tells
its own changes by the event's `cause` (its request id) or by rev (`prev_rev` and `touched`), as the
canvas does: an event at or below a rev it has already confirmed changes nothing.

## Checking it

`scripts/design-smoke.py` (inside a headless compositor, never on a desktop; its docstring lists the
shots and how to start it) starts a disposable engine, writes fixture designs through `design.apply`
as an agent would, and drives the real client (`design/drive.rs`, `RELAY_SMOKE_DESIGN_SCRIPT`):

- **Shots** capture the gallery, the board, a selection, a narrow window, the layers panel and typing
  in place; run gestures and commands (nudge, a snapped drag, editing a text, drawing a rectangle,
  undo and redo, group and ungroup, duplicate, delete) and assert that the client's scene equals the
  engine's (`script`); type an Ask into a fixture terminal whose fake agent edits the layer and
  assert the open board shows it (`agent`); place a picture and paste between frames (`media`);
  show polygons, icons, gradients, dashes, rotation, connectors and labels as an agent writes them
  and then the tools that make them (`shapes`); draw a closed shape with the vector pen, flip a
  polygon, paste a style and export the page as SVG over fixture arcs, blends, inner shadows and
  smooth paths (`more`, which also rasterises its SVG and PDF and compares them with the PNG, and
  compares shadow softness on a box and an arc); time a 1,721-node design (`perf`,
  `RELAY_DESIGN_TRACE`); check who undoes what after the person and an agent both edit
  (`agent-undo`), a text edit that loses to an agent's (`agent-stale`) and an agent deleting layers
  (`agent-delete`); re-read named colours and styles an agent changes (`reuse`); play, seek, sheet
  and export a page's motion with its rev unchanged (`motion`); answer `design.render` with and
  without the app (`render`); search, place and credit a stock photo from fixtures
  (`pictures`); drag a glow's gradient centre inside its screen and show the fixture terminal in
  the agent panel (`panel`); and Ask from the panel's conversation and see the fixture agent's
  answer drawn there (`panel-chat`). Each shot opens its design by id (`RELAY_SMOKE_DESIGN_ID`) and fails on a panic.
- **Drive steps** are `;`-separated and go through the same code the pointer and keys use, each
  waiting for the engine: `select`, `click`, `dbl`, `drag` (with `:shift` or `:alt`), `press`,
  `idle` and `release`, `tool`, `type`, `nudge`, `set`, `align`, `layout`, `duplicate`, `group`,
  `copy`, `paste`, `undo`, `ask`, `zoom`, `fit`; for motion `play`, `pause`, `seek:ms`, `stop`,
  `sheet:n`, `apng:path` and `frames:dir` (printing `DESIGN_SHEET`, `DESIGN_APNG`, `DESIGN_FRAMES`)
  and `png:path[@ms]`; and for stock photos `pictures:words[@pexels]`, `picture-pick:n`,
  `picture-caption` and `inspector:Title`. `drive.rs` lists them all.
- **Shared fixtures** keep the engine, the canvas and the phone to one set of rules. The phone draws
  with ports of `layout.rs`, `shapes.rs` and `bounds.rs` and a copy of the icon set, so
  `crates/relay-client/tests/phone_fixtures.rs` lays out every case of
  `apps/relay-android/core/src/test/resources/design/layout-cases.json` with the PC's own code (`PHONE_FIXTURES=write`
  rewrites the file from it) and holds `icons.json` to the PC's icons; the phone's `DesignLayoutTest`
  and `DesignShapesTest` hold the Kotlin port to the same file.
  `crates/relay-bus/tests/phone_fixture.rs` fails when a node gains a field, kind or enum value that
  `get-reply.json` does not use, and `crates/relay-core/tests/design_phone.rs` makes that reply again
  through `design.apply` and reads it back. A change to layout or geometry fails on one side until
  the fixture says so, and on the other until the port follows, in the same piece of work.

`scripts/native-smoke.py` captures the Design view among its pages. The engine side is
`crates/relay-core/tests/design*.rs` (`design.rs` the tree, undo and lifecycle; `design_expect.rs`
replies, expectations and skip reasons; `design_revs.rs` revisions and `since_rev`; `design_pins.rs`
the follow rule; `design_reuse.rs` copy, colours and styles; `design_compat.rs` bodies from other
builds; `design_render.rs` the render door with a fake desktop app; and the rest by their names); the
scene, layout, hit testing, snapping, edit builders and boxes are tested in `crates/relay-client`.
Readings of what the canvas costs are in `docs/perf/` (`DESIGN-2026-10-16.md`,
`DESIGN-2026-10-20-play.md`, `DESIGN-2026-10-20-scale.md`), and `scripts/perf/ratchet.py` guards the
engine's rows (PLAYBOOK §4).
