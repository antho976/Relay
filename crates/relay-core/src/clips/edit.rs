//! A clip's edit: its spans snapped to the words (never starting or ending inside a word or a
//! sentence), its length once the dead air is cut, the footage it needs, and the props the
//! template (`plugins/video/runtime/clips/short/`) draws it from.

use super::store::Section;
use relay_bus::ops::clips::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

/// A word as `streamclips.py transcribe` writes it (the kit's `Caption`), in the VOD's ms.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct W {
    pub text: String,
    #[serde(rename = "startMs")]
    pub start_ms: f64,
    #[serde(rename = "endMs")]
    pub end_ms: f64,
    #[serde(rename = "timestampMs", default)]
    pub timestamp_ms: Option<f64>,
    #[serde(default)]
    pub confidence: Option<f64>,
}

/// The seconds of picture kept after the last word, under the end card.
pub const TAIL_S: f64 = 1.6;
/// Without an end card, a short beat after the last word.
pub const BEAT_S: f64 = 0.4;
/// Handles cut around a clip's footage, so a span can be moved a little without cutting again.
pub const HANDLE_BEFORE_S: f64 = 2.0;
pub const HANDLE_AFTER_S: f64 = 3.0;

pub fn load_words(path: &Path) -> Result<Vec<W>, String> {
    let raw = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut words: Vec<W> = serde_json::from_slice(&raw).map_err(|e| format!("{}: {e}", path.display()))?;
    words.retain(|w| !w.text.trim().is_empty() && w.end_ms >= w.start_ms);
    words.sort_by(|a, b| a.start_ms.total_cmp(&b.start_ms));
    Ok(words)
}

/// The words from `a` to `b` (seconds).
pub fn between(words: &[W], a: f64, b: f64) -> &[W] {
    let (a, b) = (a * 1000.0, b * 1000.0);
    let lo = words.partition_point(|w| w.end_ms <= a);
    let hi = words.partition_point(|w| w.start_ms < b);
    &words[lo.min(hi)..hi]
}

fn ends_sentence(w: &W) -> bool {
    let t = w.text.trim_end();
    t.ends_with(['.', '!', '?']) || t.ends_with("…")
}

/// `[start, end]` (seconds) moved onto the words: the start just before a word that opens a
/// sentence (looking back at most `reach` seconds for one), the end just after a word that closes
/// one (looking on at most `reach`), never inside a word.
pub fn snap(words: &[W], start: f64, end: f64, reach: f64) -> (f64, f64) {
    if words.is_empty() || end <= start {
        return (start.max(0.0), end.max(start));
    }
    let (a_ms, b_ms, reach_ms) = (start * 1000.0, end * 1000.0, reach * 1000.0);
    // A sentence opens at a word after a full stop or a pause, and closes at one before them.
    let opens = |j: usize| j == 0 || ends_sentence(&words[j - 1]) || words[j].start_ms - words[j - 1].end_ms >= 1200.0;
    let closes = |m: usize| m + 1 == words.len() || ends_sentence(&words[m]) || words[m + 1].start_ms - words[m].end_ms >= 1200.0;
    // The first word heard from (a little before) the start, then back to where its sentence
    // opens when that is close.
    let mut i = words.partition_point(|w| w.start_ms < a_ms - 350.0).min(words.len() - 1);
    let mut j = i;
    while !opens(j) && words[i].start_ms - words[j - 1].start_ms <= reach_ms {
        j -= 1;
    }
    if opens(j) {
        i = j;
    }
    let first = &words[i];
    let floor = if i > 0 { words[i - 1].end_ms + 30.0 } else { 0.0 };
    let s = (first.start_ms - 160.0).max(floor).max(0.0);
    // The last word heard by (a little after) the end, then on to its sentence's end, when close.
    let mut k = words.partition_point(|w| w.end_ms <= b_ms + 500.0).max(i + 1) - 1;
    let mut m = k;
    while !closes(m) && words[m + 1].end_ms - words[k].end_ms <= reach_ms {
        m += 1;
    }
    if closes(m) {
        k = m;
    }
    let last = &words[k];
    let ceiling = words.get(k + 1).map_or(f64::MAX, |n| n.start_ms - 30.0);
    let e = (last.end_ms + 260.0).min(ceiling).max(last.end_ms);
    (s / 1000.0, e / 1000.0)
}

