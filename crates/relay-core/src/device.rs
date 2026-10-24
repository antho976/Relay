//! Live Phase 10 device streams. Runtime transports are bounded and exist only while a
//! mirror or run is active; durable run metadata lives in `device_runs`.

use relay_bus::types::Id;
use std::collections::VecDeque;
use std::io::{self, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, watch};

const RUN_LOG_LINES: usize = 512;

#[derive(Debug, Default)]
pub struct DeviceWatchRuntime {
    stop: AtomicBool,
    child: Mutex<Option<Child>>,
}

#[derive(Debug, Default)]
pub struct DeviceWatchState {
    pub clients: usize,
    pub runtime: Option<Arc<DeviceWatchRuntime>>,
}

impl DeviceWatchRuntime {
    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
    pub fn install(&self, mut child: Child) -> bool {
        let mut slot = self.child.lock().unwrap();
        if self.stopped() {
            // Reaped too: the orphan reaper skips the engine's own children.
            let _ = child.kill();
            let _ = child.wait();
            return false;
        }
        *slot = Some(child);
        true
    }
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(child) = self.child.lock().unwrap().as_mut() {
            let _ = child.kill();
        }
    }
    /// Reap the watcher once its stream ended. The child leaves the slot before the wait, so a
    /// `stop()` (`device.watch` off, under the store lock) never queues behind it; it is killed
    /// first, since an ended stream does not prove `adb track-devices` exited.
    pub fn finish(&self) {
        let child = self.child.lock().unwrap().take();
        if let Some(mut child) = child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// The vendored jar's file name, wherever it is installed.
const MIRROR_SERVER_FILE: &str = "scrcpy-server-v4.1";

/// Where the scrcpy server jar is looked for, first existing file wins: `$RELAY_SCRCPY_SERVER`,
/// then `~/.local/share/relay-v4/scrcpy-server-v4.1` (where an installed engine — `cargo
/// install`, the relay-remote service — finds it once copied there), then the checkout this
/// binary was built from, which is all a development build needs.
fn mirror_server_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("RELAY_SCRCPY_SERVER").filter(|path| !path.is_empty()) {
        candidates.push(PathBuf::from(path));
    }
    if let Some(base) = directories::BaseDirs::new() {
        candidates.push(base.data_local_dir().join("relay-v4").join(MIRROR_SERVER_FILE));
    }
    candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/relay-native/resources").join(MIRROR_SERVER_FILE));
    candidates
}

/// Why there is no server jar to push: a typed code for `device.mirror.*` and the words.
pub type MirrorServerError = (&'static str, String);

/// The scrcpy server jar to push, read and checked against [`crate::mirror::SCRCPY_SERVER_SHA256`]
/// on every call. It runs on the device with shell privileges, so a jar that differs from the
/// pinned one — changed in the checkout by a branch switch or a merge nobody could review —
/// refuses the mirror instead of being pushed. Hashing ~720 KB takes a few milliseconds, so
/// callers keep it off the store lock.
pub fn mirror_server() -> Result<PathBuf, MirrorServerError> {
    let Some(path) = mirror_server_candidates().into_iter().find(|path| path.is_file()) else {
        return Err(("device.mirror_server_missing", format!(
            "the bundled scrcpy server is missing; set RELAY_SCRCPY_SERVER or copy {MIRROR_SERVER_FILE} into ~/.local/share/relay-v4/"
        )));
    };
    verify_mirror_server(&path)?;
    Ok(path)
}

fn verify_mirror_server(path: &Path) -> Result<(), MirrorServerError> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path)
        .map_err(|error| ("device.mirror_server_missing", format!("cannot read the scrcpy server {}: {error}", path.display())))?;
    let digest: String = Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect();
    if digest != crate::mirror::SCRCPY_SERVER_SHA256 {
        return Err(("device.mirror_server_mismatch", format!(
            "{} is not the pinned scrcpy {} server (sha256 {digest}); Relay will not push it to a device",
            path.display(), crate::mirror::SCRCPY_VERSION,
        )));
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct MirrorChunk {
    pub seq: u64,
    pub data: Arc<Vec<u8>>,
}

#[derive(Debug, Default)]
struct MirrorControl {
    socket: Option<TcpStream>,
    pending: VecDeque<Vec<u8>>,
    /// Why the control channel died, once a write to it failed. Input has nowhere to go after
    /// that: it is refused, never queued (the queue drains only when the socket is installed).
    lost: Option<String>,
}

/// How long one control message may take to leave. A device that stops reading its control
/// socket would otherwise park the writer in `write_all` with the control lock held.
const CONTROL_WRITE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Where a mirror is in its life. `Stopped`, `Failed` and `Lost` are terminal: the
/// runtime never leaves them, and the socket door ends the window's stream on them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MirrorState {
    #[default]
    Starting,
    Running,
    /// Someone asked for it to end (`device.mirror.stop`, or the window went away).
    Stopped,
    /// The transport or the server failed while the device was still there.
    Failed,
    /// The device itself went away — unplugged, rebooted, adb lost it.
    Lost,
}

