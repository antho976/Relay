# Clips

The fourth space beside Dev, Threads and Aide: a stream's VODs in, vertical Shorts out. You
import a VOD (a file, or a Twitch or YouTube link) and say what you want from it, in chips (Funny,
Fails, Stories, Guides & tips, Hype, Wholesome, Rage) and in your own words ("the IdleOn tips,
anything with his dog"). Relay transcribes it on the GPU, finds the moments with your Claude
account, cuts and frames each one, captions it in the channel's style, draws it, and lines it up
for your YouTube channel. You review each clip, change what needs it, approve it, and it goes out
at the next posting time: or you let good clips go out on their own.

It is Relay's take on [BridgeClip](https://github.com/bridge-mind/bridgeclip): the same idea
(complete moments, an editor, a library, a queue, posting), built on Relay's own parts: whisper.cpp
for the words, Claude through the person's subscription for the judgement (no API key, no
OpenRouter), the Video plugin's Remotion renderer for the picture, and the person's own Google
client for posting. It costs nothing past the Claude plan the person already has.

## The space

| Page | What it holds |
| --- | --- |
| **Overview** | What is being worked on (each stage, the one running lit with its bar), the clips waiting for your yes or no, and the VODs. Empty, it is a drop zone for a VOD. |
| **A VOD** | Its stages while it is worked on, then its clips as a grid of tall frames, filtered by kind; **Look again** asks for other moments in your words. |
| **A clip** (the editor) | The clip playing beside its edit: title, description, tags and kind; the moment's edges (±1 s keys) and its words, each one clickable (cut it, keep it, start or end there, fix how it is spelled); cut dead air and the end card; the opening line, the framing and the captions. **Save and draw again** redraws it; Approve, Reject, Post now. |
| **Queue** | What goes out next and when, what failed (with Try again), and what went out (with its YouTube link). |
| **Settings** | The streamer (name, where they stream, the words the transcript should know, what their viewers like), how clips look, finding (lengths, how many, the model, the frame rate), posting (ask or on their own, who sees them, the day's posting times, a description footer), YouTube, and the tools on this PC. |

The sidebar holds Import VOD (Ctrl N), the Overview with how many clips wait, the Queue with how
many go out, the VODs, and Settings at its foot. A file dropped on the Overview or the import
sheet opens the sheet with it. The start screen has a fourth card (key 4); a notification
(`clips` category) says when a VOD's clips are ready.

## The pipeline

One VOD at a time, oldest first, on the engine's `clips` thread (`crates/relay-core/src/clips/`).
Each stage leaves files in the VOD's folder, `videos/p<project>/vods/<id>-<slug>/`, and is skipped
when they are there: a VOD stopped, failed or cut off by a restart goes on from the stage it was in
(**Try again**).

| Stage | What it does | Leaves |
| --- | --- | --- |
| probe | Reads the file (ffprobe); a link is downloaded first with yt-dlp, at most 1080p, with its chat replay when Twitch has one | the VOD's length and size, `poster.jpg` |
| audio | Takes the sound out (a copy of its stream; AAC when it cannot be copied) | `audio.m4a` |
| transcribe | Every word with its time, on the GPU (`streamclips.py transcribe --progress`: whisper.cpp large-v3-turbo with DTW word times), told the streamer's name and the words in Settings | `words.json`, `transcript.txt` |
| signals | Loudness by the second, and chat's pace and hype for a VOD with its replay; the 90 strongest seconds (`streamclips.py signals`, `candidates`) | `signals.json`, `candidates.json` |
| layout | What the picture is made of: a facecam over gameplay, one camera, or a screen alone, with the boxes, from three frames with a grid drawn on them (Claude Sonnet, the images in its message and no tools: on-screen text, a chat overlay's above all, is anyone's words) | the VOD's layout, `layout/` |
| find | The moments asked for (below) | the clips, as drafts; `moments.json` |
| cut | Each clip's footage cut with 2 s and 3 s handles into the video's `public/assets/` (NVENC when ffmpeg has it); one look at every clip's scene; each clip's framing; its cut edges checked against the sound and the words (`streamclips.py cuts`) | `scenes/`, each clip's section |
| render | Each clip drawn by the Video plugin (`video.render`, composition `short`, Shorts loudness), waited on, its first frame kept as its poster | `out/clip-<id>.mp4`, `posters/` |

A clip drawn again (a changed edit, **Draw again**, a VOD's framing changed for all its clips) waits,
`rendering` with no render yet, for the `clips-render` thread, which cuts its footage again when it
needs it and queues its render, one clip at a time; an engine stopped in the middle leaves it
waiting, and the next one draws it. An approved or scheduled clip drawn again goes back to the
queue once drawn, and cannot be approved or posted while it is drawn (its file is the old
picture); rejected or taken back meanwhile, it stays as the person left it. A bug in one VOD's
work fails that VOD, not the thread.

A VOD's clips are compositions of one video of the Video plugin (made by `video.create`, titled
*Clips · streamer · VOD*), so they also show in the Design view's Videos and any agent with the
`video-code` skill can open one. Every clip is the same composition, `short`, drawn from its edit
as props: the template is `plugins/video/runtime/clips/short/` (synced to `videos/.relay/clips/`
with the kit, and written into the video's `src/` when it differs).

### Finding the moments

`clips::find`. The transcript is read in parts of 40 minutes that overlap by 2.5, three at a time,
each by Claude (`claude -p`, no tools, a JSON schema for the answer, the model from Settings: Opus
by default). Each read is told the stream, what the person asked for and the kinds, the clip
lengths, the words that matter, what the channel's viewers like, what is already clipped, and the
part of the transcript as `[h:mm:ss.s] line`, with the loud seconds (and chat's spikes) marked at
the end of their line. The prompt (`find::PROMPT`) is the editing taste learned from the first
hand-reviewed clips: open on the hook, one thing, end on the payoff, whole sentences, no chat or
TTS reading, honest scores, titles in a fan editor's voice.

Each moment comes back with its kind, start and end, optional cuts inside it and a teaser (the
payoff played first), title, opening line, description, tags, the words to colour, a punch-in, a
framing and a score. The parts' moments are put together: overlaps keep the better, those over
what is already clipped go, only the kinds asked for, best first, and past half the places no one
kind takes a place from another kind scoring at most one less.

Each moment's edges are then moved onto the words (`edit::snap`): the start just before a word that
opens a sentence, the end just after one that closes one, looking a few seconds either way, never
inside a word. A moment that runs past the longest clip ends at a sentence before it.

### Framing

| Framing | What | When |
| --- | --- | --- |
| Split | The facecam over the game, 36 / 64, captions on the seam | A facecam scene: reactions, fails, funny |
| Full | The whole game window across the middle on a blurred copy of itself, the opening line above, captions below | Guides, where what is on screen matters; a screen with no camera |
| Follow | A tall crop where the person is, from where the picture moves (a person moves, a kitchen does not), a crop of its own for a part only where they would leave it | One camera: IRL, cooking, just chatting |
| Cam | Only the facecam | A facecam big enough not to blur (35% of the width or more) |
| Game | A tall crop of the game | Chosen by hand |

A stream switches scenes: a game with a facecam box, then a camera filling the frame while the
streamer just chats. So the VOD's layout holds only for the clips in its scene: after the footage
is cut, Claude looks at two frames of every clip in one go (`frame::scenes`), and a clip in another
scene gets that scene in its edit and the framing that suits it (a split asked for in a camera
scene follows the person instead). Found on the first real VOD: its best moment (a gender reveal)
was in a just-chatting scene, and the split framing showed chat where the facecam had been.

### The look

The template (`Short.tsx`) keeps what the person asked for when they reviewed the first
hand-made clips (memory: clip-editing-taste): every cut dissolves over 5 frames, picture and
sound; one crop per clip and a reframe only where needed; at most one punch-in, eased; captions
one word at a time in Pixelify Sans Bold with a thick black outline and a hard shadow, key words
in the highlight colour; the opening line across the top for the first 2.4 s; 1.6 s of picture
after the last word under a big end card (LIKE & SUBSCRIBE on a red key, LIVE ON TWITCH
@HANDLE). Dead air is cut (pauses over 0.7 s down to 0.12 s each side) unless turned off.
Captions also come Bold (a heavy sans, three words, the spoken one popping), Clean (a sentence on a
dark box, for guides) or None. The font (OFL) ships with the template.

## Posting to YouTube

Relay has no Google client of its own: you make one, once.

1. In [console.cloud.google.com](https://console.cloud.google.com) make a project and turn on the
   **YouTube Data API v3**.
2. On its **OAuth consent screen**, choose External, and add yourself as a test user.
3. Under **Credentials**, create an **OAuth client ID** of type **Desktop app**.
4. In Clips → Settings → YouTube, paste its ID and secret, then **Connect YouTube**: your browser
   opens Google's page, you allow Relay to upload, and Google sends you back to Relay on this PC.

The client and the sign-in are kept in the desktop keyring (`secret-tool`), never in `clips.db`,
never answered by an op or logged. The sign-in is PKCE over a one-shot listener on 127.0.0.1; the
scopes are upload and read-only (for the channel's name). Requests go to Google's hosts only, over
https, never following a redirect.

Two limits are Google's, not Relay's:

- **Uploads stay private until your project passes Google's audit** (the YouTube API's rule for
  projects made after July 2020). Until then you set each Short public in YouTube Studio. Relay
  says so in Settings.
- **A project left in Testing signs you out after seven days.** Publishing the consent screen (it
  stays unverified, with Google's warning when you sign in) keeps the sign-in. When it lapses, the
  next upload says so and Settings asks you to connect again.

Approved clips take the day's posting times (Settings, local time; none posts at once), best
first, one clip a time; **Post now** skips the queue. With posting set to **on their own**, clips
scoring the threshold or more are approved as you would, once a channel is connected: only clips
the pipeline drew that nobody has decided on, never one you took back, restored or rejected, one
an agent changed, or one whose upload failed. The
`clips-post` thread uploads what is due, resumably, with the title, the description (its line,
the credit "From <streamer>'s stream: twitch.tv/<handle>", the footer and #shorts), the tags and
the Gaming category. Relay must be running for a post to go out at its time; one missed while it
was off goes out when it next runs. An upload's address is kept the moment Google gives it: a
slow line, a lost answer or Google's own error goes on from what Google has; a Relay stopped in
the middle asks Google, when it next starts, how that upload ended, and posts the clip (it
arrived), goes on with it at its slot (it did not), or fails it with a word to look on the channel
first (Google could not say); and a try after a failure asks about the upload it opened before
opening another: never twice on a guess. Each drawing is a file of its own whose hash is checked
before it goes out, so nothing written to its path since is what YouTube gets. What the keyring
says of the sign-in is read once and kept until Relay next writes it; disconnecting revokes the
sign-in at Google too.

Posting is the person's alone: `clips.clip.post`, `clips.clip.set` (approve), the YouTube ops and
deleting are `Actors::UserOnly`. An agent may import, find, edit and draw clips (with the Video
plugin on), never post one: an approved or scheduled clip an agent changes or draws again goes back
to the person for review, with a note saying so. An agent's links are the internet's only (not
this PC's or the local network's, which yt-dlp would fetch), and a download stops at 20 GB.

When Relay quits, every tool Clips started (whisper, yt-dlp, `claude -p`, ffmpeg) is stopped with
it, its whole process group, and dies with Relay if Relay is killed; the VOD stays `working`, and
the next start goes on from its stage. Deleting a clip or a VOD takes its cut footage, drawing and
poster off the disk.

## The ops

`clips.*`, BUS.md §10.20. Events: `clips.changed` (`{what, vod_id, clip_id}`).

| Op | What |
| --- | --- |
| `clips.summary` | The VOD being worked on, what waits for review, the next post, the channel, missing tools |
| `clips.tools` | ffmpeg, python3, whisper.cpp and its model, Claude, the video kit, yt-dlp: found, or how to install |
| `clips.settings.get` / `.set` | The streamer, the look, finding, posting |
| `clips.vod.import` | A file or a link, what to look for; starts the pipeline |
| `clips.vod.list` / `.get` | The VODs; one with its clips |
| `clips.vod.find` | Look through a transcribed VOD again ("more IdleOn tips") |
| `clips.vod.stop` / `.retry` / `.delete` | Stop; start again where it stopped; remove (never the file it came from) |
| `clips.vod.layout` | Correct where the facecam and game are, and redraw |
| `clips.clip.get` / `.list` | A clip with the words around it; clips by VOD and state |
| `clips.clip.update` | Title, description, tags, kind, the edit (spans, framing, captions, hook, fixes); a changed edit is drawn again |
| `clips.clip.render` | Draw again |
| `clips.clip.set` | Approve, reject, or back to undecided |
| `clips.clip.post` | Post now or at a time |
| `clips.clip.delete` | Remove (a posted clip stays on YouTube) |
| `clips.youtube.setup` / `.status` / `.connect` / `.disconnect` | The Google client, the sign-in |

## Where things are

| Part | Where |
| --- | --- |
| The contract | `crates/relay-bus/src/ops/clips.rs` |
| The ops | `crates/relay-core/src/handlers/clips.rs` |
| `clips.db`, the stages, finding, framing, edits, YouTube | `crates/relay-core/src/clips/` |
| The transcript, loudness, candidates and cut-edge steps | `plugins/video/runtime/clips/streamclips.py` (the `stream-clips` skill's script) |
| The template | `plugins/video/runtime/clips/short/` |
| The space | `apps/relay-native/src/clips_pages.rs`, `css/clips.css`, wired in `money.rs` |

## Checking it

- `cargo test -p relay-core --lib clips`: snapping onto words, dead air, sections and handles,
  checked edits, reading and merging the finder's answers, parts, the marked transcript, layouts
  read from Claude, the busiest window, slots, descriptions, the store.
- `cargo test -p relay-core --test all clips`: settings, importing (a `file://` path, a missing
  file), a clip's edit checked and saved, approving only a drawn clip, posting only when signed in,
  and the YouTube sign-in (state checked, PKCE, the browser's return) and a resumable upload
  against a stand-in for Google.
- `python3 scripts/clips-smoke.py [--from DIR]`, inside `kwin_wayland --virtual`: the space's pages
  captured by relay-native against a disposable engine, with a real run's VODs and clips copied in.

### Readings

On the owner's PC (RTX 4070, debug-profile engine with optimizations, Opus as the finder), from
the stage files' times:

| VOD | Length | Transcript | Loudness | Layout | Moments | Cut + scenes | 10 clips drawn | Import to clips |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| IdleOn (BPo2vYmNFZo, 6.8 GB) | 5:13:25 | 3 min 26 s | 11 s | 8 s | 73 s (8 parts) | ~1 min | ~6 min (~35 s each) | ~12 min |
| Cooking (Y1STa6HHaGg, 4.2 GB) | 3:49:47 | 2 min 26 s | 8 s | 6 s | 55 s (6 parts) | 24 s | 4 min 4 s (~25 s each) | ~9 min |

What they found, asked for "fun moments and IdleOn guide stuff" (IdleOn) and Funny, Fails and
Stories (cooking): a gender reveal live on stream, five years of trying for a baby, a boss hiding
$300 in Easter eggs, a leaderboard cheater, Minehead hitting a mine on the first click, three
Spelunking and Sushi Station guides; marshmallows that became an "alien egg", the wife finding out
they burned, the dog that eats pig poop, saying please to Alexa before the "clanker uprising".
Every clip came out of the pipeline drawn, framed and captioned; none was touched by hand.

What the runs taught, each fixed before these numbers:

- A stream switches scenes, so one layout a VOD framed a just-chatting moment as a split with chat
  where the facecam was: each clip's scene is read now.
- Following the person by motion alone held the cereal box once: Claude now says where the
  streamer stands in each clip's frames, and a clip where they are off camera shows the whole
  picture.
- A small facecam (488 × 276) blurs when it fills the frame: Cam is offered only for a big one.
- A stage that made the VOD's video read the VOD row it started with, and made a new video at every
  clip: the row is read again.

The first run's renders failed on the test setup, not the pipeline: the kit linked from the
person's own install was older than this branch's `@relay/video`, and the template passed its
footage without `staticFile()`; both were fixed and the stage run again.
