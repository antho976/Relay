//! A music file's beat grid (`video.audio.beats`): its tempo, where its first beat and first
//! downbeat fall, the bars, and where its sections change — what an editor cuts on.
//!
//! 1. **Onsets**: the spectral flux of a short-time Fourier transform (1024-point frames every 256
//!    samples at 22.05 kHz, about 86 a second), log-compressed, summed over the rises only; and the
//!    same below 200 Hz, where kicks and bass land, for the downbeats.
//! 2. **Tempo**: the onset curve's autocorrelation between 50 and 200 BPM, weighted toward 120 BPM
//!    by an octave-wide prior (or toward the BPM given), so a beat is not read as half or double.
//! 3. **Beats**: Ellis's dynamic-programming tracker (2007) — each beat an onset that keeps the
//!    tempo — then a least-squares constant-tempo grid through them, which sets the tempo to a few
//!    thousandths of a BPM and the first beat to a few ms; the fit's residual says how well a fixed
//!    grid holds.
//! 4. **Downbeats**: of the bar's phases, the one whose beats carry the most low-band onset energy.
//! 5. **Sections**: each bar's loudness and low-band energy against the two bars before it.

use super::media::Pcm;

/// Rate the analysis runs at.
pub const RATE: u32 = 22_050;
const N: usize = 1024;
const HOP: usize = 256;
/// Most bars answered.
const MOST_BARS: usize = 600;
/// How far the onset curve leads an attack: the flux rises once the attack is about a hop from a
/// frame's centre. Measured on click tracks at 85 to 140 BPM: 11.1 to 11.5 ms.
const ONSET_LEAD_S: f64 = 0.0113;
/// A beat this close before the start is the beat at the start (what is left of the lead after
/// its correction).
const START_SLACK_S: f64 = 0.005;

/// A bar of the grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    /// From 1, at the first downbeat.
    pub n: u32,
    pub start_s: f64,
    /// Its loudness, RMS dBFS.
    pub loudness_db: f64,
    /// Its energy under 200 Hz against its whole, dB.
    pub low_db: f64,
}

/// Where the music changes.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub bar: u32,
    pub start_s: f64,
    /// `louder`, `quieter`, `bass_in` or `bass_out`.
    pub kind: &'static str,
    pub change_db: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    pub bpm: f64,
    pub beat_s: f64,
    pub first_beat_s: f64,
    pub first_downbeat_s: f64,
    pub beats_per_bar: u32,
    /// Beats the tracker found and the grid was fitted through.
    pub beats_found: u32,
    /// How far those beats sit from the fitted grid, RMS ms.
    pub fit_rms_ms: f64,
    /// 0 to 1: how clearly the beat stands out, and the downbeat from the bar's other beats.
    pub beat_confidence: f64,
    pub downbeat_confidence: f64,
    pub seconds: f64,
    pub bars: Vec<Bar>,
    pub sections: Vec<Section>,
}

/// In-place radix-2 FFT of `re` + i`im`, `N` long.
fn fft(re: &mut [f64], im: &mut [f64], cos: &[f64], sin: &[f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let step = n / len;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (wr, wi) = (cos[k * step], -sin[k * step]);
                let (a, b) = (start + k, start + k + len / 2);
                let (xr, xi) = (re[b] * wr - im[b] * wi, re[b] * wi + im[b] * wr);
                re[b] = re[a] - xr;
                im[b] = im[a] - xi;
                re[a] += xr;
                im[a] += xi;
            }
        }
        len <<= 1;
    }
}

