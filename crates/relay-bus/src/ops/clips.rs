//! `clips.*` (docs/CLIPS.md): the Clips space. A stream's VOD goes in; Relay transcribes it, finds
//! the moments the person asked for (funny ones, fails, stories, guides, or all of them), cuts and
//! frames each as a vertical Short in the channel's style, renders it through the Video plugin,
//! and posts the ones the person approves to their YouTube channel.
//!
//! A VOD's work runs on the engine's own thread, one VOD at a time, stage by stage (`Stage`);
//! each stage keeps its files in the VOD's folder, so a VOD stopped or failed starts again where
//! it was. A clip is an edit (`ClipEdit`): the spans it keeps, its framing, captions and hook,
//! rendered as one composition of the VOD's video with the edit as its props.
//!
//! Posting is the person's alone (`Actors::UserOnly`): an agent may find, edit and render clips,
//! never post one or touch the YouTube account.
//!
//! Events: `clips.changed` (`{vod_id?, clip_id?, what}`: a VOD, a clip, the settings or the
//! account changed, or a stage moved on).
use crate::registry::{Actors, Audit, OpMeta, Scope};
use crate::types::Id;
use crate::{op, Empty};
use std::collections::BTreeMap;

/// The plugin the clips ops come with: a VOD's clips are a video of the Video plugin.
pub const PLUGIN: &str = "video";

const CHANGED: &[&str] = &["clips.changed"];

/// Longest a clip may run, in seconds (YouTube's Shorts limit is three minutes; ours is tighter).
pub const MAX_CLIP_S: f64 = 180.0;
/// Most moments one find keeps.
pub const MAX_FIND: u32 = 30;

const fn person(meta: OpMeta) -> OpMeta {
    meta.actors(Actors::UserOnly)
}

/// What kind of moment a clip is. The person asks for kinds; the finder labels each moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ClipKind")]
pub enum Kind {
    /// A joke, a bit, a funny reaction.
    Funny,
    /// Something going wrong: a death, a burn, a misplay.
    Fail,
    /// A story told on stream: life, family, the dog.
    Story,
    /// A tip, an explanation, how something in the game works.
    Guide,
    /// A big win, a clutch, a rare drop: the stream's loud high.
    Hype,
    /// Warm, kind, a chat or family moment.
    Wholesome,
    /// Anger, frustration, a rant.
    Rage,
    /// Anything else worth a Short.
    Other,
}

impl Kind {
    pub const ALL: [Kind; 8] = [Kind::Funny, Kind::Fail, Kind::Story, Kind::Guide, Kind::Hype, Kind::Wholesome, Kind::Rage, Kind::Other];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Funny => "funny",
            Kind::Fail => "fail",
            Kind::Story => "story",
            Kind::Guide => "guide",
            Kind::Hype => "hype",
            Kind::Wholesome => "wholesome",
            Kind::Rage => "rage",
            Kind::Other => "other",
        }
    }

    /// As the space's chips say it.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Funny => "Funny",
            Kind::Fail => "Fails",
            Kind::Story => "Stories",
            Kind::Guide => "Guides & tips",
            Kind::Hype => "Hype",
            Kind::Wholesome => "Wholesome",
            Kind::Rage => "Rage",
            Kind::Other => "Other",
        }
    }

    pub fn parse(raw: &str) -> Option<Kind> {
        let t = raw.trim().to_ascii_lowercase();
        Kind::ALL.into_iter().find(|k| k.name() == t || k.label().to_ascii_lowercase() == t).or(match t.as_str() {
            "fun" | "joke" | "comedy" | "reaction" => Some(Kind::Funny),
            "fails" | "death" | "misplay" => Some(Kind::Fail),
            "stories" | "life" => Some(Kind::Story),
            "guides" | "tip" | "tips" | "tutorial" | "explain" | "explanation" => Some(Kind::Guide),
            "win" | "clutch" | "highlight" => Some(Kind::Hype),
            "rant" | "anger" => Some(Kind::Rage),
            _ => None,
        })
    }
}

