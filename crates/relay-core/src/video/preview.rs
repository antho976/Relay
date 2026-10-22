//! The live preview (`video.preview`): Remotion Studio for one video at a time, for the person to
//! watch it with sound, scrub and step through frames without rendering. It runs
//! `remotion studio` from the kit with `.relay/loopback.cjs` loaded first, which keeps every
//! server it opens on 127.0.0.1 and its update check off the network. Started on request, it
//! stops when another video is previewed, after two hours, or with Relay (parent-death signal,
//! as the worker).

use super::Video;
use crate::engine::Engine;
use relay_bus::error::BusError;
use relay_bus::types::Id;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

/// How long a preview may take to open: Studio's first bundle.
const OPENS_WITHIN: Duration = Duration::from_secs(120);
/// How long one runs.
const RUNS_FOR: Duration = Duration::from_secs(2 * 60 * 60);

struct Running {
    video_id: Id,
    port: u16,
    pid: u32,
    gone: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct Slot {
    live: Mutex<Option<Running>>,
    /// Held for the whole of an open: two at once would stop each other's Studio.
    opening: Mutex<()>,
    /// Tests only: the program to run instead of Studio, `{port}` in its arguments replaced.
    program: Mutex<Option<Vec<String>>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Slot {
    #[doc(hidden)]
    pub fn use_program(&self, program: Vec<String>) {
        *lock(&self.program) = Some(program);
    }
}

fn url(port: u16, composition: &str) -> String {
    format!("http://127.0.0.1:{port}/{composition}")
}

/// Stop the preview, if one runs.
pub fn stop(engine: &Engine) {
    if let Some(running) = lock(&engine.video.preview.live).take() {
        kill(running.pid);
    }
}

fn kill(pid: u32) {
    #[cfg(unix)]
    // SAFETY: a signal to the process group the preview leads; nothing else is touched.
    unsafe {
        libc::killpg(pid as i32, libc::SIGTERM);
    }
}

/// Whether video `id`'s preview runs, and on which port.
pub fn running(engine: &Engine, id: Id) -> Option<u16> {
    let live = lock(&engine.video.preview.live);
    live.as_ref().filter(|r| r.video_id == id && !r.gone.load(Ordering::SeqCst)).map(|r| r.port)
}

/// The preview of `video` on `composition`: the one running, or a new one (another video's is
/// stopped first). Its link, and whether this started it.
pub fn open(engine: &Engine, video: &Video, composition: &str) -> Result<(String, bool), BusError> {
    let _one = lock(&engine.video.preview.opening);
    if let Some(port) = running(engine, video.id) {
        return Ok((url(port, composition), false));
    }
    stop(engine);
    let port = TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map(|a| a.port())
        .map_err(|e| BusError::unavailable("video.preview", format!("no free port: {e}")))?;
    let program = match lock(&engine.video.preview.program).clone() {
        Some(program) => program.into_iter().map(|a| a.replace("{port}", &port.to_string())).collect::<Vec<_>>(),
        None => {
            super::kit::ready(engine)?;
            let root = engine.store.videos_dir();
            super::kit::sync(&root).map_err(|e| BusError::unavailable("video.preview", format!("could not write the kit's files: {e}")))?;
            let cli = root.join("node_modules/@remotion/cli/remotion-cli.js");
            if !cli.is_file() {
                return Err(BusError::unavailable("video.kit.outdated", "the video kit was set up without Remotion's studio")
                    .with_hint("call video.kit.install to bring the kit up to date"));
            }
            let node = which::which("node").map_err(|_| BusError::unavailable("video.kit.no_node", "node is not installed"))?;
            vec![node.display().to_string(), "--require".into(), root.join(".relay/loopback.cjs").display().to_string(), cli.display().to_string(),
                "studio".into(), "src/index.ts".into(), "--port".into(), port.to_string(), "--no-open".into(), "--log=error".into()]
        }
    };
    let mut cmd = Command::new(&program[0]);
    cmd.args(&program[1..]).current_dir(&video.dir).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
        // SAFETY: prctl is async-signal-safe and touches nothing of the parent's.
        unsafe {
            cmd.pre_exec(|| {
                #[cfg(target_os = "linux")]
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }
    }
    // The thread that forks waits on the child, as the worker's: the parent-death signal follows it.
    let gone = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let ended = gone.clone();
    std::thread::Builder::new().name("video-preview".into()).spawn(move || {
        match cmd.spawn() {
            Ok(mut child) => {
                let _ = tx.send(Ok(child.id()));
                let _ = child.wait();
            }
            Err(e) => {
                let _ = tx.send(Err(e.to_string()));
            }
        }
        ended.store(true, Ordering::SeqCst);
    }).map_err(|e| BusError::internal(format!("preview thread: {e}")))?;
    let pid = rx.recv().map_err(|_| BusError::internal("the preview did not start"))?
        .map_err(|e| BusError::unavailable("video.preview", format!("could not start the preview: {e}")))?;
    *lock(&engine.video.preview.live) = Some(Running { video_id: video.id, port, pid, gone: gone.clone() });
    // A stop after its time, unless another took its place.
    if let Some(arc) = engine.arc() {
        let weak = Arc::downgrade(&arc);
        let _ = std::thread::Builder::new().name("video-preview-timer".into()).spawn(move || {
            std::thread::sleep(RUNS_FOR);
            if let Some(engine) = weak.upgrade() {
                if lock(&engine.video.preview.live).as_ref().is_some_and(|r| r.pid == pid) {
                    stop(&engine);
                }
            }
        });
    }
    let deadline = Instant::now() + OPENS_WITHIN;
    let at: SocketAddr = ([127, 0, 0, 1], port).into();
    while Instant::now() < deadline {
        if gone.load(Ordering::SeqCst) {
            let mut live = lock(&engine.video.preview.live);
            if live.as_ref().is_some_and(|r| r.pid == pid) {
                live.take();
            }
            drop(live);
            return Err(BusError::unavailable("video.preview", "the preview stopped as it started")
                .with_hint("video.check the code first; then try again"));
        }
        if TcpStream::connect_timeout(&at, Duration::from_millis(200)).is_ok() {
            return Ok((url(port, composition), true));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    if lock(&engine.video.preview.live).as_ref().is_some_and(|r| r.pid == pid) {
        stop(engine);
    }
    Err(BusError::unavailable("video.preview", format!("the preview did not open within {} s", OPENS_WITHIN.as_secs())))
}