/// The onset curves, one value a hop: broadband and under 200 Hz. Frame `t` is centred on sample
/// `t * HOP`.
fn onsets(samples: &[f32]) -> (Vec<f64>, Vec<f64>) {
    let cos: Vec<f64> = (0..N).map(|k| (2.0 * std::f64::consts::PI * k as f64 / N as f64).cos()).collect();
    let sin: Vec<f64> = (0..N).map(|k| (2.0 * std::f64::consts::PI * k as f64 / N as f64).sin()).collect();
    let window: Vec<f64> = (0..N).map(|k| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * k as f64 / N as f64).cos()).collect();
    let frames = samples.len() / HOP + 1;
    let bins = N / 2;
    let top = (8000.0 / (RATE as f64 / N as f64)) as usize;
    let low = (200.0 / (RATE as f64 / N as f64)).ceil() as usize;
    let mut previous = vec![0f64; bins];
    let (mut flux, mut low_flux) = (Vec::with_capacity(frames), Vec::with_capacity(frames));
    let (mut re, mut im) = (vec![0f64; N], vec![0f64; N]);
    for t in 0..frames {
        let centre = (t * HOP) as isize;
        for k in 0..N {
            let at = centre - (N / 2) as isize + k as isize;
            re[k] = if at >= 0 && (at as usize) < samples.len() { samples[at as usize] as f64 * window[k] } else { 0.0 };
            im[k] = 0.0;
        }
        fft(&mut re, &mut im, &cos, &sin);
        let (mut all, mut under) = (0f64, 0f64);
        for b in 1..bins.min(top) {
            let level = (1.0 + 100.0 * (re[b] * re[b] + im[b] * im[b]).sqrt()).ln();
            let rise = (level - previous[b]).max(0.0);
            all += rise;
            if b < low {
                under += rise;
            }
            previous[b] = level;
        }
        flux.push(if t == 0 { 0.0 } else { all });
        low_flux.push(if t == 0 { 0.0 } else { under });
    }
    (flux, low_flux)
}

/// The curve less its moving average over `half` frames each side, rises only, then scaled to
/// unit spread: what is left are the onsets that stand out of their neighbourhood.
fn whiten(curve: &[f64], half: usize) -> Vec<f64> {
    let n = curve.len();
    let mut prefix = vec![0f64; n + 1];
    for i in 0..n {
        prefix[i + 1] = prefix[i] + curve[i];
    }
    let out: Vec<f64> = (0..n).map(|i| {
        let (a, b) = (i.saturating_sub(half), (i + half + 1).min(n));
        (curve[i] - (prefix[b] - prefix[a]) / (b - a) as f64).max(0.0)
    }).collect();
    let mean = out.iter().sum::<f64>() / n.max(1) as f64;
    let spread = (out.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n.max(1) as f64).sqrt().max(1e-9);
    out.into_iter().map(|v| v / spread).collect()
}

/// The beat period in hops (fractional), and how far its peak stands over the rest (0 to 1).
fn tempo(onset: &[f64], rate: f64, hint: Option<f64>) -> Option<(f64, f64)> {
    let (min_lag, max_lag) = ((rate * 60.0 / 200.0).floor() as usize, (rate * 60.0 / 50.0).ceil() as usize);
    if onset.len() < max_lag * 2 + 4 {
        return None;
    }
    let mean = onset.iter().sum::<f64>() / onset.len() as f64;
    let centred: Vec<f64> = onset.iter().map(|v| v - mean).collect();
    let ac = |lag: usize| -> f64 { centred.iter().zip(&centred[lag..]).map(|(a, b)| a * b).sum::<f64>() / (centred.len() - lag) as f64 };
    let raw: Vec<f64> = (0..=max_lag * 2).map(|lag| if lag < min_lag / 2 { 0.0 } else { ac(lag) }).collect();
    let centre = hint.unwrap_or(120.0);
    let width = if hint.is_some() { 0.25 } else { 1.0 };
    let score = |lag: usize| -> f64 {
        let bpm = rate * 60.0 / lag as f64;
        let prior = (-0.5 * ((bpm / centre).log2() / width).powi(2)).exp();
        // A beat is also there at twice its period: count that, so a true beat beats its half.
        prior * (raw[lag] + 0.5 * raw.get(lag * 2).copied().unwrap_or(0.0))
    };
    let scores: Vec<(usize, f64)> = (min_lag..=max_lag).map(|lag| (lag, score(lag))).collect();
    let &(best, peak) = scores.iter().max_by(|a, b| a.1.total_cmp(&b.1))?;
    if peak <= 0.0 {
        return None;
    }
    // Between hops: the parabola through the peak and its neighbours.
    let (l, c, r) = (score(best - 1), peak, score((best + 1).min(max_lag * 2 - 1)));
    let shift = if l - 2.0 * c + r < 0.0 { 0.5 * (l - r) / (l - 2.0 * c + r) } else { 0.0 };
    let typical = scores.iter().map(|s| s.1.max(0.0)).sum::<f64>() / scores.len() as f64;
    Some((best as f64 + shift.clamp(-0.5, 0.5), (1.0 - typical / peak).clamp(0.0, 1.0)))
}