/// Where a VOD's work stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ClipVodState")]
pub enum VodState {
    /// Waiting for the VOD before it to finish.
    Queued,
    /// A stage is running.
    Working,
    /// Its clips are made: they wait for the person.
    Ready,
    /// A stage failed: `error` says why; `clips.vod.retry` starts again where it stopped.
    Failed,
    /// The person stopped it.
    Stopped,
}

impl VodState {
    pub fn name(self) -> &'static str {
        match self {
            VodState::Queued => "queued",
            VodState::Working => "working",
            VodState::Ready => "ready",
            VodState::Failed => "failed",
            VodState::Stopped => "stopped",
        }
    }
    pub fn parse(raw: &str) -> VodState {
        match raw {
            "working" => VodState::Working,
            "ready" => VodState::Ready,
            "failed" => VodState::Failed,
            "stopped" => VodState::Stopped,
            _ => VodState::Queued,
        }
    }
}

/// A VOD's stages, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ClipStage")]
pub enum Stage {
    /// Reading the file (or fetching a link's sound and details).
    Probe,
    /// Taking its sound out.
    Audio,
    /// Every word with its time, on the GPU (whisper.cpp).
    Transcribe,
    /// How loud each second is.
    Signals,
    /// Where the facecam and the game sit, or where the person stands.
    Layout,
    /// Reading the transcript for the moments asked for.
    Find,
    /// Cutting each moment's footage and setting its edit.
    Cut,
    /// Drawing each clip (Remotion).
    Render,
}

impl Stage {
    pub const ALL: [Stage; 8] = [Stage::Probe, Stage::Audio, Stage::Transcribe, Stage::Signals, Stage::Layout, Stage::Find, Stage::Cut, Stage::Render];

    pub fn name(self) -> &'static str {
        match self {
            Stage::Probe => "probe",
            Stage::Audio => "audio",
            Stage::Transcribe => "transcribe",
            Stage::Signals => "signals",
            Stage::Layout => "layout",
            Stage::Find => "find",
            Stage::Cut => "cut",
            Stage::Render => "render",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Stage::Probe => "Reading the VOD",
            Stage::Audio => "Taking the sound out",
            Stage::Transcribe => "Transcribing",
            Stage::Signals => "Measuring loudness",
            Stage::Layout => "Finding the facecam",
            Stage::Find => "Finding moments",
            Stage::Cut => "Cutting clips",
            Stage::Render => "Rendering",
        }
    }
    pub fn parse(raw: &str) -> Option<Stage> {
        Stage::ALL.into_iter().find(|s| s.name() == raw)
    }
}

/// Where a clip stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ClipState")]
pub enum ClipState {
    /// Its edit is set; it has not been drawn yet.
    Draft,
    /// Queued or drawing.
    Rendering,
    /// Drawn: waiting for the person's yes or no.
    Ready,
    /// The person said yes: it goes out at the next posting slot, or when they post it.
    Approved,
    /// It goes out at `publish_at`.
    Scheduled,
    /// Uploading to YouTube now.
    Posting,
    /// On YouTube: `youtube_url`.
    Posted,
    /// The person said no.
    Rejected,
    /// Its render or upload failed: `error` says why.
    Failed,
}

impl ClipState {
    pub fn name(self) -> &'static str {
        match self {
            ClipState::Draft => "draft",
            ClipState::Rendering => "rendering",
            ClipState::Ready => "ready",
            ClipState::Approved => "approved",
            ClipState::Scheduled => "scheduled",
            ClipState::Posting => "posting",
            ClipState::Posted => "posted",
            ClipState::Rejected => "rejected",
            ClipState::Failed => "failed",
        }
    }
    pub fn parse(raw: &str) -> ClipState {
        match raw {
            "rendering" => ClipState::Rendering,
            "ready" => ClipState::Ready,
            "approved" => ClipState::Approved,
            "scheduled" => ClipState::Scheduled,
            "posting" => ClipState::Posting,
            "posted" => ClipState::Posted,
            "rejected" => ClipState::Rejected,
            "failed" => ClipState::Failed,
            _ => ClipState::Draft,
        }
    }
}

