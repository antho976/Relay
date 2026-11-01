//! How long a VOD's work (or a clip's drawing) has left, as the space and an agent show it: the
//! running stage's own pace once it has one, blended in from this PC's readings while it is young,
//! and each stage after it from the readings alone (docs/CLIPS.md, Readings: an RTX 4070, a
//! 5 h 13 min VOD transcribed in 3 min 26 s, ten clips drawn in about six minutes).

use relay_bus::ops::clips::{Stage, Vod, VodState};

/// What one stage takes, in seconds, for a VOD `duration` long making `clips` clips.
pub fn stage_s(stage: Stage, duration: f64, clips: u32, link: bool) -> f64 {
    let n = clips.max(1) as f64;
    match stage {
        // A link's download has no reading: it is shown at its own pace once it has one.
        Stage::Probe => if link { duration * 0.02 } else { 2.0 },
        Stage::Audio => (duration * 0.001).max(3.0),
        Stage::Transcribe => duration * 0.011,
        Stage::Signals => duration * 0.0006 + 2.0,
        Stage::Layout => 8.0,
        // Parts read three at once, about 25 s a round.
        Stage::Find => (super::find::parts(duration.max(1.0)).len() as f64 / 3.0).ceil() * 25.0,
        Stage::Cut => 6.0 * n + 15.0,
        Stage::Render => 33.0 * n,
    }
}

/// What is left of something expected to take `expected` seconds, `elapsed` in and `progress`
/// (0 to 1) along: the readings first, its own pace more as it gets on.
pub fn blend(expected: f64, elapsed: f64, progress: f64) -> f64 {
    let by_reading = (expected - elapsed).max(expected * 0.1);
    if progress < 0.03 || elapsed < 3.0 {
        return by_reading;
    }
    let by_pace = elapsed * (1.0 - progress) / progress;
    let w = (progress * 2.0).clamp(0.0, 1.0);
    w * by_pace + (1.0 - w) * by_reading
}

fn since(at: Option<&str>, now: jiff::Timestamp) -> f64 {
    at.and_then(|a| a.parse::<jiff::Timestamp>().ok()).map_or(0.0, |a| (now - a).total(jiff::Unit::Second).unwrap_or(0.0).max(0.0))
}

/// VOD `v`'s work left, in seconds: `None` once it is not worked on or waiting. `to_draw` is how
/// many of its clips are still to be cut or drawn (ten are assumed before the finder has said).
pub fn left(v: &Vod, stage_at: Option<&str>, to_draw: u32, now: jiff::Timestamp) -> Option<f64> {
    if !matches!(v.state, VodState::Working | VodState::Queued) {
        return None;
    }
    let clips = if to_draw > 0 { to_draw } else { 10 };
    let link = super::is_link(&v.source);
    let at = v.stage.unwrap_or(Stage::Probe);
    let mut total = 0.0;
    for s in Stage::ALL.into_iter().filter(|s| *s >= at) {
        let expected = stage_s(s, v.duration_s, clips, link);
        total += if s == at && v.state == VodState::Working { blend(expected, since(stage_at, now), v.progress) } else { expected };
    }
    Some(total.round())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stage_is_read_by_its_pace_once_it_has_one() {
        // Young: the reading.
        assert_eq!(blend(100.0, 1.0, 0.0), 99.0);
        // Half way in 30 s: its own pace says 30 s more, whatever the reading said.
        assert!((blend(100.0, 30.0, 0.5) - 30.0).abs() < 1e-9);
        // Past what the reading said and not done: never nothing left.
        assert_eq!(blend(100.0, 500.0, 0.0), 10.0);
    }

    #[test]
    fn the_readings_add_up_to_about_what_a_vod_took() {
        // The IdleOn VOD (5:13:25, ten clips) took about twelve minutes from import to clips.
        let total: f64 = Stage::ALL.iter().map(|s| stage_s(*s, 18805.0, 10, false)).sum();
        assert!((600.0..900.0).contains(&total), "{total}");
    }
}
