//! `video.*` (docs/VIDEO.md, BUS.md §10.19): videos written as code. A video is a folder of React
//! components (Remotion) under Relay's data dir, which agents edit with their own file tools;
//! these ops make one, check it, look at frames of it and render it, through the Node worker the
//! engine keeps for them (D167).
//!
//! Every op but `video.kit.status` comes with the Video plugin (`.plugin("video")`): an agent may
//! call them only in a project with it on. The person is never gated.
use crate::registry::{OpMeta, Scope, Undo};
use crate::types::{Id, Ts};
use crate::{op, Empty};
use serde_json::Value;

/// The plugin the video ops come with.
pub const PLUGIN: &str = "video";

/// Most frames one `video.still` draws.
pub const MAX_STILLS: usize = 24;
/// Longest side a video may have, in px (8K).
pub const MAX_SIDE: u32 = 7680;
/// Longest video `video.create` sets up, in seconds; the code may make it as long as it likes.
pub const MAX_SECONDS: f64 = 600.0;

/// A video's shape when it is made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "VideoShape")]
pub enum Shape {
    /// 1080 × 1920: Shorts, Reels, TikTok, stories.
    Vertical,
    /// 1920 × 1080: YouTube, a site, a presentation.
    Landscape,
    /// 1080 × 1080.
    Square,
    /// 1080 × 1350: a 4:5 feed post.
    Portrait,
}

impl Shape {
    /// Width and height in px.
    pub fn size(self) -> (u32, u32) {
        match self {
            Shape::Vertical => (1080, 1920),
            Shape::Landscape => (1920, 1080),
            Shape::Square => (1080, 1080),
            Shape::Portrait => (1080, 1350),
        }
    }
}

/// A moment of a composition: seconds as a number, or text — `"1.5s"`, `"1500ms"`, `"0:01.5"`,
/// `"f45"` (frame 45), `"50%"` of its length, `"start"` or `"end"` (its last frame).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
#[schemars(rename = "VideoMoment")]
pub enum Moment {
    Seconds(f64),
    Text(String),
}

impl Moment {
    /// The frame this moment falls on in a composition `frames` long at `fps`, held to its
    /// frames; why not when it cannot be read.
    pub fn frame(&self, fps: f64, frames: u32) -> Result<u32, String> {
        let last = frames.saturating_sub(1) as f64;
        let at = |seconds: f64| -> Result<u32, String> {
            if !seconds.is_finite() || seconds < 0.0 {
                return Err(format!("{seconds} s is not a moment of the video"));
            }
            Ok((seconds * fps).round().min(last) as u32)
        };
        let text = match self {
            Moment::Seconds(s) => return at(*s),
            Moment::Text(t) => t.trim().to_ascii_lowercase(),
        };
        let number = |s: &str| s.trim().parse::<f64>().map_err(|_| format!("{text:?} is not a moment: use 1.5, \"1.5s\", \"1500ms\", \"0:01.5\", \"f45\", \"50%\", \"start\" or \"end\""));
        match text.as_str() {
            "start" => return Ok(0),
            "end" => return Ok(last as u32),
            _ => {}
        }
        if let Some(f) = text.strip_prefix('f') {
            let f = number(f)?;
            if f < 0.0 || f.fract() != 0.0 {
                return Err(format!("{text:?} is not a frame"));
            }
            return Ok((f as u32).min(last as u32));
        }
        if let Some(p) = text.strip_suffix('%') {
            let p = number(p)?;
            if !(0.0..=100.0).contains(&p) {
                return Err(format!("{text:?} is not within the video"));
            }
            return Ok((p / 100.0 * last).round() as u32);
        }
        if let Some(ms) = text.strip_suffix("ms") {
            return at(number(ms)? / 1000.0);
        }
        if let Some(s) = text.strip_suffix('s') {
            return at(number(s)?);
        }
        if let Some((m, s)) = text.split_once(':') {
            return at(number(m)? * 60.0 + number(s)?);
        }
        at(number(&text)?)
    }
}

/// What a frame is called in a still's labels and an issue: `1.5s · f45`.
pub fn frame_label(frame: u32, fps: f64) -> String {
    let s = frame as f64 / fps.max(1.0);
    let s = format!("{s:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    format!("{s}s · f{frame}")
}

result!(#[schemars(rename = "VideoRenderInfo")] RenderInfo {
    pub id: Id,
    pub state: RenderState,
    pub format: Format,
    /// The finished file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub preview: bool,
    /// The version of the code it drew.
    pub version: i64,
    pub finished_at: Option<Ts>,
});

/// A video: a folder of code (`dir`) and what Relay knows of it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[schemars(rename = "Video")]
pub struct Video {
    pub id: Id,
    pub project_id: Id,
    /// The terminal it belongs to, as a design's (`design.session`).
    pub session: Option<String>,
    pub title: String,
    /// Its folder: `src/` holds the code, `public/` what `staticFile()` names, `out/` its renders.
    pub dir: String,
    /// The composition stills and renders draw unless told otherwise (`main` at first).
    pub composition: String,
    /// The default composition's size, rate and length, as the last check found them.
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration_frames: u32,
    /// The latest version of its code Relay recorded (each check, still and render records the
    /// code it read when it changed); 0 before the first.
    pub version: i64,
    /// A frame of it to show for it: the latest still drawn of its default composition.
    pub poster: Option<String>,
    /// The newest render.
    pub last_render: Option<RenderInfo>,
    /// `user` or the session that made it.
    pub created_by: String,
    pub updated_by: String,
    pub created_at: Ts,
    pub updated_at: Ts,
    pub deleted_at: Option<Ts>,
}