/// Ellis's tracker: the frames of the beats that best follow the onsets at `period` hops.
fn track(onset: &[f64], period: f64) -> Vec<usize> {
    let n = onset.len();
    let tightness = 100.0;
    let mut score = vec![0f64; n];
    let mut back = vec![usize::MAX; n];
    for t in 0..n {
        let (from, to) = ((t as f64 - 2.0 * period).round().max(0.0) as usize, (t as f64 - period / 2.0).round());
        let mut best = (0.0, usize::MAX);
        if to >= 0.0 {
            let last = (to as usize).min(t.saturating_sub(1));
            for (prev, before) in score.iter().enumerate().take(last + 1).skip(from) {
                let gap = (t - prev) as f64;
                let value = before - tightness * (gap / period).ln().powi(2);
                if best.1 == usize::MAX || value > best.0 {
                    best = (value, prev);
                }
            }
        }
        score[t] = onset[t] + if best.1 == usize::MAX { 0.0 } else { best.0.max(0.0) };
        back[t] = if best.0 > 0.0 { best.1 } else { usize::MAX };
    }
    // The last beat: the best-scoring frame of the final period.
    let tail = (n as f64 - period).max(0.0) as usize;
    let Some(mut t) = (tail..n).max_by(|a, b| score[*a].total_cmp(&score[*b])) else { return Vec::new() };
    let mut beats = vec![t];
    while back[t] != usize::MAX {
        t = back[t];
        beats.push(t);
    }
    beats.reverse();
    beats
}

/// A beat's time between hops: the parabola through its onset frame and the frames beside it.
fn refine(onset: &[f64], t: usize) -> f64 {
    // The strongest onset within a hop of where the tracker put it.
    let t = (t.saturating_sub(1)..=(t + 1).min(onset.len() - 1)).max_by(|a, b| onset[*a].total_cmp(&onset[*b])).unwrap_or(t);
    if t == 0 || t + 1 >= onset.len() {
        return t as f64;
    }
    let (l, c, r) = (onset[t - 1], onset[t], onset[t + 1]);
    let d = l - 2.0 * c + r;
    t as f64 + if d < 0.0 { (0.5 * (l - r) / d).clamp(-0.5, 0.5) } else { 0.0 }
}

/// The least-squares line `start + i * period` through beats at `times`, each given its index on
/// the grid, dropping the far ones: (start, period, RMS residual), in the times' unit.
fn fit(times: &[f64], period: f64) -> Option<(f64, f64, f64, usize)> {
    let mut period = period;
    let mut start = *times.first()?;
    let mut kept: Vec<(f64, f64)> = Vec::new();
    for _ in 0..4 {
        kept = times.iter().map(|t| (((t - start) / period).round(), *t)).filter(|(i, t)| (t - (start + i * period)).abs() < period / 4.0).collect();
        if kept.len() < 4 {
            return None;
        }
        let n = kept.len() as f64;
        let (si, st) = (kept.iter().map(|k| k.0).sum::<f64>(), kept.iter().map(|k| k.1).sum::<f64>());
        let (sii, sit) = (kept.iter().map(|k| k.0 * k.0).sum::<f64>(), kept.iter().map(|k| k.0 * k.1).sum::<f64>());
        let d = n * sii - si * si;
        if d.abs() < 1e-9 {
            return None;
        }
        period = (n * sit - si * st) / d;
        start = (st - period * si) / n;
    }
    let rms = (kept.iter().map(|(i, t)| (t - (start + i * period)).powi(2)).sum::<f64>() / kept.len() as f64).sqrt();
    Some((start, period, rms, kept.len()))
}

