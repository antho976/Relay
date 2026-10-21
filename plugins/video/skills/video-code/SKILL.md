---
name: video-code
description: Make videos with code in Relay - a video is a folder of React components that Remotion draws frame by frame (compositions, props and zod schemas, timing with useCurrentFrame, interpolate and spring, Sequence, Series and TransitionSeries, Img, OffthreadVideo and Audio with staticFile), Relay's kit (@relay/video - Title, Words, FitText, Counter, Highlight, Phone, Window, Tap, Cursor, Captions, Gradient, Grid, Glow, Grain, safe areas, bundled fonts, motion helpers), the loop of editing with your file tools then video.check (errors at file and line) and video.still (a picture or contact sheet with the problems found - clipped, offscreen, unsafe, contrast, small_text, text_overlap, font_fallback), assets through video.asset.add, putting a screen from a Relay design into a video, rendering with video.render and waiting on video.render.finished, versions, and the craft of pacing, type and layout for promos, explainers and social clips. Use whenever a task asks to make, edit, check or render a video, an animation to export as a video, a promo, a reel, a short, or a message starts with "[Relay video]".
---

# Videos as code in Relay

**Act only when asked.** A video is the person's: start, change or delete one only when they asked
(a task, a message, or an Ask from the Design view). Every version of its code is kept, so they can
go back.

