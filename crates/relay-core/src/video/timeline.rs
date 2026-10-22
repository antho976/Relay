//! A video's timeline (`video.timeline`, and the sound in `video.scan` and `video.still {sound}`):
//! its scenes and sounds on one time axis, checked against the music's beat grid, with the mix
//! measured and drawn — what an editor's timeline shows, for an agent that cannot hear.
//!
//! - **Scenes** are the Sequences Remotion's sequence manager holds, which `relayRoot` asks it to
//!   keep while Relay measures (`kit/root.tsx`): each with its name, its parent and its `from`
//!   in the parent's frames, so the composition's frame is the sum up the chain.
//! - **Sounds** come from two places. The kit's `Music`, `Voice` and `Cue` mark themselves on
//!   every frame they play (`kit/sound.tsx`): their role, file, how long they have played and how
//!   loud they are, so their start is exact. Any other `<Audio>`, and a clip's own sound, is in
//!   the assets Remotion collects for the mix on each frame drawn; its start is where it first
//!   appears (with a pass of every frame, exactly). A file's length (ffprobe) ends each.
//! - **The grid** is the video's own `useBeatGrid`, or one the call gives, or one found in the
//!   music (`beats`).

use super::probe::Probe;
use super::sheet::{draw_text, text_width, Rgba};
use relay_bus::ops::video::{frame_label, AudioReport, Issue, TimelineGrid, TimelineItem};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

/// Most scenes and sounds listed.
const MOST_ITEMS: usize = 300;
/// Most issues answered.
const MOST_ISSUES: usize = 40;
/// A start this many frames from a beat is on it.
const ON_BEAT: f64 = 1.0;
/// Seconds of nothing that count as a silence.
const SILENCE_S: f64 = 2.0;
/// Below this a stretch of the mix is silent, dBFS (RMS over 100 ms).
const SILENT_DB: f64 = -50.0;

/// A sound the kit marked on a frame (`kit/sound.tsx`).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SoundMark {
    pub role: String,
    pub src: String,
    #[serde(default)]
    pub name: String,
    /// Frames it has played on this one.
    pub elapsed: f64,
    pub volume: f64,
    pub frames: Option<f64>,
    pub trim: Option<f64>,
    pub rate: Option<f64>,
}

/// The grid a video set up with `useBeatGrid` (`kit/beats.ts`).
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridSet {
    pub bpm: f64,
    #[serde(default)]
    pub first_beat: f64,
    #[serde(default = "four")]
    pub beats_per_bar: u32,
}

fn four() -> u32 {
    4
}

/// A Sequence as `relayRoot` reports it.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SequenceSeen {
    pub id: String,
    pub parent: Option<String>,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub from: f64,
    pub duration: Option<f64>,
    #[serde(default)]
    pub hidden: bool,
}

/// The sound Remotion collected on one frame, for the mix (`worker.mjs`'s `soundsOf`).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FrameSounds {
    pub frame: u32,
    #[serde(default)]
    pub assets: Vec<SoundAsset>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SoundAsset {
    pub id: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    pub src: String,
    #[serde(default)]
    pub volume: f64,
    /// Where in its file it is, in the composition's frames.
    #[serde(default)]
    pub media: f64,
    #[serde(default = "one")]
    pub rate: f64,
    /// The frame it was collected on, when the page said (it can be read a frame late).
    #[serde(default)]
    pub frame: Option<u32>,
}

fn one() -> f64 {
    1.0
}

/// A beat grid in a composition's frames.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    pub bpm: f64,
    pub first_beat_s: f64,
    pub beats_per_bar: u32,
    pub fps: f64,
}

impl Grid {
    pub fn frames_per_beat(&self) -> f64 {
        60.0 / self.bpm * self.fps
    }
    fn first(&self) -> f64 {
        self.first_beat_s * self.fps
    }
    /// The nearest beat to `frame`: its index from the first downbeat, and how far `frame` is
    /// after it (negative: before).
    pub fn nearest(&self, frame: f64) -> (i64, f64) {
        let n = ((frame - self.first()) / self.frames_per_beat()).round();
        (n as i64, frame - (self.first() + n * self.frames_per_beat()))
    }
    /// Whether `frame` is within a frame of a bar's first beat.
    pub fn on_bar(&self, frame: f64) -> bool {
        let bar = self.frames_per_beat() * self.beats_per_bar as f64;
        let m = ((frame - self.first()) / bar).round();
        (frame - (self.first() + m * bar)).abs() <= ON_BEAT
    }
    /// Beat `n` as an editor says it: `bar 9 · beat 3`.
    pub fn name(&self, n: i64) -> Option<String> {
        (n >= 0).then(|| format!("bar {} · beat {}", n / self.beats_per_bar as i64 + 1, n % self.beats_per_bar as i64 + 1))
    }
    pub fn to_bus(&self, from: &str) -> TimelineGrid {
        TimelineGrid {
            bpm: (self.bpm * 1000.0).round() / 1000.0,
            first_beat_s: (self.first_beat_s * 1000.0).round() / 1000.0,
            beats_per_bar: self.beats_per_bar,
            frames_per_beat: (self.frames_per_beat() * 1000.0).round() / 1000.0,
            from: from.into(),
        }
    }
}

/// A sound on the timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub role: String,
    pub name: String,
    pub file: String,
    pub start: f64,
    pub end: f64,
    /// Its volume on the frames it was seen.
    pub volumes: Vec<(u32, f64)>,
    /// From the kit's marks (exact) rather than Remotion's assets.
    pub marked: bool,
}

impl Track {
    pub fn peak(&self) -> f64 {
        self.volumes.iter().map(|v| v.1).fold(0.0, f64::max)
    }
}

