//! Bounded subprocess execution.
//!
//! Handlers run with the store mutex held (BUS.md §5.1), so a child process that never returns
//! freezes the whole bus — including PTY keystrokes. Every subprocess Relay forks from inside a
//! handler goes through [`output_with_timeout`], which kills the child at the deadline and reports
//! a timeout the caller can turn into a typed refusal (D144).
//!
//! Long-running work belongs on a worker thread after commit, not here. This is for the short
//! probes — `adb devices`, `<provider> --version` — that are logically synchronous.

use std::io;
use std::process::{Command, ExitStatus, Output, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

/// The longest the wait loop sleeps between checks on a child that has not exited yet.
const POLL_MAX: Duration = Duration::from_millis(5);
/// The first sleep. Most of what goes through here — `git status`, `adb devices`,
/// `<provider> --version` — finishes in single-digit milliseconds, and a flat 5 ms poll made
/// every one of them cost a 5 ms sleep it had already outlived. The interval doubles from here,
/// so a long-running child still settles onto the cheap `POLL_MAX` cadence.
const POLL_MIN: Duration = Duration::from_micros(150);
/// The most of each stream a caller gets back. The rest is still read, so the child never
/// blocks on a full pipe, but dropped: a runaway child cannot grow the handler's memory.
const MAX_OUTPUT: usize = 64 * 1024 * 1024;
/// How long past the deadline the pipes may take to close. Killing the process group closes
/// them at once; only a descendant that left the group (`setsid`, a daemonizing server) can
/// hold them open, and the call does not wait for it.
const DRAIN_GRACE: Duration = Duration::from_millis(250);
/// How long a child past its deadline has to exit on SIGTERM before it is SIGKILLed.
pub(crate) const TERM_GRACE: Duration = Duration::from_secs(1);

/// Run `cmd` to completion, or kill it once `timeout` elapses.
///
/// Returns `Ok(None)` when the child outlived the deadline. `stdin` is closed and both output
/// pipes are drained on their own threads, so a child that writes more than a pipe buffer can
/// never deadlock the wait. Each stream keeps at most [`MAX_OUTPUT`] bytes, and the drains are
/// waited on only until the deadline (plus [`DRAIN_GRACE`]): a pipe still held open by an escaped
/// descendant yields what it has delivered so far.
pub fn output_with_timeout(cmd: &mut Command, timeout: Duration) -> io::Result<Option<Output>> {
    run(cmd, None, timeout)
}

/// [`output_with_timeout`] for a tool that reads an answer from stdin: `input` is written on
/// its own thread and stdin is then closed, so a child that never reads cannot block the wait.
pub fn output_with_input(cmd: &mut Command, input: &[u8], timeout: Duration) -> io::Result<Option<Output>> {
    run(cmd, Some(input.to_vec()), timeout)
}

/// How often, and how long apart, a spawn refused with `ETXTBSY` is tried again.
const BUSY_TEXT_RETRIES: u32 = 5;
const BUSY_TEXT_PAUSE: Duration = Duration::from_millis(5);

/// `cmd.spawn()`, tried again a few times while the program is "Text file busy". Exec refuses a
/// file some process holds open for writing, and a script Relay (or a test) has just written can
/// be held that way for a moment by a child another thread forked meanwhile: the child inherits
/// the write handle until its own exec closes it. Waiting a few milliseconds is enough.
fn spawn_past_busy_text(cmd: &mut Command) -> io::Result<std::process::Child> {
    let mut pause = BUSY_TEXT_PAUSE;
    for _ in 0..BUSY_TEXT_RETRIES {
        match cmd.spawn() {
            Err(e) if e.raw_os_error() == Some(libc::ETXTBSY) => {
                std::thread::sleep(pause);
                pause *= 2;
            }
            other => return other,
        }
    }
    cmd.spawn()
}

fn run(cmd: &mut Command, input: Option<Vec<u8>>, timeout: Duration) -> io::Result<Option<Output>> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = spawn_past_busy_text(cmd)?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        std::thread::spawn(move || {
            let _ = io::Write::write_all(&mut stdin, &input);
        });
    }

    let pid = child.id();
    let stdout = child.stdout.take().map(drain);
    let stderr = child.stderr.take().map(drain);

    let deadline = Instant::now() + timeout;
    let mut interval = POLL_MIN;
    let status: Option<ExitStatus> = loop {
        match child.try_wait()? {
            Some(status) => break Some(status),
            None if Instant::now() < deadline => {
                std::thread::sleep(interval);
                interval = (interval * 2).min(POLL_MAX);
            }
            None => {
                terminate_group(pid, TERM_GRACE, || !matches!(child.try_wait(), Ok(None)));
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };

    // A short-lived probe must not leave descendants holding its pipes open.
    signal_group(pid, Signal::Kill);
    let until = deadline.max(Instant::now()) + DRAIN_GRACE;
    let stdout = stdout.map(|drain| drain.collect(until)).unwrap_or_default();
    let stderr = stderr.map(|drain| drain.collect(until)).unwrap_or_default();
    Ok(status.map(|status| Output {
        status,
        stdout,
        stderr,
    }))
}

/// End process group `pgid`: SIGTERM first, then SIGKILL to whatever is left once `exited` says
/// its leader is gone or `grace` has passed. Git removes its `index.lock` and ref locks on
/// SIGTERM, while a SIGKILLed git leaves them behind to block every later git command in that
/// checkout (RA-160); a build's children (gradle, a test harness) clean up the same way. The
/// SIGKILL goes to the group even after the leader exits: a descendant still in it holds pipes.
pub fn terminate_group(pgid: u32, grace: Duration, mut exited: impl FnMut() -> bool) {
    signal_group(pgid, Signal::Term);
    let until = Instant::now() + grace;
    // A SIGTERMed git is usually gone in a few milliseconds: look soon, then back off.
    let mut interval = POLL_MIN;
    while Instant::now() < until && !exited() {
        std::thread::sleep(interval);
        interval = (interval * 2).min(POLL_MAX);
    }
    signal_group(pgid, Signal::Kill);
}

/// SIGTERM to process group `pgid`, and nothing more: for a caller whose child's own wait loop
/// finishes the job with [`terminate_group`]. A SIGKILL sent later from here could land on the
/// group after that loop has reaped it.
pub fn ask_group_to_stop(pgid: u32) {
    signal_group(pgid, Signal::Term);
}

/// [`terminate_group`] for a caller that must not wait, one holding the store's lock or the
/// child's slot: the grace runs out on a thread of its own.
pub fn terminate_group_soon(pgid: u32, grace: Duration) {
    signal_group(pgid, Signal::Term);
    let started = std::thread::Builder::new().name("group-stop".into()).spawn(move || {
        std::thread::sleep(grace);
        signal_group(pgid, Signal::Kill);
    });
    if started.is_err() {
        signal_group(pgid, Signal::Kill);
    }
}

#[derive(Clone, Copy)]
enum Signal {
    Term,
    Kill,
}

fn signal_group(pgid: u32, signal: Signal) {
    #[cfg(unix)]
    {
        let signal = match signal {
            Signal::Term => libc::SIGTERM,
            Signal::Kill => libc::SIGKILL,
        };
        // Never 0 or 1: kill(0, …) is this process's own group, kill(-1, …) every process this
        // user may signal.
        if let Ok(pgid @ 2..) = i32::try_from(pgid) {
            unsafe { libc::kill(-pgid, signal); }
        }
    }
    #[cfg(not(unix))]
    let _ = (pgid, signal);
}

/// Make a network `git` command fail instead of waiting for a person: no terminal prompt, no
/// GUI askpass (an empty `GIT_ASKPASS` skips `core.askPass` and `SSH_ASKPASS` too), and a
/// transfer that stalls below 1 KB/s for a minute is abandoned. Credential helpers still run.
/// Call before adding the subcommand: the `-c` options belong to `git` itself.
pub fn quiet_network_git(cmd: &mut Command) {
    cmd.env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "")
        .env("GCM_INTERACTIVE", "never")
        .args(["-c", "http.lowSpeedLimit=1000", "-c", "http.lowSpeedTime=60"]);
}

