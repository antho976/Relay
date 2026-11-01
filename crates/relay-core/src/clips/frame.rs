//! Where things are in the picture. Once a VOD: what the stream's picture is made of (a facecam
//! over gameplay, one camera, or a screen alone), which Claude reads from a few frames with a
//! grid drawn on them. For each clip of a one-camera stream: where the person is, from where the
//! picture moves (a person moves; a kitchen does not), so the 9:16 crop holds them.

use super::{Halt, Step};
use crate::engine::Engine;
use relay_bus::ops::clips::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// The size frames are looked at in.
const LOOK_W: f64 = 960.0;
const LOOK_H: f64 = 540.0;

const PROMPT: &str = "You measure the layout of livestream video frames precisely. Use the Read tool to look at each image you are given, then answer.";

fn rect_schema() -> Value {
    json!({"type": "object", "properties": {"x": {"type": "number"}, "y": {"type": "number"}, "w": {"type": "number"}, "h": {"type": "number"}}, "required": ["x", "y", "w", "h"]})
}

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "kind": {"type": "string", "enum": ["facecam", "camera", "screen"]},
            "cam": rect_schema(), "game": rect_schema(), "subject": rect_schema(),
            "note": {"type": "string"}
        },
        "required": ["kind", "note"]
    })
}

const ASK: &str = "Read f1.jpg, f2.jpg and f3.jpg in this folder: three frames of one livestream VOD, each 960x540 px, with thin grid lines every 48 px and yellow lines every 240 px across and 270 px down. \
Say what the picture is made of: 'facecam' (a camera box over gameplay or a screen), 'camera' (one camera fills the frame: cooking, IRL, just chatting), or 'screen' (gameplay or a screen with no camera). \
For facecam give cam: the camera box's exact edges (x, y, w, h in these 960x540 pixels, tight to the box), and game: the game window's area without the cam, chat overlays, title bars or empty borders. \
For camera give subject: a box around where the person mostly is. For screen give game. Ignore chat text overlays and small widgets.";

/// Grab a frame of `media` at `at` seconds, `width` wide, with ffmpeg filters `extra` after the
/// scale.
pub fn grab(media: &Path, at: f64, out: &Path, width: u32, extra: &str) -> Result<(), String> {
    let vf = format!("scale={width}:-2{extra}");
    let o = crate::proc::output_with_timeout(
        Command::new(super::ffmpeg()).args(["-hide_banner", "-loglevel", "error", "-y", "-ss", &format!("{at:.2}"), "-i"]).arg(media)
            .args(["-frames:v", "1", "-vf", &vf, "-q:v", "3"]).arg(out),
        Duration::from_secs(60),
    ).map_err(|e| format!("ffmpeg: {e}"))?.ok_or("ffmpeg took too long to grab a frame")?;
    if !o.status.success() || !out.is_file() {
        return Err(format!("ffmpeg could not grab a frame at {at:.0} s: {}", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("")));
    }
    Ok(())
}

fn scaled(v: &Value, sx: f64, sy: f64, w: f64, h: f64) -> Option<Rect> {
    let (x, y, rw, rh) = (v["x"].as_f64()?, v["y"].as_f64()?, v["w"].as_f64()?, v["h"].as_f64()?);
    if rw < 8.0 || rh < 8.0 {
        return None;
    }
    let mut r = Rect { x: (x * sx).round(), y: (y * sy).round(), w: (rw * sx).round(), h: (rh * sy).round() };
    r.x = r.x.clamp(0.0, w - 16.0);
    r.y = r.y.clamp(0.0, h - 16.0);
    r.w = r.w.min(w - r.x);
    r.h = r.h.min(h - r.y);
    Some(r)
}

