---
name: design-canvas
description: Draw, animate and revise designs on Relay's design canvas with the design.* tools - the node model (pages, frames, groups, rects, ellipses, lines and arrows, text, stickies, pen paths, images, icons), coordinates relative to the parent, auto layout (row, column or wrap, gap, padding, justify, items, hug or fill), pinning, absolute children, min and max sizes and scrolling frames, copies, named colours and styles, rings, glows, gradients and styled runs of text, motion (keyframes, easing, timelines), choosing your own node ids so one design.apply builds a whole screen, revising exactly the layers a person selected and asked about, looking at what you drew with design.render, reading validation errors and limits, and the craft that makes a mockup look real. Use whenever a task asks to design, sketch, mock up, wireframe, diagram or animate something, or a message starts with "[Relay design]".
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
| `design.list {project_id, session?}` | The project's designs, newest change first. Yours carry your session name. |
| `design.create {project_id, title?}` | A new design with one empty page (`page-1`); answers `{design, nodes:[page]}`. Tied to your terminal. |
| `design.get {design_id, page?, nodes?, since_rev?, pages_only?}` | The design and its nodes (parents before children, siblings bottom to top), plus `colors`, `styles` and picture `credits`. `page`: one page, by id or by a name no other page has. `nodes`: 1 to 500 ids and what is inside them. `since_rev`: only what changed after that rev (`nodes`, and `removed` ids). `pages_only`: the page list alone. |
| `design.find {design_id, kind?, name?, text?, match?, fill?, stroke?, color?, any_color?, font?, page?, inside?, with_nodes?}` | The ids of nodes that match every filter, in paint order; `with_nodes: true` adds the first 200 as stored. `name` and `text` match whole words by default ("Edit" passes over "CREDIT"); `match: "prefix"`, `"contains"` or `"exact"` asks for the others. Use it instead of reading a whole design to find a colour, a word or a kind. |
| `design.fonts {name?}` | The font families the desktop app can draw. |
| `design.apply {design_id, ops, base_rev?, lenient?, reply?}` | Add, update, move, copy, delete nodes; name colours and styles. Answers `{rev, prev_rev, ids, removed_count, audit_id}`, with `rebased`, `copied`, `contrast`, `skipped` and `warnings` when there are any. `contrast` lists the contrast flags your change made (words it made too faint, or whose ground it changed), on every page a colour or style edit reaches: read it before you move on, and fix what it names. It is worked out without the desktop app, so `design.render` has the last word. |
| `design.render {design_id, page?, nodes?, at_ms? or frames?, …}` | A picture of a page or some nodes, and every node's laid-out box. See "Check what you drew". |
| `design.update {design_id, title?}` | Rename the design. |
| `design.asset.add {design_id, path}` | Store a PNG/JPEG/GIF/WebP/SVG (a file up to 20 MiB, or `bytes_b64` up to 1 MiB); answers `{src, w, h}` for an image node. |
| `design.duplicate`, `design.delete`, `design.restore` | Copy a design, or move it to the deleted ones and back. Delete only when the person asked. |

**Photos.** Stock photographs come through `design.picture.search` / `thumb` / `add`, only in a
project where the Pictures plugin is on: load the `design-pictures` skill. A stored photo keeps its
credit (`design.get` answers `credits`); name the credits in your reply.

## Nodes

Every node has `kind` and (except pages) `parent`. **`x`/`y` are relative to the parent's top-left**,
so moving a frame moves what is in it. Children paint in order: later ones on top. A page is dark
by default (`#1E1D1B`, the canvas colour): put light screens in a frame (white by default).

