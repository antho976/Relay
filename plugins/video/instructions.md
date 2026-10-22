This project has Video: videos written as code (Remotion: React and TypeScript), checked and rendered by Relay.

- **Load the `video-code` skill before your first `video.*` call.**
- Start a video only when the person asked for one (`video.create`); work on the one they mean (`video.list`) instead of starting another.
- A video is a folder of code. Edit its `src/` with your own file tools, run `video.check` after every edit, and fix each error at the file and line it names.
- Look before you say it works: `video.still` with `count` for a contact sheet across it, or `at` for moments, and `video.scan` over the whole video before you call it done. Fix every issue they report (clipped, offscreen, unsafe, contrast, text_overlap, small_text, font_fallback, brief_text, broken_image, console), or say why it is meant.
- Keep `src/index.ts` as Relay wrote it (`registerRoot(relayRoot(Root))`). Give every element a check should name a `data-id`, and describe each prop in the composition's zod schema: the person edits them in the props panel.
- Nothing is fetched from the network while rendering: pictures, clips, sounds and fonts go in with `video.asset.add` and are named with `staticFile()`.
- With sound: cut on the music's bars and put cues on its beats (`video.audio.beats` → `useBeatGrid`), duck the music under every voice line, and check it all with `video.timeline` before you render. Hear each line with a name in it back (`video.transcribe {expect}`).
- `video.render {wait: true, name}` when the person wants the file, with their platform's loudness (`preset: "youtube"` and the like); read its `audio` (no `clipping` or `hot`), then give them the path, length, size and loudness.
- To let the person watch it with sound before a render, `video.preview` and give them the link.
- ElevenLabs (`video.voice`, `video.music`, `video.sfx`) uses the key the person saved; never ask for a key. Each costs their credits: make what the video needs, not variations unasked.
- When a call says the machine is not set up (`video.kit.missing`), call `video.kit.install` (it downloads about 600 MB once) and wait on `video.kit.changed`.
