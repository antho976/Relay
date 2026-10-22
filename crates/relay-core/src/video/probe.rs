//! What `video.still` finds wrong in the frames it drew (docs/VIDEO.md, "Checks"). The kit's probe
//! (`plugins/video/runtime/kit/probe.ts`) measures each frame in the page — every element with a
//! `data-id`, every run of words with its font, colour, opacity and the box that clips it — and
//! this reads those measurements, and the pixels Remotion drew, into issues an agent can act on.
//!
//! A frame in the middle of an entrance can be clipped or partly off the frame on purpose; an
//! issue names every frame it was found on, so one that holds across frames reads as a fault.

use super::sheet::{css_color, text_contrast, Rgba};
use relay_bus::ops::video::{frame_label, Issue, VideoBox};
use serde::Deserialize;
use std::collections::BTreeMap;

/// One frame as the probe measured it.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Probe {
    pub frame: u32,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub boxes: Vec<ProbeBox>,
    #[serde(default)]
    pub texts: Vec<ProbeText>,
    #[serde(default)]
    pub images: Vec<ProbeImage>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProbeBox {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    #[serde(default = "one")]
    pub opacity: f64,
}

/// Words one element holds of its own, where they are drawn.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProbeText {
    pub id: Option<String>,
    #[serde(default)]
    pub path: String,
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Font size in px.
    pub size: f64,
    #[serde(default)]
    pub family: String,
    #[serde(default = "yes")]
    pub resolved: bool,
    #[serde(default)]
    pub color: String,
    #[serde(default = "one")]
    pub opacity: f64,
    /// The box that hides what overflows it: x, y, w, h.
    pub clip: Option<[f64; 4]>,
    /// The other texts (by index) this one sits inside.
    #[serde(default)]
    pub within: Vec<usize>,
    /// Not meant to be read (inside `aria-hidden`): no reading checks.
    #[serde(default)]
    pub decorative: bool,
    /// Part of an app's screen in a device frame (`data-relay-screen`): context, not held to the
    /// video's type sizes or reading times.
    #[serde(default)]
    pub screen: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProbeImage {
    pub id: Option<String>,
    pub src: String,
}

fn one() -> f64 {
    1.0
}
fn yes() -> bool {
    true
}

/// The kit's safe area (`safe.ts`): a frame taller than 3:2 keeps 10% top, 12% right, 20% bottom
/// and 6% left free; any other 5% each side. Left, top, right, bottom edges in px.
pub fn safe_area(w: f64, h: f64) -> [f64; 4] {
    if h > w * 1.5 {
        [w * 0.06, h * 0.1, w * (1.0 - 0.12), h * (1.0 - 0.2)]
    } else {
        [w * 0.05, h * 0.05, w * 0.95, h * 0.95]
    }
}

/// Px a box may pass an edge by before it counts: anti-aliasing and rounding.
const SLACK: f64 = 2.0;
/// Most issues one answer lists.
const MOST_ISSUES: usize = 40;

/// A frame drawn: what the probe measured, the picture (when it drew) at `scale`, the errors the
/// page logged, and which of its texts are in motion (by index; none when empty).
pub struct Drawn<'a> {
    pub probe: &'a Probe,
    pub picture: Option<&'a Rgba>,
    pub scale: f64,
    pub console: &'a [String],
    pub moving: &'a [bool],
}