| kind | for | key fields |
| --- | --- | --- |
| `page` | a sheet; its `fill` is the canvas colour | `name`, `fill`, `timeline` |
| `frame` | artboards, cards, buttons, rows: a box that clips and can lay out children | `w h fill radius stroke shadow layout gap row_gap padding justify items clip scroll` |
| `group` | nodes moved together; paints nothing; its box is its children's | children at offsets from the group |
| `rect`, `ellipse` | shapes, avatars, dividers; an ellipse with `arc` is a pie, donut, ring or gauge | `w h fill stroke stroke_width radius`, `arc: {start, sweep, inner, cap}` |
| `line` | lines and arrows; with `closed` (and `fill`) a shape of your own | `points: [[x,y],…]` relative to `x/y`, `stroke`, `stroke_width`, `arrow_end`, `arrow_start`, `closed`, `smooth`, `fill` |
| `text` | words | `text font font_size font_weight color align line_height letter_spacing italic underline strike text_case runs max_lines` |
| `sticky` | a note card | `text`, `fill` (paper), `w h` |
| `path` | a freehand stroke | `points`, `stroke`, `stroke_width` |
| `image` | a picture | `src` from `design.asset.add`, `w h` (both needed), `fit: cover|contain|stretch`, `radius` |
| `polygon` | triangles, diamonds, hexagons, stars, flowchart decisions | `sides` (3 triangle, 4 diamond, 6 hexagon…, 3 to 24), `star` (inner ratio 0.05 to 0.95, e.g. 0.4; **a star with no `sides` has 5 points**), `fill`, `stroke`, `text` |
| `icon` | interface icons in mockups | `icon` (a name below, or one saved to Relay's set), `stroke` (its colour), `w h` (24 is the native size, which an icon added without one gets) |

Icons are outline icons on a 24-unit grid, drawn at the smaller of `w` and `h`, stroke 2 px unless
set. Relay's own: home, search, settings, user, users, bell, mail, heart, star, plus, minus, x, check, chevron-left, chevron-right, chevron-up, chevron-down, arrow-left, arrow-right, arrow-up,
arrow-down, menu, more-horizontal, more-vertical, trash, edit, copy, download, upload, share, link,
lock, unlock, eye, eye-off, calendar, clock, camera, image, file, folder, shopping-cart,
credit-card, globe, map-pin, phone, message, send, filter, sliders, grid, list, play, pause,
refresh, log-out, bookmark, info, alert, help, sun, moon, wifi, battery, terminal, code, zap, cloud,
git-branch, bar-chart, wallet, bank, receipt, tag, coins, banknote, dollar-sign, percent,
calculator, trending-up, trending-down, pie-chart, line-chart, shopping-bag, gift, utensils, coffee,
car, bus, plane, fuel, briefcase, book, graduation-cap, music, film, tv, gamepad, dumbbell,
activity, pill, shirt, scissors, wrench, lightbulb, droplet, flame, leaf, umbrella, thermometer,
inbox, archive, layers, external-link, paperclip, at-sign, hash, smile, thumbs-up, flag, award,
target, shield, key, mic, video, headphones, smartphone, laptop, printer, map, navigation, compass,
package, truck, store, database, server, cpu, rotate-ccw, repeat, shuffle, skip-forward, skip-back,
volume, maximize, minimize, plus-circle, check-circle, x-circle, log-in, user-plus, sparkles, timer,
history.

**When none fits, save one to Relay's set instead of reusing a near miss** (one icon for three
different things reads as one thing). `design.icon.list {query?}` names Relay's own and every saved
icon, which every project's designs share; check it first, since another agent may have saved what
you need. `design.icon.save {name, path | svg, keywords?}` adds one: `path` is SVG path data on the
24-unit grid, drawn as an outline (`"M4 12a8 6 0 1 0 16 0a8 6 0 1 0-16 0M8 17v3M16 17v3"`); `svg`
takes an outline SVG as most icon sets ship them (`<svg viewBox="0 0 24 24" fill="none"
stroke="currentColor">…</svg>`) and fits it onto the grid. Name it for what it shows (`piggy-bank`,
not `icon-3`); a filled picture belongs in an image node instead. Then `"icon": "piggy-bank"` on any
icon node draws it: the node keeps the outline in its own `icon_path`, so it draws the same if the
saved icon changes or goes. `design.icon.save` with `replace: true` changes a saved icon, and
`design.icon.delete` removes one, only when the person asks.

**A kind refuses fields that are not its own**, naming the kinds that use it: `radius: ellipse
nodes do not use it (it is for frame, rect, sticky, image)`. `null` always clears a field, on any
kind (it never refuses), so an update can drop a stale one. A page takes no `style`, `motion`,
`opacity`, `pin_*` or `absolute`; only a frame or page takes `timeline`; only a frame takes
`scroll`, `layout`, `gap` and `clip`; `padding` is for frames and the labels of rect, ellipse and
polygon.

Any node but a page can also take `rotation` (degrees clockwise about its centre, ±3600; **not a
`line`: its turn is its points**), `opacity`, `blur` (0 to 200 px), `blend` and a shape `gradient`
instead of `fill`:
`{"kind": "linear", "angle": 135, "stops": [{"at": 0, "color": "#4C9DFF"}, {"at": 1, "color": "#A178FF"}]}`
(2 to 8 stops; `angle` reads as CSS: 180 top to bottom; `"radial"` from the centre; `"conic"` round
the centre, clockwise from `angle`, 0 the top). Rects, frames, stickies and images take
`radii: [tl, tr, br, bl]` for different corners; any stroke takes `dash`: `[6, 4]` dashed, `[0, 4]`
dotted (a dot needs `stroke_cap` round or square). A `rect`, `ellipse` or `polygon` with `text`
shows it centred inside: a labelled box for a diagram.

