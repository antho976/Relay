//! Finding the moments: Claude reads the VOD's transcript, a part at a time, with the loud
//! seconds (and chat's, for a Twitch VOD with its replay) marked, and answers the moments that
//! would make Shorts of the kinds the person asked for, each with its title, hook, kind and score.
//!
//! It runs through the person's own Claude account (`claude -p`, as Threads does), with no tools
//! and a JSON schema for its answer. Parts of the stream are read side by side, a few at once;
//! the moments found are put together, overlaps dropped, and the best kept.

use super::edit::W;
use super::{clock, seconds, Halt, Step};
use crate::engine::Engine;
use relay_bus::ops::clips::*;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

/// How much of the stream one read covers, and how much each overlaps the next.
const PART_S: f64 = 40.0 * 60.0;
const OVERLAP_S: f64 = 2.5 * 60.0;
/// Reads at once.
const AT_ONCE: usize = 3;
/// The longest one read may take.
const READ_WITHIN: Duration = Duration::from_secs(12 * 60);

/// A moment as the finder answers it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Moment {
    pub kind: Kind,
    pub start: f64,
    pub end: f64,
    /// Parts inside to drop: a digression, a long read of chat.
    pub cut: Vec<[f64; 2]>,
    /// A short part played first: the payoff, before what led to it.
    pub teaser: Option<[f64; 2]>,
    pub title: String,
    pub hook: String,
    pub description: String,
    pub tags: Vec<String>,
    pub emphasis: Vec<String>,
    pub punch: Option<f64>,
    pub layout: Option<Layout>,
    pub score: u32,
    pub why: String,
}