/// The grid's phase set on the attacks themselves: the shift, within 30 ms either way, at which
/// the beats land where the sound's level rises fastest (a 2 ms envelope), in seconds. The flux
/// frames are 46 ms long and a soft attack peaks in them early; this puts a beat where it is heard.
fn attack_shift(samples: &[f32], first_s: f64, beat_s: f64) -> f64 {
    let rate = RATE as f64;
    let width = (rate * 0.002) as usize;
    let mut level = Vec::with_capacity(samples.len());
    let mut sum = 0f64;
    for i in 0..samples.len() {
        sum += (samples[i] as f64).powi(2);
        if i >= width {
            sum -= (samples[i - width] as f64).powi(2);
        }
        level.push((sum.max(0.0) / width as f64).sqrt());
    }
    let rise: Vec<f64> = level.windows(2).map(|w| (w[1] - w[0]).max(0.0)).collect();
    let mut smooth = vec![0f64; rise.len()];
    let half = (rate * 0.00125) as usize;
    let mut acc = 0f64;
    for i in 0..rise.len() + half {
        if i < rise.len() {
            acc += rise[i];
        }
        if i > 2 * half {
            acc -= rise[i - 2 * half - 1];
        }
        if i >= half && i - half < smooth.len() {
            smooth[i - half] = acc;
        }
    }
    let score = |shift: f64| -> f64 {
        let (mut total, mut n) = (0f64, 0usize);
        let mut t = first_s + shift;
        while t < samples.len() as f64 / rate {
            if t >= 0.0 {
                total += smooth.get((t * rate) as usize).copied().unwrap_or(0.0);
                n += 1;
            }
            t += beat_s;
        }
        total / n.max(1) as f64
    };
    let base = score(0.0);
    let (best, at) = (-30..=30).map(|ms| (score(ms as f64 / 1000.0), ms as f64 / 1000.0)).fold((base, 0.0), |a, b| if b.0 > a.0 { b } else { a });
    // Only a clear gain moves it: a steady drone has no attack to set the phase by.
    if best > base * 1.05 { at } else { 0.0 }
}

/// `samples` under about 150 Hz: two one-pole low-passes in a row.
fn low_band(samples: &[f32]) -> Vec<f64> {
    let a = (-2.0 * std::f64::consts::PI * 150.0 / RATE as f64).exp();
    let (mut y1, mut y2) = (0f64, 0f64);
    samples.iter().map(|s| {
        y1 = (1.0 - a) * *s as f64 + a * y1;
        y2 = (1.0 - a) * y1 + a * y2;
        y2
    }).collect()
}

fn rms_db(samples: &[f32]) -> f64 {
    if samples.is_empty() {
        return -120.0;
    }
    let ms = samples.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / samples.len() as f64;
    if ms <= 1e-12 { -120.0 } else { 10.0 * ms.log10() }
}

/// The energy under 200 Hz of `samples` against its whole, dB: a one-pole low-pass, compared.
fn low_share_db(samples: &[f32], rate: u32) -> f64 {
    let a = (-2.0 * std::f64::consts::PI * 200.0 / rate as f64).exp();
    let (mut y, mut low, mut all) = (0f64, 0f64, 0f64);
    for s in samples {
        y = (1.0 - a) * *s as f64 + a * y;
        low += y * y;
        all += (*s as f64).powi(2);
    }
    if all <= 1e-12 { -120.0 } else { 10.0 * (low.max(1e-12) / all).log10() }
}

