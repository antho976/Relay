# Video plugin for Relay

Turn this plugin on for a project whose agents should make videos: app promos, explainers,
social clips, animated charts. It is off by default; switch it on per project from the **Plugins**
page. A video here is code: a folder of React components that [Remotion](https://www.remotion.dev)
draws one frame at a time, which an agent writes, looks at and renders, and which you watch, tweak
and ask about in the project's Design view.

## What an agent gets

| Piece | Where it lands | When |
| --- | --- | --- |
| The `video-code` skill | `.claude/skills/video-code/`, `.agents/skills/video-code/` | As soon as the plugin is on |
| Working rules (`instructions.md`) | The agent's session brief | Every start, wake and resume |
| The `video.*` tools | The agent's tools | Its next start or resume; calls follow the switch at once |
| Write access to the project's video folders | Its guardrails, and `--add-dir` for Claude and Codex | At once for the guardrails; the folder itself at its next start |

In a project with Video off, agents are neither offered the tools nor able to call them: a call is
refused `plugin.off`, with a hint to ask you.

## What a video is

`video.create` makes a folder under Relay's data folder,
`~/.local/share/relay-v4/<instance>/videos/p<project>/<id>-<title>/`:

| Path | What it holds |
| --- | --- |
| `src/index.ts` | Written by Relay and left alone: it registers the video so Relay can see each frame |
| `src/Root.tsx` | The compositions: size, frame rate, length, and the props you can change |
| `src/Video.tsx` and the rest of `src/` | The video itself |
| `public/` | Pictures, clips, sounds and fonts, named in the code with `staticFile()` |
| `out/` | Renders |

Relay records a **version** of the code each time an agent checks, looks at or renders it after a
change, so any earlier state can be put back (`video.version.restore`).

## Setting up the machine

Rendering needs **Node.js 20 or newer** with npm. The first time a video is checked, an agent (or
you) calls `video.kit.install`, which downloads, once, into the `videos/` folder: Remotion 4, React,
TypeScript and four font families under the SIL Open Font License (Inter, Space Grotesk,
Fraunces, JetBrains Mono) from npm, about 350 MB; and Remotion's headless Chrome from Google,
about 240 MB. Nothing is installed system-wide. After that nothing leaves the machine while videos
render: Relay does not pass Remotion a licence key, so Remotion sends no usage reports.

Remotion is free for individuals and companies of up to three people; larger companies need a
[company licence](https://www.remotion.dev/license).

## What Relay checks

`video.still` answers a picture (one frame, or a labelled contact sheet of several) and the
problems it finds in what it drew:

| Issue | Means |
| --- | --- |
| `clipped` | Words cut off by a box that hides its overflow |
| `offscreen` | Words running past the frame's edge |
| `unsafe` | Words where a platform's buttons and captions sit (a vertical video keeps 10% top, 12% right, 20% bottom and 6% left free; others 5% each side) |
| `contrast` | Words too faint against the pixels under them |
| `small_text` | Words too small to read on a phone |
| `text_overlap` | Words drawn over other words |
| `font_fallback` | A font that is not there, so another stood in |
| `broken_image` | A picture that did not load |
| `console` | An error the page logged |

`video.check` builds and typechecks the code first, answering each error at its file and line.

## Where the work runs

One Node.js process per Relay instance (`videos/.relay/worker.mjs`) keeps each video's bundle and
one headless browser between requests, so a still after an edit takes a fraction of a second. It
starts on the first request that needs it and stops after ten idle minutes. Renders run one at a
time; each reports its progress to the Design view and its row.

## Switching it off

The tools and the write access leave the agents of that project. Videos, their folders and their
renders stay.