result!(#[schemars(rename = "VideoComposition")] Composition {
    /// Its `<Composition id>`.
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration_frames: u32,
    /// Its `defaultProps`. `video.get` answers it; checks and stills leave it out.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub default_props: Value,
    /// Its zod `schema` as JSON Schema, when it has one: what the props panel edits. A string
    /// described `__remotion-color` is a colour (`zColor()`). `video.get` answers it; checks and
    /// stills leave it out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub props_schema: Option<Value>,
});

/// What kind of problem a check found in the code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "VideoErrorKind")]
pub enum ErrorKind {
    /// It does not parse.
    Syntax,
    /// It parses but does not build: an import that does not resolve, a missing file.
    Bundle,
    /// TypeScript's check. It still renders; fix it anyway.
    Type,
    /// It threw while drawing (`frame` says which), or while listing its compositions.
    Runtime,
    /// Props that do not fit the composition's schema.
    Props,
    /// `src/index.ts` no longer goes through `relayRoot`, so stills cannot see the frame.
    Probe,
}

result!(#[schemars(rename = "VideoCodeError")] CodeError {
    pub kind: ErrorKind,
    /// The file, relative to the video's folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub col: Option<u32>,
    pub message: String,
    /// The frame it threw on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<u32>,
});

result!(#[schemars(rename = "VideoVersion")] Version {
    pub n: i64,
    /// `user`, `system` or the session whose call recorded it.
    pub actor: String,
    pub files: u32,
    pub created_at: Ts,
    /// What changed from the version before: `src/Hook.tsx` (changed), `+src/Outro.tsx` (added),
    /// `-src/Old.tsx` (removed); at most 12.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed: Vec<String>,
});

/// Where a render is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "VideoRenderState")]
pub enum RenderState {
    Queued,
    Rendering,
    Done,
    Failed,
    Cancelled,
}

impl RenderState {
    pub fn name(self) -> &'static str {
        match self {
            RenderState::Queued => "queued",
            RenderState::Rendering => "rendering",
            RenderState::Done => "done",
            RenderState::Failed => "failed",
            RenderState::Cancelled => "cancelled",
        }
    }
    pub fn from_name(s: &str) -> Option<RenderState> {
        Some(match s {
            "queued" => RenderState::Queued,
            "rendering" => RenderState::Rendering,
            "done" => RenderState::Done,
            "failed" => RenderState::Failed,
            "cancelled" => RenderState::Cancelled,
            _ => return None,
        })
    }
    /// Whether it will not change again.
    pub fn is_final(self) -> bool {
        matches!(self, RenderState::Done | RenderState::Failed | RenderState::Cancelled)
    }
}

/// The file a render makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "VideoFormat")]
pub enum Format {
    /// H.264 with AAC: plays everywhere.
    Mp4,
    /// VP8 with Opus.
    Webm,
    /// An animated GIF, no sound.
    Gif,
    /// ProRes 4444 with its transparency kept, to lay over other footage in an editor.
    Prores,
    /// The sound alone.
    Mp3,
    Wav,
}

impl Format {
    pub fn name(self) -> &'static str {
        match self {
            Format::Mp4 => "mp4",
            Format::Webm => "webm",
            Format::Gif => "gif",
            Format::Prores => "prores",
            Format::Mp3 => "mp3",
            Format::Wav => "wav",
        }
    }
    pub fn from_name(s: &str) -> Option<Format> {
        Some(match s {
            "mp4" => Format::Mp4,
            "webm" => Format::Webm,
            "gif" => Format::Gif,
            "prores" => Format::Prores,
            "mp3" => Format::Mp3,
            "wav" => Format::Wav,
            _ => return None,
        })
    }
    /// The file's extension.
    pub fn ext(self) -> &'static str {
        match self {
            Format::Prores => "mov",
            other => other.name(),
        }
    }
}

/// How much care a render takes over its pictures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "VideoQuality")]
pub enum Quality {
    /// Half size and quick: to watch the whole thing through.
    Preview,
    /// Full size; the default.
    Standard,
    /// Full size, a lower CRF: a larger file for uploading.
    High,
}

/// A render of a video.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[schemars(rename = "VideoRender")]
pub struct VideoRender {
    pub id: Id,
    pub video_id: Id,
    pub project_id: Id,
    pub composition: String,
    pub format: Format,
    pub quality: Quality,
    pub state: RenderState,
    /// 0 to 1.
    pub progress: f64,
    /// The version of the code it draws.
    pub version: i64,
    /// The frames it covers, first and last (inclusive), when not the whole composition.
    pub range: Option<[u32; 2]>,
    /// The file's name as asked for (`out/<name>.<ext>`); `None` is `<composition>-<id>`.
    #[serde(default)]
    pub name: Option<String>,
    /// The file, once done.
    pub path: Option<String>,
    pub bytes: Option<i64>,
    /// Why it failed.
    pub error: Option<String>,
    /// Its sound as measured once done (formats with sound): loudness, peaks, clipping, and the
    /// targets it was brought to.
    #[serde(default)]
    pub audio: Option<AudioReport>,
    /// The loudness it was asked to reach, LUFS, and the true peak it may not pass, dBTP.
    #[serde(default)]
    pub loudness: Option<f64>,
    #[serde(default)]
    pub peak: Option<f64>,
    /// `user` or the session that asked for it.
    pub actor: String,
    pub created_at: Ts,
    pub started_at: Option<Ts>,
    pub finished_at: Option<Ts>,
}

