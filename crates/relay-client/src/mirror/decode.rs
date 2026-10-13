//! The video half of the mirror without the FFmpeg child: the decoder's command line, what its
//! stdin receives for each engine packet, which packets it can take, and the pictures it sends
//! back.
//!
//! The FFmpeg line is measured, not guessed (scratchpad harness: a real 464×1024 H.264 stream
//! at 30 fps through a disposable engine, then a static screen):
//!
//! | input options                                   | AUD | median latency | frames never shown |
//! |-------------------------------------------------|-----|----------------|--------------------|
//! | `-probesize 32 -analyzeduration 0` (the old line) | no  | 535 ms         | 16 of 180          |
//! | `+ -fflags nobuffer -flags low_delay -threads 1`  | no  | 2039 ms        | 61                 |
//! | `+ -threads 1`                                    | no  | 34 ms          | 1                  |
//! | `+ -threads 1`, `-fps_mode passthrough`           | yes | 1 ms           | 0                  |
//!
//! Two separate holds were stacking. Frame threading keeps one picture per decoder thread in
//! flight (16 on a 16-core machine: 535 ms at 30 fps), and the raw H.264 parser cannot finish a
//! frame until the *next* one starts — so the last picture before the screen goes still never
//! appears at all. `-threads 1` removes the first; an access-unit delimiter written after each
//! packet ends the frame at once and removes the second. `-fflags nobuffer` is a trap here: it
//! makes this FFmpeg (n9) hold frames far longer, so it is deliberately absent.
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

/// H.264 access-unit delimiter (NAL type 9, primary_pic_type 7 = any slice type).
pub const AUD: [u8; 6] = [0, 0, 0, 1, 0x09, 0xf0];

pub fn ffmpeg_args() -> Vec<&'static str> {
    vec![
        "-hide_banner",
        "-loglevel",
        "error",
        "-probesize",
        "32",
        "-analyzeduration",
        "0",
        "-threads",
        "1",
        "-f",
        "h264",
        "-i",
        "pipe:0",
        "-an",
        "-fps_mode",
        "passthrough",
        "-f",
        "image2pipe",
        "-vcodec",
        "ppm",
        "-pix_fmt",
        "rgb24",
        "pipe:1",
    ]
}

/// What the decoder's stdin receives for one engine packet (`[flags][annex-b bytes]`, flag bit 0
/// = codec config). Config packets carry SPS/PPS and belong to the frame that follows them, so
/// only a picture gets the delimiter that closes it.
pub fn decoder_bytes(packet: &[u8]) -> Option<Vec<u8>> {
    let (flags, payload) = packet.split_first()?;
    if payload.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(payload.len() + AUD.len());
    out.extend_from_slice(payload);
    if flags & 1 == 0 {
        out.extend_from_slice(&AUD);
    }
    Some(out)
}

/// Decides, packet by packet, whether the decoder can take what arrives. A decoder cannot join
/// an H.264 stream mid-GOP, so after a sequence gap (the engine dropped packets for a slow
/// window) or a late join, it waits for the next codec config — and asks for one once.
///
/// It also says when the decoder itself has to go. FFmpeg scales every picture to the size of
/// the first one it decoded (`-autoscale`), so after the device rotates, a landscape stream came
/// back squashed into the portrait frame — and with it the size every touch is sent with, which
/// the server then ignores as "generated for a different device size". `-noautoscale` is no way
/// out: FFmpeg n9 crashes on the size change. So a stream of a new size gets a new decoder.
#[derive(Debug)]
pub struct Gate {
    last: Option<u64>,
    waiting: bool,
    asked: bool,
    /// The stream's size from the engine's latest status.
    size: Option<(u32, u32)>,
    /// The size the running decoder took its codec config at; `None` until it has one.
    decoding: Option<(u32, u32)>,
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
        Self { last: None, waiting: true, asked: false, size: None, decoding: None }
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
            if self.decoding.is_none() {
                // With no status seen yet the size is unknown; the first one then replaces this
                // decoder, which costs a restart rather than a squashed picture.
                self.decoding = Some(self.size.unwrap_or((0, 0)));
            }
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

impl Gate {
    /// The stream's size from an engine status. `true` when the running decoder holds a stream of
    /// another size and must be replaced; the gate then waits for the codec config that follows
    /// the size change (the engine sends the status first), asking for one only if a picture
    /// arrives without it.
    pub fn resize(&mut self, width: u32, height: u32) -> bool {
        if width == 0 || height == 0 {
            return false;
        }
        self.size = Some((width, height));
        if self.decoding.is_none_or(|size| size == (width, height)) {
            return false;
        }
        self.decoding = None;
        self.waiting = true;
        self.asked = false;
        true
    }
}

pub struct Pixels {
    pub width: i32,
    pub height: i32,
    pub rgb: Vec<u8>,
}

pub async fn ppm<R: tokio::io::AsyncRead + Unpin>(read: &mut BufReader<R>) -> Result<Pixels, String> {
    async fn line<R: tokio::io::AsyncRead + Unpin>(read: &mut BufReader<R>) -> Result<String, String> {
        let mut value = String::new();
        let count = read.read_line(&mut value).await.map_err(|e| e.to_string())?;
        if count == 0 || value.len() > 64 {
            return Err("The video decoder stopped.".into());
        }
        Ok(value)
    }
    if line(read).await?.trim() != "P6" {
        return Err("The video decoder sent something other than an RGB picture.".into());
    }
    let size = line(read).await?;
    let mut sizes = size.split_whitespace();
    let mut dimension = || {
        sizes
            .next()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|n| *n > 0 && *n <= 4096)
            .ok_or("The video decoder reported an impossible picture size.")
    };
    let width = dimension()?;
    let height = dimension()?;
    if line(read).await?.trim() != "255" {
        return Err("The video decoder used an unsupported pixel depth.".into());
    }
    let mut rgb = vec![0; width * height * 3];
    read.read_exact(&mut rgb).await.map_err(|e| e.to_string())?;
    Ok(Pixels { width: width as i32, height: height as i32, rgb })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn decoder_frames_preserve_rotation_and_reject_bad_dimensions() {
        let data = b"P6\n2 1\n255\nabcdefP6\n1 2\n255\nghijkl";
        let mut reader = BufReader::new(&data[..]);
        let a = ppm(&mut reader).await.unwrap();
        let b = ppm(&mut reader).await.unwrap();
        assert_eq!((a.width, a.height, a.rgb), (2, 1, b"abcdef".to_vec()));
        assert_eq!((b.width, b.height, b.rgb), (1, 2, b"ghijkl".to_vec()));
        let mut reader = BufReader::new(&b"P6\n9000 9000\n255\n"[..]);
        assert!(ppm(&mut reader).await.is_err());
    }