/// The layout Claude's answer gives, in the VOD's pixels, checked: a facecam with no sensible box
/// is taken as one camera.
pub fn read_layout(answer: &Value, width: u32, height: u32, frame: Option<String>) -> SourceLayout {
    let (w, h) = (width.max(16) as f64, height.max(16) as f64);
    let (sx, sy) = (w / LOOK_W, h / LOOK_H);
    let mut kind = answer["kind"].as_str().unwrap_or("camera").to_string();
    let cam = scaled(&answer["cam"], sx, sy, w, h).filter(|r| r.w >= w * 0.06 && r.w <= w * 0.7 && r.h >= h * 0.06);
    let game = scaled(&answer["game"], sx, sy, w, h).filter(|r| r.w >= w * 0.3 && r.h >= h * 0.3);
    let subject = scaled(&answer["subject"], sx, sy, w, h);
    if kind == "facecam" && cam.is_none() {
        kind = if game.is_some() { "screen".into() } else { "camera".into() };
    }
    if !["facecam", "camera", "screen"].contains(&kind.as_str()) {
        kind = "camera".into();
    }
    SourceLayout {
        cam: cam.filter(|_| kind == "facecam"),
        game: if kind == "camera" { None } else { game.or(Some(Rect { x: 0.0, y: 0.0, w, h })) },
        subject: if kind == "camera" { subject.or(Some(Rect { x: w * 0.25, y: 0.0, w: w * 0.5, h })) } else { None },
        kind,
        by: "claude".into(),
        frame,
    }
}

/// The layout when nothing better is known: one camera, the person in the middle.
pub fn fallback(width: u32, height: u32, frame: Option<String>) -> SourceLayout {
    let (w, h) = (width.max(16) as f64, height.max(16) as f64);
    SourceLayout { kind: "camera".into(), cam: None, game: None, subject: Some(Rect { x: w * 0.25, y: 0.0, w: w * 0.5, h }), by: "default".into(), frame }
}

/// Find what VOD `vod`'s picture is made of, from three frames of `media` (a quarter, half and
/// three quarters in). A clean frame of the middle is kept as `layout/frame.jpg`.
pub fn detect(engine: &Engine, vod: &Vod, media: &Path, dir: &Path) -> Step<SourceLayout> {
    let look = dir.join("layout");
    std::fs::create_dir_all(&look).map_err(|e| Halt::Failed(format!("{}: {e}", look.display())))?;
    let d = vod.duration_s.max(1.0);
    let grid = ",drawgrid=w=48:h=54:t=1:c=white@0.25,drawgrid=w=240:h=270:t=2:c=yellow@0.7";
    for (i, f) in [0.25, 0.5, 0.75].iter().enumerate() {
        grab(media, d * f, &look.join(format!("f{}.jpg", i + 1)), LOOK_W as u32, grid)?;
    }
    let frame = look.join("frame.jpg");
    grab(media, d * 0.5, &frame, 1280, "")?;
    let frame_s = Some(frame.display().to_string());
    let claude = match super::claude(engine) {
        Ok(c) => c,
        Err(_) => return Ok(fallback(vod.width, vod.height, frame_s)),
    };
    let mut cmd = Command::new(claude);
    cmd.args(["-p", ASK, "--output-format", "json", "--model", "sonnet", "--system-prompt", PROMPT, "--tools", "Read", "--allowedTools", "Read",
        "--strict-mcp-config", "--setting-sources", "", "--no-session-persistence", "--json-schema", &schema().to_string()])
        .current_dir(&look);
    let answer = match super::run(engine, vod.id, &mut cmd, Duration::from_secs(4 * 60), |_| {}) {
        Ok(out) => super::last_json(&out),
        Err(Halt::Stopped) => return Err(Halt::Stopped),
        Err(Halt::Failed(why)) => {
            tracing::warn!(vod = vod.id, %why, "clips: the layout could not be read; one camera assumed");
            None
        }
    };
    let layout = answer.as_ref().and_then(|a| a.get("structured_output")).filter(|v| v.is_object())
        .map(|a| read_layout(a, vod.width, vod.height, frame_s.clone()))
        .unwrap_or_else(|| fallback(vod.width, vod.height, frame_s));
    let _ = std::fs::write(look.join("layout.json"), serde_json::to_vec_pretty(&layout).unwrap_or_default());
    Ok(layout)
}

// ---------------------------------------------------------------- each clip's scene

const SCENES_PROMPT: &str = "You classify livestream video frames. Use the Read tool to look at each image you are given, then answer.";

fn scenes_schema() -> Value {
    json!({
        "type": "object",
        "properties": {"scenes": {"type": "array", "items": {"type": "object", "properties": {
            "file": {"type": "string"},
            "kind": {"type": "string", "enum": ["facecam", "camera", "screen"]},
            "streamer": {"type": "array", "items": {"type": "number"}, "description": "for camera: the horizontal centre of the streamer in the left frame and in the right frame, each 0 (its left edge) to 1 (its right edge), -1 when they are not in it"}
        }, "required": ["file", "kind"]}}},
        "required": ["scenes"]
    })
}

