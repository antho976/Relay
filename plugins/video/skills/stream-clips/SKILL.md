---
name: stream-clips
description: Turn a stream's VOD (Twitch, or any long recording) into vertical Shorts that look hand-edited, and edit gameplay footage into guides - finding the moments with streamclips.py (whisper.cpp words with timings, loudness and chat replay peaks, ranked candidates), choosing clips that start on the hook and end on the payoff, the facecam-over-gameplay layout (StreamClip, Camera, cam and game rects found once per channel), dead air cut with captions moved to match (keepSpeech, remapCaptions), punch-ins, caption styles (lit, box, pop, plain) and a reference channel's style measured and copied, the opening Hook, guide marks over footage the person gives (Camera keys that glide and zoom, Callout rings with labels, Arrow, Step), rendering at Shorts loudness, and the review list the person approves before anything is posted. Use with video-code when a task asks for clips, Shorts, highlights or a guide from a stream, a VOD or game footage.
---

# Stream clips and guides

Load `video-code` first: this skill adds to it. The kit pieces named here come from `@relay/video`.

**The person approves every clip.** You make the files and a review list. Nothing is uploaded or
posted unless the person turned that on and says so. The streamer is credited in every
description.

## The script

`streamclips.py` ships with the kit, at `<root>/.relay/clips/streamclips.py`, where `root` is
`video.kit.status`'s. It needs only `python3` and ffmpeg. `tools` says whether yt-dlp, whisper.cpp
and its model are installed. When one is missing, give the person the command `tools` prints and
stop: **never install, download or build tools yourself.**

| Command | What it does |
| --- | --- |
| `tools` | What is installed, and the install command for what is not. |
| `fetch URL --out DIR` | The VOD's audio, chat replay and info (a few hundred MB for hours, never the full picture). Add `--cookies FILE` for a VOD only subscribers can watch. |
| `fetch URL --out DIR --section 1:02:10-1:03:05 …` | Only those parts, picture and sound, at up to 1080p. |
| `transcribe AUDIO --out DIR --prompt "names, game terms"` | `words.json` (every word with its time; the kit's `Caption[]`) and `transcript.txt` (lines stamped `[h:mm:ss]`). Whisper runs on the GPU. `--prompt` spells the streamer's name, the game and its terms. |
| `signals AUDIO --chat CHAT --out signals.json` | Loudness, chat messages and hype (KEKW, LUL, "clip it", "?????") for each second. |
| `candidates --transcript words.json --signals signals.json --out candidates.json` | The 25 strongest moments, at least 75 s apart. Each has its peak, a window, why it scored (`chat`, `loud`, `hype`, `talk`) and the transcript around it. Chat is moved back 6 s for its lag. |
| `words --transcript words.json --start S --end E --out FILE` | One clip's words, timed from 0. |
| `cut VIDEO --start S --end E --out FILE` | A part of a local recording, re-encoded from the exact frame at a constant rate. |
| `style VIDEO… --out DIR` | For a reference channel's Shorts: length, cuts a minute, median shot, loudness, and a contact sheet of each to look at. |

Keep a VOD's files in `videos/p<project>/vods/<vod id>/`, where you may write. Delete that VOD's
audio once its clips are made.

## From a VOD to clips

1. **Channel set up, once.** Find where the facecam and the game sit in the stream's layout. Grab a
   frame with `ffmpeg -ss 600 -i <section>.mp4 -frames:v 1 frame.png` and Read it. Write
   `vods/channel.json`: `{"source": {"width", "height"}, "cam": {x, y, w, h}, "game": {x, y, w, h},
   "style": {...}}`. Check the cam rect again on each new VOD: an overlay change moves it.
2. **Fetch, transcribe, measure:** run `fetch`, then `transcribe` with `--prompt`, then `signals`
   with the chat, then `candidates`.
3. **Pick 5–10 clips.** Read `candidates.json`. Also skim `transcript.txt` for stories and bits
   that chat did not spike on. A good clip:
   - opens on the hook in its first second: the shout, the line, the moment just before it lands;
   - is 20–50 s long and is about one thing;
   - ends on the payoff (the punchline, the death, the reaction), not after it;
   - makes sense without the hour before it. Cut a moment that needs a backstory.
   Never start or end in the middle of a sentence: the start and end come from `words.json` times.
   A VOD re-uploaded to YouTube has no chat replay: `candidates` then ranks on loudness alone, so
   read the transcript closely, and look for what chat's TTS or a guest said too.
4. **Fetch only those parts** (`--section`), each with 3 s of handles each side.
   `video.asset.add {link: true}` them into one video for the VOD, titled with the streamer and
   the date. Each clip is a composition of that video, so the person reviews them side by side as
   cuts. Copy `channel.json` and each clip's words (`words --start --end`, from the section's own
   start) into the video's `src/`.
5. **Edit each clip** (the example below). Cut the dead air with `keepSpeech`, keep the captions on
   the edited timing with `remapCaptions`, punch in on the one or two biggest reactions, and open
   with a `Hook` on frame 0: it is the thumbnail.
6. **Check** each composition: `video.check`, `video.still {composition, count: 8}`, `video.scan`.
   Fix captions under the buttons, faint words and a face out of frame.
7. **Render** each clip with `video.render.batch` and `preset: "shorts"`, named
   `<streamer>-<date>-<n>`.
8. **The review list:** write `vods/<vod id>/clips.json`, one entry per clip: `file`, `title`
   (under 60 characters, in the channel's voice), `description` (one line, with the streamer
   credited: "From <name>'s stream: twitch.tv/<name>"), `tags`, `vod_at` (where in the VOD it is),
   `why` and `score`. Tell the person where it is.