/// A rectangle in the VOD's own pixels.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[schemars(rename = "ClipRect")]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// What the stream's picture is made of, found once per VOD.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[schemars(rename = "ClipSourceLayout")]
pub struct SourceLayout {
    /// `facecam`: a camera box over gameplay; `camera`: one camera fills the frame (IRL, cooking,
    /// just chatting); `screen`: gameplay or a screen with no camera.
    pub kind: String,
    /// The facecam, for `facecam`.
    pub cam: Option<Rect>,
    /// The part of the game that matters, for `facecam` and `screen`.
    pub game: Option<Rect>,
    /// Where the person mostly is, for `camera`: the crop a clip starts from.
    pub subject: Option<Rect>,
    /// Who or what found it: `claude`, `motion`, `person` (set by hand) or `default`.
    pub by: String,
    /// The frame it was found on, for the editor to draw the boxes over.
    pub frame: Option<String>,
}

/// How a clip fills the 9:16 frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ClipLayout")]
pub enum Layout {
    /// The facecam over the gameplay.
    Split,
    /// The whole picture across the middle, on a blurred copy of itself.
    Fit,
    /// A 9:16 crop that follows the person (one camera).
    Crop,
    /// The facecam fills the frame: a reaction where the game adds nothing.
    Cam,
    /// The game fills the frame.
    Game,
}

/// The captions' look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ClipCaptions")]
pub enum CaptionStyle {
    /// One word at a time, a chunky pixel face, a thick black outline and a hard shadow, key words
    /// in the highlight colour.
    Pixel,
    /// Two or three words, a heavy sans, uppercase, the spoken word lit.
    Bold,
    /// A sentence at a time on a soft dark box, mixed case: for guides.
    Clean,
    /// No captions.
    None,
}

/// A clip's edit: everything the template draws it from. Times are seconds of the VOD.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[schemars(rename = "ClipEdit")]
pub struct ClipEdit {
    /// The spans kept, in the order they play: `[[start, end], …]`. Usually one; more when the
    /// middle of a moment was cut, or when it opens on the payoff.
    pub keep: Vec<[f64; 2]>,
    /// Inside each span, pauses longer than this many ms are cut down: dead air. `None` keeps
    /// pauses as they were.
    #[serde(default)]
    pub tighten_ms: Option<u32>,
    pub layout: Layout,
    /// The crop for `crop`, `game` and `cam`, in the VOD's pixels; the VOD's layout when absent.
    #[serde(default)]
    pub view: Option<Rect>,
    /// A crop of its own for a span, by its index in `keep`: only where the person would leave
    /// `view`.
    #[serde(default)]
    pub reframe: BTreeMap<String, Rect>,
    /// The opening line, across the top for the first two seconds: the hook.
    pub hook: String,
    pub captions: CaptionStyle,
    /// Where the captions sit, as a fraction of the height.
    #[serde(default)]
    pub caption_y: Option<f64>,
    /// Words drawn in the highlight colour.
    #[serde(default)]
    pub emphasis: Vec<String>,
    /// Fixes to what was misheard: `{"heard": "meant"}`.
    #[serde(default)]
    pub fixes: BTreeMap<String, String>,
    /// Moments to punch in on (seconds of the VOD): the one line the clip is for.
    #[serde(default)]
    pub punch_at: Vec<f64>,
    /// End on the card (like and subscribe, where the streamer is live).
    #[serde(default = "yes")]
    pub end_card: bool,
    /// What this clip's picture is made of, when it is not the VOD's: a stream that switches
    /// between a game scene (facecam over gameplay) and a camera scene (just chatting).
    #[serde(default)]
    pub scene: Option<SourceLayout>,
}

fn yes() -> bool {
    true
}