**Colours** are `#RGB`, `#RRGGBB` or `#RRGGBBAA` (`#00000000` is transparent), or a `$name` the
design names (see Reuse). Defaults: a frame is white, a rect, ellipse or polygon `#D9D9D9`, a
sticky `#FFE58A`; a border or outline is 1 px and drawn only when you set `stroke`, a line, path or
icon 2 px; text is 16 px, weight 400, **dark on a light ground and light on a dark one** unless you
set `color`.

**Fonts.** `Geist` (400, 500, 600 and 700 draw), `Geist Mono` (400, 500) and `Sora` (600 only, a
display face) are always there, so the ramp's 700 titles use Geist. `font_weight` takes 1 to 1000
and the nearest face the family has is drawn. Any other family must be on the desktop
(`design.fonts`); one that is not draws in a fallback face and the reply warns.

More you can reach for:

- **Arcs and progress rings**: `"arc": {"start": 0, "sweep": 252, "inner": 0.82, "cap": "round"}`
  on an ellipse: `start` and `sweep` in degrees clockwise from the top, `inner` the hole as a
  fraction of the radius, `cap` `butt` (default), `round` or `square` for the ends of a ring
  segment (needs `inner` above 0 and `sweep` under 360). For a gradient ring add a conic
  `gradient` with `"angle": 350`, a little before `arc.start`, so the round start cap takes the
  first colour, not the last. A progress ring is two ellipses: a full ring in a muted colour, the
  progress on top.
- **Your own shapes**: a `line` with three or more `points`, `"closed": true` and a `fill`;
  `"smooth": true` curves through the points (blobs, waves, chart areas).
- **Strokes**: `stroke_cap` (`butt`, `round`, `square`; a solid line or path is round by default,
  dashes butt) and `stroke_join` (`miter`, `round`, `bevel`; ellipse, polygon, line, path).
  `stroke_gradient` (shaped like `gradient`) paints along the stroke, even with no `stroke`.
- **Shadows and glow.** Frame, rect, ellipse, polygon, sticky and image take `shadow` (outer, or
  `"inset": true` inside the edge) or `shadows`, a list of 1 to 8 painted in order (give one field
  or the other; blur 0 to 200). **Text, line, path and icon cast an outer shadow only, without
  `spread`** (inset or spread is refused); with x and y 0 it is a glow:
  `"shadows": [{"blur": 4, "color": "#4AF0FFCC"}, {"blur": 18, "color": "#4AF0FF66"}]`. A group and
  a page cast none.
- **Runs**: parts of a text in a style of their own: `"text": "8.4 km", "runs": [{"start": 3,
  "end": 6, "font_size": 14, "color": "#8C877F"}]`. 1 to 64, in order, not overlapping, counted in
  characters (`end` not included), each setting `color`, `font`, `font_size`, `font_weight`,
  `italic`, `underline`, `strike` or `rise` (px above the baseline). Changing `text` means sending
  `runs` (or `null`) with it.
- **`max_lines`** (1 to 1000) cuts a text after that many lines with "…". It needs a width to wrap
  at: a sticky, a shape's label, or a text with a fixed `w` or `w_sizing: "fill"`; a hugging text is
  refused.
- **Label padding**: on a rect, ellipse or polygon, `padding` is the label's inset (default
  `[0, 12]`, less on a narrow shape): `"text": "Continue", "padding": [8, 24]`.
- **Blend**: `"blend": "multiply"` (also `screen`, `overlay`, `darken`, `lighten`, `color_dodge`,
  `color_burn`, `hard_light`, `soft_light`, `difference`, `exclusion`, `hue`, `saturation`,
  `color`, `luminosity`) mixes a node with what is under it in the same parent.
- **Text case**: `"text_case": "upper"` (or `lower`, `title`) draws the letters so while the words
  stay as written: eyebrow labels, overlines.
- **Flip**: `"flip_x": true` / `"flip_y": true` mirrors a polygon, image or icon.
- `line_height` is a multiple of the font size (0.1 to 10, default 1.3): `1.0` for tight display
  type, `1.4`–`1.5` for body.

**Connectors.** A `line` with `from` and `to` (node ids on its page) runs from one node's edge to
the other's and follows them when they move — `points` are not needed. `route` is `straight` (default),
`elbow` (right angles) or `curve`; `arrow_end: true` puts a head on it:
`{"op":"add","node":{"parent":"page-1","kind":"line","from":"start","to":"decide","route":"elbow","arrow_end":true}}`.
Build flowcharts this way: boxes and diamonds with `text`, connectors between them.

