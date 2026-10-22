//! What a clip, a sound or a picture is (`video.asset.inspect`): its size and length, its frame
//! rate and whether its frames are evenly spaced, its sound's loudness, and the frames where its
//! picture changes — where a screen recording's taps, new screens and scrolls are, so a cut can be
//! put on them without stepping through stills.
//!
//! Motion is read from the clip shrunk to 96 px across in grey: a pixel changed when it moved more
//! than 10 levels from the frame before. An event starts when more than 0.2% of the picture
//! changes after three still frames, and lasts until three still frames again.

use super::assets;
use super::audio;
use super::media;
use crate::engine::Engine;
use relay_bus::error::BusError;
use relay_bus::ops::video::{AssetInfo, MotionEvent};
use std::path::Path;

/// Width the picture is read at.
const WIDTH: u32 = 96;
/// A pixel that moved more than this many levels changed.
const LEVELS: u8 = 10;
/// Share of the picture that makes a frame move.
const MOVING: f64 = 0.002;
/// Still frames that end an event, or must come before one.
const STILL_RUN: usize = 3;
/// Most events answered.
const MOST_EVENTS: usize = 300;

/// What changed between two grey frames: the share of pixels, and their bounding box in the
/// shrunk picture (x0, y0, x1, y1).
fn change(a: &[u8], b: &[u8], width: usize) -> (f64, [usize; 4]) {
    let mut n = 0usize;
    let mut bbox = [usize::MAX, usize::MAX, 0, 0];
    for (i, (pa, pb)) in a.iter().zip(b).enumerate() {
        if pa.abs_diff(*pb) > LEVELS {
            n += 1;
            let (x, y) = (i % width, i / width);
            bbox = [bbox[0].min(x), bbox[1].min(y), bbox[2].max(x), bbox[3].max(y)];
        }
    }
    (n as f64 / a.len().max(1) as f64, bbox)
}

/// The events in `frames` (each `width` × `height` grey), at `times` (seconds, one a frame), in a
/// picture `scale` times the shrunk one.
pub fn motion(frames: &[&[u8]], times: &[f64], width: usize, scale: f64) -> Vec<MotionEvent> {
    let mut out = Vec::new();
    let mut still = STILL_RUN;
    let mut open: Option<(usize, usize, f64, [usize; 4])> = None;
    let time = |i: usize| times.get(i).copied().unwrap_or(i as f64 / 30.0);
    let close = |(start, end, peak, bbox): (usize, usize, f64, [usize; 4]), out: &mut Vec<MotionEvent>| {
        let frames = end - start + 1;
        let seconds = time(end) - time(start);
        let kind = if peak >= 0.25 && frames <= 4 { "cut" } else if seconds >= 0.5 { "motion" } else { "change" };
        let px = |v: usize| (v as f64 * scale).round() as u32;
        out.push(MotionEvent {
            frame: start as u32,
            end_frame: end as u32,
            at_s: (time(start) * 1000.0).round() / 1000.0,
            end_s: (time(end) * 1000.0).round() / 1000.0,
            kind: kind.into(),
            size: (peak * 1000.0).round() / 1000.0,
            region: [px(bbox[0]), px(bbox[1]), px(bbox[2] + 1 - bbox[0]), px(bbox[3] + 1 - bbox[1])],
        });
    };
    for i in 1..frames.len() {
        let (share, bbox) = change(frames[i - 1], frames[i], width);
        if share > MOVING {
            match open.as_mut() {
                Some((_, end, peak, b)) => {
                    *end = i;
                    if share > *peak {
                        *peak = share;
                    }
                    *b = [b[0].min(bbox[0]), b[1].min(bbox[1]), b[2].max(bbox[2]), b[3].max(bbox[3])];
                }
                None if still >= STILL_RUN => open = Some((i, i, share, bbox)),
                None => {}
            }
            still = 0;
        } else {
            still += 1;
            if still >= STILL_RUN {
                if let Some(event) = open.take() {
                    close(event, &mut out);
                    if out.len() >= MOST_EVENTS {
                        return out;
                    }
                }
            }
        }
    }
    if let Some(event) = open.take() {
        close(event, &mut out);
    }
    out
}

/// Whether frames at `times` are evenly spaced: the shortest and longest gaps, ms, and whether they
/// differ by more than a quarter of the usual one (the last gap aside: a clip's tail is often short).
pub fn spacing(times: &[f64]) -> Option<([f64; 2], bool)> {
    let gaps: Vec<f64> = times.windows(2).map(|w| w[1] - w[0]).filter(|g| *g > 0.0).collect();
    let gaps = &gaps[..gaps.len().saturating_sub(1)];
    if gaps.len() < 2 {
        return None;
    }
    let mut sorted = gaps.to_vec();
    sorted.sort_by(f64::total_cmp);
    let (least, most, usual) = (sorted[0], sorted[sorted.len() - 1], sorted[sorted.len() / 2]);
    let ms = |s: f64| (s * 10000.0).round() / 10.0;
    Some(([ms(least), ms(most)], most - least > usual * 0.25))
}