/// The spans a span keeps once pauses over `max_gap_ms` are cut down to 120 ms each side: the
/// kit's `keepSpeech`, so the length shown is the length drawn.
pub fn tighten(words: &[W], a: f64, b: f64, max_gap_ms: f64) -> Vec<[f64; 2]> {
    const PAD: f64 = 120.0;
    let (a_ms, b_ms) = (a * 1000.0, b * 1000.0);
    let inside: Vec<&W> = words.iter().filter(|w| w.end_ms > a_ms && w.start_ms < b_ms).collect();
    if inside.is_empty() {
        return vec![[a, b]];
    }
    let mut keep = Vec::new();
    let mut s = a_ms.max(inside[0].start_ms - PAD);
    let mut last = inside[0].end_ms;
    for w in &inside[1..] {
        if w.start_ms - last > max_gap_ms {
            keep.push([s / 1000.0, (last + PAD) / 1000.0]);
            s = w.start_ms - PAD;
        }
        last = last.max(w.end_ms);
    }
    keep.push([s / 1000.0, b_ms.min(last + PAD * 3.0) / 1000.0]);
    keep
}

/// Every span the clip plays, in order, dead air cut when the edit says so.
pub fn played(words: &[W], edit: &ClipEdit) -> Vec<[f64; 2]> {
    edit.keep.iter().flat_map(|&[a, b]| match edit.tighten_ms {
        Some(gap) => tighten(words, a, b, gap as f64),
        None => vec![[a, b]],
    }).collect()
}

/// How long the clip runs once drawn, seconds.
pub fn length(words: &[W], edit: &ClipEdit) -> f64 {
    let spoken: f64 = played(words, edit).iter().map(|[a, b]| (b - a).max(0.0)).sum();
    spoken + if edit.end_card { TAIL_S } else { BEAT_S }
}

/// Where the clip starts and ends in the VOD.
pub fn bounds(edit: &ClipEdit) -> (f64, f64) {
    let a = edit.keep.iter().map(|k| k[0]).fold(f64::MAX, f64::min);
    let b = edit.keep.iter().map(|k| k[1]).fold(f64::MIN, f64::max);
    (a.max(0.0), b.max(a))
}

/// The footage a clip needs, with its handles, held to the VOD.
pub fn section_for(edit: &ClipEdit, duration: f64) -> (f64, f64) {
    let (a, b) = bounds(edit);
    let tail = if edit.end_card { TAIL_S } else { BEAT_S };
    let end = if duration > 0.0 { (b + tail + HANDLE_AFTER_S).min(duration) } else { b + tail + HANDLE_AFTER_S };
    ((a - HANDLE_BEFORE_S).max(0.0), end)
}

/// Whether `section` still holds what `edit` plays.
pub fn covers(section: &Section, edit: &ClipEdit) -> bool {
    let (a, b) = bounds(edit);
    let tail = if edit.end_card { TAIL_S } else { BEAT_S };
    a >= section.start_s - 0.01 && b + tail <= section.end_s + 0.01
}

/// The edit checked: spans inside the VOD, each at least half a second, the whole no longer
/// than [`MAX_CLIP_S`], rects inside the picture, the text sized.
pub fn checked(mut edit: ClipEdit, duration: f64, width: u32, height: u32) -> Result<ClipEdit, String> {
    if edit.keep.is_empty() || edit.keep.len() > 40 {
        return Err("a clip keeps 1 to 40 spans".into());
    }
    for span in &mut edit.keep {
        let [a, b] = *span;
        if !a.is_finite() || !b.is_finite() || a < 0.0 || b - a < 0.3 || (duration > 0.0 && b > duration + 0.5) {
            return Err(format!("the span {a:.2}–{b:.2} s is not inside the VOD (or is under 0.3 s)"));
        }
        *span = [(a * 1000.0).round() / 1000.0, (b * 1000.0).round() / 1000.0];
    }
    let total: f64 = edit.keep.iter().map(|[a, b]| b - a).sum();
    if total > MAX_CLIP_S {
        return Err(format!("a clip is at most {MAX_CLIP_S} s; this one keeps {total:.0} s"));
    }
    if edit.tighten_ms.is_some_and(|g| !(200..=5000).contains(&g)) {
        return Err("cut pauses of 200 to 5000 ms".into());
    }
    let (w, h) = (width.max(1) as f64, height.max(1) as f64);
    let fit = |r: &mut Rect| {
        r.w = r.w.clamp(16.0, w);
        r.h = r.h.clamp(16.0, h);
        r.x = r.x.clamp(0.0, w - r.w);
        r.y = r.y.clamp(0.0, h - r.h);
    };
    if let Some(v) = edit.view.as_mut() {
        fit(v);
    }
    for r in edit.reframe.values_mut() {
        fit(r);
    }
    edit.reframe.retain(|k, _| k.parse::<usize>().is_ok_and(|i| i < edit.keep.len()));
    edit.hook = super::cut(edit.hook.trim(), 80);
    edit.emphasis = edit.emphasis.iter().map(|e| e.trim().to_string()).filter(|e| !e.is_empty()).take(24).collect();
    edit.fixes.retain(|k, v| !k.trim().is_empty() && !v.trim().is_empty());
    if edit.caption_y.is_some_and(|y| !(0.05..=0.92).contains(&y)) {
        return Err("captions sit between 5% and 92% of the height".into());
    }
    edit.punch_at.retain(|t| edit.keep.iter().any(|[a, b]| t >= a && t <= b));
    edit.punch_at.truncate(3);
    Ok(edit)
}