/// A clip's scene as read: what its picture is made of, and for one camera where the streamer
/// stands (0 to 1 across the frame), when they are in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub kind: String,
    pub streamer: Option<f64>,
    /// Claude looked for the streamer and they are in neither frame: off camera for the moment.
    pub absent: bool,
}

/// What each clip's picture is made of: a stream switches scenes (a game with a facecam box, then
/// just chatting with the camera filling the frame), so the VOD's layout holds only for the clips
/// in its scene. `items` are (clip, its footage, a moment of it and another, in seconds of the
/// footage); Claude reads two frames of each, all in one look. A clip it could not read is left out.
pub fn scenes(engine: &Engine, vod: &Vod, dir: &Path, items: &[(i64, PathBuf, f64, f64)]) -> Step<std::collections::HashMap<i64, Scene>> {
    let mut out = std::collections::HashMap::new();
    if items.is_empty() {
        return Ok(out);
    }
    let look = dir.join("scenes");
    std::fs::create_dir_all(&look).map_err(|e| Halt::Failed(format!("{}: {e}", look.display())))?;
    let mut files = Vec::new();
    for (id, footage, a, b) in items {
        let (fa, fb) = (look.join(format!(".{id}-a.jpg")), look.join(format!(".{id}-b.jpg")));
        if grab(footage, *a, &fa, 480, "").is_err() || grab(footage, *b, &fb, 480, "").is_err() {
            continue;
        }
        let name = format!("scene-{id}.jpg");
        let joined = crate::proc::output_with_timeout(
            Command::new(super::ffmpeg()).args(["-hide_banner", "-loglevel", "error", "-y", "-i"]).arg(&fa).arg("-i").arg(&fb)
                .args(["-filter_complex", "[0][1]hstack", "-q:v", "4"]).arg(look.join(&name)),
            Duration::from_secs(30),
        );
        let _ = (std::fs::remove_file(&fa), std::fs::remove_file(&fb));
        if joined.ok().flatten().is_some_and(|o| o.status.success()) {
            files.push((*id, name));
        }
    }
    if files.is_empty() {
        return Ok(out);
    }
    let Ok(claude) = super::claude(engine) else { return Ok(out) };
    let ask = format!(
        "Read {}: each image is two frames, side by side, of one clip of a livestream VOD. For each image say what the picture is made of: \
'facecam' (gameplay or a screen with a small camera box over it), 'camera' (a camera fills the frame: just chatting, cooking, IRL; chat text or small widgets over it do not count as a screen), \
or 'screen' (gameplay or a screen with no camera). For 'camera', also give streamer: where the streamer (the person the stream is about, \
usually the one talking to the camera) stands across each of the two frames, 0 at that frame's left edge to 1 at its right, -1 when they are not in it. \
Answer one entry per file, with its file name.",
        files.iter().map(|(_, n)| n.as_str()).collect::<Vec<_>>().join(", ")
    );
    let mut cmd = Command::new(claude);
    cmd.args(["-p", &ask, "--output-format", "json", "--model", "sonnet", "--system-prompt", SCENES_PROMPT, "--tools", "Read", "--allowedTools", "Read",
        "--strict-mcp-config", "--setting-sources", "", "--no-session-persistence", "--json-schema", &scenes_schema().to_string()])
        .current_dir(&look);
    let answer = match super::run(engine, vod.id, &mut cmd, Duration::from_secs(6 * 60), |_| {}) {
        Ok(text) => super::last_json(&text),
        Err(Halt::Stopped) => return Err(Halt::Stopped),
        Err(Halt::Failed(why)) => {
            tracing::warn!(vod = vod.id, %why, "clips: the clips' scenes could not be read; the VOD's layout is used");
            None
        }
    };
    let list = answer.as_ref().and_then(|a| a["structured_output"]["scenes"].as_array().cloned()).unwrap_or_default();
    for s in list {
        let (Some(file), Some(kind)) = (s["file"].as_str(), s["kind"].as_str()) else { continue };
        if let Some((id, _)) = files.iter().find(|(_, n)| n == file) {
            if ["facecam", "camera", "screen"].contains(&kind) {
                let said: Vec<f64> = s["streamer"].as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
                let seen: Vec<f64> = said.iter().copied().filter(|x| (0.0..=1.0).contains(x)).collect();
                let streamer = (!seen.is_empty()).then(|| seen.iter().sum::<f64>() / seen.len() as f64);
                out.insert(*id, Scene { kind: kind.to_string(), streamer, absent: !said.is_empty() && seen.is_empty() });
            }
        }
    }
    Ok(out)
}