/// The issues across the frames drawn, each once with every frame it was found on, in the order
/// they were first found; at most 40.
pub fn issues(frames: &[Drawn], fps: f64) -> Vec<Issue> {
    let mut found: BTreeMap<(String, String), (usize, Issue)> = BTreeMap::new();
    let mut add = |kind: &str, key: String, id: Option<String>, text: Option<String>, frame: u32, detail: String| {
        let n = found.len();
        let entry = found.entry((kind.to_string(), key)).or_insert_with(|| {
            (n, Issue { kind: kind.to_string(), id, text, frames: Vec::new(), detail })
        });
        if !entry.1.frames.contains(&frame) {
            entry.1.frames.push(frame);
        }
    };
    for drawn in frames {
        let p = drawn.probe;
        let (fw, fh) = (p.width.max(1.0), p.height.max(1.0));
        let safe = safe_area(fw, fh);
        let short = fw.min(fh);
        let visible: Vec<(usize, &ProbeText)> = p.texts.iter().enumerate().filter(|(_, t)| t.opacity > 0.05 && t.w > 0.0 && t.h > 0.0).collect();
        // Words in motion are where an animation puts them for a moment: where they are is
        // judged only once they settle.
        let moving = |i: usize| drawn.moving.get(i).copied().unwrap_or(false);
        for &(i, t) in &visible {
            let key = t.id.clone().unwrap_or_else(|| format!("{}|{}", t.path, t.text));
            let name = || Some(t.text.clone());
            let (x0, y0, x1, y1) = (t.x, t.y, t.x + t.w, t.y + t.h);

            if let Some([cx, cy, cw, ch]) = t.clip.filter(|_| !moving(i)) {
                let (cx1, cy1) = (cx + cw, cy + ch);
                let shows = x1 > cx && x0 < cx1 && y1 > cy && y0 < cy1;
                let cut = [cx - x0, cy - y0, x1 - cx1, y1 - cy1];
                if shows && cut.iter().any(|&c| c > SLACK) {
                    let sides: Vec<&str> = ["left", "top", "right", "bottom"].into_iter().zip(cut).filter(|(_, c)| *c > SLACK).map(|(s, _)| s).collect();
                    add("clipped", key.clone(), t.id.clone(), name(), p.frame,
                        format!("cut off at the {} by a box that hides its overflow ({} px)", sides.join(" and "), cut.iter().fold(0.0f64, |a, &b| a.max(b)).round()));
                }
            }

            let inside_frame = x1 > 0.0 && x0 < fw && y1 > 0.0 && y0 < fh && !moving(i);
            // A run's box is its glyphs', which rise past its line box by part of the font's
            // size: words laid out exactly at an edge are not past it.
            let edge = SLACK.max(t.size * 0.2);
            let past = [-x0, -y0, x1 - fw, y1 - fh];
            if inside_frame && past.iter().any(|&c| c > edge) {
                let sides: Vec<&str> = ["left", "top", "right", "bottom"].into_iter().zip(past).filter(|(_, c)| *c > edge).map(|(s, _)| s).collect();
                add("offscreen", key.clone(), t.id.clone(), name(), p.frame, format!("runs past the {} edge of the frame", sides.join(" and ")));
            } else if inside_frame {
                let into = [safe[0] - x0, safe[1] - y0, x1 - safe[2], y1 - safe[3]];
                if into.iter().any(|&c| c > edge) {
                    let sides: Vec<&str> = ["left", "top", "right", "bottom"].into_iter().zip(into).filter(|(_, c)| *c > edge).map(|(s, _)| s).collect();
                    add("unsafe", key.clone(), t.id.clone(), name(), p.frame,
                        format!("outside the safe area at the {} (useSafeArea / useSafePadding), where a platform's buttons and captions sit", sides.join(" and ")));
                }
            }

            if t.opacity > 0.5 && t.size < short * 0.022 && !t.decorative && !t.screen {
                add("small_text", key.clone(), t.id.clone(), name(), p.frame,
                    format!("{} px is hard to read on a phone; at least {} px for a frame {} px across", t.size, (short * 0.022).ceil(), short));
            }

            if !t.resolved && !t.family.is_empty() {
                add("font_fallback", t.family.clone(), t.id.clone(), name(), p.frame,
                    format!("the font {:?} is not there, so another stood in: use a kit family (Inter, Space Grotesk, Fraunces, JetBrains Mono), one fc-list shows, or load its file with loadFonts", t.family));
            }

            // Judged as drawn, dimmed or not; below half its opacity it is fading, not text to read.
            if let (Some(picture), Some((ink, alpha))) = (drawn.picture, css_color(&t.color)) {
                if alpha * t.opacity >= 0.5 && !t.decorative {
                    if let Some(c) = text_contrast(picture, drawn.scale, [t.x, t.y, t.w, t.h], ink, alpha * t.opacity) {
                        let large = t.size >= short * 0.04;
                        let need = if large { 3.0 } else { 4.5 };
                        // Judged as it is said, to the hundredth: 4.497 reads 4.50 and passes, so
                        // a flag never quotes the threshold back as a failure.
                        let said = (c * 100.0).round() / 100.0;
                        if said < need {
                            add("contrast", key.clone(), t.id.clone(), name(), p.frame,
                                format!("{said:.2}:1 against what is under it; {need:.1}:1 is the least for {} text", if large { "large" } else { "body" }));
                        }
                    }
                }
            }
        }

        // Texts over one another, both drawn whole, and not one inside the other: while one fades
        // in or out (a crossfade between scenes draws both) the overlap is the transition's.
        for (i, &(ai, a)) in visible.iter().enumerate() {
            for &(bi, b) in &visible[i + 1..] {
                if a.opacity < 0.95 || b.opacity < 0.95 || a.within.contains(&bi) || b.within.contains(&ai) || moving(ai) || moving(bi) {
                    continue;
                }
                let w = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
                let h = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
                if w <= SLACK || h <= SLACK {
                    continue;
                }
                let smaller = (a.w * a.h).min(b.w * b.h).max(1.0);
                if w * h / smaller > 0.2 {
                    let name = |t: &ProbeText| t.id.clone().unwrap_or_else(|| format!("{:?}", t.text));
                    let (an, bn) = (name(a), name(b));
                    let key = if an < bn { format!("{an}|{bn}") } else { format!("{bn}|{an}") };
                    add("text_overlap", key, a.id.clone().or_else(|| b.id.clone()), Some(format!("{} / {}", a.text, b.text)), p.frame,
                        format!("{} and {} are drawn over each other ({}% of the smaller)", an, bn, (w * h / smaller * 100.0).round()));
                }
            }
        }

        for img in &p.images {
            add("broken_image", img.src.clone(), img.id.clone(), None, p.frame,
                format!("{} did not load: add the file with video.asset.add and name it with staticFile()", img.src));
        }
        for line in drawn.console {
            add("console", line.clone(), None, None, p.frame, line.clone());
        }
    }
    let mut out: Vec<(usize, Issue)> = found.into_values().collect();
    out.sort_by_key(|(n, _)| *n);
    out.into_iter().take(MOST_ISSUES).map(|(_, mut issue)| {
        issue.frames.sort_unstable();
        if issue.frames.len() > 1 {
            let first = frame_label(issue.frames[0], fps);
            issue.detail = format!("{} (from {first}, {} frames)", issue.detail, issue.frames.len());
        }
        issue
    }).collect()
}