/// Where a platform wants the sound: integrated loudness and the true peak it may not pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "VideoLoudnessPreset")]
pub enum LoudnessPreset {
    /// -14 LUFS, -1 dBTP: YouTube turns louder uploads down.
    Youtube,
    /// -14 LUFS, -1 dBTP: YouTube Shorts.
    Shorts,
    /// -14 LUFS, -1 dBTP: Instagram Reels.
    Reels,
    /// -14 LUFS, -1 dBTP.
    Tiktok,
    /// -16 LUFS, -1 dBTP: Discord does not normalise, and clips played beside voice chat sit
    /// better a little under the streaming level.
    Discord,
    /// -16 LUFS, -1 dBTP: Apple Podcasts, a spoken explainer.
    Podcast,
    /// -23 LUFS, -1 dBTP: EBU R128 broadcast.
    Broadcast,
}

impl LoudnessPreset {
    /// Integrated loudness in LUFS and the true-peak ceiling in dBTP.
    pub fn target(self) -> (f64, f64) {
        match self {
            LoudnessPreset::Youtube | LoudnessPreset::Shorts | LoudnessPreset::Reels | LoudnessPreset::Tiktok => (-14.0, -1.0),
            LoudnessPreset::Discord | LoudnessPreset::Podcast => (-16.0, -1.0),
            LoudnessPreset::Broadcast => (-23.0, -1.0),
        }
    }
}

result!(#[schemars(rename = "VideoAudioWarning")] AudioWarning {
    /// `clipping` (samples at full scale, or a true peak over 0 dBTP), `hot` (a true peak over
    /// -1 dBTP: lossy encoding will clip it), `silent` (no sound at all), `loud` / `quiet`
    /// (more than 2 LU from the loudness asked for, or from -14 LUFS when none was).
    pub kind: String,
    pub detail: String,
});

result!(#[schemars(rename = "VideoAudioReport")] AudioReport {
    /// Integrated loudness (ITU-R BS.1770-4, EBU R128 gating), LUFS; `None` when it is silent.
    pub lufs: Option<f64>,
    /// Loudness range (EBU Tech 3342), LU.
    pub lra: Option<f64>,
    /// The loudest 3 s, LUFS.
    pub short_term_max: Option<f64>,
    /// The highest true peak (4× oversampled), dBTP.
    pub true_peak: f64,
    /// The highest sample, dBFS.
    pub sample_peak: f64,
    /// Samples at full scale (clipped).
    pub clipped: u64,
    pub seconds: f64,
    pub channels: u32,
    /// The gain applied to reach the loudness asked for, dB (a limiter held the peaks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gain_db: Option<f64>,
    /// The loudness and true peak before that gain, when one was applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lufs_before: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub true_peak_before: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<AudioWarning>,
});

/// Whether the video kit (Node, Remotion and its browser) is set up on this machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "VideoKitState")]
pub enum KitState {
    /// Not set up: `video.kit.install`.
    Missing,
    /// Being set up; `video.kit.changed` says when it is done.
    Installing,
    Ready,
    /// Set up for an older Relay: `video.kit.install` brings it up to date.
    Outdated,
    /// The last setup failed (`error`).
    Failed,
    /// Node.js 20 or newer is not installed.
    NoNode,
}

result!(#[schemars(rename = "VideoKitProgress")] KitProgress {
    /// `packages` (npm installs the pinned packages) or `browser` (Remotion's headless Chrome).
    pub stage: String,
    /// 0 to 1 across the whole setup, as far as it can be told.
    pub progress: f64,
    /// What it is doing, in words.
    pub detail: String,
    /// Seconds since it started.
    pub elapsed_s: u64,
});

result!(#[schemars(rename = "VideoKitStatus")] KitStatus {
    pub state: KitState,
    /// While `installing`: how far it has come (`video.kit.progress` says so every few seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<KitProgress>,
    /// Where the kit lives: every video's folder is under it.
    pub root: String,
    /// The `@relay/video` kit's sources, to read when the skill is not enough; `node_modules/`
    /// beside them holds Remotion's own types.
    pub kit_dir: String,
    /// `node --version`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    /// The Remotion version the kit pins.
    pub remotion: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// What setting up does, for the person to agree to.
    pub about: String,
});

payload!(#[schemars(rename = "VideoIdIn")] IdIn { pub video_id: Id });

payload!(#[schemars(rename = "VideoListIn")] ListIn {
    pub project_id: Id,
    /// Only those tied to this terminal (session name).
    pub session: Option<String>,
    /// Also deleted ones (`deleted_at` set).
    pub include_deleted: Option<bool>,
});
result!(#[schemars(rename = "VideoListOut")] ListOut { pub videos: Vec<Video> });
op!(List, "video.list", ListIn => ListOut,
    OpMeta::query(Scope::Project, 12, "A project's videos (code that renders to a video), newest change first; optionally one terminal's")
        .plugin(PLUGIN));

result!(#[schemars(rename = "VideoGetOut")] GetOut {
    pub video: Video,
    /// The file to start reading: its default composition's component.
    pub entry: String,
    /// Its compositions as the last check found them.
    pub compositions: Vec<Composition>,
    /// The props the person set, by composition id: stills and renders draw with them unless
    /// given others.
    pub props: Value,
    /// The newest versions first, at most 20.
    pub versions: Vec<Version>,
    /// The newest renders first, at most 20.
    pub renders: Vec<VideoRender>,
});
op!(Get, "video.get", IdIn => GetOut,
    OpMeta::query(Scope::Project, 12, "A video: its folder, compositions, the props the person set, versions and renders")
        .plugin(PLUGIN));