/// A VOD and where its work stands.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[schemars(rename = "ClipVod")]
pub struct Vod {
    pub id: Id,
    pub title: String,
    /// The file it came from, or its link.
    pub source: String,
    pub streamer: String,
    pub duration_s: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    /// What the person asked for, in their words.
    pub request: String,
    /// The kinds they asked for; empty is every kind.
    pub kinds: Vec<Kind>,
    pub state: VodState,
    pub stage: Option<Stage>,
    /// How far the stage is, 0 to 1.
    pub progress: f64,
    /// What the stage is doing now, in a few words.
    pub detail: Option<String>,
    /// Why it failed; on a ready VOD, what its last find could not do (parts of the transcript
    /// Claude could not read).
    pub error: Option<String>,
    pub layout: Option<SourceLayout>,
    /// The video its clips are compositions of.
    pub video_id: Option<Id>,
    pub project_id: Id,
    /// Where its files are.
    pub dir: String,
    /// A frame of it, for its card.
    pub poster: Option<String>,
    pub clips: u32,
    pub clips_ready: u32,
    pub clips_approved: u32,
    pub clips_posted: u32,
    pub created_at: String,
    pub updated_at: String,
}

/// A clip.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[schemars(rename = "Clip")]
pub struct Clip {
    pub id: Id,
    pub vod_id: Id,
    pub kind: Kind,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    /// How good a Short it would make, 1 to 10, as the finder judged it.
    pub score: u32,
    /// Why it was picked.
    pub why: String,
    /// Where it starts and ends in the VOD, seconds.
    pub start_s: f64,
    pub end_s: f64,
    /// Its length once cut, seconds.
    pub length_s: f64,
    pub edit: ClipEdit,
    pub state: ClipState,
    pub error: Option<String>,
    /// The rendered file.
    pub file: Option<String>,
    /// A frame of it.
    pub poster: Option<String>,
    pub render_id: Option<Id>,
    /// When it goes out (RFC 3339), once scheduled.
    pub publish_at: Option<String>,
    pub privacy: Option<Privacy>,
    pub youtube_id: Option<String>,
    pub youtube_url: Option<String>,
    pub posted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Who may see a posted clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ClipPrivacy")]
pub enum Privacy {
    Public,
    Unlisted,
    Private,
}

impl Privacy {
    pub fn name(self) -> &'static str {
        match self {
            Privacy::Public => "public",
            Privacy::Unlisted => "unlisted",
            Privacy::Private => "private",
        }
    }
    pub fn parse(raw: &str) -> Option<Privacy> {
        match raw {
            "public" => Some(Privacy::Public),
            "unlisted" => Some(Privacy::Unlisted),
            "private" => Some(Privacy::Private),
            _ => None,
        }
    }
}

/// Whether clips wait for the person before going out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ClipPostingMode")]
pub enum PostingMode {
    /// Nothing goes out until the person approves it (the default).
    #[default]
    Ask,
    /// Clips scoring `min_score` or more go out at the next free slot on their own.
    Auto,
}

/// When and how clips go out.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[schemars(rename = "ClipPosting")]
pub struct Posting {
    pub mode: PostingMode,
    pub privacy: Privacy,
    /// The day's posting times, local, `"HH:MM"`: one clip each. Empty posts an approved clip at once.
    pub slots: Vec<String>,
    /// In `auto`, the score a clip needs to go out without the person.
    pub min_score: u32,
    /// Added to every description, after the clip's own line.
    pub footer: String,
}

