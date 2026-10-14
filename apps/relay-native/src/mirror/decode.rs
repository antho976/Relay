//! The video half of the mirror: base64 packets in, YUV pictures out, through libavcodec on a
//! thread of its own (`avcodec.rs`).
//!
//! The packet gate, the access units and the colors are GTK-free and live in
//! `relay_client::mirror::decode`, with the measurements behind the decoder's settings.
use super::avcodec::{self, Decoder};
use super::trace::{self, trace};
use super::Control;
use crate::client::Notice;
use base64::Engine as _;
pub use avcodec::Picture;
use relay_client::mirror::decode::{split, Admit, Assembler, Gate, CONFIG};

/// Access units waiting for the decoder. A unit cannot be dropped (every later picture refers to
/// it), so a decoder that falls behind holds the pump back here, as a full pipe used to.
const UNITS: usize = 32;

/// One access unit for the decoder thread, with the packet that completed it.
struct Unit {
    mirror: i64,
    seq: u64,
    bytes: Vec<u8>,
}

/// Runs on the tokio runtime for one mirror session. Video packets go to the decoder thread;
/// pictures come back on `frames` (only the newest is kept — a slow paint skips pictures, never
/// queues them, so what it paints next is never older than what the decoder has); status
/// objects, resync requests and the reason the stream ended go to `control`, which is never
/// lossy. Returns when the engine reports a terminal state or anything breaks.
pub async fn run(
    notices: async_channel::Receiver<Notice>,
    frames: async_channel::Sender<Picture>,
    stale: async_channel::Receiver<Picture>,
    control: async_channel::Sender<Control>,
) {
    let result = pump(notices, frames, stale, &control).await;
    if let Err(message) = result {
        let _ = control.send(Control::Broken(message)).await;
    }
}

/// The decoder thread: one libavcodec context for the whole session, rotations included. It ends
/// when the pump drops its end of `units`, or the view stops taking pictures.
fn spawn_decoder(
    units: async_channel::Receiver<Unit>,
    frames: async_channel::Sender<Picture>,
    stale: async_channel::Receiver<Picture>,
    control: async_channel::Sender<Control>,
) -> Result<(), String> {
    let mut decoder = Decoder::new()?;
    std::thread::Builder::new()
        .name("mirror-decode".into())
        .spawn(move || {
            // A damaged unit leaves the pictures after it broken until the next key frame; ask
            // for one, once, until a picture decodes again.
            let mut asked = false;
            while let Ok(unit) = units.recv_blocking() {
                let started = trace::now();
                match decoder.decode(&unit.bytes, unit.mirror, unit.seq) {
                    Ok(Some(picture)) => {
                        asked = false;
                        if trace::on() {
                            let (probe, field) = picture.probe().map_or((-1, -1), |(bits, white)| (i64::from(bits), i64::from(white)));
                            trace!(stage = "dec", mono_ns = trace::monotonic(), mirror_id = unit.mirror, seq = unit.seq, in_ns = started, width = picture.width, height = picture.height, matrix = picture.color.matrix, full_range = picture.color.full_range, probe, field);
                        }
                        if frames.is_full() {
                            if let Ok(old) = stale.try_recv() {
                                trace!(stage = "stale", mono_ns = trace::monotonic(), mirror_id = old.mirror, seq = old.seq);
                            }
                        }
                        if frames.try_send(picture).is_err() && frames.is_closed() {
                            return;
                        }
                    }
                    Ok(None) => trace!(stage = "nopic", mono_ns = trace::monotonic(), mirror_id = unit.mirror, seq = unit.seq, in_ns = started),
                    Err(avcodec::Damaged(code)) => {
                        trace!(stage = "damaged", mono_ns = trace::monotonic(), mirror_id = unit.mirror, seq = unit.seq, in_ns = started, code);
                        if !asked {
                            asked = true;
                            let _ = control.send_blocking(Control::Resync);
                        }
                    }
                }
            }
        })
        .map(|_| ())
        .map_err(|e| format!("The video decoder could not start: {e}"))
}