payload!(#[schemars(rename = "VideoCreateIn")] CreateIn {
    pub project_id: Id,
    /// Default "Untitled video".
    pub title: Option<String>,
    /// Default vertical (1080 × 1920). `width` and `height` set a size of their own.
    pub shape: Option<Shape>,
    /// 16 to 7680, even; with `height`, instead of a shape.
    #[schemars(range(min = 16, max = 7680))]
    pub width: Option<u32>,
    #[schemars(range(min = 16, max = 7680))]
    pub height: Option<u32>,
    /// Frames a second, 1 to 120; default 30.
    #[schemars(range(min = 1, max = 120))]
    pub fps: Option<f64>,
    /// Its length to start with, 1 to 600; default 10. The code may change it.
    #[schemars(range(min = 1, max = 600))]
    pub seconds: Option<f64>,
    /// The terminal it belongs to. An agent's video is its own session's; the person may name
    /// any session of the project, or none.
    pub session: Option<String>,
});
result!(#[schemars(rename = "VideoCreateOut")] CreateOut {
    pub video: Video,
    /// The component to edit first: `src/Video.tsx`.
    pub entry: String,
    /// What the starter wrote, relative to the folder.
    pub files: Vec<String>,
    pub kit: KitStatus,
    /// What to do next.
    pub next: String,
});
op!(Create, "video.create", CreateIn => CreateOut,
    OpMeta::mutation(Scope::Project, 12, "Start a video, only when the person asked for one: a folder of Remotion code with a starter composition, to edit with your file tools")
        .undo(Undo::Inverse).emits(&["video.changed"]).plugin(PLUGIN));

payload!(#[schemars(rename = "VideoUpdateIn")] UpdateIn {
    pub video_id: Id,
    pub title: Option<String>,
    /// Tie it to another terminal (session name), or `null` to the project alone.
    #[serde(default, deserialize_with = "crate::nullable")] pub session: Option<Option<String>>,
    /// The composition stills and renders draw by default.
    pub composition: Option<String>,
    /// Props for one composition (`{"main": {"title": "…"}}`): stills and renders draw with them.
    /// A composition named with `null` goes back to its `defaultProps`.
    pub props: Option<serde_json::Map<String, Value>>,
});
op!(Update, "video.update", UpdateIn => Video,
    OpMeta::mutation(Scope::Project, 12, "Rename a video, tie it to another terminal, pick its default composition or set the props it draws with")
        .undo(Undo::Inverse).emits(&["video.changed"]).plugin(PLUGIN));

op!(Delete, "video.delete", IdIn => Empty,
    OpMeta::mutation(Scope::Project, 12, "Delete a video, when the person asked; video.restore brings it back, folder and all")
        .undo(Undo::Inverse).emits(&["video.changed"]).plugin(PLUGIN));
op!(Restore, "video.restore", IdIn => Video,
    OpMeta::mutation(Scope::Project, 12, "Bring a deleted video back")
        .undo(Undo::Inverse).emits(&["video.changed"]).plugin(PLUGIN));

result!(#[schemars(rename = "VideoCheckOut")] CheckOut {
    /// No syntax, bundle or runtime errors: it renders. Type errors leave it `ok`; fix them too.
    pub ok: bool,
    /// At most 50, the most useful first.
    pub errors: Vec<CodeError>,
    pub compositions: Vec<Composition>,
    /// The version of the code it read.
    pub version: i64,
});
op!(Check, "video.check", IdIn => CheckOut,
    OpMeta::query(Scope::Project, 12, "Build a video's code and typecheck it: errors with file and line, and its compositions; run after every edit")
        .emits(&["video.changed"]).plugin(PLUGIN));