## Auto layout: let screens lay themselves out

You cannot measure text, so never stack things by guessing `y` values. Put them in a frame with
`layout: "column"` (or `"row"`) and let the frame place them:

- `gap` between children, `padding` as `[all]`, `[vertical, horizontal]` or `[top, right, bottom, left]`.
- `justify` along the direction: `start` (default), `center`, `end`, `space_between`.
- `items` across it: `start` (default), `center`, `end`, `stretch`.
- On a child: `w_sizing` / `h_sizing` of `fixed` (its `w`/`h`), `hug` (shrink to content: text's
  words, a laid-out frame's children; only a text, a sticky or a frame hugs) or `fill` (take the
  room left, or the full depth across).
- A text with no `w` hugs its words; give it `w_sizing: "fill"` in a column to wrap at the column's
  width. A laid-out frame with `h_sizing: "hug"` grows with its children.
- Children of a laid-out frame ignore their own `x`/`y` (unless `absolute`, below). A group in a
  layout takes its children's bounds.
- **`layout: "wrap"`** is a row that starts a new line when the next child would pass the frame's
  width: chips and tags. `gap` along a line, `row_gap` between lines (default `gap`); a frame that
  hugs its width wraps only at its `max_w`; `fill` takes the room left in its line.
- **Bounds**: `min_w`, `max_w`, `min_h`, `max_h` limit whatever decides a size (hug, fill, stretch,
  a pin): a button `min_w: 120`, a chat bubble text `w_sizing: "hug", max_w: 260` (it wraps there).
  A fixed `w` must already lie between its bounds.
- **Turning layout off** (`layout: null`) leaves each child at its *stored* `x/y/w/h`, not where
  the layout drew it: send each child's placed `x/y/w/h` (and `fixed` sizing) in the same
  `design.apply`. The `design.render` boxes give the placed values (`rx`, `ry`, `w`, `h`).

## Pinning, absolute children and scrolling

A child's `x/y/w/h` are its box while its parent frame is at its **stored** `w`/`h`; `pin_x` /
`pin_y` say how it follows when the parent is another size. Only a child that is free in its frame
(the frame has no auto layout, or the child is `absolute`) is pinned; children of a page or group
never are.

| pin | follows the parent by |
| --- | --- |
| `pin_x`: `left` (default), `right`, `center` | keeping its distance from that edge, or from the centre line |
| `pin_x`: `left_right` | keeping both distances: its width stretches (needs a fixed width) |
| `pin_x`: `scale` | keeping its place and width as fractions of the parent's |
| `pin_y`: `top` (default), `bottom`, `center`, `top_bottom`, `scale` | the same, for height |

- **Resize a frame and its pinned children follow in the same apply**: when an `update` changes a
  frame's stored `w` or `h`, each pinned free or absolute child moves on every axis your list did
  not set on that child itself (x/y you give in that list are read against the new size). Each
  follow is part of the same undo step.
- A tab bar in a 390×844 screen: `{"parent":"screen","kind":"frame","name":"Tab bar","x":0,"y":764,"w":390,"h":80,"pin_x":"left_right","pin_y":"bottom"}`;
  update the screen to `h: 700` and the bar stays at the bottom, 390 wide. A floating button:
  `"pin_x": "right", "pin_y": "bottom"`.
- A frame with no stored `w` (it fills or hugs) reads pins against 0: a right-pinned child's `x`
  counts from the right edge (`x: -40, w: 24` sits 16 px in).
- **`absolute: true`** takes a child of a laid-out frame out of the flow: it keeps its own `x/y`
  from the frame's top-left corner (padding ignored), takes no room, is not counted when the frame
  hugs, and can be pinned (a close icon: `absolute: true, pin_x: "right", x: -40`). It cannot
  `fill`: pin it `left_right` instead.
- A pin that cannot work is refused naming the field: a stretching pin on a line, path or group,
  or on a node whose size is not fixed.
- **`scroll: "x" | "y" | "both"`** on a frame: what overflows is clipped (`clip: false` is
  refused), and every still picture, export and `design.render` shows the start. A carousel is a
  `layout: "row"` frame with `scroll: "x"`, a fixed `w` and fixed-width cards, the next card
  peeking in. The scrolling axis needs a fixed or `fill` size, not `hug`. `design.render` does not
  flag the scrolled-away part as overflow.

## Ids: build a screen in one call

Give nodes your own ids (letters, digits, `- _ . :`, at most 64), then later edits in the same list can use them
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

