# Videos written as code

Agents make videos — app promos, explainers, social clips, animated stats — by writing React that
[Remotion](https://www.remotion.dev) draws one frame at a time, and Relay checks, shows and renders
them. You watch them, change what their props allow, render them and ask for the rest in the
project's **Design** view. Why code, and why Remotion: D167.

## What a video is

A folder of code beside the store and a `videos` row. `video.create` makes the folder from the
starter, `~/.local/share/relay-v4/<instance>/videos/p<project>/<id>-<title>/`:

| Path | What it holds |
| --- | --- |
| `src/index.ts` | Relay's: `registerRoot(relayRoot(Root))`, so Relay can see each frame and read the props' schemas |
| `src/Root.tsx` | The compositions: id, size, frame rate, length, zod `schema`, `defaultProps` |
| `src/Video.tsx`, … | The video |
| `public/` | What `staticFile()` names; `video.asset.add` copies files into `public/assets/` |
| `out/` | Renders, `<composition>-<render id>.<ext>` |
| `tsconfig.json` | Extends the kit's settings |

Agents edit the folder with their own file tools; the starter is recorded as version 1 when the
video is made. With the **Video** plugin on (`plugins/video/`),
every session of the project may write under `videos/p<project>/` (`guardrail::config`), Claude and
Codex get that folder with `--add-dir` at launch, and every agent gets the `video-code` skill and
the plugin's rules. The ops are BUS.md §10.19.

## The kit

`videos/` is also the kit every video resolves its packages from, by walking up:

| Path | Written by | What |
| --- | --- | --- |
| `package.json`, `node_modules/` | `video.kit.install` (npm) | Remotion 4.0.534 and its packages, React 19, TypeScript 6, zod 4, Lottie, four Fontsource families (OFL) |
| `node_modules/.remotion/` | `video.kit.install` (Remotion's `ensureBrowser`) | Chrome Headless Shell |
| `tsconfig.base.json`, `.relay/kit/`, `.relay/worker.mjs` | the engine, whenever its build's copy differs | the shared TypeScript settings, `@relay/video` (linked as `file:./.relay/kit`), the worker |
| `.relay/kit.json` | the end of a setup | the pinned packages' digest: a different one reads as `outdated` |
| `.relay/bundles/<video>/` | the worker | each video's newest bundle |
| `.relay/blobs/` | versions | file contents by sha-256 |
| `.relay/posters/<video>.png` | stills | each video's poster |
| `.relay/worker.log` | the worker | its stderr, from its latest start |

Setting up is never automatic: an agent calls `video.kit.install` when a call refuses
`video.kit.missing`, or you press **Set up** in the gallery. It needs Node.js 20 or newer and npm,
takes about 600 MB, and runs on a thread of its own (`npm install`, then the browser), saying how it
went with `video.kit.changed`. Relay passes Remotion no licence key, so Remotion sends no usage
reports; Remotion is free for individuals and companies of up to three people.

## The worker

One Node.js process per engine, `node videos/.relay/worker.mjs`, started by the first request that
needs it and stopped after ten idle minutes or when the engine stops. It keeps, between requests,
each video's bundle (rebuilt only when `src/` or `public/` changed), a TypeScript language service
per video, and one headless browser. It speaks JSON lines on stdin and stdout (`check`, `meta`,
`still`, `render`, `cancel`); Remotion's own console output is moved to stderr. Errors come back as
places in the video's code: esbuild's `file:line:col`, webpack's unresolved imports, TypeScript's
diagnostics, and a frame that threw mapped through the bundle's source maps.

## Checks

`relayRoot` wraps Remotion's frame seek: while a still is drawn or a scan runs (`RELAY_PROBE`), each
frame is measured once every hold on it (pictures, fonts) has gone and React has committed it, and
the measurement goes out as one console line. The commit is known from `FrameMark`, a component
`relayRoot` draws beside `Root` that records the timeline's frame in a layout effect: Remotion
seeks by setting state, and a render's tabs go from frame to frame, so a page measured on the
animation frame Remotion releases its own hold could still show the frame before. It holds the box and opacity of every element with a `data-id`, every run of words
with its box, font (and whether that font is really there, by measuring against two fallbacks),
colour, opacity and the box that clips it, and the pictures that did not load. The engine
(`video/probe.rs`) turns that and the drawn pixels into issues, each once with the frames it was
found on:

| Issue | Found when |
| --- | --- |
| `clipped` | words pass a box that hides its overflow by more than 2 px, while some of them show |
| `offscreen` | words run past the frame's edge, while some of them are in it |
| `unsafe` | words inside the frame cross the safe area: a frame taller than 3:2 keeps 10% top, 12% right, 20% bottom and 6% left free (Shorts, Reels and TikTok lay their buttons and captions there); any other 5% each side |
| `small_text` | words under 2.2% of the frame's short side as drawn (scaling included), at least half opaque, not in an app's screen (`data-relay-screen`, which `Phone` and `Window` mark) or decoration (`aria-hidden`) |
| `font_fallback` | the first family named is not there |
| `contrast` | under 4.5:1 (3:1 for words at least 4% of the short side), the words judged as drawn (their colour over the ground at their opacity, so dimmed words read dim) against the median of the pixels under them that are not the words' colour; words fading below half their opacity are not judged |
| `text_overlap` | two runs of words, both drawn whole and neither inside the other, overlap by more than 20% of the smaller (a crossfade's are not) |
| `brief_text` | (`video.scan` only) a run of two words or more on screen less than 0.4 s and 0.3 s a word, not cut short by the end; a number counting keeps its run, and words typed in count as their longest |
| `broken_image`, `console` | a picture that did not load; an error the page logged |

`offscreen` and `unsafe` allow a fifth of the words' size past an edge: a run's box is its glyphs',
which rise above its line box, so words laid out exactly at the safe edge are inside it.

`video.scan` measures every few frames (default 3) with Remotion's `renderFrames` and no pictures
(`imageFormat: none`): about a second for 15 s of video. It judges position (`clipped`,
`offscreen`, `unsafe`, `text_overlap`) only for words at rest — the same box and opacity as in the
frames measured before and after — so an entrance, an exit or a transition is not a fault. It has
no `contrast`, which needs the pixels; a still's frames have it.

## Versions

Agents' edits never pass through Relay, so each check, still and render reads the folder first and
records a version when its files differ from the newest one's: a manifest of every file but `out/`,
`public/assets/`, dot-files and files over 2 MiB, with contents kept once by hash. A restore records
the code as it is, then puts the version's back, and is undone by restoring the one it recorded.

## In the Design view

- The gallery shows the project's videos after its designs, under the same chips and search:
  each card has its poster (the frame drawn nearest two fifths in, by the latest still), its shape
  and length, its terminal and its newest render. Where the machine is not set up, a banner says
  what setting up downloads, with **Set up**.
- A video's board plays its newest MP4 or WebM render, and says when the code has changed since
  (with **Render a preview**). Beside it: the composition, the **props** its schema declares as
  fields (text, numbers, switches, choices, colours), applied with a preview render; its renders,
  to play or show in their folder; and its versions, to restore.
- **Render** offers a preview (half size), MP4, high-quality MP4, WebM, GIF, ProRes with
  transparency, and the sound alone. Progress shows over the player, with **Stop**.
- A video opens on its poster; **Play** loads its newest render into the player (the media pipeline
  starts then, not when the board opens).
- **Ask agent** pauses the player and sends the video's terminal one line: the video, the moment
  on screen (`1.5s · f45`), your request and the `video.still` that shows the agent that frame. It
  goes as a design's Ask does: typed at an agent at its prompt, priority mail to one at work, held
  for one waiting on you.

## Implementation

| Part | Where |
| --- | --- |
| Ops, shapes and moments | `crates/relay-bus/src/ops/video.rs` |
| Handlers | `crates/relay-core/src/handlers/video.rs` |
| The hub, folders and rows | `crates/relay-core/src/video/mod.rs` |
| The worker's process and protocol | `crates/relay-core/src/video/worker.rs` |
| Setting up the kit | `crates/relay-core/src/video/kit.rs` |
| check, still and scan: versions first, stills, sheets, posters | `crates/relay-core/src/video/stills.rs` |
| Issues from probed frames, motion between them, reading time | `crates/relay-core/src/video/probe.rs` |
| PNGs, sheets with labels, contrast | `crates/relay-core/src/video/sheet.rs` |
| Versions | `crates/relay-core/src/video/versions.rs` |
| The render queue | `crates/relay-core/src/video/jobs.rs` |
| Storage (migration v34) | `crates/relay-core/src/store.rs` |
| The worker, the kit, the starter | `plugins/video/runtime/` |
| Rules and the skill | `plugins/video/instructions.md`, `plugins/video/skills/video-code/SKILL.md` |
| The gallery's section | `apps/relay-native/src/design/videos.rs` |
| A video's board | `apps/relay-native/src/design/video_board.rs` |
| MCP pictures | `crates/relay-cli/src/mcp.rs` (`still_success`) |

## Checking it

- `crates/relay-core/tests/video.rs` drives every op through a fake worker
  (`tests/fixtures/video/fake_worker.py`, the same protocol): the folder, the plugin switch and the
  write root, checks, stills and their issues, versions and restores, the render queue, its tenths
  and its end, cancelling, props against their schema, assets.
- `crates/relay-core/tests/video_e2e.rs` (ignored; needs Node.js and, once, the network) sets the kit
  up for real and checks, looks at and renders the starter, finding a planted line in a missing font,
  too small and faint, under a platform's buttons; and a typo and a throw at their lines.
- `scripts/video-smoke.py` makes two videos through the real worker on a disposable engine, borrowing
  the kit `video_e2e` set up, and captures the gallery and both boards in a private virtual KWin.

Readings on this machine (Ryzen, the engine's debug build, the kit warm in the page cache): a first
check 3.9 s (worker start, bundle, typecheck, browser), a check with nothing changed 0.10 s, a contact
sheet of six frames 1.2 s, one frame 0.37 s, a still after an edit (rebundled) 1.2 s, a 4 s
1080×1920 preview render 3.3 s.
