//! The video half of the mirror: base64 packets in, RGB pictures out, through one FFmpeg child.
//!
//! The decoder's command line, the packet gate and the picture reader are GTK-free and live in
//! `relay_client::mirror::decode`, with the measurements behind the FFmpeg line.
use super::Control;
use crate::client::Notice;
use base64::Engine as _;
pub use relay_client::mirror::decode::Pixels;
use relay_client::mirror::decode::{decoder_bytes, ffmpeg_args, ppm, Admit, Gate};
use std::process::Stdio;
use tokio::io::{AsyncWriteExt, BufReader};

/// Runs on the tokio runtime for one mirror session. Video packets go to FFmpeg; pictures come
/// back on `frames` (only the newest is kept — a slow paint skips pictures, never queues them,
/// so what it paints next is never older than what the decoder has); status objects, resync
/// requests and the reason the stream ended go to `control`, which is never lossy. Returns when
/// the engine reports a terminal state or anything breaks.
pub async fn run(
    notices: async_channel::Receiver<Notice>,
    frames: async_channel::Sender<Pixels>,
    stale: async_channel::Receiver<Pixels>,
    control: async_channel::Sender<Control>,
) {
    let result = pump(notices, frames, stale, &control).await;
    if let Err(message) = result {
        let _ = control.send(Control::Broken(message)).await;
    }
}

/// One FFmpeg child and the task reading its pictures. Replaced whole when the stream changes
/// size (see `Gate::resize`).
struct Decoder {
    child: tokio::process::Child,
    input: tokio::process::ChildStdin,
    pictures: tokio::task::JoinHandle<Result<(), String>>,
}

impl Decoder {
    fn spawn(frames: &async_channel::Sender<Pixels>, stale: &async_channel::Receiver<Pixels>) -> Result<Self, String> {
        let mut child = tokio::process::Command::new("ffmpeg")
            .args(ffmpeg_args())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("{FFMPEG_MISSING} ({e})"))?;
        let input = child.stdin.take().ok_or("The video decoder has no input.")?;
        let output = child.stdout.take().ok_or("The video decoder has no output.")?;
        widen(&input);
        widen(&output);
        let (frames, stale) = (frames.clone(), stale.clone());
        let pictures = tokio::spawn(async move {
            let mut output = BufReader::with_capacity(PIPE_BYTES, output);
            loop {
                let picture = ppm(&mut output).await?;
                if frames.is_full() {
                    let _ = stale.try_recv();
                }
                if frames.try_send(picture).is_err() && frames.is_closed() {
                    return Ok(());
                }
            }
        });
        Ok(Self { child, input, pictures })
    }

    async fn end(mut self) {
        self.pictures.abort();
        let _ = self.child.kill().await;
    }
}

/// A pipe's default 64 KiB holds a thirtieth of one picture, so every frame used to cost the
/// decoder and this reader dozens of wake-ups each; a key frame going in, several more. 1 MiB is
/// the most an unprivileged process may ask for (`/proc/sys/fs/pipe-max-size`); a refusal only
/// leaves the default.
const PIPE_BYTES: usize = 1 << 20;

fn widen(pipe: &impl std::os::fd::AsRawFd) {
    unsafe {
        libc::fcntl(pipe.as_raw_fd(), libc::F_SETPIPE_SZ, PIPE_BYTES as libc::c_int);
    }
}