A **video** is a folder of React code that [Remotion](https://www.remotion.dev) draws one frame at a
time, plus a row in Relay. You edit the folder with **your own file tools** (Read, Write, Edit);
Relay builds it, checks it, shows you frames of it, and renders it. The person watches it in the
project's **Design** view.

## The loop

1. `video.list {project_id}` — is there one already? Work on the one the person means.
2. `video.create {project_id, title, shape?}` — only when asked for a new one. Answers `dir` (the
   folder), `entry` (`src/Video.tsx`) and the kit's state.
3. Edit `src/Video.tsx` and friends with your file tools.
4. `video.check {video_id}` — after **every** edit. Fix every error at its `file`:`line`. `ok: false`
   means it does not build or throws; type errors leave `ok: true` but fix them anyway.
5. `video.still {video_id, count: 8}` — a contact sheet across the whole video, every frame
   labelled `1.5s · f45`; or `at: ["0.5s", "2s", "end"]` for exact moments. Read the picture **and**
   `issues`; fix what they show, then look again.
6. `video.render {video_id}` when the person wants the file. Wait:
   `bus.wait {events: ["video.render.finished"], matching: {"id": <render id>}, timeout_ms: 600000}`,
   then give them `path` (and say how long it is and its size).

If a call answers `video.kit.missing`, call `video.kit.install` once and
`bus.wait {events: ["video.kit.changed"], timeout_ms: 900000}` until `state` is `ready` (it downloads
about 600 MB the first time; say so). `video.kit.no_node` means Node.js is missing: ask the person.

## The tools

| Tool | Use |
| --- | --- |
| `video.list {project_id, session?}` | The project's videos, newest change first, each with its `dir`, size, length and last render. |
| `video.get {video_id}` | One video: its `dir`, `entry`, compositions (with their props schema), the props the person set, the last 20 versions and renders. |
| `video.create {project_id, title?, shape?, width?, height?, fps?, seconds?}` | A new folder from the starter. `shape`: `vertical` (1080×1920, default), `landscape` (1920×1080), `square`, `portrait` (1080×1350). Sizes must be even. |
| `video.check {video_id}` | Build + typecheck + list compositions. Errors: `{kind, file, line, col, message}`; kinds `syntax`, `bundle`, `type`, `runtime`, `probe`. |
| `video.still {video_id, at? \| count?, from?, to?, composition?, scale?, props?, boxes?}` | The picture (`png`, attached through MCP) and `issues`; `boxes: true` adds every `data-id` element's box per frame. Default: 6 frames across it. |
| `video.render {video_id, format?, quality?, from?, to?, composition?, props?}` | A render job; answers the render row at once. `format`: `mp4` (default), `webm`, `gif`, `prores` (keeps transparency), `mp3`, `wav`. `quality`: `preview` (half size, quick), `standard`, `high`. Writes `out/<composition>-<id>.<ext>`. |
| `video.render.get {render_id}` / `video.render.cancel {render_id}` | Where a render is; stop one. |
| `video.asset.add {video_id, path, name?}` | Copy a file (picture, clip, sound, font) into `public/assets/`; answers `static_file` for `staticFile()`. |
| `video.update {video_id, title?, composition?, props?}` | Rename; pick the default composition; set props (`{"main": {...}}`, `null` to reset). |
| `video.version.restore {video_id, version}` | Put the code back as a version had it (the current code is recorded first). |
| `video.delete {video_id}` / `video.restore {video_id}` | Only when the person asked. |
| `video.kit.status {}` / `video.kit.install {}` | Whether this machine can render; set it up. |

Moments (`at`, `from`, `to`) are seconds as a number (`1.5`), or `"1.5s"`, `"1500ms"`, `"0:01.5"`,
`"f45"` (a frame), `"50%"`, `"start"`, `"end"`. Past the end is the last frame.

## The folder

```
<dir>/
  src/index.ts     ← Relay's: registerRoot(relayRoot(Root)). Never change it.
  src/Root.tsx     ← compositions: id, size, fps, length, schema, defaultProps
  src/Video.tsx    ← the video; split scenes into their own files as it grows
  public/          ← what staticFile("…") names; video.asset.add puts files in public/assets/
  out/             ← renders
  tsconfig.json    ← extends the kit's settings
```

Packages you may import (nothing else is installed, and nothing can be): `remotion`,
`@remotion/transitions` (+ `/fade`, `/slide`, `/wipe`, `/flip`, `/clock-wipe`, `/none`),
`@remotion/captions`, `@remotion/layout-utils`, `@remotion/shapes`, `@remotion/paths`,
`@remotion/noise`, `@remotion/media-utils`, `@remotion/motion-blur`, `@remotion/lottie`,
`@remotion/fonts`, `@remotion/zod-types`, `zod`, `react`, and Relay's kit `@relay/video`.

## Remotion's rules (break one and frames flicker or differ between renders)

- **Everything is a function of the frame.** Read `useCurrentFrame()` and `useVideoConfig()`
  (`fps`, `durationInFrames`, `width`, `height`); compute every position, opacity and colour from
  them with `interpolate(frame, [a, b], [from, to], {extrapolateLeft: 'clamp', extrapolateRight:
  'clamp', easing})` or `spring({frame, fps, config: {damping}})`.
- **Never** CSS `transition`/`animation`/`@keyframes`, `setTimeout`, `setInterval`,
  `requestAnimationFrame` loops, `Date.now()` or `Math.random()` (use `random("seed")` from
  `remotion`). Frames are drawn out of order, in parallel tabs.
- Time in frames: `Math.round(seconds * fps)`. Think in seconds, convert once.
- `<Sequence from={30} durationInFrames={60}>` shifts time: inside it `useCurrentFrame()` starts
  at 0. It wraps children in an `AbsoluteFill` unless `layout="none"`.
- `<Series>` plays `<Series.Sequence durationInFrames>` one after another — scenes in order.
- `<TransitionSeries>` is Series with `<TransitionSeries.Transition presentation={fade()}
  timing={linearTiming({durationInFrames: 15})} />` between scenes; each transition **overlaps**
  its neighbours, so the total is the sum of scenes minus the transitions.
- Media: `<Img src={staticFile("assets/x.png")} />` (waits for the picture, unlike `<img>`),
  `<OffthreadVideo src=… />` for clips (`startFrom`, `endAt`, `muted`, `volume`),
  `<Audio src=… volume={(f) => …} />` for sound. Only `staticFile()` paths: no web addresses.
- Async work in a frame (fetching a JSON file from `public/`) goes between
  `delayRender()` and `continueRender(handle)`.
- A video whose length depends on data (an audio file's length) sets it in the composition's
  `calculateMetadata` (`getAudioDurationInSeconds(staticFile(…))` from `@remotion/media-utils`).
- Props must be plain JSON. Describe every prop in the zod `schema` (`.describe("Headline")`) and
  use `zColor()` from `@remotion/zod-types` for colours: the person edits them in the Design view
  without touching code, and `video.update {props}` checks values against the schema.

## The kit: `import { … } from '@relay/video'`

Motion (all pure functions of the frame):

| Export | Use |
| --- | --- |
| `useProgress({delay, duration})` | 0 → 1, eased out, after `delay` frames over `duration` (default 18). |
| `useSpring(delay, damping = 200)` | A spring 0 → 1; damping 200 settles, 12 bounces. |
| `useExit(end, duration = 12)` | 1 → 0 before frame `end`. |
| `rise(p, px)`, `pop(p)`, `slide(p, side, px)`, `focus(p, blur)` | Styles for an entrance at progress `p`. |
| `stagger(i, step = 4, base = 0)` | Delay of the i-th item in a list. |
| `mix(p, a, b)`, `easeOut`, `easeIn`, `easeInOut`, `useSeconds()` | Helpers. |

Text (each takes `id` → its `data-id`, `delay`, `style`, and `size`, `color`, `weight`, `font`, `align`):

| Export | Use |
| --- | --- |
| `<Title>…</Title>` | A headline rising in on a spring. |
| `<Words text step highlight={["word"]} highlightColor>` | Words entering one after another; highlighted words in the accent. |
| `<FitText text width lines maxSize>` | Sized to fill `width` on `lines` lines (waits for its font). |
| `<Counter to from duration decimals prefix suffix locale>` | A number counting up: `$1,240`, `98%`. |
| `<Highlight>…</Highlight>` | A marker sweep behind inline words. |

Screens and hands:

| Export | Use |
| --- | --- |
| `<Phone src={staticFile(…)} width={520}>` or children | A modern phone, 19.5:9, with its island. |
| `<Window src url title width height dark>` | A desktop app or browser window. |
| `<Tap x y at>` | A tap ripple at frame `at` (coordinates in its parent). |
| `<Cursor stops={[{x, y, at, click}]}>` | A pointer gliding between stops and clicking. |

Captions, grounds, layout, look:

| Export | Use |
| --- | --- |
| `<Captions captions position highlight size>` | A few words at a time, the spoken one lit. `captionsFromText(text, startMs, wpm)` times plain text; `parseSrt` from `@remotion/captions` reads an `.srt`. |
| `<Gradient from to angle turn>`, `<Grid>`, `<Glow x y color>`, `<Grain>` | Grounds that move a little. Put them first in an `AbsoluteFill`. |
| `useSafeArea()`, `useSafePadding()`, `safeArea(w, h)` | The area text should stay in (the same numbers `unsafe` checks). |
| `theme`, `defineTheme({colors, fonts, type})`, `scaleType(size, width)` | A starting look: dark ground, one accent, a type ramp for 1080 px. |
| `fonts`, `loadFonts({family, file, weight})` | Bundled families: `Inter` (100–900, italic), `"Space Grotesk"` (300–700), `Fraunces` (serif, 100–900), `"JetBrains Mono"` (100–800). Other font files: `video.asset.add`, then `loadFonts` at module level. |

## Look at it — and read what it tells you

`video.still` draws real frames in the real renderer and checks them. Each issue names its kind,
the element (`id`: its `data-id`, or the nearest parent's), the words, the frames it was found on,
and what to do:

| Issue | Fix |
| --- | --- |
| `clipped` | A box with `overflow: hidden` cuts the words: give them room, shrink them (`FitText`), or allow wrapping. |
| `offscreen` | Words run past the frame. Fine mid-entrance (one frame); a fault if it holds. |
| `unsafe` | Words where Shorts/Reels/TikTok put their buttons and captions (vertical: 10% top, 12% right, 20% bottom, 6% left) or past a 5% margin: lay out inside `useSafePadding()`. |
| `contrast` | Below 4.5:1 (body) or 3:1 (large) against the actual pixels under it: darken the ground behind the words, add a scrim (`linear-gradient(transparent, rgba(0,0,0,.6))`), or change the colour. |
| `small_text` | Under about 2.2% of the frame's short side (24 px at 1080): people watch on phones. |
| `text_overlap` | Two runs of words over each other (not one inside the other). |
| `font_fallback` | The family is not there and something else drew: use a kit family, one `fc-list` shows, or load the file. |
| `broken_image` / `console` | A picture did not load / the page logged an error. |

An entrance animates through clipped or offscreen states for a frame or two; an issue on only the
first frames of a scene is usually that. Look at the picture before deciding. `errors` in a still
are frames that threw: fix them like `video.check`'s.

Use `boxes: true` when you need exact positions: every element with a `data-id`, per frame, in
the composition's px.

## Assets and Relay's own designs

- A file on this machine: `video.asset.add {video_id, path}` → `staticFile(static_file)`.
- **A screen from a Relay design** (an app screen to show in a phone):
  `design.render {design_id, nodes: ["<frame id>"], scale: 2}` → its `png` path →
  `video.asset.add {video_id, path: <png>}` → `<Phone src={staticFile("assets/…png")} />`. Render
  the design at the size it will be shown (a 390-wide phone screen at scale 2 fills a 520 px phone
  sharply).
- Stock photos: with the Pictures plugin on, `design.picture.search` / `.add` store a photo for a
  design; its `path` goes through `video.asset.add` like any file. Keep its credit and name it.
- Never write a web address into a video: renders are offline and must not change over time.

## Rendering and versions

- `video.render` answers the render row (`state: "queued"`); renders run one at a time.
  `video.render.changed` reports each tenth; `video.render.finished` the end (`done` with `path`
  and `bytes`, `failed` with `error`, or `cancelled`).
- A `preview` render is half size: fine to check pacing; render `standard` or `high` for the file
  the person will post.
- Each check, still and render records a version when the code changed (`video.get` lists them).
  `video.version.restore {version}` puts one back; the code before is recorded first, so a restore
  is undone by restoring the version it answers as `previous`.

## Craft

- **Hook in the first second.** Social video is decided in 1–2 s: open on the boldest statement,
  in motion, never on a logo or a blank fade.
- **Pace for reading.** Hold words on screen at least 0.4 s + 0.3 s per word; about 3 words a
  second is a comfortable pace. A scene of a social video lasts 1.5–4 s.
- **One idea per scene,** one focal point per frame. Big type: a vertical headline is 96–140 px,
  body 40–56 px, never under 32 px.
- **Motion with purpose:** ease out on entrances (fast, then settle), ease in on exits, stagger
  lists by 3–6 frames, and keep a little movement in long holds (a slow `Glow`, a drifting
  `Grid`, a 2–4% scale over the scene) so nothing looks frozen.
- **Stay in the safe area,** and test the vertical crop: Shorts, Reels and TikTok cover the
  bottom fifth and the right edge with their own buttons and captions.
- **Sound:** captions on by default (most watch muted); music under −18 LUFS-ish with `volume`
  ramps (`interpolate` at the start and end), never cut mid-note: fade the last 0.5 s.
- **App promos:** show the real screen (`Phone` with a design render or a screenshot), point at
  what matters (`Cursor`, `Tap`, `Highlight`), one benefit per scene, end on the name and one
  call to action, held 1.5–2 s.
- **Colour:** one accent; dark grounds suit phones; check `contrast` on every scene.
- Name everything a check should report with `data-id` — titles, captions, buttons, cards.

## Worked example: a 9:16 app promo scene

```tsx
import { Phone, Tap, Title, Glow, Gradient, rise, theme, useSafePadding, useSpring } from '@relay/video';
import { AbsoluteFill, Sequence, staticFile } from 'remotion';

export const BudgetScene: React.FC<{ accent: string }> = ({ accent }) => {
  const padding = useSafePadding();
  const phone = useSpring(8);
  return (
    <AbsoluteFill>
      <Gradient />
      <Glow color={accent} y={0.6} />
      <AbsoluteFill style={{ ...padding, alignItems: 'center', gap: 56 }}>
        <Title id="scene-title" size={96}>Every euro, in its place</Title>
        <div style={rise(phone, 120)}>
          <Phone id="phone" width={500} src={staticFile('assets/tally-home.png')} />
        </div>
      </AbsoluteFill>
      <Sequence from={40} layout="none">
        <Tap x={540} y={1210} at={0} />
      </Sequence>
    </AbsoluteFill>
  );
};
```

Then `video.check`, then `video.still {at: ["0.3s", "1s", "2s"]}`: the title enters, the phone
rises after it, and the tap lands on the button. Read the issues before moving on.

## Refusals

| Code | Means |
| --- | --- |
| `plugin.off` | Video is off for this project (its tools are not in your `can_call`): ask the person to turn it on, or do without it. |
| `video.kit.missing` / `.installing` / `.outdated` / `.no_node` | Set up (`video.kit.install`), wait (`video.kit.changed`), or ask for Node.js. |
| `video.build` | It did not build: the message is the first error, `details.errors` all of them. |
| `video.runtime` | Every frame asked for threw: fix the error at its file and line. |
| `video.composition` | No composition by that id: `video.check` lists them. |
| `video.moment` | A moment that cannot be read, or `to` before `from`. |
| `video.props` | Props that do not fit the composition's schema. |
| `video.timeout` / `video.worker` | The worker was slow or stopped: try again; a first bundle can take a while. |
| `video.deleted` | It was deleted; `video.restore` brings it back (only when asked). |