impl MirrorState {
    pub fn is_terminal(self) -> bool {
        matches!(self, MirrorState::Stopped | MirrorState::Failed | MirrorState::Lost)
    }
}

/// What a window needs to draw the mirror's state: the live picture size, the device's
/// own name once the handshake gave it, and on a terminal state the typed code and the
/// message to show next to Retry. Forwarded verbatim as a `mirror` frame whose `data` is
/// this object (video frames carry a base64 string instead).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct MirrorStatus {
    pub state: MirrorState,
    pub width: u32,
    pub height: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

fn pack_size(width: u32, height: u32) -> u64 {
    (u64::from(width) << 32) | u64::from(height)
}

#[derive(Debug)]
pub struct MirrorRuntime {
    pub id: Id,
    pub device: String,
    pub max_size: u32,
    pub bitrate: u32,
    pub scid: u32,
    pub adb: String,
    stop: AtomicBool,
    child: Mutex<Option<Child>>,
    video: Mutex<Option<TcpStream>>,
    control: Mutex<MirrorControl>,
    /// A clone of the control socket, under its own lock, so `stop()` can shut it down — and
    /// so fail a writer blocked inside `send_control` — without waiting for the control lock.
    control_shutdown: Mutex<Option<TcpStream>>,
    /// The local `tcp:` port of the adb forward this mirror opened, until someone removes it:
    /// the worker on its way out, or the engine's own shutdown when it goes first.
    forward: Mutex<Option<u16>>,
    /// The last packet's number. Windows detect a lagged subscription by a gap in it. There is
    /// no catch-up ring: the one attach comes before the worker's first packet, and a late
    /// joiner recovers with RESET_VIDEO (fresh config + key frame), not a replay.
    seq: AtomicU64,
    tx: broadcast::Sender<MirrorChunk>,
    /// The stream's current picture size, `width << 32 | height`. Starts at the estimate
    /// from `wm size` and follows every session packet, so tap/swipe coordinates are
    /// checked and scaled against what the device is actually sending — after a rotation
    /// too, which is exactly when the estimate is wrong.
    size: AtomicU64,
    status: watch::Sender<MirrorStatus>,
}

pub struct MirrorRuntimeConfig {
    pub id: Id,
    pub device: String,
    pub width: u32,
    pub height: u32,
    pub max_size: u32,
    pub bitrate: u32,
    pub scid: u32,
    pub adb: String,
}