/// What identifies a run of words from frame to frame: its element (data-id and path) and its
/// words, with digits as one mark so a counting number stays the same run.
fn key(t: &ProbeText) -> String {
    let mut words = String::with_capacity(t.text.len());
    for c in t.text.chars() {
        if c.is_ascii_digit() {
            if !words.ends_with('#') {
                words.push('#');
            }
        } else {
            words.push(c);
        }
    }
    format!("{}|{}|{}", t.id.as_deref().unwrap_or_default(), t.path, words)
}

/// Which texts of `now` are in motion against `other`, a frame measured just before or after it:
/// not in it (entering or leaving), or there with another box or opacity.
pub fn moving(before: Option<&Probe>, now: &Probe) -> Vec<bool> {
    let Some(before) = before else { return vec![false; now.texts.len()] };
    let mut taken = vec![false; before.texts.len()];
    now.texts.iter().map(|t| {
        let k = key(t);
        let found = before.texts.iter().enumerate().position(|(j, b)| !taken[j] && key(b) == k);
        match found {
            Some(j) => {
                taken[j] = true;
                let b = &before.texts[j];
                [t.x - b.x, t.y - b.y, t.w - b.w, t.h - b.h].iter().any(|d| d.abs() > 0.5) || (t.opacity - b.opacity).abs() > 0.02
            }
            None => true,
        }
    }).collect()
}

/// Seconds a run of `words` words should stay on screen to be read: 0.4 s and 0.3 s a word.
fn reading_time(words: usize) -> f64 {
    0.4 + 0.3 * words as f64
}