/// A scene on the timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub name: String,
    pub depth: u32,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Built {
    pub scenes: Vec<Scene>,
    pub tracks: Vec<Track>,
}

/// `http://localhost:3000/static-abc/assets/vo%2001.mp3` or `/static-abc/assets/x.wav` as
/// `staticFile()` names it: `assets/vo 01.mp3`.
pub fn static_name(src: &str) -> String {
    let path = src.split_once("://").map_or(src, |(_, rest)| rest.split_once('/').map_or("", |(_, p)| p));
    let path = path.split(['?', '#']).next().unwrap_or("").trim_start_matches('/');
    let path = match path.split_once('/') {
        Some((first, rest)) if first.starts_with("static-") || first == "public" => rest,
        _ => path,
    };
    let mut out = Vec::with_capacity(path.len());
    let bytes = path.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&path[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn stem(file: &str) -> String {
    let name = file.rsplit('/').next().unwrap_or(file);
    name.rsplit_once('.').map_or(name, |(s, _)| s).to_string()
}

/// What a sound with no role of its own is, by its file's name and length.
fn guess_role(file: &str, kind: &str, seconds: Option<f64>) -> String {
    let name = stem(file).to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| name.contains(w));
    if kind == "video" {
        "clip"
    } else if name.starts_with("vo-") || name.starts_with("vo_") || has(&["voice", "narration", "voiceover", "vo-", "line"]) {
        "voice"
    } else if has(&["music", "song", "bed", "track", "theme", "score"]) || seconds.is_some_and(|s| s >= 20.0) {
        "music"
    } else if has(&["sfx", "fx-", "fx_", "whoosh", "swoosh", "tap", "click", "pop", "ding", "hit", "impact", "riser"]) {
        "sfx"
    } else {
        "sound"
    }
    .into()
}

/// Whether a Sequence reads as a scene: one of the video's own, named or not, and not the kit's
/// wrapper of a sound (`sfx …`, `voice …`) nor a component of Remotion's that registers itself
/// (`<AbsoluteFill>`, `<Img>`).
fn is_scene(s: &SequenceSeen) -> bool {
    let kit_sound = ["sfx ", "voice ", "music ", "ui ", "whoosh ", "impact ", "ambience "].iter().any(|p| s.name.starts_with(p));
    let remotions = s.name.starts_with('<') && !s.name.contains("Sequence");
    !s.hidden && (s.kind == "sequence" || s.kind.is_empty()) && !kit_sound && !remotions
}

/// The scenes and sounds in `probes` (and the assets Remotion collected in `sounds`), measured
/// every `every` frames of a composition `frames` long at `fps`. `length` is a file's length in
/// the composition's frames, by its `staticFile()` name.
pub fn build(probes: &[Probe], sounds: &[FrameSounds], fps: f64, frames: u32, every: u32, length: &dyn Fn(&str) -> Option<f64>) -> Built {
    let total = frames as f64;
    let step = every.max(1) as f64;
    // Scenes: every Sequence seen, its start the sum of `from` up its parents.
    let mut seen: HashMap<String, SequenceSeen> = HashMap::new();
    for p in probes {
        for s in p.sequences.iter().flatten() {
            seen.entry(s.id.clone()).or_insert_with(|| s.clone());
        }
    }
    let starts = {
        let mut memo: HashMap<String, (f64, u32)> = HashMap::new();
        fn place(id: &str, seen: &HashMap<String, SequenceSeen>, memo: &mut HashMap<String, (f64, u32)>, guard: u32) -> (f64, u32) {
            if let Some(v) = memo.get(id) {
                return *v;
            }
            let Some(s) = seen.get(id) else { return (0.0, 0) };
            let (base, depth) = match s.parent.as_deref().filter(|_| guard < 64) {
                Some(parent) if seen.contains_key(parent) => {
                    let (b, d) = place(parent, seen, memo, guard + 1);
                    // A wrapper that is not a scene (an AbsoluteFill) adds no depth.
                    (b, d + u32::from(is_scene(&seen[parent])))
                }
                _ => (0.0, 0),
            };
            let v = (base + s.from, depth);
            memo.insert(id.to_string(), v);
            v
        }
        for id in seen.keys().cloned().collect::<Vec<_>>() {
            place(&id, &seen, &mut memo, 0);
        }
        memo
    };
    let mut scenes: Vec<Scene> = seen.values()
        .filter(|s| is_scene(s))
        .filter_map(|s| {
            let (start, depth) = *starts.get(&s.id)?;
            let end = s.duration.filter(|d| d.is_finite()).map_or(total, |d| (start + d).min(total));
            let name = if s.name.is_empty() || s.name.starts_with('<') { "Sequence".to_string() } else { s.name.clone() };
            (end > start && start < total).then(|| Scene { name, depth, start: start.max(0.0), end })
        })
        .collect();
    scenes.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.depth.cmp(&b.depth)).then(a.name.cmp(&b.name)));
    scenes.dedup_by(|a, b| a.name == b.name && a.start == b.start && a.end == b.end);
    scenes.truncate(MOST_ITEMS);

    // Sounds the kit marked: exact starts.
    let mut tracks: BTreeMap<(String, String, i64), Track> = BTreeMap::new();
    for p in probes {
        for m in &p.sounds {
            let file = static_name(&m.src);
            let start = p.frame as f64 - m.elapsed;
            let key = (file.clone(), m.name.clone(), start.round() as i64);
            let t = tracks.entry(key).or_insert_with(|| {
                let rate = m.rate.filter(|r| *r > 0.0).unwrap_or(1.0);
                let playable = length(&file).map(|l| (l - m.trim.unwrap_or(0.0)).max(0.0) / rate);
                let span = match (m.frames, playable) {
                    (Some(f), Some(l)) => Some(f.min(l)),
                    (Some(f), None) => Some(f),
                    (None, l) => l,
                };
                Track {
                    role: m.role.clone(),
                    name: if m.name.is_empty() { stem(&file) } else { m.name.clone() },
                    file: file.clone(),
                    start,
                    end: span.map_or(start + step, |s| start + s),
                    volumes: Vec::new(),
                    marked: true,
                }
            });
            t.volumes.push((p.frame, m.volume));
            if length(&file).is_none() && m.frames.is_none() {
                t.end = t.end.max(p.frame as f64 + step);
            }
        }
    }
    // Every other sound: Remotion's assets, by id (from the pass that rendered them, or as the
    // page read them, at the frame each was collected on).
    let read: Vec<FrameSounds> = probes.iter().flat_map(|p| p.assets.iter().map(move |a| FrameSounds { frame: a.frame.unwrap_or(p.frame), assets: vec![a.clone()] })).collect();
    let mut assets: BTreeMap<String, Track> = BTreeMap::new();
    for f in sounds.iter().chain(&read) {
        for a in &f.assets {
            let file = static_name(&a.src);
            let rate = if a.rate > 0.0 { a.rate } else { 1.0 };
            let t = assets.entry(a.id.clone()).or_insert_with(|| {
                // Where it is in its file says when it started: exact for a sound played from its
                // start (a trimmed one reads as starting earlier), and never later than first seen.
                let start = if a.media >= 0.0 { (f.frame as f64 - a.media / rate).min(f.frame as f64) } else { f.frame as f64 };
                let start = if every > 1 { start.max(f.frame as f64 - (step - 1.0)) } else { start };
                let seconds = length(&file).map(|l| l / fps);
                Track { role: guess_role(&file, &a.kind, seconds), name: stem(&file), file: file.clone(), start, end: f.frame as f64 + step, volumes: Vec::new(), marked: false }
            });
            t.end = t.end.max(f.frame as f64 + step);
            if let Some(l) = length(&file) {
                t.end = t.end.min(t.start + l / rate);
            }
            t.volumes.push((f.frame, a.volume));
        }
    }
    let marked: Vec<Track> = tracks.into_values().collect();
    let mut all = marked.clone();
    for t in assets.into_values() {
        // The kit's mark of the same sound says more.
        if !marked.iter().any(|m| m.file == t.file && (m.start - t.start).abs() <= step + 1.0) {
            all.push(t);
        }
    }
    for t in &mut all {
        t.end = t.end.min(total).max(t.start + 1.0);
    }
    all.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.name.cmp(&b.name)));
    all.truncate(MOST_ITEMS);
    Built { scenes, tracks: all }
}