/// The Clips space's settings: the channel, its look, the finder and posting.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[schemars(rename = "ClipSettings")]
pub struct Settings {
    /// The streamer, as titles and descriptions name them.
    pub streamer: String,
    /// Where they stream (`Twitch`, `YouTube`, `Kick`), and their handle there, for the end card
    /// and the credit line.
    pub platform: String,
    pub handle: String,
    /// Names and words the transcript should spell right: the streamer, the game, its terms.
    pub terms: String,
    /// What the channel's viewers like, said to the finder with every VOD.
    pub notes: String,
    pub captions: CaptionStyle,
    /// The colour key words are drawn in, `#RRGGBB`.
    pub highlight: String,
    pub end_card: bool,
    /// A clip's length, seconds.
    pub min_s: u32,
    pub max_s: u32,
    /// Most clips one find makes.
    pub max_clips: u32,
    /// The Claude model that finds the moments: `opus` or `sonnet`.
    pub model: String,
    /// The frame rate clips are drawn at: 30 or 60.
    pub fps: u32,
    pub posting: Posting,
}

/// What Clips needs on this PC, and whether each is there.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[schemars(rename = "ClipTool")]
pub struct Tool {
    pub name: String,
    pub found: bool,
    /// Its path, when found.
    pub path: Option<String>,
    /// What it is for.
    pub purpose: String,
    /// How the person installs it, when missing.
    pub install: Option<String>,
}

result!(#[schemars(rename = "ClipsSummary")] SummaryOut {
    pub vods: u32,
    /// The VOD being worked on now.
    pub working: Option<Vod>,
    pub queued: u32,
    /// Clips waiting for the person's yes or no.
    pub to_review: u32,
    pub approved: u32,
    pub scheduled: u32,
    pub posted: u32,
    /// The next clip to go out, and when.
    pub next_post: Option<Clip>,
    pub youtube: YoutubeStatus,
    /// Missing tools: Clips cannot work until they are installed.
    pub missing: Vec<Tool>,
});
op!(Summary, "clips.summary", Empty => SummaryOut,
    OpMeta::query(Scope::Global, 12, "Clips at a glance: the VOD being worked on, clips waiting for review, the next post, the YouTube account and any missing tool").plugin(PLUGIN));

result!(#[schemars(rename = "ClipsToolsOut")] ToolsOut { pub tools: Vec<Tool> });
op!(Tools, "clips.tools", Empty => ToolsOut,
    OpMeta::query(Scope::Global, 12, "What Clips needs on this PC (ffmpeg, whisper.cpp and its model, yt-dlp for links, Claude, the video kit) and how to install what is missing").plugin(PLUGIN));

op!(SettingsGet, "clips.settings.get", Empty => Settings,
    OpMeta::query(Scope::Global, 12, "The Clips settings: the streamer and their channel, the captions' look, clip lengths, the finder's model and posting").plugin(PLUGIN));
op!(SettingsSet, "clips.settings.set", Settings => Settings,
    person(OpMeta::mutation(Scope::Global, 12, "Replace the Clips settings").emits(CHANGED).plugin(PLUGIN)));

payload!(#[schemars(rename = "ClipsImportIn")] ImportIn {
    /// A video file on this PC (absolute path), or a link to a VOD (Twitch, YouTube).
    pub source: String,
    /// Default: the file's name or the link's title.
    pub title: Option<String>,
    /// What the person wants from it, in their words: "funny moments", "the IdleOn tips", "all".
    pub request: Option<String>,
    /// The kinds of moment to look for; empty or absent is every kind.
    pub kinds: Option<Vec<Kind>>,
    /// The project whose video the clips are made in; default the first project.
    pub project_id: Option<Id>,
});
op!(Import, "clips.vod.import", ImportIn => Vod,
    OpMeta::mutation(Scope::Global, 12, "Add a VOD (a file or a link) and start working on it: transcribe, find the moments asked for, cut, frame and render them").emits(CHANGED).plugin(PLUGIN));

