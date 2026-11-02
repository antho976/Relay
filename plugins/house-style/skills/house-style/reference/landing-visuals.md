# Landing pages, hero visuals and the unslop story video

For surfaces meant to persuade or show off: a landing page, a hero, an empty state, onboarding, a
release beat. App screens almost never need any of this (`../SKILL.md`). Adapted from **unslop** by
Damir Sokolovsky ([@heydamir](https://x.com/heydamir)), CC BY 4.0, condensed for this package. Keep
the credit.

## What good looks like

- **One bright element** carries the colour and the craft (a rendered object, a painted scene, a rich
  chart), and everything around it stays quiet.
- **A clear idea:** you can say in one sentence what the visual shows and why it belongs to this
  product.
- **Text on calm space:** the headline sits on space left empty for it, with one primary and one
  secondary action and nothing stacked under them.
- **Real product, real material:** previews look like the real app with plausible data, and objects
  have weight, light and shadow.

Before choosing a visual, look at the finished examples in `refs/` and take their qualities
(composition, light, restraint, how the moment is placed), never their subject or layout:

| File | Why it works |
| --- | --- |
| `unslop-hero-after.webp` | An invoicing hero: a paper plane drawn in digits and currency signs, the product rising from below. The visual is made of the product's own material, so it's the moment and the signature at once |
| `unslop-dashboard-after.webp` | Quiet graphite chrome, with all the colour in two moments: rendered payment cards and a chart where each colour is a series |
| `beacon-engraving.webp` | One blue engraving of a lighthouse with a serif headline in the same ink. A print style works because the whole page lives in it |
| `meadow-3d.webp` | A soft 3D meadow footer with type that becomes part of the scene, and quiet links on open sky |
| `story-dashboard-frames.webp` | Eight frames of the story video (below) |

## Hero layout

- **Strongest: a scene.** One generated image fills the hero edge to edge, its subject on one side,
  the headline on the calm part of the same image, upper area, at the optical centre (a little above
  the true middle), with the product view rising from the bottom edge and running past it.
- **Without a scene: an asymmetric split.** Text on one side, the visual at least half the width,
  centred vertically against each other (shorten a tall visual rather than push the text down), or
  text over a large product view below. Prefer one solid visual to floating cards. Everything stacked
  and centred is the template default: keep it only for a short hero with no visual.
- The hero fills the first screen with no dead band. Shorter content is centred, or the product view
  starts low and runs past the edge to invite a scroll.
- Spacing: 96–128 px from the navigation to the headline on desktop (48–64 on mobile) in a stacked
  hero. Buttons 44–48 px tall. Headline 56–72 px on desktop and 36–44 on mobile, line height
  1.02–1.08, weight 400–500. Subhead 18–20 px in the secondary colour, at most 36 rem wide.
- Small screens stack in reading order, with the visual after the actions. A dense preview gets
  simpler (fewer rows and columns) rather than zoomed down, and no text in it goes under 11 px.

## Choosing the visual

1. **Does the screen need one?** Landing heroes and empty states usually do. Dashboards and settings
   don't.
2. **Show the product.** Build a preview in code of the one moment that proves the headline (a
   scheduling tool shows one week with a booked slot), with the project's components or careful HTML
   and plausible data. Showing what the copy already claims is not inventing a feature. It carries a
   moment, like a rendered object or a rich chart.
3. **With an image tool, add a scene from the world.** Write it as one plain sentence first ("a single
   cup of tea steaming on a windowsill at first light"). If it doesn't fit in one sentence, it isn't
   clear yet. Avoid the AI defaults: glossy blobs, orbs, nebulas, chrome liquid, cubes, neon
   circuits, holograms, glowing brains, robots, rockets, crystals.

**Styles that work** (pick one for the whole page): painted background art (crisp clouds and soft
light with open sky, for warm and human products); a cinematic photo of one object in dramatic light
(calm, serious); soft stylised 3D with matte materials (playful, friendly); a product object render
(anything physical); light and optics on a dark field (technical, data); an engraving or woodcut in
one ink on a tinted page, only when the whole brand lives in it; a code treatment over an image
(below).

**What fails:** prompts asking for film or paper grain (models bake in a checker pattern, so add
texture in code), a print style on one image of a plain modern page (it reads as a filter), busy
collages, and purple-blue glossy abstractions.

**Objects for moments.** One object, studio lit, on a plain background in the exact page colour,
with a soft contact shadow and the angle the layout needs ("tilted 20 degrees, seen slightly from
above"). Cut it out if the tool can, otherwise mask it into the page. Place it overlapping a card's
edge, slightly rotated, shadowed in the ramp's darkest colour. Background removal eats dark areas
near the background colour (a black phone screen), so ask for the screen in a contrasting flat
colour. For real UI on a generated device, generate it straight on, measure the screen rectangle,
lay HTML over it and rotate both together. Everything in one visual tells the same story: the
invoice in the list is the one on the phone.

## Generating it

- Find an image tool among the available tools (an MCP server for fal, Replicate, OpenAI images,
  Higgsfield, a design tool). In Relay, real photographs come only through `design.picture.search`
  (load `design-pictures`).
- Show the owner the prompt, model and size and ask before running, because it can cost money. Run
  once, judge it against the sentence and the palette, and change the prompt rather than rerolling.
- Save into the project's assets, never a temporary URL.
- No tool? Say so before the visuals pass and offer to connect one. If skipped, leave an empty slot:
  a plain surface step of the exact size and radius with no text inside, so nothing unfinished ships.
  Put the prompt and the file name to save under in the report.

**Prompt recipe, in order:** subject (the one sentence); setting, kept simple; light and palette,
naming the page background colour so the edges can blend and the light's direction; one style;
composition ("subject on the right half, calm empty space on the left"; for a scene behind the whole
hero, 16:9 or wider with the lower fifth calm too); exclusions (no text, letters, logos, UI or
watermark); format (the slot's aspect ratio at twice the display size).

**On the page:** WebP or AVIF at twice the display size with real `alt` text. Match the edges to the
page with a mask (`mask-image: radial-gradient(closest-side, #000 70%, transparent)`) or an overlay in
the page colour, because a hard rectangle in a different black is slop. Static, or one slow subtle
motion that fits the subject, and no parallax. High load priority and fixed dimensions so nothing
jumps.

## Code treatments over an image

Optional, on the main visual only, and only when it fits the character. Never as decoration
elsewhere.

- **Glyphs:** the image drawn as a grid of characters, brightness choosing the character, ideally from
  the product's own material (finance `. : 1 7 4 0 8 $ %`, code `. : / { } < > = #`, monitoring
  `. · : | + ▪`). Cells 6–10 px in the chosen mono, coloured from the image or the ramp, with a few
  cells changing slowly so it feels alive.
- **Halftone dots:** the same sampling, drawing circles of radius `light × cell / 2` in one ink.
- **Dithering:** ordered or error diffusion into two or three ramp colours, done once on load.
- **Duotone:** brightness mapped onto the darkest surface and the accent: a greyscale image under a
  `mix-blend-mode: color` layer, or an SVG `feColorMatrix`.

```js
function glyphs(canvas, image, { cell = 8, chars = ". :1740$%", font = "Geist Mono", fps = 8 } = {}) {
  const ctx = canvas.getContext("2d");
  const columns = Math.floor(canvas.width / cell), rows = Math.floor(canvas.height / cell);
  const sample = Object.assign(document.createElement("canvas"), { width: columns, height: rows });
  const sampler = sample.getContext("2d", { willReadFrequently: true });
  sampler.drawImage(image, 0, 0, columns, rows);
  const pixels = sampler.getImageData(0, 0, columns, rows).data; // read once, never in the loop

  const atlas = Object.assign(document.createElement("canvas"), { width: cell * chars.length, height: cell });
  const pen = atlas.getContext("2d");
  Object.assign(pen, { font: `500 ${cell + 1}px ${font}`, textAlign: "center", textBaseline: "middle", fillStyle: "#fff" });
  [...chars].forEach((char, i) => pen.fillText(char, i * cell + cell / 2, cell / 2));

  const cells = [];
  for (let y = 0; y < rows; y++) for (let x = 0; x < columns; x++) {
    const i = (y * columns + x) * 4;
    const light = (0.2126 * pixels[i] + 0.7152 * pixels[i + 1] + 0.0722 * pixels[i + 2]) / 255;
    cells.push({ x, y, light, color: `rgb(${pixels[i]},${pixels[i + 1]},${pixels[i + 2]})`, seed: Math.random() });
  }
  const tinted = Object.assign(document.createElement("canvas"), { width: cell, height: cell });
  const tint = tinted.getContext("2d");
  const draw = (step) => {
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    for (const c of cells) {
      if (c.light < 0.06) continue;
      const jitter = Math.sin(c.seed * 999 + step) > 0.92 ? 1 : 0;
      const pick = Math.min(chars.length - 1, Math.floor(c.light * chars.length) + jitter);
      tint.globalCompositeOperation = "copy";
      tint.drawImage(atlas, pick * cell, 0, cell, cell, 0, 0, cell, cell);
      tint.globalCompositeOperation = "source-in";
      tint.fillStyle = c.color;
      tint.fillRect(0, 0, cell, cell);
      ctx.drawImage(tinted, c.x * cell, c.y * cell);
    }
  };
  let frame = 0, last = 0, step = 0, visible = true;
  new IntersectionObserver(([entry]) => (visible = entry.isIntersecting)).observe(canvas);
  const loop = (now) => {
    frame = requestAnimationFrame(loop);
    if (!visible || now - last < 1000 / fps) return;
    last = now;
    draw(step++);
  };
  frame = requestAnimationFrame(loop);
  return () => cancelAnimationFrame(frame);
}
```

These effects have broken weak laptops before: read pixels once on load, prepare atlases and tinted
cells ahead, animate at 8–12 steps a second changing a small share of cells, pause off screen, and
cap the canvas at 2× device pixel ratio. On a large canvas, bake each cell's tinted glyph once and
redraw only the cells that change.

## The story video

A roughly 10 second square video in which the screen unslops itself, with red redlines on the slop.
It takes a few minutes, so make it only when asked. Look at `refs/story-dashboard-frames.webp`
first. How it works: nothing is swapped and everything morphs (each step only changes CSS, so values
tween in place); changes run in a wave from the top left; changed copy blurs out and the new words
rise in letter by letter; redlines come first (red boxes on the slop, red spacing measurements that
turn white once fixed, corner radii outlined); and the frame is cropped close on what changes most.
For a release video in Relay's own style, the Video plugin (`video-code`) is the better tool.

1. **`unslop-story/story.html`:** a standalone copy of the original screen with its styles inline.
   The original rules are step 0.
2. **Three to five steps,** merging passes so each one visibly changes something, listed as
   `<meta name="unslop-steps" content="Spacing, Color and icons, Type and copy, Details">`. A name
   containing Spacing shows spacing measurements, and one containing Details shows corner outlines.
   A new chart or visual goes in the last step.
3. **Each step is CSS on top,** under `.s1`, `.s2` and so on (`.s1 .card { padding: 24px }`). Steps
   accumulate. Change values, never structure.
4. **What can't tween:** put gradients, glows and washes on their own layer and fade its opacity
   (gradient text goes on the `.from` span only). Write pill radii as half the height (`23px`), not
   `9999px`. Changed words use `<span class="swap" data-at="3"><span class="from">old</span><span
   class="to">new</span></span>`, at most eight in a story, because weight, size, tracking and colour
   tween on their own. Emoji becoming icons: stack both in one box and crossfade (not a swap).
   `data-out="2"` collapses an element at step 2, and `data-in="4"` fades one in at step 4 (reserve
   its space).
5. **Redlines:** `data-gap` on the blocks of a vertical stack, four to six per stack, named per stack
   (`data-gap="sidebar"`). `data-radius` on two to four elements whose corners change. Optionally,
   `data-slop="Gradient text"` on at most five clear examples.
6. **Only the screen:** no window or shadow around it. Render 1440 wide and at least 900 tall, don't
   size from `100vh`, and use `position: fixed` only for background layers. A window that grows is
   anchored left.
7. **Camera:** `<meta name="unslop-frame" content="x y width">` in page pixels, starting at the
   screen's top left, cropped close.
8. **Render:** `node <skill folder>/scripts/story.mjs unslop-story/story.html unslop-story.mp4` (square;
   `--wide` for 16:9; `--play` instead of the output to loop it in a window). It needs Playwright and
   ffmpeg: if one is missing, give the one install command. Pull a few frames with ffmpeg and look at
   them before handing it over.
9. The story file is a presentation piece. The clean result ships as normal code, and every video
   keeps its small `unslop by @heydamir` credit, as the licence asks.