/// A clip's scene as a layout: the VOD's own when the kinds match, else one of `kind` with no
/// boxes (the clip's framing follows the person, or shows the whole screen).
pub fn scene_layout(vod_layout: Option<&SourceLayout>, kind: &str, width: u32, height: u32) -> Option<SourceLayout> {
    if vod_layout.is_some_and(|l| l.kind == kind) {
        return None;
    }
    let whole = Rect { x: 0.0, y: 0.0, w: width.max(16) as f64, h: height.max(16) as f64 };
    Some(SourceLayout {
        kind: kind.to_string(),
        cam: None,
        game: (kind == "screen").then_some(whole),
        subject: None,
        by: "claude".into(),
        frame: None,
    })
}

/// The framing that suits a scene, from the one a clip was given: a facecam's framings need a
/// facecam, a crop that follows the person needs one camera.
pub fn suited(layout: Layout, scene: &str) -> Layout {
    match (scene, layout) {
        ("camera", Layout::Split | Layout::Cam | Layout::Game) => Layout::Crop,
        ("screen", Layout::Split | Layout::Cam | Layout::Crop) => Layout::Fit,
        ("facecam", Layout::Crop) => Layout::Split,
        (_, l) => l,
    }
}

// ---------------------------------------------------------------- following the person

/// How much the picture moves in each column, summed over `[a, b]` seconds of `media`: frames at
/// 4 a second, 192 × 108 and grey, the bottom quarter (chat boxes, music widgets) left out.
pub fn motion(media: &Path, a: f64, b: f64) -> Result<Vec<f64>, String> {
    const W: usize = 192;
    const H: usize = 108;
    let o = crate::proc::output_with_timeout(
        Command::new(super::ffmpeg()).args(["-hide_banner", "-loglevel", "error", "-ss", &format!("{a:.2}"), "-t", &format!("{:.2}", (b - a).max(0.5)), "-i"])
            .arg(media).args(["-vf", &format!("fps=4,scale={W}:{H},format=gray"), "-f", "rawvideo", "-"]),
        Duration::from_secs(120),
    ).map_err(|e| format!("ffmpeg: {e}"))?.ok_or("ffmpeg took too long to read the motion")?;
    let frames: Vec<&[u8; W * H]> = o.stdout.as_chunks::<{ W * H }>().0.iter().collect();
    let mut cols = vec![0.0; W];
    let rows = H * 3 / 4;
    for pair in frames.windows(2) {
        for y in 0..rows {
            for (x, col) in cols.iter_mut().enumerate() {
                let d = (pair[1][y * W + x] as i32 - pair[0][y * W + x] as i32).unsigned_abs();
                // Small changes are noise and compression; a moving person changes a lot.
                if d > 12 {
                    *col += d as f64;
                }
            }
        }
    }
    Ok(cols)
}

/// The centre of the window `share` of the width wide holding the most motion, as a fraction of
/// the width, and how much of all the motion it holds; `None` when nothing moved.
pub fn busiest(cols: &[f64], share: f64) -> Option<(f64, f64)> {
    let n = cols.len();
    let total: f64 = cols.iter().sum();
    if n == 0 || total <= 0.0 {
        return None;
    }
    let win = ((n as f64 * share).round() as usize).clamp(1, n);
    let mut sum: f64 = cols[..win].iter().sum();
    let (mut best, mut at) = (sum, 0);
    for i in win..n {
        sum += cols[i] - cols[i - win];
        if sum > best {
            best = sum;
            at = i + 1 - win;
        }
    }
    Some(((at as f64 + win as f64 / 2.0) / n as f64, best / total))
}