payload!(#[schemars(rename = "VideoStillIn")] StillIn {
    pub video_id: Id,
    /// Default the video's composition.
    pub composition: Option<String>,
    /// The moments to draw, up to 24: seconds (1.5) or "1.5s", "1500ms", "0:01.5", "f45",
    /// "50%", "start", "end". Several make one contact sheet, labelled.
    #[schemars(length(max = 24))]
    pub at: Option<Vec<Moment>>,
    /// Instead of `at`: this many moments, 2 to 24, evenly across it (or across `from`–`to`).
    #[schemars(range(min = 2, max = 24))]
    pub count: Option<u32>,
    pub from: Option<Moment>,
    pub to: Option<Moment>,
    /// Size of one frame against the video's, 0.05 to 1. Default: one frame's longer side at
    /// 1280 px; a sheet's frames at 480.
    #[schemars(range(min = 0.05, max = 1.0))]
    pub scale: Option<f64>,
    /// Props to draw with instead of the video's (`video.get`'s `props`).
    pub props: Option<Value>,
    /// Answer the laid-out box of every element with a `data-id` (default false).
    pub boxes: Option<bool>,
});
result!(#[schemars(rename = "VideoStillFrame")] StillFrame {
    pub frame: u32,
    pub at_ms: f64,
    /// As the sheet labels it: `1.5s · f45`.
    pub label: String,
});
result!(#[schemars(rename = "VideoBox")] VideoBox {
    /// Its `data-id`.
    pub id: String,
    pub frame: u32,
    /// In the composition's px.
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// As drawn: its own and its parents' opacity together.
    pub opacity: f64,
});
result!(#[schemars(rename = "VideoIssue")] Issue {
    /// `clipped` (text cut off by a box that hides its overflow), `offscreen` (partly outside
    /// the frame), `unsafe` (outside the area a platform's buttons and captions leave free),
    /// `text_overlap`, `small_text` (hard to read on a phone), `font_fallback` (the font it names
    /// is not there, so another stood in), `contrast` (against the pixels under it, as drawn),
    /// `brief_text` (from `video.scan`: on screen too short a time to read), `broken_image`,
    /// `console` (an error the page logged).
    pub kind: String,
    /// The element's `data-id`, or its nearest parent's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The words concerned, cut to 60 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The frames it was found on.
    pub frames: Vec<u32>,
    pub detail: String,
});
result!(#[schemars(rename = "VideoStillOut")] StillOut {
    /// The picture: one frame, or a labelled contact sheet of them in order. Through `relay mcp`
    /// it is attached to the answer.
    pub png: String,
    pub composition: Composition,
    pub frames: Vec<StillFrame>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub boxes: Vec<VideoBox>,
    pub issues: Vec<Issue>,
    /// What stopped a frame from drawing, or broke the build.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<CodeError>,
    pub version: i64,
});
op!(Still, "video.still", StillIn => StillOut,
    OpMeta::query(Scope::Project, 12, "Look at a video: frames at moments of it (or a contact sheet across it) as a picture, with the problems found in them; through MCP the picture comes with the answer")
        .emits(&["video.changed"]).plugin(PLUGIN));

payload!(#[schemars(rename = "VideoScanIn")] ScanIn {
    pub video_id: Id,
    /// Default the video's composition.
    pub composition: Option<String>,
    /// Check every this many frames, 1 to 30; default 3 (ten times a second at 30 fps).
    #[schemars(range(min = 1, max = 30))]
    pub every: Option<u32>,
    /// Only part of it, from this moment (as `video.still`'s `at`) …
    pub from: Option<Moment>,
    /// … to this one.
    pub to: Option<Moment>,
    /// Props to draw with instead of the video's.
    pub props: Option<Value>,
});
result!(#[schemars(rename = "VideoScanOut")] ScanOut {
    pub composition: Composition,
    /// How many frames were measured.
    pub frames: u32,
    pub every: u32,
    /// As `video.still`'s, from every frame measured but without `contrast` (no pictures are
    /// drawn), with words in motion (entering, leaving, mid-transition) left out of the checks of
    /// where they are, and `brief_text`: words on screen too short a time to read.
    pub issues: Vec<Issue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<CodeError>,
    pub version: i64,
});
op!(Scan, "video.scan", ScanIn => ScanOut,
    OpMeta::query(Scope::Project, 12, "Check a whole video without pictures: the problems across every few frames, words in motion left out, and words shown too briefly to read")
        .emits(&["video.changed"]).plugin(PLUGIN));

/// Longest `wait` a render call blocks for, in ms (an hour).
pub const MAX_WAIT_MS: u64 = 3_600_000;
/// Most renders one `video.render.batch` queues.
pub const MAX_BATCH: usize = 12;

payload!(#[schemars(rename = "VideoRenderIn")] RenderIn {
    pub video_id: Id,
    /// Default the video's composition.
    pub composition: Option<String>,
    /// Default mp4.
    pub format: Option<Format>,
    /// Default standard.
    pub quality: Option<Quality>,
    /// Only part of it: from this moment (as `video.still`'s `at`) …
    pub from: Option<Moment>,
    /// … to this one, inclusive.
    pub to: Option<Moment>,
    /// Props to draw with instead of the video's.
    pub props: Option<Value>,
    /// The file's name, `out/<name>.<ext>`: letters, digits, dots, dashes and underscores. A
    /// render of the same name and format replaces the file. Default `<composition>-<render id>`.
    #[schemars(length(min = 1, max = 80))]
    pub name: Option<String>,
    /// Bring the sound to this integrated loudness once rendered, -40 to -5 LUFS (a gain, with a
    /// limiter holding the peaks under `peak`).
    #[schemars(range(min = -40.0, max = -5.0))]
    pub loudness: Option<f64>,
    /// The true peak the sound may not pass, -9 to 0 dBTP; default -1 when `loudness` or `preset`
    /// is given.
    #[schemars(range(min = -9.0, max = 0.0))]
    pub peak: Option<f64>,
    /// A platform's loudness and peak instead of `loudness` and `peak`.
    pub preset: Option<LoudnessPreset>,
    /// Answer once the render has finished (done, failed or cancelled) instead of at once.
    pub wait: Option<bool>,
    /// How long `wait` blocks, ms: default 30 minutes, at most an hour. A render still going then
    /// is answered as it stands.
    pub timeout_ms: Option<u64>,
});
op!(Render, "video.render", RenderIn => VideoRender,
    OpMeta::mutation(Scope::Project, 12, "Render a video to a file in its out/ folder, its sound measured (and brought to a loudness when asked); with wait: true answers once finished, else at once and video.render.wait waits")
        .emits(&["video.render.changed", "video.render.finished"]).plugin(PLUGIN));