async fn pump(
    notices: async_channel::Receiver<Notice>,
    frames: async_channel::Sender<Pixels>,
    stale: async_channel::Receiver<Pixels>,
    control: &async_channel::Sender<Control>,
) -> Result<(), String> {
    let mut decoder = Decoder::spawn(&frames, &stale)?;
    let mut gate = Gate::default();
    let result = loop {
        let notice = tokio::select! {
            notice = notices.recv() => notice,
            ended = &mut decoder.pictures => break ended.map_err(|e| e.to_string()).and_then(|ended| ended),
        };
        match notice {
            Ok(Notice::Frame(frame)) if frame.stream == "mirror" => {
                if let Some(encoded) = frame.data.as_str() {
                    let packet = match base64::engine::general_purpose::STANDARD.decode(encoded) {
                        Ok(packet) => packet,
                        Err(e) => break Err(format!("The engine sent an unreadable video packet: {e}")),
                    };
                    let config = packet.first().is_some_and(|flags| flags & 1 != 0);
                    match gate.admit(frame.seq, config) {
                        Admit::Write => {
                            if let Some(bytes) = decoder_bytes(&packet) {
                                if decoder.input.write_all(&bytes).await.is_err() {
                                    break Err("The video decoder stopped.".into());
                                }
                            }
                        }
                        Admit::Skip => {}
                        Admit::Reset => {
                            let _ = control.send(Control::Resync).await;
                        }
                    }
                } else {
                    let size = |key: &str| frame.data[key].as_u64().unwrap_or(0) as u32;
                    if gate.resize(size("width"), size("height")) {
                        match Decoder::spawn(&frames, &stale) {
                            Ok(fresh) => std::mem::replace(&mut decoder, fresh).end().await,
                            Err(message) => break Err(message),
                        }
                    }
                    let terminal = matches!(frame.data["state"].as_str(), Some("stopped" | "failed" | "lost"));
                    let _ = control.send(Control::Status(frame.data)).await;
                    if terminal {
                        break Ok(());
                    }
                }
            }
            Ok(Notice::Disconnected(error)) => break Err(format!("Relay's engine connection closed ({error}).")),
            Ok(_) => {}
            Err(_) => break Err("Relay's engine connection closed.".into()),
        }
    };
    decoder.end().await;
    result
}

pub const FFMPEG_MISSING: &str = "FFmpeg is needed to show the device screen. Install the ffmpeg package, then retry.";

#[cfg(test)]
mod tests {
    use super::*;
    use relay_bus::envelope::Frame;
    use serde_json::{json, Value};

    fn frame(seq: u64, data: Value) -> Notice {
        Notice::Frame(Frame { v: 1, stream: "mirror".into(), session: None, run_id: None, mirror_id: Some(1), epoch: None, seq, data })
    }

    /// A whole H.264 stream from FFmpeg's test source, or `None` where there is no encoder.
    fn stream(size: &str) -> Option<Vec<u8>> {
        let output = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", &format!("testsrc=size={size}:rate=30")])
            .args(["-frames:v", "10", "-c:v", "libx264", "-bf", "0", "-f", "h264", "pipe:1"])
            .output()
            .ok()?;
        (output.status.success() && !output.stdout.is_empty()).then_some(output.stdout)
    }

    async fn picture_of(pictures: &async_channel::Receiver<Pixels>, size: (i32, i32)) -> bool {
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

    #[tokio::test]
    async fn a_rotated_stream_comes_back_at_its_own_size() {
        let (Some(portrait), Some(landscape)) = (stream("320x640"), stream("640x320")) else {
            eprintln!("skipped: no ffmpeg with libx264");
            return;
        };
        let (notices, incoming) = async_channel::bounded(16);
        let (frames, pictures) = async_channel::bounded(1);
        let (control, controls) = async_channel::unbounded();
        let pump = tokio::spawn(run(incoming, frames, pictures.clone(), control));
        let packet = |bytes: &[u8]| {
            let mut packet = vec![1u8];
            packet.extend_from_slice(bytes);
            json!(base64::engine::general_purpose::STANDARD.encode(packet))
        };
        notices.send(frame(0, json!({"state":"running","width":320,"height":640}))).await.unwrap();
        notices.send(frame(1, packet(&portrait))).await.unwrap();
        assert!(picture_of(&pictures, (320, 640)).await, "no portrait picture");
        // The device turned: the engine reports the new size, then the new stream's config.
        notices.send(frame(0, json!({"state":"running","width":640,"height":320}))).await.unwrap();
        notices.send(frame(2, packet(&landscape))).await.unwrap();
        // FFmpeg alone kept scaling these to 320×640.
        assert!(picture_of(&pictures, (640, 320)).await, "the landscape stream came back at another size");
        notices.send(frame(0, json!({"state":"stopped","width":640,"height":320}))).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), pump).await.unwrap().unwrap();
        while let Ok(message) = controls.try_recv() {
            assert!(!matches!(message, Control::Broken(_) | Control::Resync), "the pipeline broke or lost its place");
        }
    }
}