/// The grid a video set up itself, if any probe saw one: one a timeline can draw (a tempo of 20
/// to 400 BPM, a first beat within the hour).
pub fn code_grid(probes: &[Probe], fps: f64) -> Option<Grid> {
    probes.iter().find_map(|p| p.grid).filter(|g| (20.0..=400.0).contains(&g.bpm) && g.first_beat.abs() <= 3600.0)
        .map(|g| Grid { bpm: g.bpm, first_beat_s: g.first_beat, beats_per_bar: g.beats_per_bar.max(1), fps })
}

fn is_cue(role: &str) -> bool {
    matches!(role, "sfx" | "ui" | "whoosh" | "impact")
}

/// The bus's items: scenes and sounds with their frames and, with a grid, how they sit on it.
pub fn items(built: &Built, grid: Option<&Grid>, fps: f64) -> (Vec<TimelineItem>, Vec<TimelineItem>) {
    let placed = |kind: &str, name: &str, start: f64, end: f64| {
        let s = start.round().max(0.0) as u32;
        let e = end.round().max(start.round() + 1.0) as u32;
        let mut item = TimelineItem {
            kind: kind.into(), name: name.into(), file: None, depth: None, start: s, frames: e - s, end: e,
            at: frame_label(s, fps), volume: None, on_beat: None, on_bar: None, off_by: None, beat: None,
        };
        if let Some(g) = grid {
            let (n, off) = g.nearest(start);
            item.on_beat = Some(off.abs() <= ON_BEAT);
            item.on_bar = Some(g.on_bar(start));
            item.off_by = Some((off * 10.0).round() / 10.0);
            item.beat = g.name(n);
        }
        item
    };
    let scenes = built.scenes.iter().map(|s| TimelineItem { depth: Some(s.depth), ..placed("scene", &s.name, s.start, s.end) }).collect();
    let sounds = built.tracks.iter().map(|t| TimelineItem {
        file: Some(t.file.clone()),
        volume: Some((t.peak() * 100.0).round() / 100.0),
        ..placed(&t.role, &t.name, t.start, t.end)
    }).collect();
    (scenes, sounds)
}

/// The mix as the checks and pictures read it: its level every millisecond (lowest and highest
/// sample), every 100 ms (RMS dBFS), and the frames where it clips.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mix {
    pub envelope: Vec<(f32, f32)>,
    pub rms_db: Vec<f64>,
    pub clipped_frames: Vec<u32>,
    pub report: Option<AudioReport>,
}

impl Mix {
    pub fn of(pcm: &super::media::Pcm, fps: f64, report: Option<AudioReport>) -> Mix {
        let ch = pcm.channels.max(1);
        let per_ms = (pcm.rate as usize / 1000).max(1);
        let mono: Vec<f32> = pcm.samples.chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32).collect();
        let envelope = mono.chunks(per_ms).map(|c| c.iter().fold((f32::MAX, f32::MIN), |(lo, hi), s| (lo.min(*s), hi.max(*s)))).collect();
        let rms_db = mono.chunks(per_ms * 100).map(|c| {
            let ms = c.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / c.len().max(1) as f64;
            if ms <= 1e-12 { -120.0 } else { 10.0 * ms.log10() }
        }).collect();
        let mut clipped_frames = Vec::new();
        for (i, frame) in pcm.samples.chunks(ch).enumerate() {
            if frame.iter().any(|s| s.abs() >= 32_766.0 / 32_768.0) {
                let f = (i as f64 / pcm.rate as f64 * fps) as u32;
                if clipped_frames.last() != Some(&f) {
                    clipped_frames.push(f);
                }
            }
        }
        Mix { envelope, rms_db, clipped_frames, report }
    }
}