```tsx
import { Captions, editedMs, Hook, keepSpeech, remapCaptions, StreamClip } from '@relay/video';
import { staticFile, useVideoConfig } from 'remotion';
import words from './clip-3.words.json';           // from `words`, timed from the section's start
import channel from './channel.json';

const KEEP = keepSpeech(words, 0, 41_000);         // the speech, pauses over 650 ms cut to 120 ms
const CAPTIONS = remapCaptions(words, KEEP);
// In Root.tsx: durationInFrames={keptFrames(KEEP, 60)}

export const Clip3: React.FC = () => {
  const { fps } = useVideoConfig();
  const at = (ms: number) => Math.round(((editedMs(ms, KEEP) ?? 0) / 1000) * fps);
  return (
    <StreamClip src={staticFile('assets/vod-1-02-07.mp4')} source={channel.source} cam={channel.cam} game={channel.game}
      keep={KEEP} punches={[{ at: at(18_400), frames: 36 }]}>
      <Hook id="hook" text="He thought the boss was dead" />
      <Captions id="captions" captions={CAPTIONS} variant="pop" maxWords={3} y={0.36} uppercase size={84} stroke={0.09} highlight="#FFD43B" />
    </StreamClip>
  );
};
```

## The layout

**One camera (IRL, cooking, just chatting):** there is no cam rect, so crop the stream itself. Draw
each clip from its own `Camera`s, one `Sequence` per kept span (`cutSegments`), and give each span
its own 9:16 crop (608×1080 of a 1080p stream) where he stands in it: a hand editor reframes at
every cut. Find where he stands by Reading frames with a grid drawn on them
(`drawgrid=w=240:h=1080`). When two people share the shot, take a wider crop and lay it on a
blurred, darkened copy of itself. Keep spans may be in any order: open on the payoff, then show
what led to it.

- `StreamClip` with a `cam` is `split`: the facecam over the gameplay, the cam taking `camShare` of
  the height (default 0.36). `layout: "cam"` fills the frame with the cam, for just chatting.
  `layout: "game"` fills it with the game, for a moment where the face adds nothing.
- `game` is the part of the game that matters: the middle of the screen in most games. Move it to
  the action when the action is off-centre (the minimap, the chat box, a corner).
- `punches` zoom the cam by default (`panel: "game"` for the game): 1.15–1.25× for 0.4–0.8 s, on a
  reaction. One or two a clip. More is the auto-edited look.
- Captions sit on the seam (`y={0.36}`) or in the game's lower third (`y={0.7}`), never under the
  bottom fifth, where Shorts puts its buttons.

## Looking hand-edited

Each of these marks a clip as auto-made. Avoid every one:

- **The same look on every clip.** Vary the caption `variant` and the highlight colour inside the
  channel's style, the hook's wording, and where the punch-ins land.
- **Captions that misspell names.** Check every name in `words.json` against `--prompt` and fix
  what whisper misheard before rendering.
- **Dead air and false starts.** Cut them, but keep a beat of silence before a punchline.
- **A clip that starts late**, after the moment has already begun.
- **Generic titles** ("INSANE moment 😱"). Write what happens, in the channel's voice.
- **A face cut off by the crop**, or the cam rect left over a changed overlay.
- **Effects for their own sake:** shakes, glitches, emoji rain. One sound effect on the payoff at
  most, and only when the channel's style has them.

## A reference channel's style

When the person names a channel whose Shorts they like:

1. They give the Shorts' links, or you list them: `yt-dlp --flat-playlist --print url <channel>/shorts`.
   Fetch 5–8 of them into `vods/style/<channel>/` with `yt-dlp -f "best[height<=1080]"`. These are
   only for study: never put that channel's footage, logo or name in a clip.
2. Run `style` on them and Read each contact sheet.
3. Write the profile into `channel.json`'s `style`:
   - captions: font (`fc-list` or a kit family), size, case, colours, outline, position, words on
     screen, `variant`;
   - the hook: whether there is one, and card or plain;
   - pacing: cuts a minute, and how often they punch in;
   - clip length;
   - how titles are written (three of theirs as examples).
4. Show the person one clip in that style before making the rest.

## Guides from footage the person gives

For a guide (a game's how-to, like "best IdleOn anvil setup"), the person gives the recording and
says what it should teach. You do not find the moments.

- **Script it first.** Write the steps the guide teaches and the line each step says. Show the
  person, then edit.
- **Use `Camera` with `keys`** to start wide, then glide into the panel each step is about
  (`frames` 20–30 for a glide, `move: "cut"` to jump). Times come from `video.asset.inspect`'s
  `motion` and from Reading frames.
- **Draw `Callout` and `Arrow` inside the `Camera`, in the recording's pixels.** They follow the zoom
  and keep their size on screen:
  - `Callout {rect, at, until, label}` rings what to press;
  - `Arrow {from, to, at}` shows where to look next.
  One ring at a time, and each label two to four words.
- **Put `Step {n, text, at, until}` beside the `Camera`, not in it.** It names the step across the
  top in the composition's pixels, like `Hook` and `Captions`.
- **Words:** the person's own voice if they record one (`video.asset.add`, then `video.transcribe`
  for its captions). Otherwise captions alone, from the script with `captionsFromText`. ElevenLabs
  voices cost the person's credits: ask before making one.
- **Pacing:** hold each step 3–6 s, long enough to read the label and see the click.
- **Check it:** `video.still` on every step's middle frame, then `video.scan`.