/// The beat grid of mono `pcm` at [`RATE`], `beats_per_bar` beats a bar; `None` when no steady
/// beat can be found (speech, a drone, silence).
pub fn analyse(pcm: &Pcm, beats_per_bar: u32, hint: Option<f64>) -> Option<Grid> {
    let samples = &pcm.samples;
    let hops = RATE as f64 / HOP as f64;
    let (flux, _) = onsets(samples);
    let onset = whiten(&flux, (hops * 0.25) as usize);
    let (period, beat_confidence) = tempo(&onset, hops, hint)?;
    let beats = track(&onset, period);
    let times: Vec<f64> = beats.iter().map(|&t| refine(&onset, t)).collect();
    let (start, period, rms, found) = fit(&times, period)?;
    let bpb = beats_per_bar.max(1) as usize;
    let hop_s = HOP as f64 / RATE as f64;
    let start = start + ONSET_LEAD_S / hop_s;
    let slack = START_SLACK_S / hop_s;
    let first = start - ((start + slack) / period).floor() * period;
    let count = ((samples.len() as f64 / HOP as f64 - first) / period).floor().max(0.0) as usize + 1;
    // The bar's phase whose beats hit hardest under 150 Hz: the kick and bass a bar starts on.
    // Energy in dB, not the whitened flux, which flattens a kick that is only louder.
    let low = low_band(samples);
    let window = (RATE as f64 * 0.12) as usize;
    let hit = |hop: f64| -> f64 {
        let a = (hop * HOP as f64).max(0.0) as usize;
        let slice = &low[a.min(low.len())..(a + window).min(low.len())];
        let ms = slice.iter().map(|v| v * v).sum::<f64>() / slice.len().max(1) as f64;
        if ms <= 1e-12 { -120.0 } else { 10.0 * ms.log10() }
    };
    let hits: Vec<f64> = (0..count).map(|k| hit(first + k as f64 * period)).collect();
    // Music most often starts on a bar: the phase of its first strong beat leads by 1.5 dB, which
    // a downbeat that really is harder outweighs.
    let loudest = hits.iter().copied().fold(f64::MIN, f64::max);
    let opening = hits.iter().position(|db| *db > loudest - 20.0).map(|k| k % bpb);
    // Sections change on bars: where the kick comes back after a break (a drop), or a bar starts
    // harder than the beat before it, the low band jumps. The phase the largest jumps land on
    // (each held to 12 dB, the three largest) is where bars begin.
    let jumps: Vec<f64> = (0..bpb).map(|phase| {
        let mut up: Vec<f64> = (phase..count).step_by(bpb).filter(|k| *k >= 1).map(|k| (hits[k] - hits[k - 1]).clamp(0.0, 12.0)).collect();
        up.sort_by(|a, b| b.total_cmp(a));
        up.iter().take(3).sum::<f64>() / 3.0
    }).collect();
    let phase_score: Vec<f64> = (0..bpb).map(|phase| {
        let picks: Vec<f64> = hits.iter().skip(phase).step_by(bpb).copied().filter(|db| *db > -100.0).collect();
        let energy = if picks.is_empty() { -120.0 } else { picks.iter().sum::<f64>() / picks.len() as f64 };
        energy + if opening == Some(phase) { 1.5 } else { 0.0 } + jumps[phase]
    }).collect();
    let (phase, best) = phase_score.iter().copied().enumerate().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap_or((0, 0.0));
    let others = phase_score.iter().enumerate().filter(|(i, _)| *i != phase).map(|(_, v)| *v).fold(f64::MIN, f64::max);
    // 6 dB over the next strongest phase is as sure as it gets.
    let downbeat_confidence = if bpb > 1 { ((best - others) / 6.0).clamp(0.0, 1.0) } else { 0.0 };

    let seconds = |hop: f64| hop * HOP as f64 / RATE as f64;
    let beat_s = seconds(period);
    let shift = attack_shift(samples, seconds(first), beat_s);
    let first = first + shift / hop_s;
    // A shift can carry the first beat past the start, or one before it in.
    let first = if first < -slack { first + period } else if first - period >= -slack { first - period } else { first };
    let first_beat_s = seconds(first).max(0.0);
    let first_downbeat_s = first_beat_s + phase as f64 * beat_s;
    let bar_s = beat_s * bpb as f64;
    let total = pcm.seconds();
    let mut bars = Vec::new();
    let mut at_s = first_downbeat_s;
    while at_s < total - bar_s * 0.25 && bars.len() < MOST_BARS {
        let (a, b) = ((at_s * RATE as f64) as usize, (((at_s + bar_s) * RATE as f64) as usize).min(samples.len()));
        let slice = &samples[a.min(b)..b];
        bars.push(Bar { n: bars.len() as u32 + 1, start_s: at_s, loudness_db: rms_db(slice), low_db: low_share_db(slice, RATE) });
        at_s += bar_s;
    }
    Some(Grid {
        bpm: 60.0 / beat_s,
        beat_s,
        first_beat_s,
        first_downbeat_s,
        beats_per_bar: bpb as u32,
        beats_found: found as u32,
        fit_rms_ms: seconds(rms) * 1000.0,
        beat_confidence,
        downbeat_confidence,
        seconds: total,
        sections: sections(&bars),
        bars,
    })
}

