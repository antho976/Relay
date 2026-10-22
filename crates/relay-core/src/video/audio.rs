//! A render's sound, measured and brought to a loudness (docs/VIDEO.md, "Sound"). Everything here
//! works on decoded samples (`media::decode`), so it reads the file as a player would:
//!
//! - **Loudness** is ITU-R BS.1770-4 integrated loudness with EBU R128's gates (K-weighting, 400 ms
//!   blocks every 100 ms, -70 LUFS absolute and -10 LU relative gates), the loudness range of EBU
//!   Tech 3342 (3 s windows, -20 LU gate, the 10th to 95th percentile) and the loudest 3 s.
//! - **True peak** is BS.1770-4 Annex 2's: the signal 4× oversampled through its 48-tap filter.
//! - **Reaching a loudness** is one gain, with a look-ahead limiter holding the true peak under the
//!   ceiling: no compression, so the mix keeps its shape (ffmpeg's `loudnorm` falls back to dynamic
//!   processing whenever a plain gain would pass the peak, which is the pumping people dislike).

use super::media::{self, Pcm};
use crate::engine::Engine;
use relay_bus::error::BusError;
use relay_bus::ops::video::{AudioReport, AudioWarning, Format, VideoRender};
use std::collections::VecDeque;
use std::path::Path;

/// Rate everything is measured at.
pub const RATE: u32 = 48_000;
/// The loudness a render is judged against when none was asked for: the streaming platforms'.
const DEFAULT_LUFS: f64 = -14.0;
/// Below this the sound counts as silence, LUFS.
const ABSOLUTE_GATE: f64 = -70.0;
/// A sample this close to full scale is clipped.
const FULL_SCALE: f32 = 32_766.0 / 32_768.0;

/// One biquad section, transposed direct form II.
#[derive(Clone, Copy)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    z: [f64; 2],
}