result!(#[schemars(rename = "ClipsVodsOut")] VodsOut { pub vods: Vec<Vod> });
op!(VodList, "clips.vod.list", Empty => VodsOut, OpMeta::query(Scope::Global, 12, "Every VOD, newest first, with where its work stands and its clips counted").plugin(PLUGIN));

payload!(#[schemars(rename = "ClipsIdIn")] IdIn { pub id: Id });
result!(#[schemars(rename = "ClipsVodOut")] VodOut {
    pub vod: Vod,
    /// Its clips, best first.
    pub clips: Vec<Clip>,
});
op!(VodGet, "clips.vod.get", IdIn => VodOut, OpMeta::query(Scope::Global, 12, "One VOD and its clips").plugin(PLUGIN));

payload!(#[schemars(rename = "ClipsFindIn")] FindIn {
    pub id: Id,
    /// What to look for this time, in the person's words.
    pub request: String,
    pub kinds: Option<Vec<Kind>>,
    /// Most new clips; default the settings'.
    pub max: Option<u32>,
});
op!(Find, "clips.vod.find", FindIn => Vod,
    OpMeta::mutation(Scope::Global, 12, "Look through a transcribed VOD again for other moments (\"more IdleOn tips\", \"the dog\"): new clips are added beside the ones it has").emits(CHANGED).plugin(PLUGIN));

op!(VodStop, "clips.vod.stop", IdIn => Vod, OpMeta::mutation(Scope::Global, 12, "Stop a VOD's work; retry starts it again where it was").emits(CHANGED).plugin(PLUGIN));
op!(VodRetry, "clips.vod.retry", IdIn => Vod, OpMeta::mutation(Scope::Global, 12, "Start a failed or stopped VOD again from the stage it stopped at").emits(CHANGED).plugin(PLUGIN));
op!(VodDelete, "clips.vod.delete", IdIn => Empty,
    person(OpMeta::mutation(Scope::Global, 12, "Remove a VOD, its clips and its working files (the original file is never touched; posted clips stay on YouTube)").emits(CHANGED).plugin(PLUGIN)));

payload!(#[schemars(rename = "ClipsLayoutSetIn")] LayoutSetIn {
    pub id: Id,
    pub layout: SourceLayout,
    /// Draw its clips again with it.
    pub rerender: Option<bool>,
});
op!(LayoutSet, "clips.vod.layout", LayoutSetIn => Vod,
    OpMeta::mutation(Scope::Global, 12, "Correct where a VOD's facecam and game are (or where the person stands), and optionally redraw its clips").emits(CHANGED).plugin(PLUGIN));

/// A word of the transcript, in the VOD's milliseconds.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[schemars(rename = "ClipWord")]
pub struct Word {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

result!(#[schemars(rename = "ClipsClipOut")] ClipOut {
    pub clip: Clip,
    pub vod: Vod,
    /// The transcript around it: from 20 s before its start to 20 s after its end.
    pub words: Vec<Word>,
});
op!(ClipGet, "clips.clip.get", IdIn => ClipOut, OpMeta::query(Scope::Global, 12, "One clip with its edit and the transcript around it").plugin(PLUGIN));

payload!(#[schemars(rename = "ClipsClipListIn")] ClipListIn {
    /// One VOD's; every VOD's when absent.
    pub vod_id: Option<Id>,
    /// Only these states.
    pub states: Option<Vec<ClipState>>,
});
result!(#[schemars(rename = "ClipsClipsOut")] ClipsOut { pub clips: Vec<Clip> });
op!(ClipList, "clips.clip.list", ClipListIn => ClipsOut, OpMeta::query(Scope::Global, 12, "Clips, newest VOD first and best first in each: the review queue, what is scheduled, what was posted").plugin(PLUGIN));

payload!(#[schemars(rename = "ClipsClipUpdateIn")] ClipUpdateIn {
    pub id: Id,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub kind: Option<Kind>,
    /// The edit, whole.
    pub edit: Option<ClipEdit>,
    /// Draw it again once saved (default true when `edit` is given).
    pub render: Option<bool>,
});
op!(ClipUpdate, "clips.clip.update", ClipUpdateIn => Clip,
    OpMeta::mutation(Scope::Global, 12, "Change a clip's title, description, tags or edit (spans, framing, captions, hook); a changed edit is drawn again").emits(CHANGED).plugin(PLUGIN));