    #[tokio::test]
    async fn a_picture_the_decoder_did_not_finish_is_an_error_not_a_frame() {
        for data in [
            &b"P3\n1 1\n255\n\0\0\0"[..],  // ASCII PPM, not raw RGB
            b"P6\n1 1\n65535\n\0\0\0\0\0\0", // 16-bit samples
            b"P6\n0 1\n255\n",              // zero-sized
            b"P6\n2\n255\n\0\0\0\0\0\0",    // one dimension
            b"P6\n2 1\n255\nabc",           // pixels cut off mid-picture
            b"",                            // decoder exited
        ] {
            let mut reader = BufReader::new(data);
            assert!(ppm(&mut reader).await.is_err(), "{:?}", String::from_utf8_lossy(data));
        }
    }

    #[test]
    fn the_decoder_line_is_the_measured_one() {
        let args = ffmpeg_args();
        let has = |pair: [&str; 2]| args.windows(2).any(|w| w == pair);
        assert!(has(["-threads", "1"]), "frame threading holds one picture per thread");
        assert!(has(["-fps_mode", "passthrough"]));
        // Input options must come before `-i` to apply to the decoder.
        let input = args.iter().position(|a| *a == "-i").unwrap();
        assert!(args.iter().position(|a| *a == "-threads").unwrap() < input);
        // Measured 4x worse than doing nothing; never add it back.
        assert!(!args.contains(&"nobuffer"));
    }

    #[test]
    fn pictures_get_a_delimiter_and_config_does_not() {
        let picture = [2u8, 0, 0, 0, 1, 0x65, 0xaa];
        let bytes = decoder_bytes(&picture).unwrap();
        assert_eq!(&bytes[..6], &picture[1..]);
        assert_eq!(&bytes[6..], &AUD);
        let config = [1u8, 0, 0, 0, 1, 0x67];
        assert_eq!(decoder_bytes(&config).unwrap(), config[1..].to_vec());
        assert!(decoder_bytes(&[0]).is_none());
        assert!(decoder_bytes(&[]).is_none());
    }

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
    fn a_new_stream_size_replaces_the_decoder_and_waits_for_its_config() {
        let mut gate = Gate::default();
        // The estimate, then the real size, both before any config: nothing to replace.
        assert!(!gate.resize(576, 1280));
        assert!(!gate.resize(576, 1264));
        assert_eq!(gate.admit(1, true), Admit::Write);
        assert_eq!(gate.admit(2, false), Admit::Write);
        // A status that repeats the size (a name, a state) keeps the decoder.
        assert!(!gate.resize(576, 1264));
        assert!(!gate.resize(0, 0));
        // The device rotated: a new decoder, fed from the config that follows.
        assert!(gate.resize(1264, 576));
        assert_eq!(gate.admit(3, true), Admit::Write);
        assert_eq!(gate.admit(4, false), Admit::Write);
        // Back to portrait; this time a picture slips in ahead of the config.
        assert!(gate.resize(576, 1264));
        assert_eq!(gate.admit(5, false), Admit::Reset);
        assert_eq!(gate.admit(6, true), Admit::Write);
        assert!(!gate.resize(576, 1264));
    }

    #[test]
    fn a_config_before_any_status_is_replaced_by_the_first_one() {
        let mut gate = Gate::default();
        assert_eq!(gate.admit(1, true), Admit::Write);
        assert!(gate.resize(576, 1264));
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
}