## Reuse: copies, colours and styles

**`copy`** `{"op":"copy","id":"card","as":"card-2","dx":360}` copies a node and everything inside
it as one edit (`parent`, `index`, `dx`, `dy` optional; a page copies as a new page, the way to
make a variant). The copy is `as` (or an assigned id, answered in `ids`), and `copied` in the reply
maps each new root to the node it copied. Nodes inside take ids from it: an id starting with the
copied id and `-`, `_`, `.` or `:` has that start swapped (`card-title` becomes `card-2-title`), any
other gets the copy's id and `:` in front (`text-14` becomes `card-2:text-14`), so later edits of
the same list can name them. **When the ids inside share a start that is not the copied id's, pick
them with `rename`**: copying `v2-phone` with `"rename": {"v2-": "v3-"}` makes `v3-phone` (when `as`
is left out) and `v3-fade` from `v2-fade`, instead of `v3-phone:v2-fade`. A later edit naming an id
the copy did not make is refused with the ids it did make. **A connector inside the copy joins the copies of its ends; an end
outside the copy is let go**: the line stays drawn where it was, attached to nothing at that end.
`dx`/`dy` add to the copy's stored `x/y`, which a laid-out parent ignores.

**Charts are data, not drawings.** Never build a chart from hand-placed rects and labels: give a
frame (no `layout`, a fixed `w` and `h`) a `chart`, and the engine draws its parts from the data as
the frame's children, placed exactly, so every label sits under its bar:

```json
{"op":"add","node":{"id":"spend","parent":"screen","kind":"frame","w":358,"h":220,"padding":[16],"fill":"$card",
  "chart":{"kind":"bar","labels":["Jan","Feb","Mar","Apr"],"series":[{"name":"Spent","values":[420,380,515,610]}],"prefix":"$"}}}
```

`kind` is `bar` (with `stacked` or `horizontal`), `line`, `area`, `donut` or `pie`; `series` holds a
value per label (a donut or pie draws the first, a slice per label, `center` words in its hole).
Options: `values` (each value written), `legend` (on by default with several series and for a ring),
`min`/`max`, `ticks` (about how many steps, 0 for none), `grid`, `prefix`/`suffix`/`decimals`,
`smooth` and `points` (lines), `inner` (a donut's hole), `gap` (bars), `color`, `grid_color`,
`font_size`, `palette` (colours for series that name none; may be `$name`s). The parts are named
`<frame>.<part>` (`spend.bar-0-2`, `spend.label-2`, `spend.tick-0`, `spend.slice-1`) and marked
`generated`: they are redrawn whenever the chart, the frame's size or its padding changes, so an edit
of one by hand is refused (`design.chart`). Change the chart instead, and add a title or note as your
own children of the frame. `chart: null` takes the parts out.

**Components** are for anything repeated with the same build: a card, a list row, a button, a tab
bar. **Build it once, then place instances of it instead of copies**:
`{"op":"copy","id":"card","instance":true,"rename":{"card":"rent"},"dy":120}` makes an instance and
makes `card` the main component (`component: true`). The instance's nodes are real nodes you can
name (`rent`, `rent-title` by the rename), and each names the main's node it follows
(`instance_of`). **Change the main and every instance follows** when the list has run, in the same
undo step: its fields, the nodes it gains or loses, its order. An edit of an instance's node is an
override: its fields are kept (`overrides` lists them) while the rest follows; `"overrides": []`
puts it back to the main. The instance's root keeps its own place (`x`, `y`, pins, rotation,
`hidden`). Its structure is the main's: adding to, moving or deleting a node inside an instance is
refused (`design.instance`); change the main, or hide the node (`hidden: true` is an override). A
main may hold instances of another (a button in a card), and both settle. `instance_of: null` on
an instance's root detaches it into plain nodes that look as they did; deleting the main, or
`component: null`, detaches every instance. An instance inside its own main is refused
(`design.component_cycle`).

**Colours and styles** keep a design consistent and restylable. **Name the palette first**, then
the type ramp and component looks, before the nodes that use them (a `$name` or `style` the design
lacks is refused):

```json
{"op":"color","name":"ink","value":"#141414"},
{"op":"style","name":"title","set":{"font":"Geist","font_size":30,"font_weight":700,"color":"$ink"}},
{"op":"style","name":"card","set":{"fill":"#FFFFFF","radius":16,"shadow":{"y":4,"blur":16,"color":"#0000001F"}}},
{"op":"add","node":{"id":"card","parent":"page-1","kind":"frame","style":"card","w":320,"h_sizing":"hug","layout":"column","gap":8,"padding":[16]}},
{"op":"add","node":{"id":"card-title","parent":"card","kind":"text","style":"title","text":"Plan"}},
{"op":"copy","id":"card","as":"card-2","dx":360},
{"op":"update","id":"card-2-title","set":{"text":"Usage"}}
```