/// A line of the transcript for reading: a sentence, or up to 14 s of speech.
#[derive(Debug, Clone)]
pub struct Line {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

pub fn lines(words: &[W]) -> Vec<Line> {
    let mut out = Vec::new();
    let mut cur: Vec<&W> = Vec::new();
    let flush = |cur: &mut Vec<&W>, out: &mut Vec<Line>| {
        if let (Some(a), Some(b)) = (cur.first(), cur.last()) {
            let text: String = cur.iter().map(|w| w.text.as_str()).collect::<String>().trim().to_string();
            if !text.is_empty() {
                out.push(Line { start: a.start_ms / 1000.0, end: b.end_ms / 1000.0, text });
            }
        }
        cur.clear();
    };
    for w in words {
        if let (Some(first), Some(last)) = (cur.first(), cur.last()) {
            if w.start_ms - last.end_ms > 900.0 || w.end_ms - first.start_ms > 14_000.0 {
                flush(&mut cur, &mut out);
            }
        }
        cur.push(w);
        let t = w.text.trim_end();
        if t.ends_with(['.', '!', '?']) {
            flush(&mut cur, &mut out);
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// The seconds that stand out, from `streamclips.py candidates`: (second, what was loud).
pub fn peaks(candidates: &Value) -> Vec<(f64, String)> {
    candidates["candidates"].as_array().map(|list| list.iter().filter_map(|c| {
        let at = c["peak_s"].as_f64()?;
        let why = &c["why"];
        let mut marks = Vec::new();
        if why["loud"].as_f64().unwrap_or(0.0) >= 1.5 {
            marks.push(format!("loud +{:.1}", why["loud"].as_f64().unwrap_or(0.0)));
        }
        if why["chat"].as_f64().unwrap_or(0.0) >= 1.5 {
            marks.push(format!("chat +{:.1}", why["chat"].as_f64().unwrap_or(0.0)));
        }
        if why["hype"].as_f64().unwrap_or(0.0) >= 1.5 {
            marks.push("chat hype".into());
        }
        (!marks.is_empty()).then(|| (at, marks.join(", ")))
    }).collect()).unwrap_or_default()
}

/// The parts of the stream each read covers: `[from, to)` seconds.
pub fn parts(duration: f64) -> Vec<(f64, f64)> {
    if duration <= PART_S * 1.25 {
        return vec![(0.0, duration.max(1.0))];
    }
    let n = (duration / PART_S).round().max(1.0) as usize;
    let step = duration / n as f64;
    (0..n).map(|i| ((i as f64 * step - OVERLAP_S).max(0.0), ((i + 1) as f64 * step).min(duration))).collect()
}

/// The transcript of `[from, to)` as the finder reads it: `[h:mm:ss] text`, with the loud
/// seconds marked at the end of their line.
pub fn transcript(lines: &[Line], peaks: &[(f64, String)], from: f64, to: f64) -> String {
    let mut out = String::new();
    for l in lines.iter().filter(|l| l.end >= from && l.start < to) {
        out.push_str(&format!("[{}] {}", clock_tenths(l.start), l.text));
        let marks: Vec<&str> = peaks.iter().filter(|(t, _)| *t >= l.start - 1.0 && *t <= l.end + 1.0).map(|(_, m)| m.as_str()).collect();
        if !marks.is_empty() {
            out.push_str(&format!("  ⟨{}⟩", marks.join("; ")));
        }
        out.push('\n');
    }
    out
}

fn clock_tenths(s: f64) -> String {
    let whole = clock(s.floor());
    format!("{whole}.{}", ((s - s.floor()) * 10.0).floor() as u32)
}

pub const PROMPT: &str = r#"You are the editor of a YouTube Shorts channel that clips a streamer's VODs. You read a part of a stream's transcript and pick the moments that would make great vertical Shorts. You write each one's title and opening hook, and you judge each honestly.

A good Short:
- opens on its hook in the first second: the line, the shout, or the moment just before it lands, never a slow setup;
- is about one thing and makes sense to someone who never saw the stream (cut a moment that needs the hour before it);
- ends on the payoff (the punchline, the death, the reaction, the answer), not after it;
- starts and ends on whole sentences.

Prefer: game talk and tips explained clearly (guides), funny reactions and bits, fails, stories from the streamer's life (family, pets, things that happened), real hype. Avoid: reading chat or TTS messages aloud, thanking subs and donations, sponsor reads, "be right back", long silences, anything that only makes sense with the picture of a menu nobody can read.

Kinds: funny (a joke, a bit, a funny reaction), fail (something going wrong), story (a story told), guide (a tip or how something works, complete with its reason), hype (a big win, a rare drop, a clutch), wholesome, rage (frustration, a rant), other.

For each moment:
- start: the time of the first word of the hook, end: the time of the last word of the payoff, from the transcript's stamps;
- cut: optional parts inside the moment to drop (a digression, reading chat), so what is left still flows;
- teaser: optional, 1 to 4 seconds of the payoff played first as a cold open, only when it makes the opening stronger;
- title: what happens, in a fan editor's voice, under 60 characters, one or two key words in CAPITALS, no emoji, never generic ("INSANE moment", "You won't believe");
- hook: the line written across the top for the first two seconds, 3 to 7 words, teasing the payoff without giving it away;
- description: one plain sentence about the clip;
- tags: 3 to 6, lower case;
- emphasis: 3 to 8 words actually said in the moment that carry it, to colour in the captions;
- punch: optional, the time of the one line to zoom in on;
- layout: "split" (facecam over gameplay: reactions), "fit" (the whole screen: when what is on screen matters, as in most guides), "cam" (only the face: pure talking or a story), "game" (only the game);
- score: 9-10 would do very well in the game's or the streamer's niche, 7-8 solid, 5-6 filler. Leave out anything under 5;
- why: one sentence on why it works.

The transcript comes from speech recognition: names and game words can be misheard. Lines marked ⟨loud …⟩ are where the stream got loud (shouting, laughing, a reaction); ⟨chat …⟩ where chat spiked. They are hints, not proof. Answer only with moments inside this part of the transcript."#;

fn schema() -> Value {
    let time = json!({"type": "string", "description": "h:mm:ss.s from the transcript"});
    json!({
        "type": "object",
        "properties": {
            "moments": {"type": "array", "items": {
                "type": "object",
                "properties": {
                    "kind": {"type": "string", "enum": ["funny", "fail", "story", "guide", "hype", "wholesome", "rage", "other"]},
                    "start": time, "end": time,
                    "cut": {"type": "array", "items": {"type": "array", "items": time, "minItems": 2, "maxItems": 2}},
                    "teaser": {"type": "array", "items": time, "minItems": 2, "maxItems": 2},
                    "title": {"type": "string"}, "hook": {"type": "string"}, "description": {"type": "string"},
                    "tags": {"type": "array", "items": {"type": "string"}},
                    "emphasis": {"type": "array", "items": {"type": "string"}},
                    "punch": time,
                    "layout": {"type": "string", "enum": ["split", "fit", "cam", "game"]},
                    "score": {"type": "integer", "minimum": 1, "maximum": 10},
                    "why": {"type": "string"}
                },
                "required": ["kind", "start", "end", "title", "hook", "description", "tags", "emphasis", "score", "why"]
            }}
        },
        "required": ["moments"]
    })
}

/// What one read is told: the channel, what the person wants, what is already clipped, and the
/// part of the transcript.
#[allow(clippy::too_many_arguments)]
pub fn ask(s: &Settings, vod: &Vod, request: &str, kinds: &[Kind], taken: &[(f64, f64)], part: (f64, f64), text: &str, want: u32) -> String {
    let streamer = if s.streamer.is_empty() { "the streamer".to_string() } else { s.streamer.clone() };
    let mut out = format!("Stream: \"{}\" by {streamer}", vod.title);
    if !s.platform.is_empty() {
        out.push_str(&format!(" (live on {})", s.platform));
    }
    out.push_str(&format!(".\nThis part: {} to {} of {}.\n", clock(part.0), clock(part.1), clock(vod.duration_s)));
    let request = request.trim();
    out.push_str(&format!("\nWhat the channel owner wants from this VOD: {}\n", if request.is_empty() { "the best moments of any kind" } else { request }));
    if !kinds.is_empty() && kinds.len() < Kind::ALL.len() {
        out.push_str(&format!("Kinds wanted: {}. Answer only these kinds.\n", kinds.iter().map(|k| k.name()).collect::<Vec<_>>().join(", ")));
    }
    out.push_str(&format!("Length of a clip: {} to {} seconds, cuts included.\n", s.min_s, s.max_s));
    if !s.terms.trim().is_empty() {
        out.push_str(&format!("Names and words that matter here (speech recognition may mishear them): {}\n", s.terms.trim()));
    }
    if !s.notes.trim().is_empty() {
        out.push_str(&format!("About the channel and its viewers: {}\n", s.notes.trim()));
    }
    let near: Vec<String> = taken.iter().filter(|(a, b)| *b >= part.0 && *a <= part.1).map(|(a, b)| format!("{}–{}", clock(*a), clock(*b))).collect();
    if !near.is_empty() {
        out.push_str(&format!("Already clipped, do not answer these again: {}\n", near.join(", ")));
    }
    out.push_str(&format!("\nAnswer up to {want} moments, best first; fewer if fewer are good.\n\nTranscript:\n{text}"));
    out
}

/// The moments in Claude's answer, read and checked against the part and the settings.
pub fn read_answer(answer: &Value, part: (f64, f64), s: &Settings) -> Vec<Moment> {
    let Some(list) = answer["moments"].as_array() else { return Vec::new() };
    let t = |v: &Value| v.as_str().and_then(seconds).or_else(|| v.as_f64());
    let pair = |v: &Value| -> Option<[f64; 2]> {
        let a = t(v.get(0)?)?;
        let b = t(v.get(1)?)?;
        (b > a).then_some([a, b])
    };
    let strings = |v: &Value, max: usize| -> Vec<String> {
        v.as_array().map(|a| a.iter().filter_map(|x| x.as_str()).map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).take(max).collect()).unwrap_or_default()
    };
    list.iter().filter_map(|m| {
        let start = t(&m["start"])?;
        let end = t(&m["end"])?;
        if end <= start + 2.0 || start < part.0 - 60.0 || end > part.1 + 120.0 {
            return None;
        }
        let score = m["score"].as_u64().unwrap_or(0).min(10) as u32;
        if score < 5 {
            return None;
        }
        let cut: Vec<[f64; 2]> = m["cut"].as_array().map(|c| c.iter().filter_map(pair).filter(|[a, b]| *a > start && *b < end).collect()).unwrap_or_default();
        let teaser = pair(&m["teaser"]).filter(|[a, b]| b - a <= 6.0 && *a >= start && *b <= end + 1.0);
        let layout = match m["layout"].as_str() {
            Some("split") => Some(Layout::Split),
            Some("fit") => Some(Layout::Fit),
            Some("cam") => Some(Layout::Cam),
            Some("game") => Some(Layout::Game),
            _ => None,
        };
        let title = super::cut(m["title"].as_str().unwrap_or("").trim(), 90);
        if title.is_empty() {
            return None;
        }
        let max = s.max_s as f64 + 20.0;
        let end = if end - start > max { start + max } else { end };
        Some(Moment {
            kind: m["kind"].as_str().and_then(Kind::parse).unwrap_or(Kind::Other),
            start, end, cut, teaser, title,
            hook: super::cut(m["hook"].as_str().unwrap_or("").trim(), 60),
            description: super::cut(m["description"].as_str().unwrap_or("").trim(), 300),
            tags: strings(&m["tags"], 8).into_iter().map(|t| t.trim_start_matches('#').to_lowercase()).collect(),
            emphasis: strings(&m["emphasis"], 10),
            punch: t(&m["punch"]).filter(|p| *p >= start && *p <= end),
            layout,
            score,
            why: super::cut(m["why"].as_str().unwrap_or("").trim(), 300),
        })
    }).collect()
}

fn overlap(a: (f64, f64), b: (f64, f64)) -> f64 {
    let shared = (a.1.min(b.1) - a.0.max(b.0)).max(0.0);
    shared / (a.1 - a.0).min(b.1 - b.0).max(0.1)
}

/// The moments from every read, put together: overlapping ones (the parts overlap, and two reads
/// may find the same moment) keep the better; those over what is already clipped go; the kinds
/// asked for only; best first, at most `max`, no one kind taking every place when others scored
/// close.
pub fn merge(mut found: Vec<Moment>, taken: &[(f64, f64)], kinds: &[Kind], max: usize) -> Vec<Moment> {
    found.retain(|m| kinds.is_empty() || kinds.contains(&m.kind));
    found.retain(|m| taken.iter().all(|t| overlap((m.start, m.end), *t) < 0.4));
    found.sort_by(|a, b| b.score.cmp(&a.score).then(a.start.total_cmp(&b.start)));
    let mut kept: Vec<Moment> = Vec::new();
    for m in found {
        if kept.iter().all(|k| overlap((k.start, k.end), (m.start, m.end)) < 0.35) {
            kept.push(m);
        }
    }
    // Variety: past half the places, a kind already holding half of them gives way to a moment of
    // another kind scoring at most one less.
    let mut out: Vec<Moment> = Vec::new();
    let mut rest = kept;
    while out.len() < max && !rest.is_empty() {
        let mut pick = 0;
        if out.len() >= max / 2 {
            let crowded = |k: Kind| out.iter().filter(|m| m.kind == k).count() * 2 >= out.len().max(1);
            if crowded(rest[0].kind) {
                if let Some(i) = rest.iter().position(|m| m.kind != rest[0].kind && m.score + 1 >= rest[0].score) {
                    pick = i;
                }
            }
        }
        out.push(rest.remove(pick));
    }
    out.sort_by(|a, b| b.score.cmp(&a.score).then(a.start.total_cmp(&b.start)));
    out
}

/// One read: Claude's answer for one part, or why not.
fn read(engine: &Engine, vod_id: i64, dir: &Path, model: &str, message: &str) -> Step<Value> {
    let claude = super::claude(engine)?;
    let mut cmd = Command::new(claude);
    cmd.args(["-p", "--output-format", "json", "--model", model, "--system-prompt", PROMPT, "--tools", "", "--strict-mcp-config",
        "--setting-sources", "", "--no-session-persistence", "--json-schema", &schema().to_string()])
        .current_dir(dir);
    let input = dir.join(format!(".find-{}.txt", uuid::Uuid::new_v4()));
    std::fs::write(&input, message).map_err(|e| Halt::Failed(format!("{}: {e}", input.display())))?;
    let file = std::fs::File::open(&input).map_err(|e| Halt::Failed(e.to_string()))?;
    let out = super::run_with(engine, vod_id, &mut cmd, file.into(), READ_WITHIN, |_| {});
    let _ = std::fs::remove_file(&input);
    let out = out?;
    let reply = super::last_json(&out).ok_or_else(|| Halt::Failed("Claude's answer could not be read".into()))?;
    if reply["is_error"].as_bool() == Some(true) {
        return Err(Halt::Failed(format!("Claude: {}", reply["result"].as_str().unwrap_or("it answered with an error"))));
    }
    if let Some(v) = reply.get("structured_output").filter(|v| v.is_object()) {
        return Ok(v.clone());
    }
    // Without the structured answer: the JSON in its text.
    let text = reply["result"].as_str().unwrap_or("");
    let start = text.find('{').ok_or_else(|| Halt::Failed("Claude answered no moments".into()))?;
    serde_json::from_str(&text[start..text.rfind('}').map_or(text.len(), |e| e + 1)]).map_err(|e| Halt::Failed(format!("Claude's moments could not be read: {e}")))
}

/// Find the moments of `vod` the person asked for: every part read (a few at once), then put
/// together. `on_progress` hears each part done.
#[allow(clippy::too_many_arguments)]
pub fn find(engine: &Engine, vod: &Vod, dir: &Path, words: &[W], candidates: &Value, request: &str, kinds: &[Kind], taken: &[(f64, f64)], max: u32,
    on_progress: &(dyn Fn(usize, usize) + Sync)) -> Step<Vec<Moment>> {
    let s = super::settings(engine)?;
    let lines = lines(words);
    if lines.is_empty() {
        return Err(Halt::Failed("the transcript is empty: nobody talks in this VOD, or the sound could not be read".into()));
    }
    let peaks = peaks(candidates);
    let parts = parts(vod.duration_s.max(lines.last().map_or(0.0, |l| l.end)));
    let total = parts.len();
    let want = ((max as f64 * 1.6 / parts.len() as f64).ceil() as u32 + 2).clamp(3, 12);
    let model = s.model.clone();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<Step<Vec<Moment>>> = std::thread::scope(|scope| {
        let mut out = Vec::new();
        for batch in parts.chunks(AT_ONCE) {
            let handles: Vec<_> = batch.iter().map(|&part| {
                let message = ask(&s, vod, request, kinds, taken, part, &transcript(&lines, &peaks, part.0, part.1), want);
                let (model, s, done) = (&model, &s, &done);
                scope.spawn(move || {
                    let answer = read(engine, vod.id, dir, model, &message);
                    let n = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                    on_progress(n, total);
                    answer.map(|a| read_answer(&a, part, s))
                })
            }).collect();
            for h in handles {
                out.push(h.join().unwrap_or_else(|_| Err(Halt::Failed("a read stopped unexpectedly".into()))));
            }
            if super::halted(engine, vod.id) {
                return vec![Err(Halt::Stopped)];
            }
        }
        out
    });
    let mut found = Vec::new();
    let mut failures = Vec::new();
    for r in results {
        match r {
            Ok(m) => found.extend(m),
            Err(Halt::Stopped) => return Err(Halt::Stopped),
            Err(Halt::Failed(why)) => failures.push(why),
        }
    }
    // A part that failed is a loss, not the end: unless every part failed.
    if found.is_empty() && !failures.is_empty() {
        return Err(Halt::Failed(failures.remove(0)));
    }
    let _ = std::fs::write(dir.join("moments.json"), serde_json::to_vec_pretty(&found).unwrap_or_default());
    Ok(merge(found, taken, kinds, max as usize))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(kind: Kind, start: f64, end: f64, score: u32) -> Moment {
        Moment { kind, start, end, cut: vec![], teaser: None, title: "t".into(), hook: "h".into(), description: String::new(), tags: vec![],
            emphasis: vec![], punch: None, layout: None, score, why: String::new() }
    }

    #[test]
    fn parts_cover_the_stream_with_overlaps() {
        assert_eq!(parts(1800.0), vec![(0.0, 1800.0)]);
        let p = parts(4.0 * 3600.0);
        assert_eq!(p.len(), 6);
        assert_eq!(p[0].0, 0.0);
        assert_eq!(p.last().unwrap().1, 4.0 * 3600.0);
        for w in p.windows(2) {
            assert!(w[1].0 < w[0].1, "parts overlap: {w:?}");
        }
    }

    #[test]
    fn merge_drops_overlaps_and_taken_and_keeps_variety() {
        let found = vec![
            m(Kind::Funny, 100.0, 130.0, 9), m(Kind::Funny, 105.0, 128.0, 8), // the same moment, read twice
            m(Kind::Funny, 300.0, 330.0, 8), m(Kind::Funny, 500.0, 530.0, 8), m(Kind::Guide, 700.0, 740.0, 7),
            m(Kind::Fail, 900.0, 920.0, 9), m(Kind::Story, 1000.0, 1040.0, 6),
        ];
        let out = merge(found.clone(), &[(895.0, 925.0)], &[], 4);
        assert_eq!(out.len(), 4);
        assert!(out.iter().all(|x| x.start != 900.0), "a taken moment is not found again");
        assert!(out.iter().filter(|x| x.start >= 100.0 && x.start <= 105.0).count() == 1);
        assert!(out.iter().any(|x| x.kind == Kind::Guide), "a guide close in score gets a place: {out:?}");
        let only = merge(found, &[], &[Kind::Guide], 5);
        assert_eq!(only.len(), 1);
    }

    #[test]
    fn answers_are_read_and_checked() {
        let s = super::super::default_settings();
        let answer = json!({"moments": [
            {"kind": "fail", "start": "0:10:05.2", "end": "0:10:40", "title": "He Burned It", "hook": "One job", "description": "d",
             "tags": ["#Cooking", "Fail"], "emphasis": ["burned"], "score": 8, "why": "w", "layout": "fit", "punch": "0:10:30", "cut": [["0:10:12", "0:10:15"]]},
            {"kind": "funny", "start": "0:12:00", "end": "0:12:30", "title": "Meh", "hook": "", "description": "", "tags": [], "emphasis": [], "score": 4, "why": ""},
            {"kind": "wat", "start": "0:15:00", "end": "0:14:00", "title": "Backwards", "hook": "", "description": "", "tags": [], "emphasis": [], "score": 9, "why": ""}
        ]});
        let got = read_answer(&answer, (0.0, 2400.0), &s);
        assert_eq!(got.len(), 1);
        let g = &got[0];
        assert_eq!(g.kind, Kind::Fail);
        assert!((g.start - 605.2).abs() < 1e-6);
        assert_eq!(g.tags, vec!["cooking", "fail"]);
        assert_eq!(g.layout, Some(Layout::Fit));
        assert_eq!(g.cut, vec![[612.0, 615.0]]);
        assert_eq!(g.punch, Some(630.0));
    }

    #[test]
    fn the_transcript_marks_loud_lines() {
        let lines = vec![Line { start: 61.0, end: 64.0, text: "No way!".into() }, Line { start: 70.0, end: 72.0, text: "Okay.".into() }];
        let text = transcript(&lines, &[(63.0, "loud +3.2".into())], 0.0, 100.0);
        assert!(text.starts_with("[1:01.0] No way!  ⟨loud +3.2⟩\n"), "{text}");
        assert!(text.contains("[1:10.0] Okay.\n"));
    }
}
