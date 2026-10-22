//! ffmpeg and ffprobe, for what Relay reads of sound and footage itself: a render's loudness, a
//! music file's beats, a clip's frame rate and where its picture changes. The kit's own builds come
//! first — Remotion ships both in its compositor package, with their libraries beside them — and the
//! machine's when the kit has none. Every run goes through `proc::output_with_timeout`.

use crate::engine::Engine;
use relay_bus::error::BusError;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Longest a decode or probe may take: a long clip at a fraction of real time.
const WITHIN: Duration = Duration::from_secs(10 * 60);

/// An ffmpeg or ffprobe to run: the binary and, for Remotion's, the folder of its libraries.
#[derive(Debug, Clone)]
pub struct Tool {
    pub path: PathBuf,
    libs: Option<PathBuf>,
}

impl Tool {
    pub fn command(&self) -> Command {
        let mut cmd = Command::new(&self.path);
        if let Some(libs) = &self.libs {
            let key = if cfg!(target_os = "macos") { "DYLD_LIBRARY_PATH" } else { "LD_LIBRARY_PATH" };
            cmd.env(key, libs);
        }
        cmd.arg("-hide_banner").arg("-v").arg("error");
        cmd
    }
}

/// The compositor folder of the kit's Remotion (`@remotion/compositor-<platform>`), if set up.
fn compositor(videos: &Path) -> Option<PathBuf> {
    let scope = videos.join("node_modules/@remotion");
    let mut found: Vec<PathBuf> = std::fs::read_dir(&scope).ok()?.flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("compositor-"))
        .map(|e| e.path())
        .filter(|dir| dir.join(exe("ffmpeg")).is_file())
        .collect();
    found.sort();
    found.into_iter().next()
}

fn exe(name: &str) -> String {
    if cfg!(windows) { format!("{name}.exe") } else { name.to_string() }
}

/// `ffmpeg` or `ffprobe`: the kit's, else the machine's.
pub fn tool(engine: &Engine, name: &str) -> Result<Tool, BusError> {
    if let Some(dir) = compositor(&engine.store.videos_dir()) {
        return Ok(Tool { path: dir.join(exe(name)), libs: Some(dir) });
    }
    which::which(name).map(|path| Tool { path, libs: None }).map_err(|_| {
        BusError::unavailable("video.ffmpeg", format!("no {name}: the video kit brings one (video.kit.install), or install ffmpeg"))
    })
}

fn run(cmd: &mut Command, what: &str) -> Result<std::process::Output, BusError> {
    let out = crate::proc::output_with_timeout(cmd, WITHIN)
        .map_err(|e| BusError::unavailable("video.ffmpeg", format!("{what}: {e}")))?
        .ok_or_else(|| BusError::unavailable("video.ffmpeg", format!("{what} took over {} minutes", WITHIN.as_secs() / 60)))?;
    Ok(out)
}

fn stderr_tail(out: &std::process::Output) -> String {
    let text = String::from_utf8_lossy(&out.stderr);
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(3)..].join(" | ").chars().take(400).collect()
}

/// Sound as numbers: interleaved samples from -1 to 1.
#[derive(Debug, Clone, Default)]
pub struct Pcm {
    pub rate: u32,
    pub channels: usize,
    pub samples: Vec<f32>,
}

impl Pcm {
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1)
    }
    pub fn seconds(&self) -> f64 {
        self.frames() as f64 / self.rate.max(1) as f64
    }
}

/// The samples of a WAV ffmpeg wrote to a pipe: after its `data` chunk's header, to the end (a
/// pipe's header cannot say how long the data is).
fn wav_data(bytes: &[u8]) -> &[u8] {
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let size = u32::from_le_bytes([bytes[at + 4], bytes[at + 5], bytes[at + 6], bytes[at + 7]]) as usize;
        if &bytes[at..at + 4] == b"data" {
            return &bytes[at + 8..];
        }
        at += 8 + size + size % 2;
    }
    &[]
}