/// A pipe being read on its own thread into a buffer the caller can take at any time.
struct Drain {
    buf: Arc<Mutex<Vec<u8>>>,
    /// Disconnects when the thread ends, which is the pipe reaching EOF (or failing).
    done: mpsc::Receiver<()>,
}

impl Drain {
    /// Wait for EOF until `until`, then take whatever has been read. A thread still blocked on
    /// the pipe is left behind; it ends on its own when the last writer goes.
    fn collect(self, until: Instant) -> Vec<u8> {
        let _ = self.done.recv_timeout(until.saturating_duration_since(Instant::now()));
        std::mem::take(&mut *self.buf.lock().unwrap_or_else(|poison| poison.into_inner()))
    }
}

fn drain<R: io::Read + Send + 'static>(mut reader: R) -> Drain {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let (done_tx, done) = mpsc::channel::<()>();
    let shared = buf.clone();
    std::thread::spawn(move || {
        let _done = done_tx;
        let mut chunk = vec![0u8; 64 * 1024];
        loop {
            match io::Read::read(&mut reader, &mut chunk) {
                Ok(0) => break,
                Ok(count) => {
                    let mut buf = shared.lock().unwrap_or_else(|poison| poison.into_inner());
                    let room = MAX_OUTPUT.saturating_sub(buf.len());
                    buf.extend_from_slice(&chunk[..count.min(room)]);
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
    });
    Drain { buf, done }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The race that failed CI once (`Text file busy` from a test's fake `adb`), made on purpose:
    /// a script still open for writing cannot be run, and is run once the handle is gone.
    #[test]
    fn a_script_held_open_for_writing_a_moment_still_runs() {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("script");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(b"#!/bin/sh\necho ran\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let busy = Command::new(&path).spawn().map(|_| ()).unwrap_err();
        assert_eq!(busy.raw_os_error(), Some(libc::ETXTBSY), "{busy}");
        let closer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(15));
            drop(file);
        });
        let output = output_with_timeout(&mut Command::new(&path), Duration::from_secs(5)).unwrap().unwrap();
        closer.join().unwrap();
        assert_eq!(output.stdout, b"ran\n");
    }

    #[test]
    fn a_child_that_finishes_returns_its_output() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "printf hello"]);
        let out = output_with_timeout(&mut cmd, Duration::from_secs(5))
            .unwrap()
            .expect("the child exited well inside the deadline");
        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout), "hello");
    }

    #[test]
    fn a_child_that_hangs_is_killed_at_the_deadline() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 30"]);
        let started = Instant::now();
        let out = output_with_timeout(&mut cmd, Duration::from_millis(150)).unwrap();
        assert!(out.is_none(), "a hung child must report a timeout");
        assert!(started.elapsed() < Duration::from_secs(5), "the wait must not outlive the deadline");
    }

    #[test]
    fn input_reaches_the_child_and_stdin_closes_after_it() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "read answer; echo got:$answer; cat"]);
        let out = output_with_input(&mut cmd, b"no\n", Duration::from_secs(5))
            .unwrap()
            .expect("stdin closes after the input, so `cat` sees EOF");
        assert_eq!(String::from_utf8_lossy(&out.stdout), "got:no\n");
    }

    #[test]
    fn output_larger_than_a_pipe_buffer_does_not_deadlock() {
        // 1 MiB is well past the 64 KiB pipe buffer: without the draining threads this hangs.
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "yes 0123456789 | head -c 1048576"]);
        let out = output_with_timeout(&mut cmd, Duration::from_secs(10))
            .unwrap()
            .expect("draining the pipes keeps the child from blocking on write");
        assert_eq!(out.stdout.len(), 1_048_576);
    }

    #[test]
    fn a_descendant_outside_the_group_cannot_hold_the_call_past_its_deadline() {
        // `setsid` leaves the process group, so the group kill misses it and it keeps stdout open.
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "setsid sleep 4 & echo done"]);
        let started = Instant::now();
        let out = output_with_timeout(&mut cmd, Duration::from_millis(300)).unwrap().unwrap();
        assert!(started.elapsed() < Duration::from_secs(2), "the drains outlived the deadline: {:?}", started.elapsed());
        assert!(String::from_utf8_lossy(&out.stdout).contains("done"));
    }

    #[test]
    fn output_past_the_cap_is_read_but_not_kept() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", &format!("head -c {} /dev/zero", MAX_OUTPUT + 1024 * 1024)]);
        let out = output_with_timeout(&mut cmd, Duration::from_secs(20))
            .unwrap()
            .expect("the child is never blocked on a full pipe");
        assert!(out.status.success());
        assert_eq!(out.stdout.len(), MAX_OUTPUT);
    }

    #[test]
    fn descendants_cannot_keep_probe_pipes_open_after_parent_exit() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 30 & echo done"]);
        let started = Instant::now();
        let out = output_with_timeout(&mut cmd, Duration::from_millis(150)).unwrap().unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(String::from_utf8_lossy(&out.stdout).contains("done"));
    }
}