/// Words on screen too short a time to read, across frames measured `every` frames apart up to
/// `last` (the last frame measured): each run of two words or more, from when it shows to when it
/// goes. A run still showing at the end is not cut short; one whose words grow (typed in) counts
/// as the longest.
pub fn brief(probes: &[Probe], fps: f64, every: u32, last: u32) -> Vec<Issue> {
    let mut seen: BTreeMap<String, (Vec<u32>, ProbeText)> = BTreeMap::new();
    for p in probes {
        for t in p.texts.iter().filter(|t| t.opacity > 0.5 && t.w > 0.0 && !t.decorative && !t.screen) {
            let entry = seen.entry(key(t)).or_insert_with(|| (Vec::new(), t.clone()));
            entry.0.push(p.frame);
        }
    }
    let keys: Vec<String> = seen.keys().cloned().collect();
    // Frames measured: a run of words ends at one of these without them, not at a gap in the
    // measurements (a frame whose measurement went unheard says nothing).
    let measured: std::collections::BTreeSet<u32> = probes.iter().map(|p| p.frame).collect();
    let mut out = Vec::new();
    for (k, (frames, t)) in &seen {
        let words = t.text.split_whitespace().count();
        let (element, text) = k.rsplit_once('|').unwrap_or(("", k));
        // Typed in: an earlier part of a longer run of the same element.
        let grows = keys.iter().any(|o| o != k && o.rsplit_once('|').is_some_and(|(e, w)| e == element && w.len() > text.len() && w.starts_with(text)));
        if words < 2 || grows {
            continue;
        }
        let mut runs: Vec<Vec<u32>> = Vec::new();
        for &f in frames {
            match runs.last_mut() {
                Some(run) if measured.range(run[run.len() - 1] + 1..f).next().is_none() => run.push(f),
                _ => runs.push(vec![f]),
            }
        }
        for run in runs {
            let (first, end) = (run[0], run[run.len() - 1]);
            if end + every > last {
                continue;
            }
            let shown = (end - first + every) as f64 / fps;
            let needed = reading_time(words);
            if shown < needed {
                out.push(Issue {
                    kind: "brief_text".into(),
                    id: t.id.clone(),
                    text: Some(t.text.clone()),
                    frames: run.clone(),
                    detail: format!("on screen about {shown:.1} s; {words} words want at least {needed:.1} s (0.4 s and 0.3 s a word)"),
                });
            }
        }
    }
    out
}

/// The issues across a whole video measured every `every` frames, without pictures: the still's
/// checks on words at rest, then those shown too briefly. At most 40.
pub fn scan(probes: &[Probe], console: &[String], fps: f64, every: u32) -> Vec<Issue> {
    // In motion against the frame before or the one after: the first frame measured has only one.
    let flags: Vec<Vec<bool>> = probes.iter().enumerate().map(|(i, p)| {
        let before = moving(i.checked_sub(1).map(|j| &probes[j]), p);
        let after = moving(probes.get(i + 1), p);
        before.iter().zip(after).map(|(a, b)| *a || b).collect()
    }).collect();
    let drawn: Vec<Drawn> = probes.iter().zip(&flags).enumerate().map(|(i, (probe, moving))| {
        Drawn { probe, picture: None, scale: 1.0, console: if i == 0 { console } else { &[] }, moving }
    }).collect();
    let mut found = issues(&drawn, fps);
    let last = probes.iter().map(|p| p.frame).max().unwrap_or(0);
    found.extend(brief(probes, fps, every, last));
    found.truncate(MOST_ISSUES);
    found
}