impl Biquad {
    fn run(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

/// BS.1770's K-weighting at `rate`: its high shelf, then its high-pass (the "RLB" curve), from the
/// filters' own parameters, so 48 kHz gives the standard's printed coefficients.
fn k_weighting(rate: u32) -> [Biquad; 2] {
    let fs = rate as f64;
    let (f0, gain_db, q) = (1681.974450955533, 3.999843853973347, 0.7071752369554196);
    let k = (std::f64::consts::PI * f0 / fs).tan();
    let vh = 10f64.powf(gain_db / 20.0);
    let vb = vh.powf(0.4996667741545416);
    let a0 = 1.0 + k / q + k * k;
    let shelf = Biquad {
        b: [(vh + vb * k / q + k * k) / a0, 2.0 * (k * k - vh) / a0, (vh - vb * k / q + k * k) / a0],
        a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
        z: [0.0; 2],
    };
    let (f0, q) = (38.13547087602444, 0.5003270373238773);
    let k = (std::f64::consts::PI * f0 / fs).tan();
    let a0 = 1.0 + k / q + k * k;
    let high_pass = Biquad { b: [1.0, -2.0, 1.0], a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0], z: [0.0; 2] };
    [shelf, high_pass]
}

/// BS.1770-4 Annex 2: the 4× oversampling filter, one row of 12 taps per phase.
const TRUE_PEAK_TAPS: [[f64; 12]; 4] = [
    [0.0017089843750, 0.0109863281250, -0.0196533203125, 0.0332031250000, -0.0594482421875, 0.1373291015625,
        0.9721679687500, -0.1022949218750, 0.0476074218750, -0.0266113281250, 0.0148925781250, -0.0083007812500],
    [-0.0291748046875, 0.0292968750000, -0.0517578125000, 0.0891113281250, -0.1665039062500, 0.4650878906250,
        0.7797851562500, -0.2003173828125, 0.1015625000000, -0.0582275390625, 0.0330810546875, -0.0189208984375],
    [-0.0189208984375, 0.0330810546875, -0.0582275390625, 0.1015625000000, -0.2003173828125, 0.7797851562500,
        0.4650878906250, -0.1665039062500, 0.0891113281250, -0.0517578125000, 0.0292968750000, -0.0291748046875],
    [-0.0083007812500, 0.0148925781250, -0.0266113281250, 0.0476074218750, -0.1022949218750, 0.9721679687500,
        0.1373291015625, -0.0594482421875, 0.0332031250000, -0.0196533203125, 0.0109863281250, 0.0017089843750],
];

/// The highest of the four oversampled values the filter puts around frame `n` of channel `c`.
fn oversampled(pcm: &Pcm, c: usize, n: usize, gain: f32) -> f64 {
    let ch = pcm.channels;
    let frames = pcm.frames();
    let mut most = 0f64;
    for taps in &TRUE_PEAK_TAPS {
        let mut acc = 0f64;
        for (k, tap) in taps.iter().enumerate() {
            // Taps run back from n + 5 to n - 6, so the filter's centre sits on n.
            let at = n as isize + 5 - k as isize;
            if at >= 0 && (at as usize) < frames {
                acc += tap * (pcm.samples[at as usize * ch + c] * gain) as f64;
            }
        }
        most = most.max(acc.abs());
    }
    most
}

/// The true peak of `pcm` with `gain` applied, linear: the sample peak, or above it where the
/// oversampled signal rises between samples. Only frames within 6 dB of the sample peak can hold
/// it (the filter raises a peak by at most a few dB), so only those are oversampled.
fn true_peak(pcm: &Pcm, gain: f32) -> f64 {
    let sample = pcm.samples.iter().fold(0f32, |m, s| m.max((s * gain).abs())) as f64;
    let floor = (sample * 0.5) as f32;
    let mut most = sample;
    for n in 0..pcm.frames() {
        for c in 0..pcm.channels {
            if (pcm.samples[n * pcm.channels + c] * gain).abs() >= floor {
                most = most.max(oversampled(pcm, c, n, gain));
            }
        }
    }
    most
}

fn db(linear: f64) -> f64 {
    if linear <= 1e-10 { -200.0 } else { 20.0 * linear.log10() }
}

/// What BS.1770 and EBU R128 make of a signal.
#[derive(Debug, Clone, PartialEq)]
pub struct Loudness {
    pub integrated: Option<f64>,
    pub range: Option<f64>,
    pub short_term_max: Option<f64>,
    pub true_peak_db: f64,
    pub sample_peak_db: f64,
    pub clipped: u64,
    pub seconds: f64,
}

/// The K-weighted mean square of each 100 ms step, all channels summed (weights 1: mono and stereo).
fn steps(pcm: &Pcm) -> Vec<f64> {
    let step = (pcm.rate / 10).max(1) as usize;
    let ch = pcm.channels.max(1);
    let mut filters: Vec<[Biquad; 2]> = (0..ch).map(|_| k_weighting(pcm.rate)).collect();
    let mut out = Vec::with_capacity(pcm.frames() / step + 1);
    let (mut sum, mut n) = (0f64, 0usize);
    for frame in pcm.samples.chunks_exact(ch) {
        for (c, s) in frame.iter().enumerate() {
            let [shelf, high] = &mut filters[c];
            let y = high.run(shelf.run(*s as f64));
            sum += y * y;
        }
        n += 1;
        if n == step {
            out.push(sum / step as f64);
            sum = 0.0;
            n = 0;
        }
    }
    out
}

fn lufs(mean_square: f64) -> f64 {
    -0.691 + 10.0 * mean_square.max(1e-20).log10()
}

/// The loudness of windows `width` steps wide, one every step.
fn windows(steps: &[f64], width: usize) -> Vec<f64> {
    if steps.len() < width {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(steps.len() - width + 1);
    let mut sum: f64 = steps[..width].iter().sum();
    out.push(sum / width as f64);
    for i in width..steps.len() {
        sum += steps[i] - steps[i - width];
        out.push(sum.max(0.0) / width as f64);
    }
    out
}

/// Measure `pcm`.
pub fn measure(pcm: &Pcm) -> Loudness {
    let steps = steps(pcm);
    // Integrated: 400 ms blocks, the absolute gate, then 10 LU under what passed it.
    let blocks = windows(&steps, 4);
    let loud: Vec<f64> = blocks.iter().copied().filter(|z| lufs(*z) > ABSOLUTE_GATE).collect();
    let integrated = (!loud.is_empty()).then(|| {
        let relative = lufs(loud.iter().sum::<f64>() / loud.len() as f64) - 10.0;
        let gated: Vec<f64> = loud.iter().copied().filter(|z| lufs(*z) > relative).collect();
        lufs(gated.iter().sum::<f64>() / gated.len().max(1) as f64)
    });
    // Range: 3 s windows, gated absolutely and 20 LU under their mean.
    let short: Vec<f64> = windows(&steps, 30);
    let short_loud: Vec<f64> = short.iter().copied().filter(|z| lufs(*z) > ABSOLUTE_GATE).collect();
    let range = (!short_loud.is_empty()).then(|| {
        let relative = lufs(short_loud.iter().sum::<f64>() / short_loud.len() as f64) - 20.0;
        let mut kept: Vec<f64> = short_loud.iter().map(|z| lufs(*z)).filter(|l| *l > relative).collect();
        kept.sort_by(f64::total_cmp);
        let at = |p: f64| kept[((kept.len() - 1) as f64 * p).round() as usize];
        at(0.95) - at(0.10)
    });
    let short_term_max = short_loud.iter().copied().fold(None, |m: Option<f64>, z| Some(m.map_or(z, |m| m.max(z)))).map(lufs);
    let sample = pcm.samples.iter().fold(0f32, |m, s| m.max(s.abs()));
    Loudness {
        integrated,
        range,
        short_term_max,
        true_peak_db: db(true_peak(pcm, 1.0)),
        sample_peak_db: db(sample as f64),
        clipped: pcm.samples.iter().filter(|s| s.abs() >= FULL_SCALE).count() as u64,
        seconds: pcm.seconds(),
    }
}

/// `pcm` with `gain_db` applied and its true peak held under `ceiling_db`: a gain per frame, all
/// channels together, that comes down over the 5 ms before a peak (the look-ahead, so nothing
/// overshoots) and recovers over about 80 ms after it. The sound is not delayed.
pub fn gain_and_limit(pcm: &Pcm, gain_db: f64, ceiling_db: f64) -> Pcm {
    let gain = 10f64.powf(gain_db / 20.0) as f32;
    // A tenth of a dB under the ceiling: the gain's own movement adds a little between samples.
    let ceiling = 10f64.powf((ceiling_db - 0.1) / 20.0);
    let (ch, frames) = (pcm.channels.max(1), pcm.frames());
    let floor = (ceiling * 0.5) as f32;
    // The gain each frame could take at most to stay under the ceiling.
    let need: Vec<f32> = (0..frames).map(|n| {
        let loudest = (0..ch).map(|c| (pcm.samples[n * ch + c] * gain).abs()).fold(0f32, f32::max);
        if loudest < floor {
            return 1.0;
        }
        let peak = (0..ch).map(|c| oversampled(pcm, c, n, gain)).fold(loudest as f64, f64::max);
        if peak > ceiling { (ceiling / peak) as f32 } else { 1.0 }
    }).collect();
    let ahead = (pcm.rate as usize / 200).max(1);
    // The least of the next `ahead` frames' needs (a sliding minimum over a deque of indices).
    let mut held = vec![1f32; frames + ahead];
    let mut window: VecDeque<usize> = VecDeque::new();
    let value = |i: usize| need.get(i).copied().unwrap_or(1.0);
    for (i, slot) in held.iter_mut().enumerate() {
        while window.back().is_some_and(|&j| value(j) >= value(i)) {
            window.pop_back();
        }
        window.push_back(i);
        while window.front().is_some_and(|&j| j + ahead < i) {
            window.pop_front();
        }
        *slot = value(*window.front().unwrap_or(&i));
    }
    // Recover slowly: instant down, about 80 ms back up.
    let release = 1.0 - (-1.0 / (0.08 * pcm.rate as f64)).exp() as f32;
    let mut level = 1f32;
    for h in held.iter_mut() {
        level = if *h < level { *h } else { level + (*h - level) * release };
        *h = level;
    }
    // Smoothed over the look-ahead, so the gain moves without a step; frame n takes the average
    // over n+1 ..= n+ahead, each of which held n's need (see the docs above).
    let mut out = Vec::with_capacity(pcm.samples.len());
    let mut sum: f64 = held[1..=ahead.min(held.len() - 1)].iter().map(|v| *v as f64).sum();
    for n in 0..frames {
        let g = (sum / ahead as f64) as f32 * gain;
        for c in 0..ch {
            out.push(pcm.samples[n * ch + c] * g);
        }
        let (enter, leave) = (n + 1 + ahead, n + 1);
        if enter < held.len() {
            sum += held[enter] as f64;
        } else {
            sum += 1.0;
        }
        sum -= held[leave] as f64;
    }
    Pcm { rate: pcm.rate, channels: pcm.channels, samples: out }
}

/// `pcm` brought to `target` LUFS with its true peak under `ceiling_db`, and the gain that took:
/// where the limiter takes loud moments down, a gain worked out before it falls short, so it is
/// worked out again from what came out, until within a tenth of a LU (at most four times).
pub fn reach(pcm: &Pcm, before: f64, target: f64, ceiling_db: f64) -> (Pcm, f64) {
    let mut gain = target - before;
    let mut out = gain_and_limit(pcm, gain, ceiling_db);
    for _ in 0..3 {
        let Some(now) = measure(&out).integrated else { break };
        let short = target - now;
        // A gain the limiter swallows whole (a sound all peaks) is as far as it goes.
        if short.abs() < 0.1 || gain + short > 40.0 {
            break;
        }
        gain += short;
        out = gain_and_limit(pcm, gain, ceiling_db);
    }
    (out, gain)
}

/// What a measure says to the person, against the loudness asked for (or the platforms').
pub fn warnings(m: &Loudness, target: Option<f64>) -> Vec<AudioWarning> {
    let mut out = Vec::new();
    let warn = |kind: &str, detail: String| AudioWarning { kind: kind.into(), detail };
    let Some(lufs) = m.integrated else {
        out.push(warn("silent", "no sound above -70 LUFS: the render is silent".into()));
        return out;
    };
    if m.clipped >= 4 || m.true_peak_db > 0.0 {
        out.push(warn("clipping", format!(
            "{} samples at full scale and a true peak of {:+.1} dBTP: the mix clips. Lower the loudest sounds' volume, or render with loudness (a limiter then holds the peak)",
            m.clipped, m.true_peak_db)));
    } else if m.true_peak_db > -1.0 {
        out.push(warn("hot", format!("a true peak of {:+.1} dBTP: AAC and Opus encoding will clip it; keep it under -1 dBTP (render with peak: -1)", m.true_peak_db)));
    }
    let aim = target.unwrap_or(DEFAULT_LUFS);
    let slack = if target.is_some() { 1.0 } else { 4.0 };
    if lufs > aim + slack {
        out.push(warn("loud", format!("{lufs:.1} LUFS is {:.1} LU over {aim} LUFS{}", lufs - aim, if target.is_some() { "" } else { ", the level YouTube, Shorts, Reels and TikTok play at: they will turn it down" })));
    } else if lufs < aim - slack {
        out.push(warn("quiet", format!("{lufs:.1} LUFS is {:.1} LU under {aim} LUFS{}", aim - lufs, if target.is_some() { "" } else { "; render with loudness or a preset to bring it up" })));
    }
    out
}

/// A measure as the bus says it, a tenth of a dB at a time.
pub fn report_of(m: &Loudness, channels: usize) -> AudioReport {
    let round = |v: f64| (v * 10.0).round() / 10.0;
    AudioReport {
        lufs: m.integrated.map(round),
        lra: m.range.map(round),
        short_term_max: m.short_term_max.map(round),
        true_peak: round(m.true_peak_db.max(-120.0)),
        sample_peak: round(m.sample_peak_db.max(-120.0)),
        clipped: m.clipped,
        seconds: (m.seconds * 1000.0).round() / 1000.0,
        channels: channels as u32,
        gain_db: None,
        lufs_before: None,
        true_peak_before: None,
        warnings: Vec::new(),
    }
}

/// The audio codec, its options and bitrate for a format's sound when it is written again.
fn encoder(format: Format, codec: &str) -> &'static [&'static str] {
    match (format, codec) {
        (Format::Webm, _) => &["-c:a", "libopus", "-b:a", "192k"],
        (Format::Mp3, _) => &["-c:a", "libmp3lame", "-b:a", "320k"],
        (Format::Wav, _) => &["-c:a", "pcm_s16le"],
        (_, c) if c.starts_with("pcm") => &["-c:a", "pcm_s16le"],
        _ => &["-c:a", "aac", "-b:a", "320k"],
    }
}

/// Measure a finished render's sound and, when it was asked for, bring it to its loudness under
/// its peak (writing the file again with the picture copied as it is). `None` for a format or a
/// video with no sound. A measure that cannot be made leaves the render as it is; a loudness that
/// cannot be reached says so in the report's warnings.
pub fn finish(engine: &Engine, render: &VideoRender, path: &Path) -> Result<Option<AudioReport>, BusError> {
    if render.format == Format::Gif {
        return Ok(None);
    }
    let pcm = match media::decode(engine, path, RATE, 2) {
        Ok(Some(pcm)) => pcm,
        Ok(None) => return Ok(None),
        Err(e) => {
            tracing::debug!(render = render.id, error = %e.message, "a render's sound could not be measured");
            return Ok(None);
        }
    };
    let measured = measure(&pcm);
    let mut out = report_of(&measured, pcm.channels);
    if render.loudness.is_none() && render.peak.is_none_or(|p| measured.true_peak_db <= p) {
        out.warnings = warnings(&measured, None);
        return Ok(Some(out));
    }
    let ceiling = render.peak.unwrap_or(-1.0);
    let Some(before) = measured.integrated else {
        out.warnings = warnings(&measured, render.loudness);
        return Ok(Some(out));
    };
    // A peak alone holds the peaks and leaves the loudness.
    let target = render.loudness.unwrap_or(before);
    let (processed, gain) = if render.loudness.is_some() { reach(&pcm, before, target, ceiling) } else { (gain_and_limit(&pcm, 0.0, ceiling), 0.0) };
    match rewrite(engine, render.format, path, &processed) {
        Ok(()) => {}
        Err(e) => {
            out.warnings = warnings(&measured, render.loudness);
            out.warnings.push(AudioWarning { kind: "not_applied".into(), detail: format!("the loudness could not be applied: {}", e.message) });
            return Ok(Some(out));
        }
    }
    // Read back what was written, as a player will.
    let after = match media::decode(engine, path, RATE, 2) {
        Ok(Some(pcm)) => measure(&pcm),
        _ => measure(&processed),
    };
    let mut done = report_of(&after, pcm.channels);
    done.gain_db = Some((gain * 10.0).round() / 10.0);
    done.lufs_before = out.lufs;
    done.true_peak_before = Some(out.true_peak);
    done.warnings = warnings(&after, render.loudness);
    Ok(Some(done))
}

/// Put `pcm` in place of `path`'s sound: a WAV or an MP3 is written from it, anything else
/// through ffmpeg with its picture copied untouched.
fn rewrite(engine: &Engine, format: Format, path: &Path, pcm: &Pcm) -> Result<(), BusError> {
    use std::ffi::OsString;
    let dir = path.parent().unwrap_or(Path::new("."));
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let sound = dir.join(format!(".{name}.sound.wav"));
    media::write_wav(&sound, pcm).map_err(|e| BusError::internal(format!("{}: {e}", sound.display())))?;
    let written = dir.join(format!(".{name}.loud.{}", format.ext()));
    let codec = media::probe(engine, path).ok()
        .and_then(|info| media::audio_stream(&info).and_then(|s| s["codec_name"].as_str().map(str::to_string)))
        .unwrap_or_default();
    let mut args: Vec<OsString> = Vec::new();
    if matches!(format, Format::Wav | Format::Mp3) {
        args.extend([OsString::from("-i"), sound.clone().into()]);
    } else {
        args.extend([OsString::from("-i"), path.into(), "-i".into(), sound.clone().into(), "-map".into(), "0:v:0".into(),
            "-map".into(), "1:a:0".into(), "-c:v".into(), "copy".into()]);
    }
    args.extend(encoder(format, &codec).iter().map(OsString::from));
    if matches!(format, Format::Mp4 | Format::Prores) {
        args.extend([OsString::from("-movflags"), "+faststart".into()]);
    }
    args.push(written.clone().into());
    let result = media::ffmpeg(engine, &args, "writing the sound");
    let _ = std::fs::remove_file(&sound);
    match result {
        Ok(()) => std::fs::rename(&written, path).map_err(|e| BusError::internal(format!("{}: {e}", path.display()))),
        Err(e) => {
            let _ = std::fs::remove_file(&written);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f64, amplitude: f64, seconds: f64, channels: usize, phase: f64) -> Pcm {
        let frames = (seconds * RATE as f64) as usize;
        let mut samples = Vec::with_capacity(frames * channels);
        for n in 0..frames {
            let v = (amplitude * (2.0 * std::f64::consts::PI * freq * n as f64 / RATE as f64 + phase).sin()) as f32;
            for _ in 0..channels {
                samples.push(v);
            }
        }
        Pcm { rate: RATE, channels, samples }
    }

    #[test]
    fn k_weighting_at_48k_is_the_standards() {
        let [shelf, high] = k_weighting(48_000);
        let close = |a: f64, b: f64| (a - b).abs() < 1e-8;
        assert!(close(shelf.b[0], 1.53512485958697) && close(shelf.b[1], -2.69169618940638) && close(shelf.b[2], 1.19839281085285));
        assert!(close(shelf.a[0], -1.69065929318241) && close(shelf.a[1], 0.73248077421585));
        assert!(close(high.a[0], -1.99004745483398) && close(high.a[1], 0.99007225036621));
    }

    #[test]
    fn a_full_scale_1k_sine_is_three_under_in_mono_and_zero_in_stereo() {
        // BS.1770-4's own check: a 0 dBFS 997 Hz sine in one channel reads -3.01 LUFS.
        let mono = measure(&sine(997.0, 1.0, 5.0, 1, 0.0));
        assert!((mono.integrated.unwrap() + 3.01).abs() < 0.05, "{mono:?}");
        let stereo = measure(&sine(997.0, 1.0, 5.0, 2, 0.0));
        assert!((stereo.integrated.unwrap() - 0.0).abs() < 0.06, "{stereo:?}");
        // 20 dB down is 20 LU down.
        let quiet = measure(&sine(997.0, 0.1, 5.0, 2, 0.0));
        assert!((quiet.integrated.unwrap() + 20.0).abs() < 0.06, "{quiet:?}");
        assert_eq!(measure(&sine(997.0, 0.0, 2.0, 2, 0.0)).integrated, None, "silence has no loudness");
    }

    #[test]
    fn the_true_peak_is_found_between_samples() {
        // A quarter of the sample rate, 45° off: every sample lands at 0.707 of the crest.
        let between = sine(12_000.0, 0.5, 0.5, 1, std::f64::consts::FRAC_PI_4);
        let m = measure(&between);
        assert!((m.sample_peak_db - db(0.5 * std::f64::consts::FRAC_1_SQRT_2)).abs() < 0.1, "{m:?}");
        assert!((m.true_peak_db - db(0.5)).abs() < 0.6, "{m:?}");
        assert!(m.true_peak_db > m.sample_peak_db + 2.0, "{m:?}");
    }

    #[test]
    fn the_loudness_range_tells_a_steady_mix_from_one_that_changes() {
        let steady = measure(&sine(440.0, 0.3, 12.0, 2, 0.0));
        assert!(steady.range.unwrap() < 0.5, "{steady:?}");
        let mut changing = sine(440.0, 0.3, 12.0, 2, 0.0);
        let half = changing.samples.len() / 2;
        for s in &mut changing.samples[half..] {
            *s *= 0.1;
        }
        let m = measure(&changing);
        assert!(m.range.unwrap() > 15.0, "{m:?}");
    }

    #[test]
    fn clipping_is_counted_and_said() {
        let mut hot = sine(440.0, 1.4, 2.0, 2, 0.0);
        for s in &mut hot.samples {
            *s = s.clamp(-1.0, 1.0);
        }
        let m = measure(&hot);
        assert!(m.clipped > 1000, "{m:?}");
        let said = warnings(&m, None);
        assert_eq!(said[0].kind, "clipping", "{said:?}");
        let fine = measure(&sine(440.0, 0.2, 2.0, 2, 0.0));
        assert!(warnings(&fine, Some(fine.integrated.unwrap())).is_empty());
    }

    #[test]
    fn a_gain_reaches_the_loudness_and_the_limiter_holds_the_peak() {
        // Quiet speech-like bursts with one loud hit: the gain lifts it 12 dB, which would put
        // the hit far over the ceiling without the limiter.
        let mut pcm = sine(300.0, 0.05, 6.0, 2, 0.0);
        let hit = (2.0 * RATE as f64) as usize * 2;
        for s in &mut pcm.samples[hit..hit + 4800] {
            *s *= 12.0;
        }
        let before = measure(&pcm);
        let target = -16.0;
        // One gain worked out beforehand falls short: the limiter takes the hit down.
        let once = measure(&gain_and_limit(&pcm, target - before.integrated.unwrap(), -1.0));
        assert!(once.integrated.unwrap() < target - 1.0, "{once:?}");
        let (out, gain) = reach(&pcm, before.integrated.unwrap(), target, -1.0);
        let after = measure(&out);
        assert!(after.true_peak_db <= -1.0 + 0.05, "{after:?}");
        assert!((after.integrated.unwrap() - target).abs() < 0.15, "{after:?} from {before:?}");
        assert_eq!(out.frames(), pcm.frames(), "nothing is delayed or cut");
        // Where nothing was near the ceiling, the gain is exactly the one it settled on.
        // A frame away from the sine's zero crossings, well after the hit.
        let quiet_at = (4 * RATE as usize + 37) * 2;
        let ratio = out.samples[quiet_at + 1] / pcm.samples[quiet_at + 1];
        assert!((db(ratio as f64) - gain).abs() < 0.05, "{ratio} for {gain}");
    }
}