/// The 9:16 crop of `source` centred on `cx`, as tall as the picture.
pub fn tall_crop(width: u32, height: u32, cx: f64) -> Rect {
    let (w, h) = (width as f64, height as f64);
    let cw = (h * 9.0 / 16.0).round().min(w);
    Rect { x: (cx - cw / 2.0).clamp(0.0, w - cw).round(), y: 0.0, w: cw, h }
}

/// What the end card says: `LIVE ON TWITCH` over `@Handle`.
fn end_lines(s: &Settings) -> Value {
    let platform = if s.platform.trim().is_empty() { "Twitch".to_string() } else { s.platform.trim().to_string() };
    let handle = if s.handle.trim().is_empty() { s.streamer.trim() } else { s.handle.trim() };
    json!({
        "top": format!("Live on {platform}"),
        "handle": if handle.is_empty() { String::new() } else { format!("@{handle}") },
    })
}

/// The template's props for a clip: its footage and words from the section's start, its spans in
/// ms of the section, its framing and look.
pub fn props(clip_edit: &ClipEdit, section: &Section, words: &[W], vod: &Vod, layout: Option<&SourceLayout>, s: &Settings) -> Value {
    let off = section.start_s;
    let ms = |t: f64| ((t - off) * 1000.0).round();
    let local: Vec<Value> = between(words, section.start_s, section.end_s).iter().map(|w| json!({
        "text": w.text,
        "startMs": (w.start_ms - off * 1000.0).max(0.0).round(),
        "endMs": (w.end_ms - off * 1000.0).max(0.0).round(),
        "timestampMs": w.timestamp_ms.map(|t| (t - off * 1000.0).max(0.0).round()),
        "confidence": w.confidence,
    })).collect();
    let rect = |r: &Rect| json!({"x": r.x, "y": r.y, "w": r.w, "h": r.h});
    let whole = Rect { x: 0.0, y: 0.0, w: vod.width.max(1) as f64, h: vod.height.max(1) as f64 };
    // The clip's own scene, when the stream switched to another one for it.
    let layout = clip_edit.scene.as_ref().or(layout);
    let cam = layout.and_then(|l| l.cam);
    let game = layout.and_then(|l| l.game).unwrap_or(whole);
    let reframe: BTreeMap<String, Value> = clip_edit.reframe.iter().map(|(k, r)| (k.clone(), rect(r))).collect();
    json!({
        "src": section.file,
        "source": {"width": vod.width.max(1), "height": vod.height.max(1)},
        "words": local,
        "keep": clip_edit.keep.iter().map(|[a, b]| [ms(*a), ms(*b)]).collect::<Vec<_>>(),
        "tightenMs": clip_edit.tighten_ms,
        "layout": serde_json::to_value(clip_edit.layout).unwrap_or(json!("fit")),
        "view": clip_edit.view.as_ref().map(rect),
        "reframe": reframe,
        "cam": cam.as_ref().map(rect),
        "game": rect(&game),
        "hook": clip_edit.hook,
        "captions": serde_json::to_value(clip_edit.captions).unwrap_or(json!("pixel")),
        "captionY": clip_edit.caption_y,
        "emphasis": clip_edit.emphasis,
        "fixes": clip_edit.fixes,
        "punchAt": clip_edit.punch_at.iter().map(|t| ms(*t)).collect::<Vec<_>>(),
        "endCard": clip_edit.end_card,
        "end": end_lines(s),
        "highlight": s.highlight,
        "fps": s.fps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(text: &str, a: f64, b: f64) -> W {
        W { text: text.into(), start_ms: a, end_ms: b, timestamp_ms: None, confidence: None }
    }

    fn sample() -> Vec<W> {
        vec![
            w(" So", 1000.0, 1200.0), w(" anyway.", 1250.0, 1700.0),
            w(" I", 5000.0, 5100.0), w(" burned", 5150.0, 5500.0), w(" the", 5550.0, 5650.0), w(" marshmallow.", 5700.0, 6400.0),
            w(" Again!", 9000.0, 9600.0), w(" And", 9700.0, 9800.0), w(" then", 9850.0, 10000.0),
        ]
    }

    #[test]
    fn snap_lands_on_sentence_edges() {
        let words = sample();
        // Asked from inside "burned" to inside "marshmallow": out to the sentence.
        let (a, b) = snap(&words, 5.3, 6.0, 3.0);
        assert!((a - 5.0 + 0.16).abs() < 0.2 && a > 1.7, "start before I: {a}");
        assert!((6.4..9.0).contains(&b), "end after marshmallow: {b}");
    }

    #[test]
    fn snap_never_cuts_a_word() {
        let words = sample();
        let (a, b) = snap(&words, 0.0, 9.3, 1.0);
        for word in &words {
            let inside = |t: f64| t * 1000.0 > word.start_ms && t * 1000.0 < word.end_ms;
            assert!(!inside(a) && !inside(b), "{a} or {b} inside {word:?}");
        }
    }

    #[test]
    fn tighten_cuts_long_pauses_only() {
        let words = sample();
        let spans = tighten(&words, 0.8, 10.2, 650.0);
        assert_eq!(spans.len(), 3, "{spans:?}");
        assert!(spans.iter().all(|[a, b]| b > a));
        let all = tighten(&words, 0.8, 10.2, 5000.0);
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn checked_refuses_spans_outside_and_tidies_rects() {
        let edit = ClipEdit {
            keep: vec![[10.0, 20.0]], tighten_ms: Some(650), layout: Layout::Crop,
            view: Some(Rect { x: 1800.0, y: -5.0, w: 608.0, h: 1080.0 }), reframe: BTreeMap::from([("4".into(), Rect { x: 0.0, y: 0.0, w: 10.0, h: 10.0 })]),
            hook: " Hi ".into(), captions: CaptionStyle::Pixel, caption_y: None, emphasis: vec![" a ".into(), "".into()],
            fixes: BTreeMap::new(), punch_at: vec![15.0, 40.0], end_card: true, scene: None,
        };
        let e = checked(edit.clone(), 100.0, 1920, 1080).unwrap();
        assert_eq!(e.view.unwrap().x, 1920.0 - 608.0);
        assert_eq!(e.view.unwrap().y, 0.0);
        assert!(e.reframe.is_empty(), "a reframe for a span that is not there goes");
        assert_eq!(e.hook, "Hi");
        assert_eq!(e.emphasis, vec!["a"]);
        assert_eq!(e.punch_at, vec![15.0]);
        let mut far = edit;
        far.keep = vec![[90.0, 120.0]];
        assert!(checked(far, 100.0, 1920, 1080).is_err());
    }

    #[test]
    fn sections_hold_their_handles() {
        let edit = ClipEdit {
            keep: vec![[30.0, 50.0], [10.0, 12.0]], tighten_ms: None, layout: Layout::Fit, view: None, reframe: BTreeMap::new(),
            hook: String::new(), captions: CaptionStyle::None, caption_y: None, emphasis: vec![], fixes: BTreeMap::new(), punch_at: vec![], end_card: true, scene: None,
        };
        let (a, b) = section_for(&edit, 1000.0);
        assert_eq!(a, 8.0);
        assert!((b - (50.0 + TAIL_S + HANDLE_AFTER_S)).abs() < 1e-9);
        assert!(covers(&Section { file: "x".into(), start_s: a, end_s: b }, &edit));
        assert!(!covers(&Section { file: "x".into(), start_s: 11.0, end_s: b }, &edit));
    }
}