impl MirrorRuntime {
    pub fn new(config: MirrorRuntimeConfig) -> Arc<Self> {
        let MirrorRuntimeConfig {
            id,
            device,
            width,
            height,
            max_size,
            bitrate,
            scid,
            adb,
        } = config;
        let (tx, _) = broadcast::channel(256);
        let (status, _) = watch::channel(MirrorStatus { width, height, ..MirrorStatus::default() });
        Arc::new(Self {
            id,
            device,
            max_size,
            bitrate,
            scid,
            adb,
            stop: AtomicBool::new(false),
            child: Mutex::new(None),
            video: Mutex::new(None),
            control: Mutex::new(MirrorControl::default()),
            control_shutdown: Mutex::new(None),
            forward: Mutex::new(None),
            seq: AtomicU64::new(0),
            tx,
            size: AtomicU64::new(pack_size(width, height)),
            status,
        })
    }
    /// The picture size the device is sending right now.
    pub fn size(&self) -> (u32, u32) {
        let packed = self.size.load(Ordering::Relaxed);
        ((packed >> 32) as u32, packed as u32)
    }
    /// A session packet: the stream changed size (start, rotation, resize). Updates the
    /// size inputs are checked against and, while running, the status windows draw from.
    pub fn set_size(&self, width: u32, height: u32) {
        self.size.store(pack_size(width, height), Ordering::Relaxed);
        self.status.send_if_modified(|status| {
            if status.state.is_terminal() || (status.width, status.height) == (width, height) {
                return false;
            }
            status.width = width;
            status.height = height;
            true
        });
    }
    pub fn status(&self) -> MirrorStatus {
        self.status.borrow().clone()
    }
    /// A receiver that already holds the current status, so an attach that arrives after a
    /// fast failure still sees it rather than waiting for a change that already happened.
    pub fn watch_status(&self) -> watch::Receiver<MirrorStatus> {
        self.status.subscribe()
    }
    /// The handshake finished: the device's name is known and frames are about to flow.
    pub fn set_running(&self, name: String) {
        let (width, height) = self.size();
        self.status.send_if_modified(|status| {
            if status.state.is_terminal() {
                return false;
            }
            *status = MirrorStatus { state: MirrorState::Running, width, height, name: Some(name), code: None, message: None };
            true
        });
    }
    /// Enter a terminal state. The first one wins — a stop that races a failure must not
    /// relabel a device loss as a clean stop, or the reverse.
    pub fn finish(&self, state: MirrorState, code: Option<String>, message: Option<String>) -> bool {
        debug_assert!(state.is_terminal());
        self.status.send_if_modified(|status| {
            if status.state.is_terminal() {
                return false;
            }
            status.state = state;
            status.code = code;
            status.message = message;
            true
        })
    }
    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
    pub fn set_child(&self, child: Child) {
        *self.child.lock().unwrap() = Some(child);
    }
    pub fn take_child(&self) -> Option<Child> {
        self.child.lock().unwrap().take()
    }
    pub fn set_video(&self, video: TcpStream) {
        *self.video.lock().unwrap() = Some(video);
    }
    pub fn install_control(&self, mut socket: TcpStream) -> io::Result<()> {
        socket.set_write_timeout(Some(CONTROL_WRITE_TIMEOUT))?;
        *self.control_shutdown.lock().unwrap() = Some(socket.try_clone()?);
        if self.stopped() {
            let _ = socket.shutdown(std::net::Shutdown::Both);
            return Ok(());
        }
        let mut control = self.control.lock().unwrap();
        while let Some(bytes) = control.pending.pop_front() {
            socket.write_all(&bytes)?;
        }
        control.socket = Some(socket);
        Ok(())
    }
    pub fn send_control(&self, bytes: Vec<u8>) -> io::Result<()> {
        let mut control = self.control.lock().unwrap();
        if let Some(socket) = control.socket.as_mut() {
            let written = socket.write_all(&bytes);
            if let Err(error) = &written {
                // A write that timed out may have left half a message on the wire; anything
                // sent after it would be parsed from the middle. The control channel is done,
                // and a mirror nobody can touch is no mirror: the video goes too, which ends
                // the worker's read and lets it report the loss (`control_lost`).
                if let Some(socket) = control.socket.take() {
                    let _ = socket.shutdown(std::net::Shutdown::Both);
                }
                control.lost = Some(error.to_string());
                if let Some(video) = self.video.lock().unwrap().take() {
                    let _ = video.shutdown(std::net::Shutdown::Both);
                }
            }
            written
        } else if self.stopped() {
            Err(io::Error::new(io::ErrorKind::NotConnected, "the mirror has stopped"))
        } else if let Some(lost) = &control.lost {
            Err(io::Error::new(io::ErrorKind::NotConnected, format!("the mirror's control channel was lost: {lost}")))
        } else {
            if control.pending.len() == 128 {
                control.pending.pop_front();
            }
            control.pending.push_back(bytes);
            Ok(())
        }
    }
    /// Why the control channel died, if a write to it failed (see `send_control`).
    pub fn control_lost(&self) -> Option<String> {
        self.control.lock().unwrap().lost.clone()
    }
    pub fn set_forward(&self, port: u16) {
        *self.forward.lock().unwrap() = Some(port);
    }
    /// The forward's port, for whoever removes it; `None` once someone has.
    pub fn take_forward(&self) -> Option<u16> {
        self.forward.lock().unwrap().take()
    }
    pub fn push(&self, data: Vec<u8>) {
        if data.is_empty() || self.stopped() {
            return;
        }
        // One writer (the worker), so the counter needs no lock to stay in send order.
        let seq = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.tx.send(MirrorChunk { seq, data: Arc::new(data) });
    }
    /// Subscribe to the stream from the next packet on. Nothing is replayed (see `seq`), so
    /// every packet the receiver yields is new.
    pub fn attach(&self) -> broadcast::Receiver<MirrorChunk> {
        self.tx.subscribe()
    }
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(video) = self.video.lock().unwrap().take() {
            let _ = video.shutdown(std::net::Shutdown::Both);
        }
        // Shut the clone down first: that fails a `send_control` stuck in a write, which is what
        // lets the control lock below be taken at all.
        if let Some(control) = self.control_shutdown.lock().unwrap().take() {
            let _ = control.shutdown(std::net::Shutdown::Both);
        }
        if let Some(control) = self.control.lock().unwrap().socket.take() {
            let _ = control.shutdown(std::net::Shutdown::Both);
        }
        if let Some(child) = self.child.lock().unwrap().as_mut() {
            let _ = child.kill();
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub seq: u64,
    pub line: Arc<String>,
}

#[derive(Debug)]
struct LogBuffer {
    seq: u64,
    lines: VecDeque<LogLine>,
}

/// How long a stopped run's process group has to exit on SIGTERM before it is SIGKILLed.
const RUN_STOP_GRACE: std::time::Duration = std::time::Duration::from_secs(3);

#[derive(Debug)]
pub struct RunRuntime {
    pub id: Id,
    stop: AtomicBool,
    /// The running child, and whether it leads its own process group. A run command is a
    /// shell (`fish -lc`) whose real work — the Gradle client, `adb install` — runs in its
    /// children, so stopping it has to reach the whole group, not just the shell.
    child: Mutex<Option<(Child, bool)>>,
    /// The checkout this run is fast-forwarding to its upstream, from the moment `device.run`
    /// records it until the sync is over. A build in that checkout waits for it (D152).
    syncing: Mutex<Option<PathBuf>>,
    buffer: Mutex<LogBuffer>,
    tx: broadcast::Sender<LogLine>,
}

impl RunRuntime {
    pub fn new(id: Id) -> Arc<Self> {
        let (tx, _) = broadcast::channel(RUN_LOG_LINES);
        Arc::new(Self {
            id,
            stop: AtomicBool::new(false),
            child: Mutex::new(None),
            syncing: Mutex::new(None),
            buffer: Mutex::new(LogBuffer {
                seq: 0,
                lines: VecDeque::new(),
            }),
            tx,
        })
    }
    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
    /// Track a child that is a single process (`adb logcat`).
    pub fn set_child(&self, child: Child) {
        self.track(child, false);
    }
    /// Track a child spawned with `process_group(0)`: stopping signals its whole group.
    pub fn set_group_child(&self, child: Child) {
        self.track(child, true);
    }
    fn track(&self, child: Child, group: bool) {
        let mut slot = self.child.lock().unwrap();
        *slot = Some((child, group));
        // A stop that landed before the child existed found nothing to signal.
        if self.stopped() {
            if let Some((child, group)) = slot.as_mut() {
                terminate(child, *group);
            }
        }
    }
    pub fn take_child(&self) -> Option<Child> {
        self.child.lock().unwrap().take().map(|(child, _)| child)
    }
    /// The run will (or does) sync `root`; `None` once it is done with it.
    pub fn set_syncing(&self, root: Option<PathBuf>) {
        *self.syncing.lock().unwrap() = root;
    }
    pub fn syncing(&self, root: &Path) -> bool {
        self.syncing.lock().unwrap().as_deref() == Some(root)
    }
    pub fn push(&self, line: impl Into<String>) {
        if self.stopped() {
            return;
        }
        let value = {
            let mut state = self.buffer.lock().unwrap();
            state.seq += 1;
            let value = LogLine {
                seq: state.seq,
                line: Arc::new(line.into()),
            };
            state.lines.push_back(value.clone());
            while state.lines.len() > RUN_LOG_LINES {
                state.lines.pop_front();
            }
            value
        };
        let _ = self.tx.send(value);
    }
    pub fn attach(&self) -> (u64, Vec<LogLine>, broadcast::Receiver<LogLine>) {
        let rx = self.tx.subscribe();
        let state = self.buffer.lock().unwrap();
        (state.seq, state.lines.iter().cloned().collect(), rx)
    }
    /// Stop the run. Never waits: `device.run.stop` calls this with the store locked, so the
    /// group's SIGTERM grace runs out on its own thread. The worker reaps the child.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some((child, group)) = self.child.lock().unwrap().as_mut() {
            terminate(child, *group);
        }
    }
}