/// What the sound gets wrong (docs/VIDEO.md, "Sound"): cues off the grid, voices over each other,
/// music that does not drop under a voice, silences, and a mix that clips.
pub fn issues(built: &Built, grid: Option<&Grid>, mix: Option<&Mix>, fps: f64, frames: u32) -> Vec<Issue> {
    let mut out: Vec<Issue> = Vec::new();
    let label = |f: f64| frame_label(f.round().max(0.0) as u32, fps);
    if let Some(g) = grid {
        for t in built.tracks.iter().filter(|t| is_cue(&t.role)) {
            let (n, off) = g.nearest(t.start);
            if off.abs() > ON_BEAT {
                let target = (t.start - off).round();
                out.push(Issue {
                    kind: "off_beat".into(), id: Some(t.name.clone()), text: Some(t.file.clone()), frames: vec![t.start.round() as u32],
                    detail: format!("starts {:.1} frames {} {} (frame {target}): move it there, unless it follows something on screen",
                        off.abs(), if off > 0.0 { "after" } else { "before" }, g.name(n).unwrap_or_else(|| "the first beat".into())),
                });
            }
        }
        for s in built.scenes.iter().filter(|s| s.depth == 0 && s.start > 0.5) {
            let (n, off) = g.nearest(s.start);
            if off.abs() > ON_BEAT {
                out.push(Issue {
                    kind: "off_beat".into(), id: Some(s.name.clone()), text: None, frames: vec![s.start.round() as u32],
                    detail: format!("the cut to {:?} is {:.1} frames {} {} (frame {})", s.name, off.abs(), if off > 0.0 { "after" } else { "before" },
                        g.name(n).unwrap_or_else(|| "the first beat".into()), (s.start - off).round()),
                });
            }
        }
    }
    let mut voices: Vec<&Track> = built.tracks.iter().filter(|t| t.role == "voice").collect();
    voices.sort_by(|a, b| a.start.total_cmp(&b.start));
    for pair in voices.windows(2) {
        let over = pair[0].end - pair[1].start;
        if over > 2.0 {
            out.push(Issue {
                kind: "voice_overlap".into(), id: Some(pair[1].name.clone()), text: Some(pair[0].name.clone()), frames: vec![pair[1].start.round() as u32],
                detail: format!("{:?} starts at {} while {:?} plays until {}: {:.0} frames over each other", pair[1].name, label(pair[1].start), pair[0].name, label(pair[0].end), over),
            });
        }
    }
    for music in built.tracks.iter().filter(|t| t.role == "music" && t.volumes.len() >= 2) {
        let under = |f: u32| voices.iter().any(|v| f as f64 >= v.start && (f as f64) < v.end);
        let mut free: Vec<f64> = music.volumes.iter().filter(|(f, _)| !voices.iter().any(|v| *f as f64 >= v.start - 15.0 && (*f as f64) < v.end + 15.0)).map(|v| v.1).collect();
        if free.is_empty() {
            continue;
        }
        free.sort_by(f64::total_cmp);
        let usual = free[free.len() / 2];
        for v in &voices {
            let during: Vec<f64> = music.volumes.iter().filter(|(f, _)| under(*f) && *f as f64 >= v.start + 3.0 && (*f as f64) < v.end).map(|x| x.1).collect();
            let Some(most) = during.iter().copied().reduce(f64::max) else { continue };
            if usual > 0.01 && most > usual * 10f64.powf(-3.0 / 20.0) {
                out.push(Issue {
                    kind: "no_duck".into(), id: Some(music.name.clone()), text: Some(v.name.clone()), frames: vec![v.start.round() as u32],
                    detail: format!("the music plays at {most:.2} under {:?} and {usual:.2} without it: duck it 6 to 10 dB under the voice (Music duckUnder)", v.name),
                });
            }
        }
    }
    // Silences: from the mix when there is one, else wherever nothing plays.
    let silent: Vec<(f64, f64)> = match mix {
        Some(mix) if !mix.rms_db.is_empty() => {
            let mut spans = Vec::new();
            let mut from = None;
            for (i, db) in mix.rms_db.iter().enumerate() {
                match (from, *db < SILENT_DB) {
                    (None, true) => from = Some(i),
                    (Some(a), false) => {
                        spans.push((a as f64 * 0.1, i as f64 * 0.1));
                        from = None;
                    }
                    _ => {}
                }
            }
            if let Some(a) = from {
                spans.push((a as f64 * 0.1, mix.rms_db.len() as f64 * 0.1));
            }
            spans
        }
        _ if !built.tracks.is_empty() => {
            let mut playing: Vec<(f64, f64)> = built.tracks.iter().filter(|t| t.peak() > 0.01).map(|t| (t.start, t.end)).collect();
            playing.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut spans = Vec::new();
            let mut at = 0.0;
            for (a, b) in playing {
                if a > at {
                    spans.push((at / fps, a / fps));
                }
                at = f64::max(at, b);
            }
            if at < frames as f64 {
                spans.push((at / fps, frames as f64 / fps));
            }
            spans
        }
        _ => Vec::new(),
    };
    for (a, b) in silent {
        if b - a >= SILENCE_S {
            out.push(Issue {
                kind: "silence".into(), id: None, text: None, frames: vec![(a * fps).round() as u32],
                detail: format!("{:.1} s of silence from {} to {}", b - a, label(a * fps), label(b * fps)),
            });
        }
    }
    if let Some(mix) = mix {
        if let Some(report) = &mix.report {
            if report.clipped >= 4 || report.true_peak > 0.0 {
                out.push(Issue {
                    kind: "clipping".into(), id: None, text: None, frames: mix.clipped_frames.iter().take(12).copied().collect(),
                    detail: format!("the mix peaks at {:+.1} dBTP with {} samples at full scale: lower the loudest sounds, or render with peak: -1", report.true_peak, report.clipped),
                });
            }
        }
    }
    out.truncate(MOST_ISSUES);
    out
}

