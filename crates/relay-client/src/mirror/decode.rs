//! The video half of the mirror without the decoder: which engine packets the decoder can take,
//! how they become the whole access units it is fed, and the colors a decoded picture is shown in.
//! The decoder itself (libavcodec, in process) and the GTK texture live in `apps/relay-native`'s
//! `mirror/avcodec.rs`; nothing here links to either, so `cargo test` stays headless.
//!
//! The pipeline is measured, not guessed (`scripts/mirror-latency/`). It used to be an `ffmpeg`
//! child: base64 packets in on stdin, PPM pictures back on stdout. Two holds stacked in that
//! child — frame threading kept one picture per thread in flight, and the raw H.264 parser could
//! not finish a frame until the next one started — and the RGB pictures coming back were
//! 130–460 MB/s through a pipe. In process, with low-delay flags, one decoding thread and whole
//! access units (no parser), a picture leaves the decoder the moment its packet goes in; GTK
//! takes the YUV planes as they are and converts them on the GPU.

/// The engine's packet flags (`[flags][annex-b bytes]`): bit 0 is codec config (SPS/PPS).
pub const CONFIG: u8 = 1;
/// Bit 1: a key frame.
pub const KEY: u8 = 2;

/// Decides, packet by packet, whether the decoder can take what arrives. A decoder cannot join
/// an H.264 stream mid-GOP, so after a sequence gap (the engine dropped packets for a slow
/// window) or a late join, it waits for the next codec config — and asks for one once.
///
/// A rotation needs nothing from the gate any more: the scrcpy server restarts its encoder and
/// sends the new SPS ahead of the new size's key frame, and libavcodec's H.264 decoder takes an
/// SPS change in stride. (The `ffmpeg` child could not: it scaled every picture to the size of
/// the first one, and crashed with `-noautoscale`, so every size change used to cost a new child.)
#[derive(Debug)]
pub struct Gate {
    last: Option<u64>,
    waiting: bool,
    asked: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Admit {
    Write,
    Skip,
    /// Skip this packet and ask the device for a fresh config + key frame.
    Reset,
}

impl Default for Gate {
    fn default() -> Self {
        Self { last: None, waiting: true, asked: false }
    }
}

impl Gate {
    pub fn admit(&mut self, seq: u64, config: bool) -> Admit {
        if self.last.is_some_and(|last| seq != last + 1) {
            self.waiting = true;
            self.asked = false;
        }
        self.last = Some(seq);
        if config {
            self.waiting = false;
            self.asked = false;
            return Admit::Write;
        }
        if !self.waiting {
            return Admit::Write;
        }
        if self.asked {
            Admit::Skip
        } else {
            self.asked = true;
            Admit::Reset
        }
    }
}

/// An engine packet's flags and its Annex B bytes; `None` when it carries nothing.
pub fn split(packet: &[u8]) -> Option<(u8, &[u8])> {
    let (flags, payload) = packet.split_first()?;
    (!payload.is_empty()).then_some((*flags, payload))
}

/// Turns admitted packets into what the decoder is fed: exactly one whole access unit per call,
/// so a picture is complete — and decoded — the moment its packet arrives, with no parser
/// waiting for the next frame to start. A config packet (SPS/PPS) is no picture: it is held and
/// goes in ahead of the picture that follows it, as scrcpy's own client does.
#[derive(Debug, Default)]
pub struct Assembler {
    config: Vec<u8>,
}

impl Assembler {
    /// The access unit to decode for one admitted packet, if it completes one.
    pub fn push(&mut self, flags: u8, payload: &[u8]) -> Option<Vec<u8>> {
        if flags & CONFIG != 0 {
            self.config.clear();
            self.config.extend_from_slice(payload);
            return None;
        }
        let mut unit = std::mem::take(&mut self.config);
        unit.extend_from_slice(payload);
        Some(unit)
    }
}

/// The colors a decoded picture is shown in, as ITU-T H.273 (CICP) code points: what GTK's
/// `GdkCicpParams` takes, and what libavcodec reports a stream's VUI as (`AVColorSpace` values are
/// H.273 matrix coefficients).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub primaries: u32,
    pub transfer: u32,
    pub matrix: u32,
    pub full_range: bool,
}

/// H.273 matrix coefficients.
pub const BT709: u32 = 1;
pub const UNSPECIFIED: u32 = 2;
pub const BT470BG: u32 = 5;
pub const BT601: u32 = 6;
pub const BT2020_NCL: u32 = 9;
/// H.273 transfer characteristics 13: IEC 61966-2-1, the sRGB curve.
pub const SRGB: u32 = 13;
/// libavcodec's `AVColorRange`: 1 is narrow ("MPEG", 16–235), 2 is full ("JPEG", 0–255).
pub const RANGE_FULL: u32 = 2;