fn terminate(child: &mut Child, group: bool) {
    if !group {
        let _ = child.kill();
        return;
    }
    let pgid = child.id() as i32;
    unsafe { libc::kill(-pgid, libc::SIGTERM); }
    let _ = std::thread::Builder::new().name("run-stop".into()).spawn(move || {
        std::thread::sleep(RUN_STOP_GRACE);
        unsafe { libc::kill(-pgid, libc::SIGKILL); }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopping_a_group_child_ends_its_grandchildren_too() {
        use std::os::unix::process::CommandExt;
        let runtime = RunRuntime::new(1);
        let dir = tempfile::tempdir().unwrap();
        let pidfile = dir.path().join("grandchild");
        // The shell forks `sleep` and waits on it, as fish does with `./gradlew`.
        let child = std::process::Command::new("sh")
            .args(["-c", &format!("sleep 30 & echo $! > {}; wait", pidfile.display())])
            .process_group(0)
            .spawn()
            .unwrap();
        runtime.set_group_child(child);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let grandchild = loop {
            if let Some(pid) = std::fs::read_to_string(&pidfile).ok().and_then(|text| text.trim().parse::<i32>().ok()) {
                break pid;
            }
            assert!(std::time::Instant::now() < deadline, "the shell never started its child");
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        runtime.stop();
        let _ = runtime.take_child().unwrap().wait();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while unsafe { libc::kill(grandchild, 0) } == 0 {
            // Reaped by init once orphaned; until then a zombie still answers kill(0).
            let zombie = std::fs::read_to_string(format!("/proc/{grandchild}/stat")).map(|s| s.contains(") Z ")).unwrap_or(true);
            if zombie { break; }
            assert!(std::time::Instant::now() < deadline, "the grandchild outlived the stop");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn a_child_set_after_stop_is_stopped_at_once() {
        let runtime = RunRuntime::new(1);
        runtime.stop();
        let child = std::process::Command::new("sleep").arg("30").spawn().unwrap();
        runtime.set_child(child);
        let status = runtime.take_child().unwrap().wait().unwrap();
        assert!(!status.success());
    }

    #[test]
    fn a_watch_stop_never_waits_behind_finish() {
        let runtime = Arc::new(DeviceWatchRuntime::default());
        assert!(runtime.install(std::process::Command::new("sleep").arg("30").spawn().unwrap()));
        let finishing = runtime.clone();
        let started = std::time::Instant::now();
        let finisher = std::thread::spawn(move || finishing.finish());
        std::thread::sleep(std::time::Duration::from_millis(50));
        runtime.stop();
        finisher.join().unwrap();
        assert!(started.elapsed() < std::time::Duration::from_secs(5), "finish held the child for {:?}", started.elapsed());
    }

    #[test]
    fn a_mirror_stop_fails_a_stalled_control_write_instead_of_waiting_for_it() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        // Accepted and never read: the "device" has stopped reading its control socket.
        let (_server, _) = listener.accept().unwrap();
        let runtime = MirrorRuntime::new(MirrorRuntimeConfig {
            id: 1, device: "S".into(), width: 1080, height: 2400,
            max_size: 1600, bitrate: 8_000_000, scid: 1, adb: "adb".into(),
        });
        runtime.install_control(client).unwrap();
        let writer = runtime.clone();
        let sending = std::thread::spawn(move || writer.send_control(vec![0u8; 64 * 1024 * 1024]));
        std::thread::sleep(std::time::Duration::from_millis(200));
        let started = std::time::Instant::now();
        runtime.stop();
        assert!(started.elapsed() < std::time::Duration::from_secs(1), "stop waited {:?} for the writer", started.elapsed());
        assert!(sending.join().unwrap().is_err());
        assert!(runtime.send_control(vec![1]).is_err(), "a stopped mirror takes no more input");
    }

    /// A failed control write ends the mirror instead of leaving it streaming with input that
    /// goes nowhere: the video is shut so the worker reports it, and later input is refused.
    #[test]
    fn a_lost_control_channel_refuses_input_and_ends_the_stream() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (_server, _) = listener.accept().unwrap();
        let video = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut video_peer, _) = listener.accept().unwrap();
        let runtime = MirrorRuntime::new(MirrorRuntimeConfig {
            id: 1, device: "S".into(), width: 1080, height: 2400,
            max_size: 1600, bitrate: 8_000_000, scid: 1, adb: "adb".into(),
        });
        runtime.set_video(video.try_clone().unwrap());
        let breaker = client.try_clone().unwrap();
        runtime.install_control(client).unwrap();
        assert!(runtime.send_control(vec![1]).is_ok());
        breaker.shutdown(std::net::Shutdown::Write).unwrap();
        assert!(runtime.send_control(vec![2]).is_err(), "a write after the socket broke fails");
        assert!(runtime.control_lost().is_some());
        let error = runtime.send_control(vec![3]).unwrap_err();
        assert!(error.to_string().contains("control channel was lost"), "{error}");
        assert!(runtime.control.lock().unwrap().pending.is_empty(), "nothing is queued for a socket that will never come");
        // The worker's video read ends: the far side sees the stream closed.
        video_peer.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut byte = [0u8; 1];
        assert_eq!(std::io::Read::read(&mut video_peer, &mut byte).unwrap_or(0), 0);
        let mut read_side = video;
        assert_eq!(std::io::Read::read(&mut read_side, &mut byte).unwrap_or(0), 0, "the worker's video read ends");
    }

    #[test]
    fn only_the_pinned_scrcpy_server_is_pushed() {
        let vendored = mirror_server_candidates().pop().unwrap();
        assert!(verify_mirror_server(&vendored).is_ok(), "the vendored jar no longer matches SCRCPY_SERVER_SHA256");
        let dir = tempfile::tempdir().unwrap();
        let tampered = dir.path().join(MIRROR_SERVER_FILE);
        let mut bytes = std::fs::read(&vendored).unwrap();
        bytes[0] ^= 1;
        std::fs::write(&tampered, bytes).unwrap();
        let (code, message) = verify_mirror_server(&tampered).unwrap_err();
        assert_eq!(code, "device.mirror_server_mismatch");
        assert!(message.contains(&tampered.display().to_string()), "{message}");
    }

    #[test]
    fn mirror_packets_are_numbered_for_gap_detection_and_never_replayed() {
        let runtime = MirrorRuntime::new(MirrorRuntimeConfig {
            id: 1, device: "S".into(), width: 1080, height: 2400,
            max_size: 1600, bitrate: 8_000_000, scid: 1, adb: "adb".into(),
        });
        runtime.push(vec![1]);
        let mut rx = runtime.attach();
        assert!(rx.try_recv().is_err(), "nothing pushed before the attach is replayed");
        runtime.push(vec![2]);
        runtime.push(vec![3]);
        assert_eq!(rx.try_recv().unwrap().seq, 2);
        assert_eq!(rx.try_recv().unwrap().seq, 3);
    }

    #[test]
    fn run_history_keeps_only_the_recent_tail() {
        let runtime = RunRuntime::new(1);
        for line in 1..=600 {
            runtime.push(format!("line {line}"));
        }
        let (cursor, history, _) = runtime.attach();
        assert_eq!(cursor, 600);
        assert_eq!(history.len(), RUN_LOG_LINES);
        assert_eq!(history.first().unwrap().seq, 89);
        assert_eq!(history.last().unwrap().seq, 600);
    }
}