/// Inspect the file at `path` (in the video at `dir`).
pub fn inspect(engine: &Engine, dir: &Path, path: &Path, look_for_motion: Option<bool>) -> Result<AssetInfo, BusError> {
    let asset = assets::describe(dir, path, None, false, false);
    let info = media::probe(engine, path)?;
    let video = media::video_stream(&info).filter(|_| asset.kind != "image");
    let sound = media::audio_stream(&info);
    let duration_s = info["format"]["duration"].as_str().and_then(|d| d.parse::<f64>().ok()).map(|d| (d * 1000.0).round() / 1000.0);
    let mut out = AssetInfo {
        width: media::video_stream(&info).and_then(|v| v["width"].as_u64()).map(|w| w as u32),
        height: media::video_stream(&info).and_then(|v| v["height"].as_u64()).map(|h| h as u32),
        duration_s: if asset.kind == "image" { None } else { duration_s },
        fps: None,
        frames: None,
        variable_rate: false,
        frame_gap_ms: None,
        video_codec: video.and_then(|v| v["codec_name"].as_str().map(str::to_string)),
        audio_codec: sound.and_then(|a| a["codec_name"].as_str().map(str::to_string)),
        audio: None,
        motion: Vec::new(),
        warnings: Vec::new(),
        asset,
    };
    if let Some(stream) = video {
        let times = media::frame_times(engine, path)?;
        out.frames = Some(times.len() as u32);
        out.fps = stream["avg_frame_rate"].as_str().and_then(media::rate).map(|f| (f * 1000.0).round() / 1000.0);
        if let Some((gaps, uneven)) = spacing(&times) {
            out.frame_gap_ms = Some(gaps);
            out.variable_rate = uneven;
            if uneven {
                out.warnings.push(format!(
                    "variable frame rate: frames {:.1} to {:.1} ms apart, so a frame number is not a moment. Add it with video.asset.add {{fps: 30}} for even frames",
                    gaps[0], gaps[1]));
            }
        }
        if look_for_motion.unwrap_or(true) {
            let (pixels, height) = media::grey_frames(engine, path, WIDTH)?;
            let size = (WIDTH * height) as usize;
            let frames: Vec<&[u8]> = pixels.chunks_exact(size).collect();
            let scale = out.width.map_or(1.0, |w| w as f64 / WIDTH as f64);
            out.motion = motion(&frames, &times, WIDTH as usize, scale);
            if out.motion.len() >= MOST_EVENTS {
                out.warnings.push(format!("the picture changes all the time: the first {MOST_EVENTS} changes are listed"));
            }
        }
    }
    if sound.is_some() {
        if let Some(pcm) = media::decode(engine, path, audio::RATE, 2)? {
            let measured = audio::measure(&pcm);
            let mut report = audio::report_of(&measured, pcm.channels);
            report.warnings = audio::warnings(&measured, None).into_iter().filter(|w| w.kind != "loud" && w.kind != "quiet").collect();
            out.audio = Some(report);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 96 × 64 grey screen: still, then a small button lights for 5 frames (a tap), then the
    /// whole screen changes at once (a new page), then a slow scroll for 20 frames.
    fn screen() -> Vec<Vec<u8>> {
        let (w, h) = (96, 64);
        let mut frames = Vec::new();
        let base = vec![40u8; w * h];
        for _ in 0..10 {
            frames.push(base.clone());
        }
        for _ in 0..5 {
            let mut f = base.clone();
            for y in 40..46 {
                for x in 20..30 {
                    f[y * w + x] = 200;
                }
            }
            frames.push(f);
        }
        for _ in 0..10 {
            frames.push(base.clone());
        }
        let page = vec![180u8; w * h];
        for _ in 0..10 {
            frames.push(page.clone());
        }
        for k in 0..20 {
            let mut f = page.clone();
            for y in 0..h {
                f[y * w + (k * 3) % w] = 20;
            }
            frames.push(f);
        }
        for _ in 0..6 {
            frames.push(frames.last().unwrap().clone());
        }
        frames
    }

    #[test]
    fn a_tap_a_new_screen_and_a_scroll_are_told_apart() {
        let frames = screen();
        let refs: Vec<&[u8]> = frames.iter().map(Vec::as_slice).collect();
        let times: Vec<f64> = (0..frames.len()).map(|i| i as f64 / 30.0).collect();
        let events = motion(&refs, &times, 96, 1080.0 / 96.0);
        let kinds: Vec<(&str, u32)> = events.iter().map(|e| (e.kind.as_str(), e.frame)).collect();
        assert_eq!(kinds, [("change", 10), ("change", 15), ("cut", 25), ("motion", 35)], "{events:?}");
        // Where the tap was, in the clip's own px.
        let tap = &events[0];
        assert_eq!(tap.region, [225, 450, 113, 68]);
        assert!(events[2].size > 0.9);
    }

    #[test]
    fn uneven_frames_are_a_variable_rate() {
        let even: Vec<f64> = (0..60).map(|i| i as f64 / 30.0).collect();
        assert_eq!(spacing(&even), Some(([33.3, 33.3], false)));
        // A phone's recording: frames only when something moved.
        let mut uneven = vec![0.0, 0.016, 0.033, 0.3, 0.316, 0.9, 0.916, 0.933, 1.4];
        uneven.push(1.41);
        let (gaps, variable) = spacing(&uneven).unwrap();
        assert!(variable && gaps[1] > 400.0, "{gaps:?}");
    }
}
