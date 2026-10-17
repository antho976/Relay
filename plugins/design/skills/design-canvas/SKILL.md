---
name: design-canvas
description: Draw and revise designs on Relay's design canvas with the design.* tools - the node model (pages, frames, groups, rects, ellipses, lines and arrows, text with rich spans and lists, stickies, pen paths, vectors, images, polygons, icons, comments), the asset packs and templates (design.kits, insert), components and their copies, shared styles, constraints and layout grids, prototype links, versions, rendering a picture to look at (design.render), HTML/CSS (design.code), coordinates relative to the parent, auto layout (layout row or column, gap, padding, justify, items, w_sizing and h_sizing hug or fill) so screens lay themselves out without measuring text, choosing your own node ids so one design.apply builds a whole screen, revising exactly the layers a person selected and asked about, pictures through design.asset.add, reading validation errors, and the craft that makes a mockup look real. Use whenever a task asks to design, sketch, mock up, wireframe or diagram something, or a message starts with "[Relay design]".
---

# Relay design canvas

**Act only when asked.** Every agent may call the `design.*` tools, but a design is the person's:
start, change or delete one only when they asked for it (a task, a message, or an Ask from the
board). They can undo any change you make from the board.

A **design** belongs to a project (and usually to your terminal) and holds **pages**; a page holds
**nodes**. You read with `design.get`, and change with `design.apply`: a list of edits run in order,
all or none, which is also **one undo step** for the person. They watch your edits appear live in
the Design view.

## The tools

| Tool | Use |
| --- | --- |
| `design.list {project_id}` | The project's designs, newest change first. Yours carry your session name. |
| `design.create {project_id, title}` | A new design with one empty page; answers `{design, nodes:[page]}`. Tied to your terminal. |
| `design.get {design_id, page?}` | The design and every node (parents before children, siblings bottom to top). |
| `design.apply {design_id, ops, base_rev?}` | Add, update, move, delete nodes. Answers the changed nodes and their ids. |
| `design.update {design_id, title?}` | Rename the design. |
| `design.asset.add {design_id, path}` | Store a PNG/JPEG/GIF/WebP/SVG; answers `{src, w, h}` for an image node. |
| `design.kits {kit?, with_nodes?}` | The asset packs and templates: `relay-ui`, `wireframe`, `flowchart`, `devices`, `templates`, each item `kit/item` with its `slots`. |
| `design.create {…, template}` | Start from a template (`templates/<item>`) instead of a blank page. |
| `design.render {design_id, node_id?, scale?}` | Ask the open window for a PNG of a frame or page; then `bus.wait` for `design.rendered` with your `request_id` and read its `path`. Look at your work this way. |
| `design.code {design_id, node_id}` | The frame as HTML and CSS. |
| `design.version.save/list/get/restore/delete` | Named versions. Relay saves one by itself before you start editing what someone else edited last. |

## Nodes

Every node has `kind` and (except pages) `parent`. **`x`/`y` are relative to the parent's top-left**,
so moving a frame moves what is in it. Children paint in order: later ones on top.

| kind | for | key fields |
| --- | --- | --- |
| `page` | a sheet; its `fill` is the canvas colour | `name` |
| `frame` | artboards, cards, buttons, rows: a box that clips and can lay out children | `w h fill radius stroke shadow layout gap padding justify items clip` |
| `group` | nodes moved together; paints nothing | children at offsets from the group |
| `rect`, `ellipse` | shapes, avatars, dividers, icons | `w h fill stroke stroke_width radius` |
| `line` | lines and arrows | `points: [[x,y],[x,y]]` relative to `x/y`, `stroke`, `stroke_width`, `arrow_end`, `arrow_start` |
| `text` | words | `text font font_size font_weight color align line_height letter_spacing italic underline` |
| `sticky` | a note card | `text`, `fill` (paper), `w h` |
| `path` | a freehand stroke | `points`, `stroke`, `stroke_width` |
| `image` | a picture | `src` from `design.asset.add`, `w h`, `fit: cover|contain|stretch`, `radius` |
| `polygon` | triangles, diamonds, hexagons, stars, flowchart decisions | `sides` (3 triangle, 4 diamond, 6 hexagon…), `star` (inner ratio, e.g. 0.4), `fill`, `stroke`, `text` |
| `icon` | interface icons in mockups | `icon` (home, search, settings, user, bell, mail, heart, star, plus, check, x, chevron-*, arrow-*, menu, more-horizontal, trash, edit, copy, calendar, clock, image, file, folder, shopping-cart, credit-card, globe, map-pin, phone, message, send, filter, sliders, grid, list, play, pause, refresh, lock, eye, info, alert, help, sun, moon, wifi, battery, terminal, code, zap, cloud, git-branch, bar-chart…), `stroke` (its colour), `w h` (24 is the native size) |