async fn pump(
    notices: async_channel::Receiver<Notice>,
    frames: async_channel::Sender<Picture>,
    stale: async_channel::Receiver<Picture>,
    control: &async_channel::Sender<Control>,
) -> Result<(), String> {
    let (units, queued) = async_channel::bounded::<Unit>(UNITS);
    spawn_decoder(queued, frames, stale, control.clone())?;
    let mut gate = Gate::default();
    let mut assembler = Assembler::default();
    loop {
        match notices.recv().await {
            Ok(Notice::Frame(frame)) if frame.stream == "mirror" => {
                let mirror = frame.mirror_id.unwrap_or(0);
                if let Some(encoded) = frame.data.as_str() {
                    let packet = match base64::engine::general_purpose::STANDARD.decode(encoded) {
                        Ok(packet) => packet,
                        Err(e) => return Err(format!("The engine sent an unreadable video packet: {e}")),
                    };
                    // Every packet counts toward the gate's sequence, even one with nothing in it.
                    let config = packet.first().is_some_and(|flags| flags & CONFIG != 0);
                    match gate.admit(frame.seq, config) {
                        Admit::Write => {
                            let Some((flags, payload)) = split(&packet) else { continue };
                            trace!(stage = "in", mono_ns = trace::monotonic(), mirror_id = mirror, seq = frame.seq, flags, len = payload.len());
                            if let Some(bytes) = assembler.push(flags, payload) {
                                if units.send(Unit { mirror, seq: frame.seq, bytes }).await.is_err() {
                                    return Err("The video decoder stopped.".into());
                                }
                            }
                        }
                        Admit::Skip => trace!(stage = "gate", mono_ns = trace::monotonic(), mirror_id = mirror, seq = frame.seq, verdict = "skip"),
                        Admit::Reset => {
                            trace!(stage = "gate", mono_ns = trace::monotonic(), mirror_id = mirror, seq = frame.seq, verdict = "reset");
                            let _ = control.send(Control::Resync).await;
                        }
                    }
                } else {
                    let terminal = matches!(frame.data["state"].as_str(), Some("stopped" | "failed" | "lost"));
                    let _ = control.send(Control::Status(frame.data)).await;
                    if terminal {
                        return Ok(());
                    }
                }
            }
            Ok(Notice::Disconnected(error)) => return Err(format!("Relay's engine connection closed ({error}).")),
            Ok(_) => {}
            Err(_) => return Err("Relay's engine connection closed.".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_bus::envelope::Frame;
    use serde_json::{json, Value};

    fn frame(seq: u64, data: Value) -> Notice {
        Notice::Frame(Frame { v: 1, stream: "mirror".into(), session: None, run_id: None, mirror_id: Some(1), epoch: None, seq, data })
    }

    /// An H.264 stream from FFmpeg's test source as the scrcpy server sends it — one codec config
    /// packet, then one packet per picture — or `None` where there is no `ffmpeg` with libx264.
    /// (The tests alone use the command; Relay itself only needs libavcodec.)
    fn stream(size: &str) -> Option<Vec<Vec<u8>>> {
        let output = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", &format!("testsrc=size={size}:rate=30")])
            .args(["-frames:v", "10", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-bf", "0", "-f", "h264", "pipe:1"])
            .output()
            .ok()?;
        (output.status.success() && !output.stdout.is_empty()).then(|| packets(&output.stdout))
    }

    /// Split an Annex B stream into scrcpy's packets: `[flags][bytes]`, the SPS and PPS in a
    /// config packet, and every other NAL unit with the picture it leads up to.
    fn packets(stream: &[u8]) -> Vec<Vec<u8>> {
        let mut starts = Vec::new();
        let mut i = 0;
        while i + 3 <= stream.len() {
            if stream[i..i + 3] == [0, 0, 1] {
                let start = if i > 0 && stream[i - 1] == 0 { i - 1 } else { i };
                starts.push((start, i + 3));
                i += 3;
            } else {
                i += 1;
            }
        }
        let mut out = Vec::new();
        let (mut config, mut pending) = (vec![CONFIG], vec![0u8]);
        for (n, &(start, body)) in starts.iter().enumerate() {
            let end = starts.get(n + 1).map_or(stream.len(), |next| next.0);
            let nal = &stream[start..end];
            match stream[body] & 0x1f {
                7 | 8 => config.extend_from_slice(nal),
                9 => {}
                kind @ (1 | 5) => {
                    if config.len() > 1 {
                        out.push(std::mem::replace(&mut config, vec![CONFIG]));
                    }
                    if kind == 5 {
                        pending[0] = relay_client::mirror::decode::KEY;
                    }
                    pending.extend_from_slice(nal);
                    out.push(std::mem::replace(&mut pending, vec![0]));
                }
                _ => pending.extend_from_slice(nal),
            }
        }
        out
    }

    async fn picture_of(pictures: &async_channel::Receiver<Picture>, size: (i32, i32)) -> bool {
        let wait = async {
            while let Ok(picture) = pictures.recv().await {
                if (picture.width, picture.height) == size {
                    return true;
                }
            }
            false
        };
        tokio::time::timeout(std::time::Duration::from_secs(10), wait).await.unwrap_or(false)
    }

    #[test]
    fn a_stream_splits_into_one_config_and_a_packet_per_picture() {
        let Some(packets) = stream("64x64") else {
            eprintln!("skipped: no ffmpeg with libx264");
            return;
        };
        assert_eq!(packets.len(), 11);
        assert_eq!(packets[0][0], CONFIG);
        assert_eq!(packets[1][0], relay_client::mirror::decode::KEY);
        assert!(packets[2..].iter().all(|packet| packet[0] == 0));
    }

    #[tokio::test]
    async fn a_rotated_stream_comes_back_at_its_own_size() {
        if let Err(missing) = avcodec::available() {
            eprintln!("skipped: {missing}");
            return;
        }
        let (Some(portrait), Some(landscape)) = (stream("320x640"), stream("640x320")) else {
            eprintln!("skipped: no ffmpeg with libx264");
            return;
        };
        let (notices, incoming) = async_channel::bounded(16);
        let (frames, pictures) = async_channel::bounded(1);
        let (control, controls) = async_channel::unbounded();
        let pump = tokio::spawn(run(incoming, frames, pictures.clone(), control));
        let packet = |bytes: &[u8]| json!(base64::engine::general_purpose::STANDARD.encode(bytes));
        let mut seq = 0;
        notices.send(frame(0, json!({"state":"running","width":320,"height":640}))).await.unwrap();
        for bytes in &portrait {
            seq += 1;
            notices.send(frame(seq, packet(bytes))).await.unwrap();
        }
        assert!(picture_of(&pictures, (320, 640)).await, "no portrait picture");
        // The device turned: the engine reports the new size, then the new stream's config —
        // into the same decoder, which must not keep the first stream's size.
        notices.send(frame(0, json!({"state":"running","width":640,"height":320}))).await.unwrap();
        for bytes in &landscape {
            seq += 1;
            notices.send(frame(seq, packet(bytes))).await.unwrap();
        }
        assert!(picture_of(&pictures, (640, 320)).await, "the landscape stream came back at another size");
        notices.send(frame(0, json!({"state":"stopped","width":640,"height":320}))).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), pump).await.unwrap().unwrap();
        while let Ok(message) = controls.try_recv() {
            assert!(!matches!(message, Control::Broken(_) | Control::Resync), "the pipeline broke or lost its place");
        }
    }

    #[tokio::test]
    async fn every_picture_comes_out_as_its_own_packet_goes_in() {
        // The low-delay settings: no picture is held back waiting for the next packet, so the
        // last picture before a still screen is shown at once rather than never.
        if avcodec::available().is_err() {
            return;
        }
        let Some(packets) = stream("128x256") else { return };
        let mut decoder = Decoder::new().unwrap();
        let mut assembler = Assembler::default();
        let mut pictures = 0;
        for (seq, packet) in packets.iter().enumerate() {
            let (flags, payload) = split(packet).unwrap();
            if let Some(unit) = assembler.push(flags, payload) {
                let picture = decoder.decode(&unit, 1, seq as u64).unwrap().expect("a picture per access unit");
                assert_eq!((picture.width, picture.height, picture.seq), (128, 256, seq as u64));
                assert_eq!(picture.format, gtk4::gdk::MemoryFormat::G8B8R8420);
                assert_eq!(picture.planes.len(), 3);
                pictures += 1;
            }
        }
        assert_eq!(pictures, 10);
    }
}
