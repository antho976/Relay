# Unslop: from AI-looking to designed

Adapted from **unslop** by Damir Sokolovsky ([@heydamir](https://x.com/heydamir),
heydamir.design/unslop), licensed CC BY 4.0. Changed for this package: merged with the house style,
its values moved into `fundamentals.md` and `motion.md`, and its rules overridden where the owner's
taste differs (noted inline). Keep this credit in any copy.

Vibecoded UI fails in the same ways every time: every surface glows, every label has an emoji,
spacing is whatever was typed that day, and the copy shouts. Unslopping fixes the craft and keeps
the product: the features, content and order of sections stay, and every size, colour, word and
image becomes deliberate. Slop spreads colour and effects over the chrome. Craft keeps the chrome
quiet and puts the richness into one or two moments of content: a chart where each colour means a
series, a real object, a figure set well.

## Run it

1. **Look at the screen** as people see it: a render or screenshot, not only the code.
   - Only a live URL? Save it with `node <skill folder>/scripts/snapshot.mjs <url> unslop/original.html`
     (`--dark`/`--light` to match what the person sees). It also saves `original.live.png`. Compare
     the two before starting. Script-drawn animated backgrounds don't survive, so describe them from
     the live page.
   - Only a screenshot? Rebuild it as one static HTML file first and treat that as the original.
   - In both cases the result is a redesigned page in code for whoever owns the real site. Say so.
2. **Audit** with the signs below, one line each, saying where each one lives.
3. **Decide how much to change**, honestly:
   - *Heavy slop* (signs across colour, type, icons and copy): the full process.
   - *Light slop*: fix only what the audit found, and keep the palette, type, layout and visuals.
     Bigger ideas go into the report as suggestions.
   - *No slop*: say so, and offer two or three refinements with exact values. Apply them only if the
     owner agrees. Never change a good screen just to make it different.
4. **Set the direction and stop:** show the audit and the direction, then wait, unless they said to
   just do it. A redesign is one version beside the before unless they ask for options (`../SKILL.md`).
5. **Fix in nine passes, in order,** each across the whole screen before the next: spacing, colour,
   icons, copy, type, layout, details, visuals, motion. Spacing comes first because everything reads
   differently once the rhythm is right. Copy comes before type and layout because the headline's
   length decides its size. Visuals and motion come last because they fit a screen that already
   works without them. When nearly every line changes, plan the passes, then write the screen in one
   go with every pass applied.
6. **Check** with the checklist, render again and compare with the audit. Render at most three
   times in a run. If an image is generating, keep doing passes while it runs.
7. **Report** the direction, a few lines per pass, and any image prompt written.

## Direction

In the owner's own apps the direction is already set: the house look, the project's tokens and its
fonts. Skip to the passes. Write a direction only for a new product or a page meant to persuade (a
landing page), and derive it from what the product does, never from what's trending:

- **Product:** what it does and for whom, in one sentence.
- **Character:** two or three concrete words ("calm, exact, a little warm"). Clean, modern, sleek,
  minimal and premium are banned, because they describe every template.
- **World:** the place or material the product belongs to (a notes app near paper and ink, a
  monitoring tool near instruments and night shifts). It tints the neutrals, the accent and the
  visual.
- **Moments:** one or two places that get rich colour and detail, chosen from what the product shows.
- **Signature:** one small detail only this product would have, used consistently (a live status dot,
  keycaps, a route line). One, not five.

A real brand (a logo, colour or typeface that isn't slop) is kept and built around. A violet-to-blue
gradient, sparkles and glow are slop even in a logo, unless the owner says otherwise.

## The passes

The values for each pass are in `fundamentals.md`: spacing scale and density (§3), neutral ramps,
accent and depth (§4), type scale, tracking and weights (§2), radius and nested radius (§6), charts
(§10), icons (§11). Motion values are in `motion.md`. What follows is what each pass looks for.

1. **Spacing:** everything on the 4 px grid; equal things equal; groups tighter inside than between;
   one aligned column for icons, labels and numbers; one-off values (`mt-[13px]`) replaced with steps
   from the scale. Measure the sidebar afterwards, because that's where spacing breaks most.
2. **Colour:** one neutral ramp, three text roles, one accent used rarely (the primary action, the
   selection, the one number that matters). App surfaces stay neutral: a background that reads as
   orange, blue or green has gone too far. The world shows in the accent and the moments. Remove hue
   gradients, gradient text, coloured glows and shadows, colour washes and neon borders. A gradient
   is allowed only as a small tone shift inside one neutral.
3. **Icons:** every emoji used as an icon or as decoration in a label becomes a line icon from one set,
   or nothing. Categories that need colour get a small status dot.
4. **Copy:** headlines say what the product does for whom, using facts already on the page. No
   greeting headlines ("Welcome back, Damir! 👋" becomes "October 2026"), no exclamation marks, no
   trailing periods on short strings, short labels ("Search", not "Search anything..."), changes
   that say what they compare to ("+12% vs last month"), plausible mock data. Unverifiable claims
   ("Loved by 10,000+ teams") are made quiet and specific if the owner confirms them, and removed
   otherwise. `PRO`, `New ✨` and `Beta 🚀` go, or become quiet text.
5. **Type:** the project's fonts stay (house style: never change fonts unasked). For a new product,
   one interface family plus at most one mono or display partner. Sentence case, no tracked uppercase
   labels, no italic for emphasis, no negative tracking on small text, one number format with
   tabular figures.
6. **Layout:** keep the sections and their order. Apps keep real app proportions (a 240–280 px
   sidebar and fluid content capped near 1440–1600 px), never shrunk into a centred column. One
   primary and at most one secondary action in a hero. Quiet navigation: logo, existing links, one
   button, with log-in as a text link. Long headlines align left. Landing heroes are in
   `landing-visuals.md`.
7. **Details:** nested radii add up, and not everything is a pill. Counts and meta become quiet mono
   text instead of glowing pills, and shortcuts become keycaps. Charts get a hierarchy and one accent
   each. The active state is a small surface step, not a gradient with a glow. Hover and focus exist
   and stay inside the palette. The logo is the product name in the main family plus a simple flat
   mark. The signature detail appears where it belongs and nowhere else.
8. **Visuals:** decorative filler goes (blobs, orbs, nebulas, floating cubes, glowing grids,
   holographic brains, robots). App screens rarely need generated images: their moments come from
   the product (a chart drawn with care, figures set well, real content). Generate images only for
   empty states, onboarding, marketing or on request (`landing-visuals.md`).
9. **Motion:** remove pulses and pings on things that aren't live, shimmer, breathing glows,
   floating elements, parallax, `transition-all`, bouncy springs and hover scale. Keep motion that
   explains something (`motion.md`). At most one entrance on load: the main content fading and
   rising with a small stagger.

**House style overrides.** The original unslop said stat cards are numbers only. Here, a section with
numbers leads with a mark, because the owner wants graphs, but never one decorative sparkline per
card (that's the stat-card wall in `failures.md`). It used hairlines between things. Here, air
separates, and a hairline appears only where space alone can't. It allowed weights 400 and 500 only.
Here, the type rules in `fundamentals.md` apply.

## Signs of slop

Each sign has its fix. Failures already named in `failures.md` (round everything, futuristic
chrome, hairline habit, stat-card wall, the AI look) are not repeated here.

**Surfaces and colour.** Purple-to-blue or violet-to-pink gradients on backgrounds, cards, buttons or
the active nav item → one neutral surface. Gradient text → solid primary text. Coloured glows
(`shadow-[0_0_20px_rgba(139,92,246,0.5)]`, drop shadows on emoji) → depth from surface steps. Radial
washes and blurred blobs behind content → remove. Five near-identical lavenders for text → three
text roles. Everything highlighted → one accent, used once or twice. Glass cards with a white inner
highlight and a coloured shadow → a plain raised surface.

**Type.** Headings at 800–900 → size carries hierarchy. `NAVIGATION`, `PAID THIS MONTH` → sentence
case. Too many sizes and weights → the ramp. `$48,210` beside `$124,860.40`, or `07` → one format,
tabular figures. Negative tracking on small text → zero. Italic for emphasis → weight or colour.

**Icons.** Emoji as nav icons, label decoration or status (🏠 📥 🚀 💰 ⚠️ 📈 🔥) → one line set or
nothing. Emoji inside counts (`18 👤`) → the number. Icons in many sizes and colours → one size, one
stroke, the text role's colour. Icons off the column → a fixed icon box. ✨ for anything AI → remove.

**Spacing.** Arbitrary values → scale steps. Rows of different heights in one list → one height.
Groups closer to their neighbours than to their own items → regroup. Uneven padding in sibling cards
→ one padding. A big gap above a section and cramped space below it → a balanced rhythm.

**Copy.** Greeting headlines, `!!`, unreferenced changes (`+12%!!`), wordy placeholders, hype words
(supercharge, seamless, unlock, magic, revolutionize, AI-powered on everything), trailing periods on
labels, fake data (`$123,456`, `[X]`).

**Components.** Glowing pills for counts, plans or shortcuts → quiet mono text or a keycap. Badges
everywhere → none or one quiet one. Rainbow, glowing or pill-shaped bars and uppercase day labels →
one or two series on a muted grid. Every chart in shades of the accent → neutral steps with the
accent on the one thing that matters. Equal cards side by side, each with the same chart → one main
chart, compact figures, varied widths. A gradient tile logo with a sparkle, or a glossy orb mark →
the wordmark plus a flat glyph.

**Landing and hero.** A pill above the headline (`✨ Introducing X 2.0`) → remove, or a quiet link
if it's real news. A headline that would fit any product → what this one does, with its own nouns.
Everything stacked and centred → a split or text over a large product view. Three or more buttons, or
chips and avatars under them → one primary and one secondary. Avatar rows and five stars with no
source → verify or remove. Feature cards with an emoji, a bold title and a vague line each → fewer
features, concrete copy.

**Visuals.** Glossy 3D blobs, liquid chrome, a purple nebula, stock robots, glowing brains, circuits
→ the product itself or one concrete metaphor. Text or fake buttons baked into an image → real text.
An image with a hard edge in a different black → matched background and a soft mask. A visual on
every section → one, usually in the hero.

**Template look** (not slop, but where a cleaned-up screen falls when nobody decides). A plain grey
page, Inter and the old brand colour on one button, for a product whose world is warm or bright. A
headline that still fits if you swap the product name. A hero visual that's an empty box. Every
product ending up with the same layout and palette.

## Do not

- Rename the product, even when its name has AI in it, or change what it is: its features, content
  and section order stay unless asked. Meaningless placeholder content may be replaced with
  meaningful content.
- Replace removed decoration with new decoration, or swap one trend for another (glass, grain, noise,
  neon, bento grids).
- Invent features, claims or data.
- A slop logo (an emoji, a sparkle in a gradient tile) is redrawn flat: the wordmark alone, or one
  simple glyph in one solid colour.

## Checklist

- [ ] Direction set (or the house look followed), and every pass follows it
- [ ] Spacing on the 4 px scale, equal things equal, icons, labels and numbers in aligned columns
- [ ] One neutral ramp, three text roles, one accent with enough contrast
- [ ] No hue gradients, gradient text, glows, coloured shadows, colour blobs or emoji
- [ ] One icon set at one size and stroke
- [ ] Type from the ramp, no tracked uppercase, no italic, no negative tracking on small text, one
      number format with tabular figures
- [ ] No exclamation marks, trailing periods on short strings, greeting headlines or unverified claims
- [ ] Nested radii add up
- [ ] A hero has a real visual or a product preview, never filler and never a labelled placeholder
- [ ] No looping or bouncy motion, no animated layout properties
- [ ] Template check: swap in another product's name. If the screen still fits, push the copy, the
      visual and the signature detail harder