/// The colors to show a picture in, from the stream's VUI as libavcodec reports it (`matrix` an
/// `AVColorSpace`, `range` an `AVColorRange`). Only the matrix and the range come from the stream:
///
/// - Android's encoders tag their screen captures in the VUI (the emulator's: BT.709, narrow
///   range) or leave them unspecified, which is read as BT.601, narrow — what swscale assumed for
///   the `ffmpeg` child too.
/// - Primaries and transfer are always sRGB's. The device composes its screen in sRGB and the
///   encoder converts those values straight to YUV; a VUI that says BT.709 or SMPTE 170M transfer
///   describes a camera, not this, and honoring it would shift every mid-tone away from what the
///   phone shows (and from what the RGB path showed — swscale applied the matrix and range only).
pub fn color(matrix: u32, range: u32) -> Color {
    let matrix = match matrix {
        BT709 => BT709,
        BT2020_NCL => BT2020_NCL,
        // BT470BG and SMPTE 170M are the same matrix; unspecified, RGB and the rest fall back to it.
        _ => BT601,
    };
    Color { primaries: 1, transfer: SRGB, matrix, full_range: range == RANGE_FULL }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gap_waits_for_the_next_config_and_asks_once() {
        let mut gate = Gate::default();
        // Normal start: config first, then pictures.
        assert_eq!(gate.admit(1, true), Admit::Write);
        assert_eq!(gate.admit(2, false), Admit::Write);
        // Packets 3..=9 were dropped for a slow window.
        assert_eq!(gate.admit(10, false), Admit::Reset);
        assert_eq!(gate.admit(11, false), Admit::Skip);
        assert_eq!(gate.admit(12, false), Admit::Skip);
        // The device answers the reset with config + key frame.
        assert_eq!(gate.admit(13, true), Admit::Write);
        assert_eq!(gate.admit(14, false), Admit::Write);
    }

    #[test]
    fn a_rotation_keeps_feeding_the_same_decoder() {
        // The engine reports the new size, then the new stream's config; the gate has nothing to
        // do with sizes, so the packets flow without a reset or a skip.
        let mut gate = Gate::default();
        assert_eq!(gate.admit(1, true), Admit::Write);
        assert_eq!(gate.admit(2, false), Admit::Write);
        assert_eq!(gate.admit(3, true), Admit::Write);
        assert_eq!(gate.admit(4, false), Admit::Write);
    }

    #[test]
    fn a_late_join_asks_for_config_instead_of_feeding_a_broken_stream() {
        let mut gate = Gate::default();
        assert_eq!(gate.admit(40, false), Admit::Reset);
        assert_eq!(gate.admit(41, false), Admit::Skip);
        assert_eq!(gate.admit(42, true), Admit::Write);
        // A gap that lands exactly on a config packet needs nothing.
        assert_eq!(gate.admit(50, true), Admit::Write);
        assert_eq!(gate.admit(51, false), Admit::Write);
    }

    #[test]
    fn packets_split_into_flags_and_bytes() {
        assert_eq!(split(&[2, 0, 0, 0, 1, 0x65]), Some((KEY, &[0u8, 0, 0, 1, 0x65][..])));
        assert_eq!(split(&[1]), None);
        assert_eq!(split(&[]), None);
    }

    #[test]
    fn config_goes_in_with_the_picture_after_it() {
        let mut units = Assembler::default();
        let sps = [0u8, 0, 0, 1, 0x67, 0x42];
        let idr = [0u8, 0, 0, 1, 0x65, 0x88];
        let p = [0u8, 0, 0, 1, 0x41, 0x9a];
        assert_eq!(units.push(CONFIG, &sps), None);
        assert_eq!(units.push(KEY, &idr), Some([&sps[..], &idr[..]].concat()));
        // The config is spent: the next picture goes in alone.
        assert_eq!(units.push(0, &p), Some(p.to_vec()));
        // A newer config replaces one that never met its picture.
        assert_eq!(units.push(CONFIG, &[0, 0, 0, 1, 0x67, 0x01]), None);
        assert_eq!(units.push(CONFIG, &sps), None);
        assert_eq!(units.push(KEY, &idr), Some([&sps[..], &idr[..]].concat()));
    }

    #[test]
    fn colors_take_the_matrix_and_range_from_the_stream_and_srgb_for_the_rest() {
        let narrow_601 = Color { primaries: 1, transfer: SRGB, matrix: BT601, full_range: false };
        // BT.601 as tagged, and what an untagged stream means.
        assert_eq!(color(BT601, 1), narrow_601);
        assert_eq!(color(BT470BG, 1), narrow_601);
        assert_eq!(color(UNSPECIFIED, 0), narrow_601);
        assert_eq!(color(0, 0), narrow_601);
        assert_eq!(color(BT709, 1), Color { matrix: BT709, ..narrow_601 });
        assert_eq!(color(BT709, RANGE_FULL), Color { matrix: BT709, full_range: true, ..narrow_601 });
        assert_eq!(color(BT2020_NCL, 1).matrix, BT2020_NCL);
        // A matrix GTK has no conversion for (BT.2020 constant luminance, YCgCo) falls back.
        assert_eq!(color(10, 1), narrow_601);
        assert_eq!(color(8, 1), narrow_601);
    }
}