payload!(#[schemars(rename = "VideoRenderJob")] RenderJob {
    /// Default the video's composition.
    pub composition: Option<String>,
    pub format: Option<Format>,
    pub quality: Option<Quality>,
    pub from: Option<Moment>,
    pub to: Option<Moment>,
    pub props: Option<Value>,
    #[schemars(length(min = 1, max = 80))]
    pub name: Option<String>,
    #[schemars(range(min = -40.0, max = -5.0))]
    pub loudness: Option<f64>,
    #[schemars(range(min = -9.0, max = 0.0))]
    pub peak: Option<f64>,
    pub preset: Option<LoudnessPreset>,
});
payload!(#[schemars(rename = "VideoRenderBatchIn")] RenderBatchIn {
    pub video_id: Id,
    /// Up to 12 renders, each as `video.render`'s fields: several compositions, the picture and
    /// the soundtrack (`format: "wav"`), several platforms' loudness. They run in this order.
    #[schemars(length(min = 1, max = 12))]
    pub jobs: Vec<RenderJob>,
    /// Answer once every one has finished.
    pub wait: Option<bool>,
    pub timeout_ms: Option<u64>,
});
result!(#[schemars(rename = "VideoRendersOut")] RendersOut {
    pub renders: Vec<VideoRender>,
    /// A wait ran out with some still queued or rendering.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub timed_out: bool,
});
op!(RenderBatch, "video.render.batch", RenderBatchIn => RendersOut,
    OpMeta::mutation(Scope::Project, 12, "Queue several renders of a video in one call (compositions, formats, platforms), optionally waiting for them all")
        .emits(&["video.render.changed", "video.render.finished"]).plugin(PLUGIN));

payload!(#[schemars(rename = "VideoRenderWaitIn")] RenderWaitIn {
    /// The renders to wait for: answered once each is done, failed or cancelled. One that has
    /// already finished counts at once, so nothing is missed however fast it was.
    #[schemars(length(min = 1, max = 50))]
    pub render_ids: Vec<Id>,
    /// Default 30 minutes, at most an hour.
    pub timeout_ms: Option<u64>,
});
op!(RenderWait, "video.render.wait", RenderWaitIn => RendersOut,
    OpMeta::query(Scope::Project, 12, "Wait for renders to finish (each done, failed or cancelled): one that already finished answers at once").plugin(PLUGIN));

payload!(#[schemars(rename = "VideoRenderIdIn")] RenderIdIn { pub render_id: Id });
op!(RenderGet, "video.render.get", RenderIdIn => VideoRender,
    OpMeta::query(Scope::Project, 12, "A render: its state, progress, file and its sound's loudness").plugin(PLUGIN));
op!(RenderCancel, "video.render.cancel", RenderIdIn => VideoRender,
    OpMeta::mutation(Scope::Project, 12, "Stop a render that is queued or running")
        .emits(&["video.render.changed", "video.render.finished"]).plugin(PLUGIN));

/// Most files one `video.asset.add` takes.
pub const MAX_ASSETS: usize = 100;

payload!(#[schemars(rename = "VideoAssetAddIn")] AssetAddIn {
    pub video_id: Id,
    /// A file on this machine: a picture, a clip, a sound or a font. It is copied in (or linked,
    /// `link`), so the video keeps it when the file moves.
    #[schemars(length(min = 1, max = 4096))]
    pub path: Option<String>,
    /// Several files at once, instead of `path`: up to 100, each under its own name.
    #[schemars(length(min = 1, max = 100))]
    pub paths: Option<Vec<String>>,
    /// A font installed on this machine by family name instead (`fc-list`): "Noto Serif". Its
    /// files for `weights` (and `italic`) are added under `assets/fonts/`, and the answer has the
    /// `loadFonts(…)` call that loads them.
    #[schemars(length(min = 1, max = 120))]
    pub font: Option<String>,
    /// With `font`: the weights wanted, 100 to 900; default 400 and 700.
    pub weights: Option<Vec<u32>>,
    /// With `font`: the italics of those weights too.
    pub italic: Option<bool>,
    /// The name to give it under `public/assets/` (one `path` only); default its own, made safe.
    #[schemars(length(min = 1, max = 120))]
    pub name: Option<String>,
    /// Replace a file of the same name (a clip recorded again). Without it a name another file
    /// has gets a number (`sweep-2.wav`), and the answer says so (`renamed_from`).
    pub overwrite: Option<bool>,
    /// Link to the file where it is instead of copying it: no second copy of a large clip. The
    /// video then needs the file to stay where it is.
    pub link: Option<bool>,
    /// Write a clip at this constant frame rate as it is added (H.264, `.mp4`): a phone's screen
    /// recording has frames unevenly spaced, so its frame numbers do not map to time.
    #[schemars(range(min = 1.0, max = 120.0))]
    pub fps: Option<f64>,
});
result!(#[schemars(rename = "VideoAsset")] VideoAsset {
    /// What `staticFile()` takes: `staticFile("assets/…")`.
    pub static_file: String,
    pub path: String,
    pub bytes: u64,
    /// `image`, `video`, `audio`, `font` or `other`.
    pub kind: String,
    /// A picture's size in px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<u32>,
    /// The name it was given when the one asked for (this) belonged to another file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renamed_from: Option<String>,
    /// It replaced a file of that name (`overwrite`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub replaced: bool,
    /// The same file was already there under this name: nothing was written.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unchanged: bool,
    /// The file it links to (`link`), when it is not a copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked: Option<String>,
    /// How it was made, when Relay made it (`video.voice`, `video.music`, `video.sfx`,
    /// `video.capture.android`): the prompt, the voice, the words' timings. Kept beside it as
    /// `<name>.json`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub made: Option<Value>,
});
result!(#[schemars(rename = "VideoFontAdded")] FontAdded {
    /// The family to name in `fontFamily`.
    pub family: String,
    /// The weights and styles found, as `"400"`, `"700 italic"`.
    pub faces: Vec<String>,
    /// What was asked for and is not installed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
    /// The call to put at module level (outside components): `loadFonts({family, file, weight}, …)`
    /// from `@relay/video`.
    pub load: String,
});
result!(#[schemars(rename = "VideoAssetAddOut")] AssetAddOut {
    /// What was added, in the order asked.
    pub assets: Vec<VideoAsset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<FontAdded>,
});
op!(AssetAdd, "video.asset.add", AssetAddIn => AssetAddOut,
    OpMeta::mutation(Scope::Project, 12, "Add pictures, clips, sounds or fonts to a video's public/assets/ (copied or linked, one or many, or a system font by name); answers each staticFile() name")
        .plugin(PLUGIN));