- `color` `{name, value}` names a colour (`#RGB`, `#RRGGBB` or `#RRGGBBAA`); any colour field of a
  node or style takes it as `"$ink"`, including gradient stops, shadows and runs. Naming it again
  changes it everywhere it is used. Motion keys take literal colours only.
- `style` `{name, set}` names a look: `set` takes the style fields (fill, gradient, stroke,
  stroke_width, dash, radius, radii, shadow, shadows, opacity, blur, blend, color, font, font_size,
  font_weight, italic, underline, strike, align, line_height, letter_spacing, text_case,
  stroke_cap, stroke_join, stroke_gradient); `null` takes a field out of the style. A node takes
  it with `"style": "card"`: each style field the node does not set itself applies (a node's own
  field wins), and changing the style restyles every node that names it.
- `delete_style` `{name}` removes a colour or style; what used it keeps its look (the values are
  written into it). Colours and styles share one namespace: names are 1 to 64 id characters, at
  most 256 together. `design.get` answers `colors` and `styles`.

## Motion

When asked to animate, make something move, or make a motion piece, give layers `motion` and the
artboard a `timeline`. Never draw a still of a motion request. A layer's own fields are its look
**at rest** (the canvas until Play, exports, `design.render` without `at_ms`): build the finished
design first, then say how it gets there. Any node but a page moves.

- `motion.keys`: 2 to 64 keys in time order, each `{"at_ms": …}` plus what it sets. Times are
  **milliseconds** (half a second is 500; the last key must be at 50 or later, none past 600000).
- **Moves**, from where layout puts it, so they work inside auto-layout frames: `dx`, `dy` (px),
  `rotate` (degrees added), `scale`, `scale_x`, `scale_y` (0 to 100), about `origin`. **Looks**,
  shown instead of the node's own: `opacity`, `fill`, `stroke`, `color` (literal colours), `blur`,
  `sweep` (an ellipse's arc: a ring that fills), `angle` (a gradient's), `trim` (0 to 1 of a line or
  path drawn on).
- Before its first key a property holds that key's value (that is how a delay is written), after
  its last it holds that one's. Between two keys it is eased.
- `ease`, on the motion or on a key (the move that starts at that key): `linear`, `ease`
  (default), `ease_in` (departures), `ease_out` (arrivals), `ease_in_out`, `overshoot` (goes a
  little past and settles: pops), `hold` (jumps at the next key: blinks).
- `origin: [0.5, 1]` turns and scales about the bottom middle (fractions of the box; default the
  centre); `loop: true` plays the layer's keys again from 0 every last-key ms (spinners, pulses).
- `timeline` `{"duration_ms": 3000, "loop": true}` on a frame or page is the clock for the motion
  inside it, the nearest one up the tree (default: as long as the latest key, looping; `loop:
  false` holds the last frame).

```json
{"op":"update","id":"hero","set":{"timeline":{"duration_ms":3000}}},
{"op":"update","id":"hero-title","set":{"motion":{"ease":"ease_out","keys":[
  {"at_ms":0,"dy":24,"opacity":0},{"at_ms":500,"dy":0,"opacity":1}]}}},
{"op":"update","id":"hero-cta","set":{"motion":{"ease":"overshoot","keys":[
  {"at_ms":300,"scale":0.6,"opacity":0},{"at_ms":700,"scale":1,"opacity":1}]}}},
{"op":"update","id":"ring","set":{"motion":{"keys":[{"at_ms":0,"sweep":0},{"at_ms":1200,"sweep":270}]}}},
{"op":"update","id":"loader","set":{"motion":{"ease":"linear","loop":true,"keys":[
  {"at_ms":0,"rotate":0},{"at_ms":1000,"rotate":360}]}}}
```

`sweep` is for an ellipse, `trim` for a line or path. **Check it:** `design.render {"design_id": 12,
"nodes": ["hero"], "frames": 6}` returns a contact sheet; look at it before you answer. If nothing
in scope moves, the reply warns and gives one picture at rest.

## Replies, revs and collisions