// ---------------------------------------------------------------- pictures

const GROUND: [u8; 4] = [20, 22, 27, 255];
const LANE: [u8; 4] = [30, 33, 40, 255];
const INK: [u8; 4] = [228, 232, 238, 255];
const MUTED: [u8; 4] = [140, 148, 160, 255];
const BEAT: [u8; 4] = [255, 255, 255, 26];
const BAR: [u8; 4] = [255, 255, 255, 70];
const WAVE: [u8; 4] = [125, 211, 252, 255];
const WARN: [u8; 4] = [255, 90, 80, 255];
const PALETTE: [[u8; 4]; 6] = [[96, 165, 250, 255], [167, 139, 250, 255], [52, 211, 153, 255], [251, 191, 36, 255], [244, 114, 182, 255], [45, 212, 191, 255]];

fn role_colour(role: &str) -> [u8; 4] {
    match role {
        "music" => [96, 165, 250, 255],
        "voice" => [52, 211, 153, 255],
        "clip" => [148, 163, 184, 255],
        r if is_cue(r) => [251, 191, 36, 255],
        _ => [244, 114, 182, 255],
    }
}

/// `text` in capitals (the pixel font draws only its labels' `s` and `f` small), cut to fit
/// `width` px at `scale`.
fn fit(text: &str, width: u32, scale: u32) -> String {
    let text = text.to_uppercase();
    let text = text.as_str();
    let most = (width / (6 * scale)) as usize;
    if text.chars().count() <= most {
        text.to_string()
    } else if most > 1 {
        text.chars().take(most - 1).collect::<String>() + "…"
    } else {
        String::new()
    }
}