/// Where a bar's loudness or low band moves 3 dB (6 dB for the low band) from the two before it;
/// at most one change every two bars, the larger kept.
fn sections(bars: &[Bar]) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    for i in 2..bars.len() {
        let before = |f: fn(&Bar) -> f64| (f(&bars[i - 1]) + f(&bars[i - 2])) / 2.0;
        let loud = bars[i].loudness_db - before(|b| b.loudness_db);
        let low = bars[i].low_db - before(|b| b.low_db);
        let found = if loud.abs() >= 3.0 {
            Some((if loud > 0.0 { "louder" } else { "quieter" }, loud))
        } else if low.abs() >= 6.0 {
            Some((if low > 0.0 { "bass_in" } else { "bass_out" }, low))
        } else {
            None
        };
        let Some((kind, change)) = found else { continue };
        let section = Section { bar: bars[i].n, start_s: bars[i].start_s, kind, change_db: (change * 10.0).round() / 10.0 };
        match out.last_mut() {
            Some(last) if section.bar - last.bar < 2 => {
                if section.change_db.abs() > last.change_db.abs() {
                    *last = section;
                }
            }
            _ => out.push(section),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A click track: a short tone burst on every beat, the downbeats louder and with a low thump,
    /// and from `louder_from` on everything 8 dB up.
    fn clicks(bpm: f64, first: f64, seconds: f64, louder_from: Option<f64>) -> Pcm {
        let rate = RATE as f64;
        let mut samples = vec![0f32; (seconds * rate) as usize];
        let beat = 60.0 / bpm;
        let mut k = 0;
        let mut t = first;
        while t < seconds {
            let down = k % 4 == 0;
            let gain = if louder_from.is_some_and(|l| t >= l) { 2.5 } else { 1.0 };
            let start = (t * rate) as usize;
            for n in 0..(0.06 * rate) as usize {
                let Some(s) = samples.get_mut(start + n) else { break };
                let x = n as f64 / rate;
                let decay = (-x * 60.0).exp();
                let mut v = 0.3 * (2.0 * std::f64::consts::PI * 2000.0 * x).sin() * decay;
                if down {
                    v += 0.5 * (2.0 * std::f64::consts::PI * 60.0 * x).sin() * (-x * 25.0).exp();
                }
                *s += (v * gain) as f32;
            }
            k += 1;
            t = first + k as f64 * beat;
        }
        Pcm { rate: RATE, channels: 1, samples }
    }

    #[test]
    fn an_fft_finds_a_tone_in_its_bin() {
        let n = 1024;
        let cos: Vec<f64> = (0..n).map(|k| (2.0 * std::f64::consts::PI * k as f64 / n as f64).cos()).collect();
        let sin: Vec<f64> = (0..n).map(|k| (2.0 * std::f64::consts::PI * k as f64 / n as f64).sin()).collect();
        let mut re: Vec<f64> = (0..n).map(|k| (2.0 * std::f64::consts::PI * 37.0 * k as f64 / n as f64).cos()).collect();
        let mut im = vec![0f64; n];
        fft(&mut re, &mut im, &cos, &sin);
        let peak = (0..n / 2).max_by(|a, b| (re[*a].hypot(im[*a])).total_cmp(&re[*b].hypot(im[*b]))).unwrap();
        assert_eq!(peak, 37);
        assert!((re[37].hypot(im[37]) - n as f64 / 2.0).abs() < 1e-6);
    }

    #[test]
    fn a_click_track_reads_as_its_tempo_first_beat_and_downbeat() {
        let grid = analyse(&clicks(120.0, 0.25, 30.0, None), 4, None).unwrap();
        assert!((grid.bpm - 120.0).abs() < 0.01, "{grid:?}");
        assert!((grid.first_beat_s - 0.25).abs() < 0.003, "{grid:?}");
        assert!((grid.first_downbeat_s - 0.25).abs() < 0.003, "{grid:?}");
        assert!(grid.fit_rms_ms < 10.0, "{grid:?}");
        assert!(grid.beats_found > 50, "{grid:?}");
        assert_eq!(grid.bars[0].n, 1);
        assert!((grid.bars[1].start_s - 2.25).abs() < 0.02, "{:?}", grid.bars[1]);
    }

    #[test]
    fn a_tempo_that_is_not_round_is_measured_to_the_hundredth() {
        let grid = analyse(&clicks(97.3, 0.6, 40.0, None), 4, None).unwrap();
        assert!((grid.bpm - 97.3).abs() < 0.03, "{grid:?}");
        assert!((grid.first_downbeat_s - 0.6).abs() < 0.015, "{grid:?}");
    }

    #[test]
    fn the_downbeat_is_the_accented_beat_not_the_first() {
        // The first click is the bar's third beat: the downbeat is two beats later.
        let grid = analyse(&clicks(120.0, 0.1, 30.0, None).cut(1.0), 4, None).unwrap();
        assert!((grid.first_beat_s - 0.1).abs() < 0.015, "{grid:?}");
        assert!((grid.first_downbeat_s - 1.1).abs() < 0.015, "{grid:?}");
        assert!(grid.downbeat_confidence > 0.2, "{grid:?}");
    }

    #[test]
    fn a_downbeat_that_is_only_louder_is_found() {
        // A kick on every beat, 5 dB harder on the bar's first, from a quarter second in.
        let rate = RATE as f64;
        let samples: Vec<f32> = (0..(rate * 16.0) as usize).map(|i| {
            let t = i as f64 / rate - 0.25;
            if t < 0.0 { return 0.0; }
            let beat = t % 0.5;
            let down = (t / 0.5).floor() as i64 % 4 == 0;
            ((2.0 * std::f64::consts::PI * 55.0 * beat).sin() * (-beat * 30.0).exp() * if down { 0.9 } else { 0.5 }) as f32
        }).collect();
        let grid = analyse(&Pcm { rate: RATE, channels: 1, samples }, 4, None).unwrap();
        assert!((grid.first_downbeat_s - 0.25).abs() < 0.02, "{grid:?}");
        assert!(grid.downbeat_confidence > 0.5, "{grid:?}");
    }

    #[test]
    fn a_section_change_is_found_at_its_bar() {
        let grid = analyse(&clicks(120.0, 0.0, 40.0, Some(16.0)), 4, None).unwrap();
        let change = grid.sections.iter().find(|s| s.kind == "louder").unwrap_or_else(|| panic!("{:?}", grid.sections));
        assert_eq!(change.bar, 9, "{:?}", grid.sections);
        assert!(change.change_db > 6.0, "{change:?}");
    }

    #[test]
    fn noise_and_silence_have_no_beat_to_give() {
        assert!(analyse(&Pcm { rate: RATE, channels: 1, samples: vec![0.0; RATE as usize * 10] }, 4, None).is_none());
    }

    impl Pcm {
        /// The same sound with its first `seconds` cut off.
        fn cut(mut self, seconds: f64) -> Pcm {
            let n = (seconds * self.rate as f64) as usize;
            self.samples.drain(..n.min(self.samples.len()));
            self
        }
    }
}
#[cfg(test)]
mod real_music {
    /// `BEATS_FILE=<music> cargo test -p relay-core --lib real_music -- --ignored --nocapture`:
    /// the grid of a real file, for checking by ear against a click.
    #[test]
    #[ignore = "needs ffmpeg and a music file in BEATS_FILE"]
    fn print_grid_of_a_file() {
        let path = std::env::var("BEATS_FILE").unwrap();
        let out = std::process::Command::new("ffmpeg").args(["-v", "error", "-i", &path, "-ac", "1", "-ar", "22050", "-f", "s16le", "pipe:1"]).output().unwrap();
        let samples = out.stdout.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b) as f32 / 32768.0).collect();
        let started = std::time::Instant::now();
        let g = super::analyse(&super::Pcm { rate: super::RATE, channels: 1, samples }, 4, None).unwrap();
        println!("{:?}: bpm {:.3}, first beat {:.4}, first downbeat {:.4}, {} beats, fit {:.2} ms, confidence {:.2}/{:.2}",
            started.elapsed(), g.bpm, g.first_beat_s, g.first_downbeat_s, g.beats_found, g.fit_rms_ms, g.beat_confidence, g.downbeat_confidence);
        println!("{:?}", g.sections);
    }
}