/// The boxes of the elements with a `data-id`, frame by frame; at most `limit`.
pub fn boxes(frames: &[&Probe], limit: usize) -> Vec<VideoBox> {
    frames.iter()
        .flat_map(|p| p.boxes.iter().map(|b| VideoBox { id: b.id.clone(), frame: p.frame, x: b.x, y: b.y, w: b.w, h: b.h, opacity: b.opacity }))
        .take(limit)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(id: &str, x: f64, y: f64, w: f64, h: f64) -> ProbeText {
        ProbeText {
            id: Some(id.into()), path: "div".into(), text: id.to_uppercase(), x, y, w, h, size: 64.0, family: "Inter".into(),
            resolved: true, color: "rgb(255, 255, 255)".into(), opacity: 1.0, clip: None, within: vec![], decorative: false, screen: false,
        }
    }

    fn frame(n: u32, texts: Vec<ProbeText>) -> Probe {
        Probe { frame: n, width: 1080.0, height: 1920.0, texts, ..Default::default() }
    }

    fn check(probes: &[Probe]) -> Vec<Issue> {
        let drawn: Vec<Drawn> = probes.iter().map(|p| Drawn { probe: p, picture: None, scale: 1.0, console: &[], moving: &[] }).collect();
        issues(&drawn, 30.0)
    }

    fn kinds(found: &[Issue]) -> Vec<&str> {
        found.iter().map(|i| i.kind.as_str()).collect()
    }

    #[test]
    fn a_well_placed_title_has_no_issues() {
        assert!(check(&[frame(0, vec![text("title", 200.0, 800.0, 680.0, 140.0)])]).is_empty());
    }

    #[test]
    fn text_past_its_clip_is_clipped_and_one_issue_gathers_its_frames() {
        let mut t = text("card", 100.0, 900.0, 600.0, 80.0);
        t.clip = Some([100.0, 900.0, 400.0, 80.0]);
        let found = check(&[frame(10, vec![t.clone()]), frame(20, vec![t])]);
        assert_eq!(kinds(&found), ["clipped"]);
        assert_eq!(found[0].frames, [10, 20]);
        assert!(found[0].detail.contains("right"), "{}", found[0].detail);
        assert!(found[0].detail.contains("from 0.33s · f10, 2 frames"), "{}", found[0].detail);
        // Hidden whole (a reveal not yet begun) is not clipped.
        let mut hidden = text("card", 100.0, 1000.0, 300.0, 80.0);
        hidden.clip = Some([100.0, 900.0, 400.0, 80.0]);
        assert!(check(&[frame(0, vec![hidden])]).is_empty());
    }

    #[test]
    fn the_frame_edge_and_the_safe_area_are_told_apart() {
        let found = check(&[frame(0, vec![text("off", -40.0, 800.0, 300.0, 80.0)])]);
        assert_eq!(kinds(&found), ["offscreen"]);
        // Inside the frame but under the platform's buttons on the right (12% of 1080).
        let found = check(&[frame(0, vec![text("edge", 700.0, 800.0, 300.0, 80.0)])]);
        assert_eq!(kinds(&found), ["unsafe"]);
        assert!(found[0].detail.contains("right"));
        // Wholly off the frame: not drawn, nothing to say.
        assert!(check(&[frame(0, vec![text("gone", -500.0, 800.0, 300.0, 80.0)])]).is_empty());
    }

    #[test]
    fn overlapping_texts_are_named_but_nested_ones_are_not() {
        let found = check(&[frame(0, vec![text("a", 200.0, 800.0, 400.0, 100.0), text("b", 250.0, 820.0, 400.0, 100.0)])]);
        assert_eq!(kinds(&found), ["text_overlap"]);
        let mut inner = text("bold", 250.0, 820.0, 100.0, 60.0);
        inner.within = vec![0];
        assert!(check(&[frame(0, vec![text("sentence", 200.0, 800.0, 400.0, 100.0), inner])]).is_empty());
        // A crossfade draws the scene leaving under the one coming: not an overlap.
        let mut leaving = text("a", 200.0, 800.0, 400.0, 100.0);
        leaving.opacity = 0.3;
        assert!(check(&[frame(0, vec![leaving, text("b", 250.0, 820.0, 400.0, 100.0)])]).is_empty());
    }

    #[test]
    fn an_apps_screen_and_decorations_are_not_held_to_the_videos_type() {
        let mut clock = text("clock", 200.0, 800.0, 40.0, 20.0);
        clock.size = 15.0;
        clock.decorative = true;
        let mut row = text("row", 200.0, 900.0, 300.0, 20.0);
        row.size = 16.0;
        row.screen = true;
        assert!(check(&[frame(0, vec![clock, row])]).is_empty());
        // Laid out at the top of the safe area, its glyphs rising a little above it.
        assert!(check(&[frame(0, vec![text("title", 200.0, 192.0 - 10.0, 600.0, 90.0)])]).is_empty());
    }

    #[test]
    fn small_text_and_missing_fonts_are_flagged() {
        let mut small = text("legal", 200.0, 800.0, 400.0, 20.0);
        small.size = 18.0;
        let mut fallback = text("brand", 200.0, 1000.0, 400.0, 80.0);
        fallback.family = "Brand Sans".into();
        fallback.resolved = false;
        let found = check(&[frame(0, vec![small, fallback])]);
        assert_eq!(kinds(&found), ["small_text", "font_fallback"]);
        assert!(found[1].detail.contains("Brand Sans"));
    }

    #[test]
    fn contrast_is_measured_on_the_picture() {
        let picture = Rgba::new(540, 960, [40, 40, 40, 255]);
        let mut dim = text("dim", 200.0, 800.0, 400.0, 100.0);
        dim.color = "rgb(70, 70, 70)".into();
        let p = frame(0, vec![dim]);
        let found = issues(&[Drawn { probe: &p, picture: Some(&picture), scale: 0.5, console: &[], moving: &[] }], 30.0);
        assert_eq!(kinds(&found), ["contrast"]);
        assert!(found[0].detail.contains(":1 against"), "{}", found[0].detail);
    }

    #[test]
    fn contrast_is_said_to_the_hundredth_and_judged_as_said() {
        let picture = Rgba::new(540, 960, [255, 255, 255, 255]);
        let judged = |grey: u8| {
            let mut words = text("caption", 200.0, 800.0, 400.0, 60.0);
            words.color = format!("rgb({grey}, {grey}, {grey})");
            words.size = 30.0;
            let p = frame(0, vec![words]);
            issues(&[Drawn { probe: &p, picture: Some(&picture), scale: 0.5, console: &[], moving: &[] }], 30.0)
        };
        // #777 on white is 4.48:1: under, and said so, not "4.5:1 … 4.5:1 is the least".
        let under = judged(119);
        assert_eq!(kinds(&under), ["contrast"]);
        assert!(under[0].detail.starts_with("4.48:1"), "{}", under[0].detail);
        // #767676 is 4.54:1: over the line, so nothing.
        assert!(judged(118).is_empty(), "{:?}", judged(118));
    }

    #[test]
    fn dimmed_text_is_judged_as_drawn() {
        // Grey words on a white card, the card at 0.68: about 3:1, too faint for small text.
        let picture = Rgba::new(540, 960, [255, 255, 255, 255]);
        let mut dim = text("this-month", 200.0, 800.0, 160.0, 30.0);
        dim.color = "rgb(85, 98, 92)".into();
        dim.size = 26.0;
        dim.opacity = 0.68;
        let p = frame(0, vec![dim]);
        let found = issues(&[Drawn { probe: &p, picture: Some(&picture), scale: 0.5, console: &[], moving: &[] }], 30.0);
        assert!(found.iter().any(|i| i.kind == "contrast"), "{found:?}");
    }

    #[test]
    fn console_errors_and_broken_images_are_issues() {
        let mut p = frame(3, vec![]);
        p.images.push(ProbeImage { id: Some("logo".into()), src: "http://localhost/static/logo.png".into() });
        let console = ["TypeError: x is undefined".to_string()];
        let found = issues(&[Drawn { probe: &p, picture: None, scale: 1.0, console: &console, moving: &[] }], 30.0);
        assert_eq!(kinds(&found), ["broken_image", "console"]);
    }

    #[test]
    fn a_scan_judges_words_where_they_settle_not_on_their_way() {
        // A title sliding in from below the frame, then resting inside it.
        let slide = |f: u32, y: f64| frame(f, vec![text("title", 200.0, y, 600.0, 120.0)]);
        let probes = vec![slide(0, 1900.0), slide(3, 1500.0), slide(6, 900.0), slide(9, 900.0), slide(12, 900.0)];
        assert!(scan(&probes, &[], 30.0, 3).is_empty(), "{:?}", scan(&probes, &[], 30.0, 3));
        // Resting past the edge is a fault.
        let probes = vec![slide(0, 1900.0), slide(3, 1850.0), slide(6, 1850.0)];
        assert_eq!(kinds(&scan(&probes, &[], 30.0, 3)), ["offscreen"]);
    }

    #[test]
    fn words_shown_too_briefly_are_found_but_a_counting_number_is_one_run() {
        let line = |f: u32, words: &str| {
            let mut t = text("line", 200.0, 900.0, 600.0, 80.0);
            t.text = words.into();
            frame(f, vec![t])
        };
        // Six words for 0.3 s (frames 30–36 at 30 fps, measured every 3), then gone.
        let mut probes: Vec<Probe> = (0..30).step_by(3).map(|f| frame(f, vec![])).collect();
        probes.extend([line(30, "Every euro finds its place here"), line(33, "Every euro finds its place here"), line(36, "Every euro finds its place here")]);
        probes.extend((39..90).step_by(3).map(|f| frame(f, vec![])));
        let found = brief(&probes, 30.0, 3, 87);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].detail.contains("6 words want at least 2.2 s"), "{}", found[0].detail);
        // A frame whose measurement went unheard does not cut a run in two.
        let mut gappy = probes.clone();
        gappy.retain(|p| p.frame != 33);
        assert_eq!(brief(&gappy, 30.0, 3, 87).len(), 1);
        // A number counting up keeps its run whatever its digits.
        let probes: Vec<Probe> = (0..90).step_by(3).map(|f| line(f, &format!("${} spent", f * 37))).collect();
        assert!(brief(&probes, 30.0, 3, 87).is_empty());
        // Still on screen at the end: not cut short.
        let probes = vec![frame(0, vec![]), line(84, "Get it today friends"), line(87, "Get it today friends")];
        assert!(brief(&probes, 30.0, 3, 87).is_empty());
    }

    #[test]
    fn the_safe_area_matches_the_kits() {
        assert_eq!(safe_area(1080.0, 1920.0).map(f64::round), [65.0, 192.0, 950.0, 1536.0]);
        assert_eq!(safe_area(1920.0, 1080.0).map(f64::round), [96.0, 54.0, 1824.0, 1026.0]);
    }
}
