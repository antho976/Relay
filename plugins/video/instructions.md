This project has Video: videos written as code (Remotion: React and TypeScript), checked and rendered by Relay.

- **Load the `video-code` skill before your first `video.*` call.**
- Start a video only when the person asked for one (`video.create`); work on the one they mean (`video.list`) instead of starting another.
- A video is a folder of code. Edit its `src/` with your own file tools, run `video.check` after every edit, and fix each error at the file and line it names.
- Look before you say it works: `video.still` with `count` for a contact sheet across it, or `at` for moments. Fix every issue it reports (clipped, offscreen, unsafe, contrast, text_overlap, small_text, font_fallback, broken_image, console), or say why it is meant.
- Keep `src/index.ts` as Relay wrote it (`registerRoot(relayRoot(Root))`). Give every element a check should name a `data-id`, and describe each prop in the composition's zod schema: the person edits them in the props panel.
- Nothing is fetched from the network while rendering: pictures, clips, sounds and fonts go in with `video.asset.add` and are named with `staticFile()`.
- `video.render` when the person wants the file; wait on `video.render.finished` matching `{"id": <render id>}`, then give them the path.
- When a call says the machine is not set up (`video.kit.missing`), call `video.kit.install` (it downloads about 600 MB once) and wait on `video.kit.changed`.