/// The first sound stream of `input` at `rate` Hz in `channels` channels (a mono file is spread
/// over both, as a player does); `None` when it has none. It comes over a pipe as a WAV: Remotion's
/// ffmpeg has no raw muxers.
pub fn decode(engine: &Engine, input: &Path, rate: u32, channels: usize) -> Result<Option<Pcm>, BusError> {
    let ffmpeg = tool(engine, "ffmpeg")?;
    let out = run(ffmpeg.command().arg("-nostdin").arg("-i").arg(input)
        .args(["-vn", "-sn", "-dn", "-ac", &channels.to_string(), "-ar", &rate.to_string(), "-c:a", "pcm_s16le", "-f", "wav", "pipe:1"]),
        "decoding the sound")?;
    if !out.status.success() {
        let why = stderr_tail(&out);
        if why.contains("does not contain any stream") || why.contains("Output file is empty") || why.contains("matches no streams") {
            return Ok(None);
        }
        return Err(BusError::invalid("video.media", format!("{} cannot be read as sound: {why}", input.display())));
    }
    let data = wav_data(&out.stdout);
    if data.is_empty() {
        return Ok(None);
    }
    let samples = data.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b) as f32 / 32768.0).collect();
    Ok(Some(Pcm { rate, channels, samples }))
}

/// Write `pcm` as a 32-bit float WAV, which ffmpeg (Remotion's too) reads.
pub fn write_wav(path: &Path, pcm: &Pcm) -> std::io::Result<()> {
    let data = (pcm.samples.len() * 4) as u32;
    let mut bytes = Vec::with_capacity(data as usize + 44);
    let channels = pcm.channels as u16;
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&3u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&pcm.rate.to_le_bytes());
    bytes.extend_from_slice(&(pcm.rate * 4 * channels as u32).to_le_bytes());
    bytes.extend_from_slice(&(4 * channels).to_le_bytes());
    bytes.extend_from_slice(&32u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data.to_le_bytes());
    for s in &pcm.samples {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, bytes)
}

/// Run ffmpeg with `args` (after `-nostdin -y`): Ok when it succeeded, else why.
pub fn ffmpeg<S: AsRef<std::ffi::OsStr>>(engine: &Engine, args: &[S], what: &str) -> Result<(), BusError> {
    let tool = tool(engine, "ffmpeg")?;
    let out = run(tool.command().arg("-nostdin").arg("-y").args(args.iter().map(AsRef::as_ref)), what)?;
    if out.status.success() {
        Ok(())
    } else {
        Err(BusError::unavailable("video.ffmpeg", format!("{what} failed: {}", stderr_tail(&out))))
    }
}

/// `source` written to `out` as H.264 at a constant `fps` (frames repeated or dropped to keep
/// time), its sound kept: a screen recording's frames come only when the screen changes.
pub fn constant_rate(engine: &Engine, source: &Path, out: &Path, fps: f64) -> Result<(), BusError> {
    let rate = format!("{fps}");
    ffmpeg(engine, &[std::ffi::OsStr::new("-i"), source.as_os_str(), "-fps_mode".as_ref(), "cfr".as_ref(), "-r".as_ref(), rate.as_ref(),
        "-c:v".as_ref(), "libx264".as_ref(), "-preset".as_ref(), "veryfast".as_ref(), "-crf".as_ref(), "16".as_ref(), "-pix_fmt".as_ref(), "yuv420p".as_ref(),
        "-c:a".as_ref(), "aac".as_ref(), "-b:a".as_ref(), "192k".as_ref(), "-movflags".as_ref(), "+faststart".as_ref(), out.as_os_str()],
        "writing the clip at a constant frame rate")
}

/// What ffprobe says of a file: its format and streams.
pub fn probe(engine: &Engine, input: &Path) -> Result<Value, BusError> {
    let ffprobe = tool(engine, "ffprobe")?;
    let out = run(ffprobe.command().args(["-print_format", "json", "-show_format", "-show_streams"]).arg(input), "probing")?;
    if !out.status.success() {
        return Err(BusError::invalid("video.media", format!("{} cannot be read: {}", input.display(), stderr_tail(&out))));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| BusError::internal(format!("ffprobe's answer: {e}")))
}