op!(ClipRender, "clips.clip.render", IdIn => Clip, OpMeta::mutation(Scope::Global, 12, "Draw a clip again from its edit").emits(CHANGED).plugin(PLUGIN));

payload!(#[schemars(rename = "ClipsClipSetIn")] ClipSetIn {
    pub id: Id,
    /// `approved` (it goes out at the next slot), `rejected`, or `ready` (undecided again; a
    /// scheduled clip is taken off the schedule).
    pub state: ClipState,
});
op!(ClipSet, "clips.clip.set", ClipSetIn => Clip,
    person(OpMeta::mutation(Scope::Global, 12, "Approve or reject a clip, or take it back to undecided").emits(CHANGED).plugin(PLUGIN)));

payload!(#[schemars(rename = "ClipsPostIn")] PostIn {
    pub id: Id,
    /// When it goes out (RFC 3339); now when absent.
    pub at: Option<String>,
    /// Default the settings'.
    pub privacy: Option<Privacy>,
});
op!(Post, "clips.clip.post", PostIn => Clip,
    person(OpMeta::mutation(Scope::Global, 12, "Post a clip to the connected YouTube channel now, or schedule it").emits(CHANGED).plugin(PLUGIN)));

op!(ClipDelete, "clips.clip.delete", IdIn => Empty,
    person(OpMeta::mutation(Scope::Global, 12, "Remove a clip and its file (a posted clip stays on YouTube)").emits(CHANGED).plugin(PLUGIN)));

/// The YouTube channel clips are posted to.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[schemars(rename = "ClipYoutube")]
pub struct YoutubeStatus {
    /// The person saved their Google OAuth client (a "Desktop app" client of their own Google
    /// Cloud project, with the YouTube Data API on).
    pub configured: bool,
    /// Signed in: Relay holds a token to upload with.
    pub connected: bool,
    pub channel: Option<String>,
    pub channel_id: Option<String>,
    /// A sign-in is waiting for the browser.
    pub pending: bool,
    /// What went wrong last, or what to know.
    pub note: Option<String>,
}

payload!(#[schemars(rename = "ClipsYoutubeSetupIn")] YoutubeSetupIn {
    /// The OAuth client's ID (`….apps.googleusercontent.com`).
    pub client_id: String,
    pub client_secret: String,
});
op!(YoutubeSetup, "clips.youtube.setup", YoutubeSetupIn => YoutubeStatus,
    person(OpMeta::mutation(Scope::Global, 12, "Save the Google OAuth client Relay signs in to YouTube with (kept in the desktop keyring)")
        .audit(Audit::Never).emits(CHANGED).plugin(PLUGIN)));

op!(YoutubeStatusGet, "clips.youtube.status", Empty => YoutubeStatus,
    OpMeta::query(Scope::Global, 12, "Whether a YouTube channel is connected, and which").plugin(PLUGIN));

result!(#[schemars(rename = "ClipsYoutubeConnectOut")] YoutubeConnectOut {
    /// Open this in the browser: Google asks the person to let Relay upload to their channel,
    /// then sends them back to Relay on this PC.
    pub url: String,
});
op!(YoutubeConnect, "clips.youtube.connect", Empty => YoutubeConnectOut,
    person(OpMeta::mutation(Scope::Global, 12, "Start signing in to YouTube: answers the page to open; clips.changed says when it is done").emits(CHANGED).plugin(PLUGIN)));

op!(YoutubeDisconnect, "clips.youtube.disconnect", Empty => YoutubeStatus,
    person(OpMeta::mutation(Scope::Global, 12, "Forget the YouTube sign-in (the OAuth client stays saved)").emits(CHANGED).plugin(PLUGIN)));

entries!(
    Summary, Tools, SettingsGet, SettingsSet, Import, VodList, VodGet, Find, VodStop, VodRetry, VodDelete, LayoutSet, ClipGet, ClipList,
    ClipUpdate, ClipRender, ClipSet, Post, ClipDelete, YoutubeSetup, YoutubeStatusGet, YoutubeConnect, YoutubeDisconnect,
);
