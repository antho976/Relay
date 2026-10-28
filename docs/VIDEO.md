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
| `.relay/worker.log` | the worker | its stderr but the page's probe lines, started again past 2 MiB |

Setting up is never automatic: an agent calls `video.kit.install` when a call refuses
`video.kit.missing`, or you press **Set up** in the gallery. It needs Node.js 20 or newer and npm,
takes about 600 MB, and runs on a thread of its own (`npm install`, then the browser), saying how it
went with `video.kit.changed`. Relay passes Remotion no licence key, so Remotion sends no usage
reports; Remotion is free for individuals and companies of up to three people.

## Renders

`video.render` queues a render and answers its row; with `wait: true` it answers once the render
has finished (`Ctx::finish_reply`, waiting on the queue's own condition with nothing locked).
`video.render.wait {render_ids}` waits for renders started without it, and `video.render.batch`
queues up to twelve at once (compositions, the soundtrack as WAV, platforms). Every wait reads the
rows, so a render that finished before anyone waited is never missed; `bus.wait` also takes
`since`, a moment from which it replays matching events it has kept (ten minutes, 1,024 events).

A render is drawn to a hidden partial file beside its own and renamed once done: `out/<name>.<ext>`
when it was given a `name` (a render of the same name replaces the file), else
`out/<composition>-<id>.<ext>`. A failed render leaves the file it would have replaced. A render
whose file a later one of the same name wrote over says so (`replaced_by`, the render whose file
it is now), so nothing plays one render's file as another's.

## Sound

Every render with sound is measured once done (`video/audio.rs`), its sound decoded by ffmpeg as a
player would read it: integrated loudness (ITU-R BS.1770-4 with EBU R128's gates), loudness range
(EBU Tech 3342), the loudest 3 s, the true peak (BS.1770-4 Annex 2's 4× oversampling), the sample
peak and the samples at full scale, with warnings: `clipping`, `hot` (over −1 dBTP), `silent`,
`loud` and `quiet` (against what was asked, or the platforms' −14 LUFS). `loudness` and `peak`, or
a `preset` (`youtube`, `shorts`, `reels`, `tiktok` −14 LUFS; `discord`, `podcast` −16;
`broadcast` −23; all −1 dBTP), bring it there: one gain with a look-ahead true-peak limiter (5 ms
ahead, about 80 ms to recover), worked out again from what came out until within 0.1 LU, the sound
written back with the picture copied. No compression: ffmpeg's `loudnorm` falls back to dynamic
processing whenever a plain gain would pass the peak.

**Sync.** Remotion's MP4 carries an edit list that skips the AAC encoder's 2,048 priming samples,
so it is in sync with its picture as players read it (0 samples against a WAV render of the same
frames, in `video_e2e_sound_from_cues_to_loudness`); read without the edit list it is 42.67 ms
late. A loudness rewrite keeps it in sync, under the same test.

ffmpeg is the kit's (Remotion's compositor package ships one with its libraries), else the
machine's. It has no raw muxers, so sound comes over a pipe as WAV and frames through
`image2pipe`.

**Beats** (`video.audio.beats`, `video/beats.rs`): the spectral flux of a 1024-point STFT every 256
samples at 22.05 kHz; tempo by autocorrelation between 50 and 200 BPM with an octave-wide prior at
120 (or at the `bpm` given); Ellis's dynamic-programming tracker; a least-squares constant-tempo
grid through the beats it found (its RMS residual answered); the phase set on the attacks (a 2 ms
envelope's steepest rise within 30 ms); downbeats where the low band jumps into a bar (a drop after
a break), its energy, and the music's opening beat; sections where a bar's loudness or low band
moves 3 or 6 dB against the two before. Readings: `docs/perf/VIDEO-2026-10-22.md`.

## The timeline

`video.timeline` makes one audio-only render with `RELAY_PROBE=timeline`: no pictures, and on each
frame the probe reports the Sequences Remotion's sequence manager holds (`relayRoot` asks it to keep
them outside the Studio: `Internals.SequenceRegistrationContext`), the kit's sounds, the sound
Remotion collects for the mix (read in the page without taking it), and the beat grid a
`useBeatGrid` set. Scenes are placed up their parents (`from` is in the parent's frames);
Remotion's own components that register (`<AbsoluteFill>`) and the kit's sound wrappers are not
scenes. A kit sound's start is exact (it says how long it has played); any other's comes from where
it is in its file; each ends where its file does (ffprobe). The render's WAV is the mix, measured
and kept as a waveform. The grid is the code's, or the call's `bpm`, or one found in `music`.

What the sound gets wrong: `off_beat` (an effect or a top-level cut more than a frame from a beat),
`voice_overlap`, `no_duck` (music under a voice line no more than 3 dB down from its usual level),
`silence` (2 s with the mix under −50 dBFS, or, in a scan, nothing playing), `clipping`. `video.scan`
checks the sound from the frames it measures; `video.still {sound: true}` draws the mix's waveform
under its sheet, each frame numbered, the cues and voice lines marked. Both pictures are drawn by
the engine (`video/timeline.rs`) in the sheets' pixel font.

## Footage and assets

- `video.asset.add` takes `path`, up to 100 `paths`, or an installed `font` by family (fontconfig:
  the faces for `weights` and `italic`, under `assets/fonts/`, with the `loadFonts(…)` call). A
  name another file has gets a number and the answer says so (`renamed_from`); `overwrite`
  replaces it; `link` links the file where it is; `fps` writes a clip at a constant frame rate.
- The worker hands Remotion a mirror of links for `public/` and hardens the links in the finished
  bundle into hard links: Remotion copies a folder's files into every bundle and its file server
  refuses links (`docs/perf/VIDEO-2026-10-22.md`).
- `video.asset.inspect`: size, length, average fps, frame count, whether the frames are evenly
  spaced (a phone's recording is not), its sound measured, and motion events from the clip shrunk
  to 96 px in grey: `cut` (the screen changed at once), `change` (a tap, a value), `motion` (half a
  second or more: a scroll), with where on screen.
- `video.capture.android` (`video/capture.rs`) records a device through adb while its steps run,
  holding the device's lease (`device_lease`, a `claim` for the capture's length): demo mode's
  status bar, `screenrecord` stopped with SIGINT so its file is finished, pulled, written at a
  constant frame rate, added, and answered with where each step fell and the motion events.

## ElevenLabs

`video.voice`, `video.sfx`, `video.music` and `video.transcribe` (`video/eleven.rs`) use the key the
person saved (`video.elevenlabs.key_set`, from a video's board: checked with ElevenLabs, kept in
the desktop keyring through the keyring tool's stdin, or a 0600 file beside the test store; never
audited, answered or logged). Requests go to api.elevenlabs.io alone, over https, never following a
redirect, each with a deadline and a cap. A voice line comes with each character's timing, made
into words (and Remotion captions); `say` sends other words for some (`{"Avex": "Ay-vex"}`) and
maps the timings back to the words as written. `video.transcribe {expect}` aligns what it heard
with what was meant (an edit distance over words) and lists the differences. What Relay makes keeps
how it was made beside it (`<file>.json`), which `video.asset.list` answers as `made`.

## The live preview

`video.preview` runs Remotion Studio (`@remotion/cli`, pinned with the kit) on one video at a
time, for the person to watch with sound, scrub and step through without a render; the board's
**Watch live** opens it in the browser. Studio binds every interface and checks npm for updates:
`.relay/loopback.cjs`, loaded first, puts every server on 127.0.0.1 and answers the check
offline. It stops when another video is previewed, after two hours, or with Relay.

## The worker

One Node.js process per engine, `node videos/.relay/worker.mjs`, started by the first request that
needs it and stopped after ten idle minutes or when the engine stops. It keeps, between requests,
each video's bundle (rebuilt only when `src/` or `public/` changed), a TypeScript language service
per video, and one headless browser. A bundle folder a worker before it left (the folder is named
by the files' sizes and times, so an unchanged video names the same one) is removed and built
again: Remotion's copy of `public/` will not write over the links in it, which failed every render
and still of a video with assets after the worker had stopped. It speaks JSON lines on stdin and stdout (`check`, `meta`,
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
the code as it is, then puts the version's back, and is undone by restoring the one it recorded. A
file over 2 MiB, or a link, that the version does not name is left in place: no version keeps a
copy to put it back.

A video has several versions in two senses, and Relay keeps both:

- **Takes** are versions of its code. `video.version.label` names one ("narrated, before the HUD"),
  by the person from the board or by an agent; `video.get` lists the newest twenty and every
  named one however old. `video.version.diff` answers what changed between two (or since one) as
  unified diffs, and `video.version.restore {paths}` puts back only some files: a scene from an
  earlier take without undoing what came after.
- **Cuts** are compositions of one video sharing its scenes: narrated and music-only, vertical and
  square, the first film kept beside the second. The board shows them side by side (below).

## In the Design view

- The gallery shows the project's videos after its designs, under the same chips and search:
  each card has its poster (the frame drawn nearest two fifths in, by the latest still), its shape
  and length, its terminal and its newest render. Where the machine is not set up, a banner says
  what setting up downloads, with **Set up**.
- A video's board plays its newest MP4 or WebM render, and says when the code has changed since
  (with **Render a preview**). Beside it: the composition, the **props** its schema declares as
  fields (text, numbers, switches, choices, colours), applied with a preview render; its renders,
  to play, compare or show in their folder; and its versions.
- A video with several compositions lists them as **Cuts**, each with its size, length and newest
  render. Picking one shows that cut: the stage plays its renders, the props are its own, and
  Render and Ask are about it. The video's default composition, which agents draw unless they name
  another, is marked and is not changed by looking.
- **Compare** on a render plays it against the one on screen: **A** and **B** under the picture
  switch between them at the same moment of their compositions (a render of part of one starts at
  its first frame; cuts at other rates meet at the same second), playing on if it was.
- A render the person picks from the list stays on screen; the cut's newest is replaced by the next
  one as it lands. The renders list marks the one on screen and says which a later render of the
  same name replaced.
- **Versions** lists each with who made it, when and what it changed, a key to play a render of
  it, and a menu to **name** it (a pin marks named ones, which stay listed however old) or
  **restore** it.
- **Render** offers a preview (half size), MP4, high-quality MP4, WebM, GIF, ProRes with
  transparency, and the sound alone. Progress shows over the player, with **Stop**.
- A video opens on its poster; **Play** loads its newest render into the player (the media pipeline
  starts then, not when the board opens).
- **Watch live** opens the live preview in the browser. **Render** also offers YouTube, Shorts,
  Reels and TikTok (−14 LUFS) and Discord (−16 LUFS); each render's row says its name and its
  sound (LUFS and true peak, red when it clips or runs hot).
- **ElevenLabs**, at the foot of the side panel: the person saves their key there (a password
  field) and sees the account's tier and characters left.
- **Ask agent** pauses the player and sends the video's terminal one line: the video, the cut and
  the moment on screen (`1.5s · f45`, in that cut's frames), your request and the `video.still`
  that shows the agent that frame of that cut, and, when the render on screen is older than the
  code, which render and version the person watched. It goes as a design's Ask does: typed at an
  agent at its prompt, priority mail to one at work, held for one waiting on you.
- In a screenshot run (`RELAY_NATIVE_SCREENSHOT`) the board's media is muted: a virtual compositor
  keeps the window off the desktop, not its sound off the speakers.

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
| PNGs, sheets with labels, contrast, the pixel font | `crates/relay-core/src/video/sheet.rs` |
| ffmpeg and ffprobe: decoding, probing, frames, constant rates | `crates/relay-core/src/video/media.rs` |
| Loudness, true peak, the limiter, a render's sound | `crates/relay-core/src/video/audio.rs` |
| Beat grids | `crates/relay-core/src/video/beats.rs` |
| The timeline, the sound's issues, its pictures | `crates/relay-core/src/video/timeline.rs` |
| Assets: add, link, fonts, list | `crates/relay-core/src/video/assets.rs` |
| Clips: frame rates and motion | `crates/relay-core/src/video/inspect.rs` |
| ElevenLabs | `crates/relay-core/src/video/eleven.rs` |
| Android capture | `crates/relay-core/src/video/capture.rs` |
| The live preview | `crates/relay-core/src/video/preview.rs`, `plugins/video/runtime/loopback.cjs` |
| Versions | `crates/relay-core/src/video/versions.rs` |
| The render queue | `crates/relay-core/src/video/jobs.rs` |
| Storage (migrations v34 to v36) | `crates/relay-core/src/store.rs` |
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
  `video_e2e_a_worker_started_again_bundles_a_video_with_assets` stops the worker between two
  stills of a video with an asset. On a machine whose Relay already set the kit up with the same
  pinned packages, `target/video-e2e/store/videos/` can be seeded from it instead of the network:
  `cp -a --reflink=always` of its `node_modules/`, and its `package.json`, `package-lock.json` and
  `.relay/kit.json`.
- `video_e2e_sound_from_cues_to_loudness` (ignored, needs ffmpeg) makes music, a voice line and
  cues, draws the kit's new pieces, checks the timeline, the beats, a render brought to −14 LUFS,
  and the MP4's sync against a WAV render to the sample; `video_e2e_the_preview_is_studio_on_loopback`
  opens Studio and checks it listens on 127.0.0.1 alone.
- `crates/relay-core/tests/video.rs` also drives ElevenLabs through a stand-in for the network
  (`Hub::use_elevenlabs`), the capture through a fake adb (`fixtures/video/fake_adb.py`), and the
  preview through a plain server.
- `scripts/video-smoke.py` makes two videos through the real worker on a disposable engine, borrowing
  the kit `video_e2e` set up (the promo with a music bed, rendered to YouTube's loudness, then a
  square cut, a named version and a diff), and captures the gallery, both boards, the player, two
  renders compared and the versions in a private virtual KWin, with no sound.

Readings on this machine (Ryzen, the engine's debug build, the kit warm in the page cache): a first
check 3.9 s (worker start, bundle, typecheck, browser), a check with nothing changed 0.10 s, a contact
sheet of six frames 1.2 s, one frame 0.37 s, a still after an edit (rebundled) 1.2 s, a 4 s
1080×1920 preview render 3.3 s.