/// A box on a picture, px.
#[derive(Clone, Copy)]
struct Area {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

/// The mix's waveform between frames `a` and `b` into `area`.
fn waveform(out: &mut Rgba, mix: &Mix, fps: f64, (a, b): (f64, f64), Area { x, y, w, h }: Area) {
    out.fill(x, y, w, h, LANE);
    let mid = y + h / 2;
    out.fill(x, mid, w, 1, [255, 255, 255, 30]);
    let ms = |f: f64| (f / fps * 1000.0).max(0.0) as usize;
    for col in 0..w {
        let (fa, fb) = (a + (b - a) * col as f64 / w as f64, a + (b - a) * (col + 1) as f64 / w as f64);
        let (lo, hi) = mix.envelope.get(ms(fa)..ms(fb).max(ms(fa) + 1).min(mix.envelope.len())).unwrap_or(&[])
            .iter().fold((0f32, 0f32), |(lo, hi), e| (lo.min(e.0), hi.max(e.1)));
        let top = mid as f64 - hi.clamp(0.0, 1.0) as f64 * (h / 2) as f64;
        let bottom = mid as f64 - lo.clamp(-1.0, 0.0) as f64 * (h / 2) as f64;
        let clipped = hi >= 0.999 || lo <= -0.999;
        out.fill(x + col, top as u32, 1, (bottom - top).max(1.0) as u32, if clipped { WARN } else { WAVE });
        // A silent stretch is shaded.
        let rms = mix.rms_db.get((fa / fps * 10.0) as usize).copied().unwrap_or(0.0);
        if rms < SILENT_DB {
            out.fill(x + col, y, 1, h, [255, 90, 80, 40]);
        }
    }
}

fn grid_lines(out: &mut Rgba, grid: Option<&Grid>, (a, b): (f64, f64), Area { x, y, w, h }: Area) {
    let Some(g) = grid else { return };
    let fpb = g.frames_per_beat();
    if !fpb.is_finite() || fpb <= 0.0 || !g.first().is_finite() || (w as f64 / ((b - a) / fpb)) < 3.0 {
        return;
    }
    let (first, _) = g.nearest(a);
    let mut n = first.saturating_sub(1);
    // Never more lines than there are columns: a grid nowhere near the frames draws none.
    for _ in 0..=w {
        let f = g.first_beat_s * g.fps + n as f64 * fpb;
        if f > b {
            break;
        }
        if f >= a {
            let col = x + ((f - a) / (b - a) * w as f64) as u32;
            let bar = n.rem_euclid(g.beats_per_bar as i64) == 0;
            out.fill(col, y, 1, h, if bar { BAR } else { BEAT });
        }
        n = n.saturating_add(1);
    }
}

/// The timeline as a picture: a title, a ruler of seconds (and bars), the scenes by depth, a lane
/// per kind of sound, and the mix's waveform, over the beat grid, with the issues marked in red.
pub fn draw(built: &Built, grid: Option<&Grid>, mix: Option<&Mix>, issues: &[Issue], fps: f64, frames: u32, title: &str) -> Rgba {
    const W: u32 = 1600;
    const LEFT: u32 = 120;
    const RIGHT: u32 = 16;
    const ROW: u32 = 26;
    let span = (W - LEFT - RIGHT) as f64;
    let total = frames.max(1) as f64;
    let x = |f: f64| LEFT + ((f / total).clamp(0.0, 1.0) * span) as u32;
    let depths = built.scenes.iter().map(|s| s.depth).max().map_or(0, |d| (d + 1).min(4));
    let lanes: Vec<(&str, Vec<&Track>)> = [("MUSIC", "music"), ("VOICE", "voice"), ("CUES", "cue"), ("OTHER", "other")].into_iter()
        .map(|(label, kind)| (label, built.tracks.iter().filter(|t| match kind {
            "cue" => is_cue(&t.role),
            "other" => !is_cue(&t.role) && t.role != "music" && t.role != "voice",
            k => t.role == k,
        }).collect::<Vec<_>>()))
        .filter(|(_, ts)| !ts.is_empty())
        .collect();
    let wave_h = if mix.is_some() { 110 } else { 0 };
    let height = 40 + 28 + depths * ROW + lanes.iter().map(|(l, _)| if *l == "CUES" { ROW * 2 } else { ROW }).sum::<u32>() + wave_h + 24;
    let mut out = Rgba::new(W, height, GROUND);
    draw_text(&mut out, &fit(title, W - 24, 2), 12, 12, 2, INK);
    let mut y = 40;
    // The ruler: seconds, every 1, 2, 5 or 10 as they fit.
    let seconds = total / fps;
    let every = [1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0].into_iter().find(|s| span / (seconds / s) >= 70.0).unwrap_or(60.0);
    let mut s = 0.0;
    while s <= seconds + 0.001 {
        let col = x(s * fps);
        out.fill(col, y + 16, 1, 10, MUTED);
        let label = format!("{}s", s);
        if col + 3 + text_width(&label, 2) < W {
            draw_text(&mut out, &label, col + 3, y + 2, 2, MUTED);
        }
        s += every;
    }
    for issue in issues {
        for f in &issue.frames {
            out.fill(x(*f as f64).saturating_sub(1), y + 20, 3, 8, WARN);
        }
    }
    y += 28;
    let body_top = y;
    for depth in 0..depths {
        draw_text(&mut out, if depth == 0 { "SCENES" } else { "" }, 10, y + 8, 2, MUTED);
        out.fill(LEFT, y, W - LEFT - RIGHT, ROW - 2, LANE);
        for (i, scene) in built.scenes.iter().filter(|s| s.depth == depth).enumerate() {
            let (a, b) = (x(scene.start), x(scene.end).max(x(scene.start) + 2));
            let mut c = PALETTE[i % PALETTE.len()];
            c[3] = if depth == 0 { 200 } else { 130 };
            out.fill(a, y + 1, b - a - 1, ROW - 4, c);
            draw_text(&mut out, &fit(&scene.name, (b - a).saturating_sub(6), 2), a + 4, y + 6, 2, [12, 14, 18, 255]);
        }
        y += ROW;
    }
    for (label, tracks) in &lanes {
        let tall = if *label == "CUES" { ROW * 2 } else { ROW };
        draw_text(&mut out, label, 10, y + 8, 2, MUTED);
        out.fill(LEFT, y, W - LEFT - RIGHT, tall - 2, LANE);
        for (i, t) in tracks.iter().enumerate() {
            let (a, b) = (x(t.start), x(t.end).max(x(t.start) + 2));
            let mut c = role_colour(&t.role);
            c[3] = (90.0 + 165.0 * t.peak().clamp(0.0, 1.0)) as u8;
            if *label == "CUES" {
                // Cues alternate between two rows, so close ones keep their names.
                let row = y + (i as u32 % 2) * ROW;
                out.fill(a, row + 2, 2, ROW - 6, [251, 191, 36, 255]);
                out.fill(a, row + 2, (b - a).max(2), ROW - 6, c);
                draw_text(&mut out, &fit(&t.name, 160, 2), a + 4, row + 7, 2, INK);
            } else {
                out.fill(a, y + 2, b - a - 1, ROW - 6, c);
                draw_text(&mut out, &fit(&t.name, (b - a).saturating_sub(6), 2), a + 4, y + 7, 2, [12, 14, 18, 255]);
            }
        }
        y += tall;
    }
    if let Some(mix) = mix {
        draw_text(&mut out, "MIX", 10, y + wave_h / 2 - 7, 2, MUTED);
        waveform(&mut out, mix, fps, (0.0, total), Area { x: LEFT, y: y + 4, w: W - LEFT - RIGHT, h: wave_h - 8 });
        y += wave_h;
    }
    grid_lines(&mut out, grid, (0.0, total), Area { x: LEFT, y: body_top, w: W - LEFT - RIGHT, h: y - body_top });
    out
}

/// The strip a still with `sound` draws under its sheet, `width` px wide: the mix's waveform from
/// frame `a` to `b`, each frame drawn marked and numbered as the sheet orders them, and where the
/// cues and voice lines play.
pub fn strip(built: &Built, grid: Option<&Grid>, mix: Option<&Mix>, fps: f64, (a, b): (u32, u32), marks: &[u32], width: u32) -> Rgba {
    let (a, b) = (a as f64, (b as f64).max(a as f64 + 1.0));
    let (left, right) = (12u32, 12u32);
    let w = width.saturating_sub(left + right).max(10);
    let x = |f: f64| left + (((f - a) / (b - a)).clamp(0.0, 1.0) * w as f64) as u32;
    let mut out = Rgba::new(width, 150, GROUND);
    let wave = (8, 64);
    match mix {
        Some(mix) => waveform(&mut out, mix, fps, (a, b), Area { x: left, y: wave.0, w, h: wave.1 }),
        None => {
            out.fill(left, wave.0, w, wave.1, LANE);
            draw_text(&mut out, "NO SOUND", left + 8, wave.0 + 26, 2, MUTED);
        }
    }
    grid_lines(&mut out, grid, (a, b), Area { x: left, y: wave.0, w, h: wave.1 + 46 });
    // The sounds: voice lines as bars, cues as ticks with their names.
    let lane = wave.0 + wave.1 + 6;
    let mut said = 0;
    for t in built.tracks.iter().filter(|t| t.end > a && t.start < b) {
        let (sa, sb) = (x(t.start.max(a)), x(t.end.min(b)).max(x(t.start.max(a)) + 2));
        if t.role == "voice" {
            out.fill(sa, lane, sb - sa, 10, [52, 211, 153, 200]);
        } else if t.role != "music" && t.role != "clip" {
            let row = lane + 14 + (said % 2) * 14;
            out.fill(sa, lane, 2, 26 + (said % 2) * 14, [251, 191, 36, 255]);
            draw_text(&mut out, &fit(&t.name, 140, 1), sa + 4, row, 1, INK);
            said += 1;
        }
    }
    // Each frame the sheet drew, numbered in its order.
    for (i, f) in marks.iter().enumerate() {
        let col = x(*f as f64);
        out.fill(col, wave.0, 1, wave.1, [255, 255, 255, 160]);
        draw_text(&mut out, &(i + 1).to_string(), col + 3, wave.0 + 2, 1, INK);
    }
    draw_text(&mut out, &format!("{} – {}", frame_label(a as u32, fps), frame_label(b as u32, fps)), left, 128, 2, MUTED);
    let width_label = text_width("MUSIC  VOICE  CUES", 1);
    draw_text(&mut out, "VOICE", width.saturating_sub(width_label + 20), 132, 1, [52, 211, 153, 255]);
    draw_text(&mut out, "CUES", width.saturating_sub(width_label + 20) + text_width("VOICE  ", 1), 132, 1, [251, 191, 36, 255]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(frame: u32, sounds: Vec<SoundMark>, sequences: Option<Vec<SequenceSeen>>) -> Probe {
        Probe { frame, width: 1920.0, height: 1080.0, sounds, sequences, ..Default::default() }
    }

    fn mark(role: &str, src: &str, elapsed: f64, volume: f64) -> SoundMark {
        SoundMark { role: role.into(), src: src.into(), name: stem(src), elapsed, volume, frames: None, trim: None, rate: None }
    }

    fn seq(id: &str, parent: Option<&str>, name: &str, from: f64, duration: f64) -> SequenceSeen {
        SequenceSeen { id: id.into(), parent: parent.map(str::to_string), kind: "sequence".into(), name: name.into(), from, duration: Some(duration), hidden: false }
    }

    #[test]
    fn a_files_url_reads_as_its_static_file() {
        assert_eq!(static_name("http://localhost:3000/static-1a2b/assets/vo%2001.mp3?x=1"), "assets/vo 01.mp3");
        assert_eq!(static_name("/static-1a2b/assets/sfx-tap.wav"), "assets/sfx-tap.wav");
        assert_eq!(static_name("/public/assets/music.mp3"), "assets/music.mp3");
        assert_eq!(guess_role("assets/vo-03-home.mp3", "audio", Some(3.0)), "voice");
        assert_eq!(guess_role("assets/x.mp3", "audio", Some(80.0)), "music");
        assert_eq!(guess_role("assets/sfx-pop.wav", "audio", Some(0.4)), "sfx");
        assert_eq!(guess_role("assets/new-home.mp4", "video", None), "clip");
    }

    #[test]
    fn scenes_are_placed_up_their_parents_and_sounds_where_they_started() {
        let sequences = vec![seq("a", None, "Cold open", 0.0, 120.0), seq("b", None, "Title", 120.0, 150.0), seq("c", Some("b"), "Logo", 30.0, 60.0),
            seq("d", Some("b"), "sfx tap", 10.0, 999.0)];
        let probes = vec![
            probe(0, vec![mark("music", "/static-x/assets/music.mp3", 0.0, 0.45)], Some(sequences)),
            // A tap cue the kit marked 2 frames in, seen at 132: it started at 130.
            probe(132, vec![mark("music", "/static-x/assets/music.mp3", 132.0, 0.45), mark("sfx", "/static-x/assets/sfx-tap.wav", 2.0, 0.6)], None),
        ];
        let lengths = |file: &str| match file { "assets/sfx-tap.wav" => Some(9.0), "assets/music.mp3" => Some(2400.0), _ => None };
        let built = build(&probes, &[], 30.0, 270, 3, &lengths);
        let scenes: Vec<(&str, u32, f64, f64)> = built.scenes.iter().map(|s| (s.name.as_str(), s.depth, s.start, s.end)).collect();
        assert_eq!(scenes, [("Cold open", 0, 0.0, 120.0), ("Title", 0, 120.0, 270.0), ("Logo", 1, 150.0, 210.0)], "the kit's cue sequence is a sound, not a scene");
        let sounds: Vec<(&str, &str, f64, f64)> = built.tracks.iter().map(|t| (t.role.as_str(), t.name.as_str(), t.start, t.end)).collect();
        assert_eq!(sounds, [("music", "music", 0.0, 270.0), ("sfx", "sfx-tap", 130.0, 139.0)]);
    }

    #[test]
    fn cues_off_the_grid_are_found_and_on_it_are_not() {
        let grid = Grid { bpm: 120.0, first_beat_s: 0.0, beats_per_bar: 4, fps: 30.0 };
        let probes = vec![
            probe(60, vec![mark("sfx", "/s/assets/on.wav", 0.0, 0.6)], Some(vec![seq("a", None, "One", 0.0, 61.0), seq("b", None, "Two", 61.0, 100.0)])),
            probe(100, vec![mark("sfx", "/s/assets/off.wav", 0.0, 0.6)], None),
        ];
        let built = build(&probes, &[], 30.0, 300, 1, &|_| Some(10.0));
        let found = issues(&built, Some(&grid), None, 30.0, 300);
        let off: Vec<(&str, &str)> = found.iter().filter(|i| i.kind == "off_beat").map(|i| (i.id.as_deref().unwrap(), i.detail.as_str())).collect();
        assert_eq!(off.len(), 1, "{found:?}");
        assert_eq!(off[0].0, "off");
        // 15 frames a beat: 100 is nearest 105, the bar's fourth beat.
        assert!(off[0].1.contains("5.0 frames before bar 2 · beat 4 (frame 105)"), "{}", off[0].1);
        // The cut at 61 is a frame from bar 2's first beat (60): on it, and on the bar.
        let (scenes, sounds) = items(&built, Some(&grid), 30.0);
        assert_eq!((scenes[1].name.as_str(), scenes[1].on_beat, scenes[1].on_bar, scenes[1].off_by), ("Two", Some(true), Some(true), Some(1.0)));
        assert_eq!((sounds[0].on_beat, sounds[0].beat.as_deref()), (Some(true), Some("bar 2 · beat 1")));
    }

    #[test]
    fn voices_over_each_other_and_music_that_does_not_duck_are_said() {
        let mut probes = Vec::new();
        for f in (0..300).step_by(3) {
            let mut sounds = vec![mark("music", "/s/assets/music.mp3", f as f64, if (25..100).contains(&f) { 0.15 } else { 0.45 })];
            if (30..90).contains(&f) {
                sounds.push(mark("voice", "/s/assets/vo-1.mp3", (f - 30) as f64, 1.0));
            }
            if (80..140).contains(&f) {
                sounds.push(mark("voice", "/s/assets/vo-2.mp3", (f - 80) as f64, 1.0));
            }
            probes.push(probe(f, sounds, None));
        }
        let lengths = |file: &str| if file.contains("vo-") { Some(60.0) } else { Some(9000.0) };
        let built = build(&probes, &[], 30.0, 300, 3, &lengths);
        let found = issues(&built, None, None, 30.0, 300);
        let kinds: Vec<&str> = found.iter().map(|i| i.kind.as_str()).collect();
        assert!(kinds.contains(&"voice_overlap"), "{found:?}");
        // Ducked under the first line, not under the second (100-140 is at full level).
        let ducks: Vec<&str> = found.iter().filter(|i| i.kind == "no_duck").map(|i| i.text.as_deref().unwrap()).collect();
        assert_eq!(ducks, ["vo-2"], "{found:?}");
    }

    #[test]
    fn a_long_gap_with_nothing_playing_is_a_silence() {
        let probes = vec![probe(0, vec![mark("sfx", "/s/assets/a.wav", 0.0, 0.6)], None), probe(200, vec![mark("sfx", "/s/assets/b.wav", 0.0, 0.6)], None)];
        let built = build(&probes, &[], 30.0, 230, 1, &|_| Some(15.0));
        let found = issues(&built, None, None, 30.0, 230);
        let silences: Vec<&Issue> = found.iter().filter(|i| i.kind == "silence").collect();
        assert_eq!(silences.len(), 1, "{found:?}");
        assert_eq!(silences[0].frames, [15]);
    }

    #[test]
    fn a_grid_far_from_the_frames_draws_nothing_and_ends() {
        let built = Built::default();
        for grid in [Grid { bpm: 120.0, first_beat_s: -1e19, beats_per_bar: 4, fps: 30.0 }, Grid { bpm: 1e-300, first_beat_s: 0.0, beats_per_bar: 4, fps: 30.0 }] {
            let picture = draw(&built, Some(&grid), None, &[], 30.0, 90, "far");
            assert!(picture.w > 0);
        }
    }

    #[test]
    fn the_pictures_are_drawn() {
        let probes = vec![
            probe(0, vec![mark("music", "/s/assets/music.mp3", 0.0, 0.45)], Some(vec![seq("a", None, "Cold open", 0.0, 40.0),
                seq("b", None, "Feature", 40.0, 50.0), seq("c", Some("b"), "Card", 10.0, 20.0)])),
            probe(32, vec![mark("voice", "/s/assets/vo-01-open.mp3", 2.0, 1.0), mark("sfx", "/s/assets/sfx-tap.wav", 0.0, 0.6)], None),
            probe(61, vec![mark("sfx", "/s/assets/sfx-pop.wav", 0.0, 0.6)], None),
        ];
        let built = build(&probes, &[], 30.0, 90, 1, &|f| Some(if f.contains("music") { 90.0 } else if f.contains("vo-") { 30.0 } else { 6.0 }));
        let pcm = super::super::media::Pcm { rate: 48_000, channels: 1, samples: (0..48_000 * 3).map(|i| (i as f32 * 0.05).sin() * 0.5).collect() };
        let mix = Mix::of(&pcm, 30.0, None);
        let grid = Grid { bpm: 120.0, first_beat_s: 0.0, beats_per_bar: 4, fps: 30.0 };
        let picture = draw(&built, Some(&grid), Some(&mix), &[], 30.0, 90, "main · 3s");
        assert_eq!(picture.w, 1600);
        assert!(picture.h > 150);
        let under = strip(&built, Some(&grid), Some(&mix), 30.0, (0, 89), &[0, 45, 89], 900);
        assert_eq!((under.w, under.h), (900, 150));
        // Some of the waveform is drawn in its colour.
        assert!(under.data.chunks(4).any(|p| p[..3] == WAVE[..3]));
        // `TIMELINE_PNG=<dir>` keeps both, to look at.
        if let Ok(dir) = std::env::var("TIMELINE_PNG") {
            super::super::sheet::encode(&std::path::Path::new(&dir).join("timeline.png"), &picture).unwrap();
            super::super::sheet::encode(&std::path::Path::new(&dir).join("strip.png"), &under).unwrap();
        }
    }
}