payload!(#[schemars(rename = "VideoAssetInspectIn")] AssetInspectIn {
    pub video_id: Id,
    /// The file: its `staticFile()` name (`assets/clip.mp4`), or an absolute path on this machine.
    #[schemars(length(min = 1, max = 4096))]
    pub asset: String,
    /// Look for where the picture changes (default true for a clip): taps, screens, scrolls.
    pub motion: Option<bool>,
});
result!(#[schemars(rename = "VideoMotionEvent")] MotionEvent {
    /// The clip's own frame it starts on, and its last.
    pub frame: u32,
    pub end_frame: u32,
    /// Seconds into the clip.
    pub at_s: f64,
    pub end_s: f64,
    /// `cut` (the screen changed at once: a new page, a tab), `motion` (half a second or more of
    /// movement: a scroll, an animation) or `change` (a short local change: a tap, a value).
    pub kind: String,
    /// How much of the picture changed at its height, 0 to 1.
    pub size: f64,
    /// Where, in the clip's px: x, y, w, h.
    pub region: [u32; 4],
});
result!(#[schemars(rename = "VideoAssetInfo")] AssetInfo {
    pub asset: VideoAsset,
    pub duration_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// Its frame rate, averaged over the clip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames: Option<u32>,
    /// Its frames are not evenly spaced (a phone's screen recording): frame numbers do not map to
    /// time. `video.asset.add {fps: 30}` adds it at a constant rate.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub variable_rate: bool,
    /// The shortest and longest gap between frames, ms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_gap_ms: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_codec: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_codec: Option<String>,
    /// Its sound, measured as a render's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioReport>,
    /// Where its picture changes, in order; at most 300.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub motion: Vec<MotionEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
});
op!(AssetInspect, "video.asset.inspect", AssetInspectIn => AssetInfo,
    OpMeta::query(Scope::Project, 12, "Read a clip, sound or picture: size, length, frame rate and frame count, a variable-rate warning, its loudness, and the frames where its picture changes (taps, screens, scrolls)").plugin(PLUGIN));

payload!(#[schemars(rename = "VideoAudioBeatsIn")] AudioBeatsIn {
    pub video_id: Id,
    /// The music: `assets/music.mp3` or an absolute path.
    #[schemars(length(min = 1, max = 4096))]
    pub asset: String,
    /// Default 4.
    #[schemars(range(min = 1, max = 12))]
    pub beats_per_bar: Option<u32>,
    /// About the tempo, when you know it: the search keeps near it (halves and doubles aside).
    #[schemars(range(min = 40.0, max = 240.0))]
    pub bpm: Option<f64>,
});
result!(#[schemars(rename = "VideoBeatBar")] BeatBar {
    /// From 1, at the first downbeat.
    pub n: u32,
    pub start_s: f64,
    /// The video's frame it starts on.
    pub frame: u32,
    /// RMS dBFS.
    pub loudness_db: f64,
});
result!(#[schemars(rename = "VideoBeatSection")] BeatSection {
    pub bar: u32,
    pub start_s: f64,
    pub frame: u32,
    /// `louder`, `quieter` (3 dB or more against the two bars before), `bass_in`, `bass_out`.
    pub kind: String,
    pub change_db: f64,
});
result!(#[schemars(rename = "VideoBeatGrid")] BeatGrid {
    pub bpm: f64,
    pub beat_s: f64,
    pub bar_s: f64,
    pub beats_per_bar: u32,
    pub first_beat_s: f64,
    pub first_downbeat_s: f64,
    /// The video's frame rate, and the grid in its frames.
    pub fps: f64,
    pub frames_per_beat: f64,
    pub first_downbeat_frame: f64,
    /// The beats the grid was fitted through, and how far they sit from it (RMS ms).
    pub beats_found: u32,
    pub fit_rms_ms: f64,
    /// 0 to 1: how clearly the beat, and the downbeat among the bar's beats, stand out.
    pub beat_confidence: f64,
    pub downbeat_confidence: f64,
    pub seconds: f64,
    /// Every bar, at most 600.
    pub bars: Vec<BeatBar>,
    /// Where the music changes.
    pub sections: Vec<BeatSection>,
    /// The kit's call for this grid: `useBeatGrid({bpm, firstBeat, beatsPerBar})`.
    pub kit: String,
});
op!(AudioBeats, "video.audio.beats", AudioBeatsIn => BeatGrid,
    OpMeta::query(Scope::Project, 12, "A music file's beat grid: BPM, first beat and downbeat, every bar in the video's frames with its loudness, and where sections change; to cut on the beat").plugin(PLUGIN));