/// The presentation times, in seconds and in order, of the first video stream's frames.
pub fn frame_times(engine: &Engine, input: &Path) -> Result<Vec<f64>, BusError> {
    let ffprobe = tool(engine, "ffprobe")?;
    let out = run(ffprobe.command().args(["-select_streams", "v:0", "-show_entries", "packet=pts_time", "-of", "csv=p=0"]).arg(input), "reading frame times")?;
    if !out.status.success() {
        return Err(BusError::invalid("video.media", format!("{} has no frames to read: {}", input.display(), stderr_tail(&out))));
    }
    let mut times: Vec<f64> = String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| l.trim().trim_end_matches(',').parse().ok()).collect();
    times.sort_by(f64::total_cmp);
    Ok(times)
}

/// Every frame of the first video stream as it is presented (no frame added or dropped), shrunk to
/// `width` px across in grey: one byte a pixel, rows of `width`; and the frames' height.
pub fn grey_frames(engine: &Engine, input: &Path, width: u32) -> Result<(Vec<u8>, u32), BusError> {
    let info = probe(engine, input)?;
    let stream = video_stream(&info).ok_or_else(|| BusError::invalid("video.media", format!("{} has no picture", input.display())))?;
    let (w, h) = (stream["width"].as_u64().unwrap_or(0) as f64, stream["height"].as_u64().unwrap_or(0) as f64);
    if w < 1.0 || h < 1.0 {
        return Err(BusError::invalid("video.media", format!("{} has no picture size", input.display())));
    }
    let height = ((h / w * width as f64 / 2.0).round() as u32 * 2).max(2);
    let ffmpeg = tool(engine, "ffmpeg")?;
    let out = run(ffmpeg.command().arg("-nostdin").arg("-i").arg(input)
        .args(["-an", "-sn", "-fps_mode", "passthrough", "-vf", &format!("scale={width}:{height}"), "-pix_fmt", "gray", "-c:v", "rawvideo", "-f", "image2pipe", "pipe:1"]),
        "reading the picture")?;
    if !out.status.success() {
        return Err(BusError::invalid("video.media", format!("{} cannot be decoded: {}", input.display(), stderr_tail(&out))));
    }
    Ok((out.stdout, height))
}

/// The first video stream of a probe's answer.
pub fn video_stream(info: &Value) -> Option<&Value> {
    info["streams"].as_array()?.iter().find(|s| s["codec_type"] == "video" && s["disposition"]["attached_pic"] != 1)
}

/// The first sound stream of a probe's answer.
pub fn audio_stream(info: &Value) -> Option<&Value> {
    info["streams"].as_array()?.iter().find(|s| s["codec_type"] == "audio")
}

/// `30000/1001` as 29.97; `None` for `0/0`.
pub fn rate(text: &str) -> Option<f64> {
    let (n, d) = text.split_once('/').unwrap_or((text, "1"));
    let (n, d): (f64, f64) = (n.trim().parse().ok()?, d.trim().parse().ok()?);
    (n > 0.0 && d > 0.0).then_some(n / d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_read_as_ffprobe_writes_them() {
        assert_eq!(rate("30/1"), Some(30.0));
        assert!((rate("30000/1001").unwrap() - 29.97).abs() < 0.001);
        assert_eq!(rate("0/0"), None);
        assert_eq!(rate("25"), Some(25.0));
    }

    #[test]
    fn a_piped_wavs_samples_follow_its_data_chunk() {
        let mut bytes = b"RIFF\xff\xff\xff\xffWAVEfmt ".to_vec();
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&[1, 0, 2, 0, 0x80, 0xbb, 0, 0, 0, 0xee, 2, 0, 4, 0, 16, 0]);
        bytes.extend_from_slice(b"LIST");
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(b"abc\0");
        bytes.extend_from_slice(b"data\xff\xff\xff\xff");
        bytes.extend_from_slice(&[1, 0, 2, 0]);
        assert_eq!(wav_data(&bytes), [1, 0, 2, 0]);
        assert_eq!(wav_data(b"RIFF"), [] as [u8; 0]);
    }

    #[test]
    fn a_float_wav_has_its_header() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        write_wav(&path, &Pcm { rate: 48000, channels: 2, samples: vec![0.5, -0.5, 0.25, -0.25] }).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(u16::from_le_bytes([bytes[20], bytes[21]]), 3, "IEEE float");
        assert_eq!(u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]), 16);
        assert_eq!(f32::from_le_bytes([bytes[44], bytes[45], bytes[46], bytes[47]]), 0.5);
    }
}