- A `design.apply` answers short: `ids` (each edit's node id, in order; an add's assigned one too),
  `rev` (the design's rev now) and `prev_rev`. Ask for `reply: "full"` only if you need the nodes as
  stored.
- Pass `base_rev` every time: the rev you read, then the `rev` each reply gives. The engine refuses
  (`design.stale`) only when a node your list changes, moves or deletes (or one inside a node it
  deletes) changed after that rev; otherwise it applies on top and says `rebased: true`. Adds and
  colour or style edits never collide. A `prev_rev` that is not the rev you last saw means someone
  else changed the design in between.
- On `design.stale`, read only what changed: `design.get {design_id, since_rev: <your base_rev>}`
  (`details.conflicts` names the nodes), redo the edits that touch them, and send again with the
  new `rev`. `since_rev` reaches back 1000 revs; older is `design.since_too_old`: read it whole.
- Read `warnings` (a font the desktop lacks, a rule a lenient list let through) and `skipped`
  (what did not apply, and why: `missing`, `changed`, `fields`, …).
- **`lenient: true`** is for replaying an undo or a list built from an old read: an edit whose node
  or parent is gone, or that would overwrite a change since your `base_rev`, is skipped and listed
  in `skipped`, and a rule a strict list would refuse (a field the kind does not take, a key that is
  no field) is applied as written with a warning instead of refusing the list. The default,
  strict, refuses and teaches: prefer it. A list that applies nothing changes nothing.

## Revising: change exactly what was asked

A request from the board is one line:

`[Relay design] "Settings" (design_id 12, page "Flows" (page-7)). Selected: save-button (frame "Button"). Picture of it: "/…/ab12….png". Request: make it green — read it with design.get {"design_id":12,"nodes":["save-button"]} and change it with design.apply.`

- `page "Flows" (page-7)` is the page's name and its **id**: `page` in `design.get` and
  `design.render` takes the id, or the name when no other page has it.
- With nothing selected it says `Nothing selected; picture of the whole page page-7: "….png".` and
  the read is `design.get {"design_id":12,"page":"page-7"}` (also used when more than 500 layers are
  selected). Past 8 selected layers the first 8 are named, the rest listed by id.
- The picture path is quoted: it is a PNG you can read. If it could not be attached the prompt says
  `(A picture was requested but could not be attached: …)`.
- If what was asked about moves, the picture is a **contact sheet of 6 frames** and the prompt gives
  their times (`It moves: the picture is a contact sheet of 6 frames at 0, 600, … ms, left to right
  then down`); with no sheet it says to use `design.render` with `frames` or `at_ms`.
- If you were busy when the person pressed Send, the request reached you as **priority mail**, which
  you read with your next tool call; at your prompt it was typed in.

1. Look at the picture and read the selection (`nodes`) or the page as the prompt says.
2. Change those layers with `update`: `{"op":"update","id":"save-button","set":{"fill":"#2EC469"}}`.
   `set` takes any node field except `id`, `kind` and `parent`; `null` puts a field back to default.
3. `move` reparents or reorders: `{"op":"move","id":"x","parent":"card","index":0}` (index 0 is the
   bottom / first in a layout; no index puts it last / on top). `"keep_position": true` keeps the
   node where it is drawn on the page, rewriting `x/y` for the new parent; it is refused
   (`design.position`) when it or a parent on either side is placed by auto layout, since that
   place is not stored.
4. Keep ids and the person's layers; don't delete and re-add what an `update` can change.
5. Pass `base_rev`; on `design.stale` read `since_rev` and redo only what collided.
6. Look at what you changed (next section), then say what you did.

**Undo.** One request → one `design.apply`, so the person's **Undo <agent>** button on the board
(after you change a design) takes it back in one press, each press one more of your changes; if they
edited what you changed since, it asks for a second press. **Ctrl+Z undoes only the person's own
edits**, never yours. A change whose undo is over 256 KiB (deleting thousands of layers) has no
undo: keep each apply to one screen.

**On the canvas** the person scrolls a `scroll` frame with Alt+wheel (view only, never stored). A
vector path (a `line` with points) turns by rewriting its `points` about the middle of their box,
since a `line` takes no `rotation`; a `path` node takes `rotation`.

## Check what you drew

You cannot measure text, and `design.get` reads hugged text as `w 0, h 0`. After you draw or change
a screen, **look at it**: `design.render {"design_id": 12, "page": "page-7", "nodes": ["login"]}`.

- `png.path` is a picture of the page, or of `nodes` (the outermost of up to 200, on one page); the
  MCP tool shows it to you as an image. `png: false` gives boxes and issues alone; `scale` 0.25 to
  2 and `max_side` 64 to 2560 (default 1568) bound its size.
- `area` is the page rectangle pictured: picture pixel (u, v) is page point (`area.x + u/scale`,
  `area.y + v/scale`).
- `boxes.rows` are `["id","kind","parent","x","y","w","h","rx","ry","rot"]` for every visible node,
  laid out, hugged text measured: x, y, w, h in page px, rx, ry from the parent's origin. At most
  `boxes_limit` rows (default 200, most 500); `boxes.total` counts all, `boxes.hidden` the hidden.
- `issues` (worst first, at most 100; `issue_count` counts all) name layout problems by `code`:
  `overflow` (a child past its frame's padding), `overlap` (siblings laid over each other in a
  layout), `text_overlap`, `clipped` (cut off by a clipping frame), `contrast` (words too faint on
  what is painted under their centre: through a ring's hole or past a round corner, not the shape's
  box). Fix them and render again. Read `warnings` too. A node you mean to leave as it is (words
  meant to be faint, a caption on a photo the check cannot read) takes `"skip_checks": true`, and
  no issue names it first again; never use it to quiet a real problem.
- Motion: `at_ms` (0 to 600000) pictures one time, `frames` (2 to 24) a contact sheet labelled by
  `times_ms`; not both (`design.render.args`). With either, `issues` also lists what the motion
  brings that the still does not have, each with `at_ms`, the first time it shows (a slide-in drawn
  over a label, a fade that leaves words faint): you may not see a later frame clearly, but you will
  hear of it. `frames: 12, png: false` checks a motion without a picture.
- It is drawn by Relay's desktop app. **`design.render.no_renderer`** means none is open: tell the
  person to open Relay, carry on without sizes (never guess them) and say you could not look. The
  other refusals say what to do: `busy` (try again in a few seconds), `timeout` and
  `renderer_gone` (once more, then say so), `args` (an option out of range). An open board shows a
  note that you looked. `base_rev` refuses a picture of a design that moved on (`design.stale`).

## Errors

A refusal names the edit and the field: `ops[3] update "hero": fill: "red" is not #RGB, #RRGGBB,
#RRGGBBAA or a $name colour`. A misnamed field, id or page gets a suggestion (`"fil" is not a node
field; did you mean "fill"?`, `did you mean "card-title"?`). Nothing in a refused list was applied;
fix that edit and send the list again. `details` carries `{index, id, field}`.

| code | meaning |
| --- | --- |
| `design.node_invalid` | a field's value or kind rule; the message says which |
| `design.node_not_found`, `design.page_not_found` | an id the design lacks (the message names the nearest) |
| `design.node_exists`, `design.parent`, `design.cycle`, `design.too_deep` | an id taken; a parent that is gone or holds nothing; a move into itself; nesting past 64 |
| `design.style_not_found`, `design.style_invalid`, `design.style_exists` | a `$name` or style the design lacks; a bad colour; a name taken |
| `design.position` | `keep_position` where a place is laid out |
| `design.stale` | a node you write changed since `base_rev`; read `since_rev` |
| `design.too_large` | a limit below |
| `design.asset_not_found` | an image `src` that `design.asset.add` did not store |

**Limits:** 5000 nodes, 1536 KiB of node JSON (as `design.get` returns it) and 64 levels of nesting
per design; 2000 edits per `design.apply`; 256 named colours and styles; 10000 characters per text;
4000 points per line or path; ids 64 characters, names and fonts 200; 1 to 8 shadows, 1 to 64 runs,
2 to 64 motion keys, 2 to 8 gradient stops; coordinates and sizes within ±1,000,000. A connector's
`from` and `to` are nodes on its own page; an image's `src` is one `design.asset.add` returned.

## Make it look real

- **Artboards**: name them (`"Settings · iPhone"`); phone 390×844, tablet 834×1194, desktop
  1440×900; put variants side by side 80 px apart on the page, flows left to right with arrows
  (`line` with `arrow_end: true`) between them.
- **Rhythm**: spacing and padding on an 8 px scale (4, 8, 12, 16, 24, 32, 48); radii 8–16 for cards
  and buttons, consistent across a screen.
- **Type**: one ramp — 32–40/700 titles, 20–24/600 headings, 15–16/400 body, 12–13/500 captions;
  line height 1.2 for headings, 1.4–1.5 for body; slight negative tracking on large titles.
- **Colour**: name the palette (`color`) and the type ramp (`style`) first and restyle by changing
  those. A neutral ground, one accent used for one job (the primary action), text contrast of at
  least 4.5:1 (`design.render` flags `contrast`); check dark variants separately.
- **Content**: real, specific copy from the product — names, numbers, states — never lorem ipsum.
  Show states that matter (empty, error, loading) as their own frames when asked about a flow.
- **Structure**: a frame per component (card, row, button) so the person can select and restyle it;
  stickies (`sticky`) for notes and open questions beside the artboards rather than inside them.