result!(#[schemars(rename = "VideoAssetListOut")] AssetListOut {
    /// Everything under `public/`, by name.
    pub assets: Vec<VideoAsset>,
});
op!(AssetList, "video.asset.list", IdIn => AssetListOut,
    OpMeta::query(Scope::Project, 12, "A video's assets: every file under public/ with its kind, size, the file a link points to, and how Relay made it").plugin(PLUGIN));

payload!(#[schemars(rename = "VideoVersionRestoreIn")] VersionRestoreIn {
    pub video_id: Id,
    /// A version `video.get` lists.
    pub version: i64,
});
result!(#[schemars(rename = "VideoVersionRestoreOut")] VersionRestoreOut {
    pub video: Video,
    /// The version recorded of the code as it was before, which a restore of it undoes this.
    pub previous: i64,
});
op!(VersionRestore, "video.version.restore", VersionRestoreIn => VersionRestoreOut,
    OpMeta::mutation(Scope::Project, 12, "Put a video's code back as an earlier version had it (the code before is recorded first)")
        .undo(Undo::Inverse).emits(&["video.changed"]).plugin(PLUGIN));

op!(KitStatusOp, "video.kit.status", Empty => KitStatus,
    OpMeta::query(Scope::Global, 12, "Whether this machine is set up to render videos (Node.js, Remotion and its browser)"));
payload!(#[schemars(rename = "VideoKitInstallIn")] KitInstallIn {
    /// Set it up again even when it is ready (a damaged install).
    pub force: Option<bool>,
});
op!(KitInstall, "video.kit.install", KitInstallIn => KitStatus,
    OpMeta::mutation(Scope::Global, 12, "Set this machine up to render videos (Remotion and its browser, about 600 MB, once); answers the real state: ready when it already is, else installing, with video.kit.progress as it goes and video.kit.changed at the end")
        .emits(&["video.kit.changed", "video.kit.progress"]).plugin(PLUGIN));

entries!(List, Get, Create, Update, Delete, Restore, Check, Still, Scan, Render, RenderBatch, RenderWait, RenderGet, RenderCancel,
    AssetAdd, AssetList, AssetInspect, AudioBeats, VersionRestore, KitStatusOp, KitInstall);

#[cfg(test)]
mod tests {
    use super::*;

    fn at(m: Moment) -> Result<u32, String> {
        m.frame(30.0, 300)
    }

    #[test]
    fn moments_read_as_agents_write_them() {
        assert_eq!(at(Moment::Seconds(1.5)), Ok(45));
        assert_eq!(at(Moment::Text("1.5s".into())), Ok(45));
        assert_eq!(at(Moment::Text("1500ms".into())), Ok(45));
        assert_eq!(at(Moment::Text("0:01.5".into())), Ok(45));
        assert_eq!(at(Moment::Text(" F45 ".into())), Ok(45));
        assert_eq!(at(Moment::Text("50%".into())), Ok(150));
        assert_eq!(at(Moment::Text("start".into())), Ok(0));
        assert_eq!(at(Moment::Text("end".into())), Ok(299));
        assert_eq!(at(Moment::Text("2".into())), Ok(60));
        // Past the end is its last frame, not a refusal: "the end of a 10 s video" is 10s.
        assert_eq!(at(Moment::Seconds(10.0)), Ok(299));
        assert_eq!(at(Moment::Text("f999".into())), Ok(299));
    }

    #[test]
    fn a_moment_that_is_not_one_says_how_to_write_one() {
        for bad in [Moment::Seconds(-1.0), Moment::Seconds(f64::NAN), Moment::Text("soon".into()),
            Moment::Text("f1.5".into()), Moment::Text("150%".into())] {
            assert!(at(bad.clone()).is_err(), "{bad:?}");
        }
        assert!(at(Moment::Text("soon".into())).unwrap_err().contains("\"1.5s\""));
    }

    #[test]
    fn frames_are_labelled_by_time_and_number() {
        assert_eq!(frame_label(45, 30.0), "1.5s · f45");
        assert_eq!(frame_label(0, 30.0), "0s · f0");
        assert_eq!(frame_label(30, 30.0), "1s · f30");
        assert_eq!(frame_label(1, 30.0), "0.03s · f1");
    }

    #[test]
    fn the_shapes_are_the_platforms_sizes() {
        assert_eq!(Shape::Vertical.size(), (1080, 1920));
        assert_eq!(Shape::Landscape.size(), (1920, 1080));
        assert_eq!(Format::Prores.ext(), "mov");
        for f in [Format::Mp4, Format::Webm, Format::Gif, Format::Prores, Format::Mp3, Format::Wav] {
            assert_eq!(Format::from_name(f.name()), Some(f));
        }
    }
}