| `vector` | any shape the others can't make: logos, blobs, custom icons, curves | `d` (SVG path data: M L H V C S Q T A Z), `view: [w, h]` (the box `d` is in; it scales to `w h`), `fill`, `stroke`, `open: true` for a line |
| `comment` | a review note pinned on the canvas, never part of the design | `text`, `replies: [{text}]` (you are filled in as the author), `resolved` |

Any node can also take `rotation` (degrees clockwise about its centre), `opacity`, `blur` (px),
`shadow`, and a shape `gradient` instead of `fill`:
`{"kind": "linear", "angle": 135, "stops": [{"at": 0, "color": "#4C9DFF"}, {"at": 1, "color": "#A178FF"}]}`
(`angle` reads as CSS: 180 top to bottom; `"kind": "radial"` from the centre). Rects, frames and
images take `radii: [tl, tr, br, bl]` for different corners; any stroke takes `dash` (`[6, 4]`
dashed, `[0, 4]` dotted). A `rect`, `ellipse` or `polygon` with `text` shows it centred inside: a
labelled box for a diagram.

**Connectors.** A `line` with `from` and `to` (node ids) runs from one node's edge to the other's
and follows them when they move — `points` are not needed. `route` is `straight` (default),
`elbow` (right angles) or `curve`; `arrow_end: true` puts a head on it:
`{"op":"add","node":{"parent":"page-1","kind":"line","from":"start","to":"decide","route":"elbow","arrow_end":true}}`.
Build flowcharts this way: boxes and diamonds with `text`, connectors between them.

**Asset packs.** Don't rebuild buttons, inputs, cards, nav bars, flowchart shapes or device frames
from scratch: `design.kits` lists them, and one `insert` places a whole piece as ordinary editable
nodes, ids `<id>.<part>`; `set` fills its slots:
`{"op":"insert","item":"relay-ui/button-primary","parent":"login","id":"cta","set":{"label":{"text":"Continue"}}}`.

**Components.** A frame with `component: true` is a main component; `{"op":"insert","item":"#card"}`
places a copy whose parts follow every change to it, except fields you set on the copy (those are
its overrides; `overridden: null` resets them). Change the component, not each copy.

**Shared styles.** A node with `shared: true` (usually on a page named Styles) is a style; others
name it in `fill_style`, `text_style` (font, size, weight, line height, spacing) or `effect_style`
(shadow, blur) and the engine keeps them in step. Setting such a field by hand detaches the style.

**Rich text.** `spans: [{"text": "Bold ", "weight": 700}, {"text": "link", "link": "https://…"}]`,
whose words together must equal `text`; spans take `weight italic underline strike color size font
code link`. `list: "bullet" | "number" | "check"` marks each line (`[x] ` ticks a checklist line).

**Pictures as fills, crop, adjustments.** Any rect, ellipse, polygon, frame or vector takes `src`
(from `design.asset.add`) to fill with a picture; `crop: [x, y, w, h]` as fractions shows part of
it. `filters: {brightness, contrast, saturation (-1..1), hue (-180..180), sepia, invert (0..1)}`
adjusts any node's colours. `flip_x` / `flip_y` mirror it.

**Constraints and grids.** Inside a frame without auto layout, `constraints: {"h": "start|end|both|center|scale", "v": …}`
says how a child follows when the frame is resized. A frame's `grids: [{"kind": "columns", "count": 12, "gutter": 20, "margin": 24}]`
(or `rows`, or `square` with `size`) is drawn while designing and snapped to.

**Prototypes.** `goto: "<frame id>"` on any node makes a click on it go to that frame when
presenting, with `transition` (`dissolve`, `instant`, `slide_left`, `slide_right`, `slide_up`,
`slide_down`); `start: true` on a top-level frame is where presenting begins.

Colours are `#RRGGBB` or `#RRGGBBAA` (`#00000000` is transparent). Defaults: a frame is white, a
shape `#D9D9D9`, a sticky yellow; text is 16 px Geist 400, dark on a light ground and light on a
dark one unless you set `color`. Fonts always present: `Geist`, `Geist Mono`, `Sora`.

## Auto layout: let screens lay themselves out

You cannot measure text, so never stack things by guessing `y` values. Put them in a frame with
`layout: "column"` (or `"row"`) and let the frame place them:

- `gap` between children, `padding` as `[all]`, `[vertical, horizontal]` or `[top, right, bottom, left]`.
- `justify` along the direction: `start` (default), `center`, `end`, `space_between`.
- `items` across it: `start` (default), `center`, `end`, `stretch`.
- On a child: `w_sizing` / `h_sizing` of `fixed` (its `w`/`h`), `hug` (shrink to content: text's
  words, a laid-out frame's children) or `fill` (take the room left, or the full depth across).
- A text with no `w` hugs its words; give it `w_sizing: "fill"` in a column to wrap at the column's
  width. A laid-out frame with `h_sizing: "hug"` grows with its children.
- Children of a laid-out frame ignore their own `x`/`y`.

## Ids: build a screen in one call

Give nodes your own ids (letters, digits, `- _ . :`), then later edits in the same list can use them
as `parent`. Use readable, stable ids — the person's selection and your next revision name them.
Send **one screen or component per `design.apply`** (tens to a few hundred edits): the person sees
it arrive piece by piece, each piece is one undo step, and the engine stays responsive.

```json
{"design_id": 12, "ops": [
  {"op": "add", "node": {"id": "login", "parent": "page-1", "kind": "frame", "name": "Log in · iPhone",
    "x": 0, "y": 0, "w": 390, "h": 844, "fill": "#FFFFFF",
    "layout": "column", "gap": 16, "padding": [72, 24, 32, 24]}},
  {"op": "add", "node": {"id": "login-title", "parent": "login", "kind": "text", "text": "Welcome back",
    "font_size": 30, "font_weight": 700, "letter_spacing": -0.6, "color": "#141414"}},
  {"op": "add", "node": {"id": "login-hint", "parent": "login", "kind": "text", "w_sizing": "fill",
    "text": "Sign in to pick up where your agents left off.", "font_size": 15, "color": "#6B6B6B"}},
  {"op": "add", "node": {"id": "email", "parent": "login", "kind": "frame", "name": "Email field",
    "w_sizing": "fill", "h": 52, "radius": 12, "fill": "#F4F4F4", "stroke": "#E2E2E2",
    "layout": "row", "items": "center", "padding": [0, 16]}},
  {"op": "add", "node": {"id": "email-text", "parent": "email", "kind": "text", "text": "you@example.com",
    "color": "#9A9A9A"}},
  {"op": "add", "node": {"id": "login-button", "parent": "login", "kind": "frame", "name": "Button",
    "w_sizing": "fill", "h_sizing": "hug", "radius": 12, "fill": "#141414",
    "layout": "row", "justify": "center", "padding": [16]}},
  {"op": "add", "node": {"id": "login-button-label", "parent": "login-button", "kind": "text",
    "text": "Continue", "font_size": 16, "font_weight": 600, "color": "#FFFFFF"}}
]}
```

## Revising: change exactly what was asked

A request from the board reads like `[Relay design] "Settings" (design_id 12, page "Page 1").
Selected: save-button (frame "Button"). Picture of it: /…/ab12….png. Request: make it green.`

1. Look at the picture (it is a PNG path you can read) and `design.get` the design (or `page`).
   After your change, `design.render` the frame and look again before you say it is done.
2. Change those layers with `update`: `{"op":"update","id":"save-button","set":{"fill":"#2EC469"}}`.
   `set` takes any node field except `id`, `kind` and `parent`; `null` puts a field back to default.
3. `move` reparents or reorders: `{"op":"move","id":"x","parent":"card","index":0}` (index 0 is the
   bottom / first in a layout; no index puts it last / on top).
4. Keep ids and the person's layers; don't delete and re-add what an `update` can change. One
   request → one `design.apply`, so one Ctrl+Z undoes it.
5. Pass `base_rev` (the `rev` you read) when your edit assumes nothing changed since; a
   `design.stale` refusal means read again first.

A **comment thread** handed to you reads `Comment thread comment-3 says: "…"`: do what it asks,
then answer in it — `update` `comment-3` with `replies` (the earlier ones plus `{"text": "…"}`),
or `resolved: true`.

## Errors

A refusal names the edit and the field: `ops[3] update "hero": fill: "red" is not #RGB, #RRGGBB or
#RRGGBBAA`. Nothing in that list was applied; fix that edit and send the list again. A design holds
at most 5000 nodes; one `design.apply` carries at most 2000 edits.

## Make it look real

- **Artboards**: name them (`"Settings · iPhone"`); phone 390×844, tablet 834×1194, desktop
  1440×900; put variants side by side 80 px apart on the page, flows left to right with arrows
  (`line` with `arrow_end: true`) between them.
- **Rhythm**: spacing and padding on an 8 px scale (4, 8, 12, 16, 24, 32, 48); radii 8–16 for cards
  and buttons, consistent across a screen.
- **Type**: one ramp — 32–40/700 titles, 20–24/600 headings, 15–16/400 body, 12–13/500 captions;
  line height 1.2 for headings, 1.4–1.5 for body; slight negative tracking on large titles.
- **Colour**: a neutral ground, one accent used for one job (the primary action), text contrast of
  at least 4.5:1; check dark variants separately.
- **Content**: real, specific copy from the product — names, numbers, states — never lorem ipsum.
  Show states that matter (empty, error, loading) as their own frames when asked about a flow.
- **Structure**: a frame per component (card, row, button) so the person can select and restyle it;
  stickies (`sticky`) for notes and open questions beside the artboards rather than inside them.