/// A 9:16 crop for a one-camera clip, and a crop of its own for a span where the person would
/// leave it: `(view, {span index: crop})`. Spans are `[a, b]` seconds of `section`.
pub fn follow(section: &Path, spans: &[[f64; 2]], width: u32, height: u32, subject: Option<Rect>, seen: Option<f64>) -> (Rect, Vec<(usize, Rect)>) {
    let share = (height as f64 * 9.0 / 16.0) / width.max(1) as f64;
    let prior = seen.unwrap_or_else(|| subject.map_or(0.5, |r| (r.x + r.w / 2.0) / width.max(1) as f64));
    let per: Vec<Option<Vec<f64>>> = spans.iter().map(|[a, b]| motion(section, *a, *b).ok()).collect();
    let mut all = vec![0.0; 192];
    for cols in per.iter().flatten() {
        for (t, c) in all.iter_mut().zip(cols) {
            *t += c;
        }
    }
    // Where the person mostly is: where Claude saw them, nudged by the motion; without that, the
    // motion, pulled a little to where they usually stand.
    let centre = match (seen, busiest(&all, share)) {
        (Some(s), Some((c, held))) if held >= 0.35 && (c - s).abs() < share * 0.5 => 0.75 * s + 0.25 * c,
        (Some(s), _) => s,
        (None, found) => found.map_or(prior, |(c, held)| if held < 0.25 { 0.6 * prior + 0.4 * c } else { 0.85 * c + 0.15 * prior }),
    };
    let view = super::edit::tall_crop(width, height, centre * width as f64);
    let mut reframes = Vec::new();
    for (i, cols) in per.iter().enumerate() {
        let Some(cols) = cols else { continue };
        let Some((c, held)) = busiest(cols, share) else { continue };
        let total: f64 = cols.iter().sum();
        // How much of this span's motion the clip's crop holds.
        let lo = (view.x / width as f64 * cols.len() as f64) as usize;
        let hi = (((view.x + view.w) / width as f64) * cols.len() as f64).ceil() as usize;
        let inside: f64 = cols[lo.min(cols.len())..hi.min(cols.len())].iter().sum();
        if (c - centre).abs() > 0.18 && inside / total.max(1.0) < 0.45 && held > 0.35 {
            reframes.push((i, super::edit::tall_crop(width, height, c * width as f64)));
        }
    }
    (view, reframes)
}

/// The 9:16 window of a layout's game, for `game`: the middle of it, as tall as it is.
pub fn game_crop(game: Rect) -> Rect {
    let w = (game.h * 9.0 / 16.0).min(game.w);
    Rect { x: (game.x + (game.w - w) / 2.0).round(), y: game.y, w: w.round(), h: game.h }
}

/// Where a VOD's frame is kept for its card: its layout's frame, else one grabbed now.
pub fn poster(media: &Path, dir: &Path, duration: f64) -> Option<PathBuf> {
    let out = dir.join("poster.jpg");
    if out.is_file() {
        return Some(out);
    }
    grab(media, (duration * 0.3).max(1.0), &out, 640, "").ok().map(|_| out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_layouts_are_scaled_and_checked() {
        let a = json!({"kind": "facecam", "cam": {"x": 0, "y": 402, "w": 244, "h": 138}, "game": {"x": 27, "y": 27, "w": 907, "h": 513}, "note": ""});
        let l = read_layout(&a, 1920, 1080, None);
        assert_eq!(l.kind, "facecam");
        assert_eq!(l.cam, Some(Rect { x: 0.0, y: 804.0, w: 488.0, h: 276.0 }));
        assert_eq!(l.game.unwrap().w, 1814.0);
        // A facecam with a box too small to be one: one camera.
        let b = json!({"kind": "facecam", "cam": {"x": 0, "y": 0, "w": 9, "h": 9}, "note": ""});
        assert_eq!(read_layout(&b, 1920, 1080, None).kind, "camera");
    }

    #[test]
    fn the_busiest_window_is_found() {
        let mut cols = vec![1.0; 100];
        for c in cols.iter_mut().take(80).skip(60) {
            *c = 50.0;
        }
        let (c, held) = busiest(&cols, 0.3).unwrap();
        assert!((c - 0.7).abs() < 0.06, "{c}");
        assert!(held > 0.8);
        assert!(busiest(&[0.0; 10], 0.3).is_none());
    }

    #[test]
    fn game_crops_are_tall_and_centred() {
        let g = game_crop(Rect { x: 54.0, y: 54.0, w: 1814.0, h: 1026.0 });
        assert!((g.w - 577.0).abs() < 1.0);
        assert!((g.x + g.w / 2.0 - (54.0 + 907.0)).abs() < 1.0);
    }
}
