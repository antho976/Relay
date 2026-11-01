//! PTYs (SPEC §1): one per spawned session. A detached reader thread feeds a scrollback ring
//! and a broadcast of frames (BUS.md §7: `epoch` per spawn, `seq` per frame). Teardown is
//! signal the child's process group → drop master, in that order, always. The reader holds its
//! own dup of the master, so it ends (EIO) only once every holder of the slave is gone: that
//! is why [`Pty::kill`] escalates on the whole group, not just on the leader (RA-332).

use anyhow::{anyhow, Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

/// Scrollback kept per PTY. Bytes, not lines: what the terminal would replay.
pub const SCROLLBACK_BYTES: usize = 8 * 1024 * 1024;
/// Maximum bytes sent when a subscriber catches up. The larger ring remains available for
/// explicit scrollback inspection and persistence, but one stale pane must not monopolize the UI.
pub const ATTACH_CATCHUP_BYTES: usize = 256 * 1024;
/// How many recent frame boundaries we remember for `attach {from_seq}` replay.
const FRAME_INDEX: usize = 8192;

#[derive(Debug, Clone)]
pub struct SpawnSpec {
    pub cmd: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    /// Variables of the engine's own environment the child does not inherit.
    pub env_remove: Vec<String>,
    pub cwd: PathBuf,
    pub cols: u16,
    pub rows: u16,
    pub epoch: u64,
    /// Prior epochs collapsed into the new terminal's initial history on resume/wake.
    pub initial_scrollback: Vec<u8>,
}

/// One frame of output as it left the PTY.
#[derive(Debug, Clone)]
pub struct Frame {
    pub epoch: u64,
    pub seq: u64,
    pub data: Vec<u8>,
}

struct Ring {
    buf: VecDeque<u8>,
    /// Absolute byte offset of `buf[0]` since spawn.
    base: u64,
    /// (seq, absolute offset where that frame starts, the terminal state there), oldest first.
    frames: VecDeque<(u64, u64, Mark)>,
}

impl Ring {
    /// The byte ring is reserved at its cap up front. Pages are only touched as output fills
    /// them, and a full ring never reallocates: grown on demand, the first push past a full
    /// ring doubled it to 16 MiB and copied 8 MiB under the ring lock (RA-329).
    fn new() -> Ring {
        Ring { buf: VecDeque::with_capacity(SCROLLBACK_BYTES), base: 0, frames: VecDeque::new() }
    }
    /// Append one frame and trim both rings back to their caps. Trimming is a single bulk
    /// `drain`, never a per-element `pop_front` loop: a provider that writes 64 KiB at a time
    /// would otherwise pay 65 536 individual ring operations for every read once the byte ring
    /// is full, which is the whole cost of the terminal data plane at steady state. Room is
    /// made before the bytes go in, so the ring never holds more than its cap even briefly.
    fn push(&mut self, seq: u64, data: &[u8], mark: Mark) {
        let start = self.base + self.buf.len() as u64;
        self.frames.push_back((seq, start, mark));
        if self.frames.len() > FRAME_INDEX {
            let excess = self.frames.len() - FRAME_INDEX;
            self.frames.drain(..excess);
        }
        // Of a frame larger than the whole ring only its tail survives.
        let skip = data.len().saturating_sub(SCROLLBACK_BYTES);
        let data = &data[skip..];
        let excess = (self.buf.len() + data.len()).saturating_sub(SCROLLBACK_BYTES);
        self.buf.drain(..excess);
        self.base += (excess + skip) as u64;
        // `extend` from a slice copies in bulk; `data.iter().copied()` would not.
        self.buf.extend(data);
    }
    /// Bytes from absolute offset `from` (clamped to what we still have). Copied through the
    /// deque's two contiguous halves — an element-wise iterator over 8 MiB is the same answer
    /// an order of magnitude slower.
    fn bytes_from(&self, from: u64) -> Vec<u8> {
        let skip = from.saturating_sub(self.base).min(self.buf.len() as u64) as usize;
        let (head, tail) = self.buf.as_slices();
        let mut out = Vec::with_capacity(self.buf.len() - skip);
        if skip < head.len() {
            out.extend_from_slice(&head[skip..]);
            out.extend_from_slice(tail);
        } else {
            out.extend_from_slice(&tail[skip - head.len()..]);
        }
        out
    }
    /// The first frame boundary after `seq`, if we still know it. Both `seq` and the offsets
    /// increase with every push, so the index is sorted and a scan is never needed.
    fn offset_after(&self, seq: u64) -> Option<u64> {
        let at = self.frames.partition_point(|(s, _, _)| *s <= seq);
        self.frames.get(at).map(|(_, offset, _)| *offset)
    }
    fn end(&self) -> u64 {
        self.base + self.buf.len() as u64
    }
    /// Restored history goes in as many frames, each ending on a line, never as one. Catch-up
    /// starts on a frame boundary, and a single multi-megabyte frame left it two choices: skip
    /// the whole history once any live frame followed it, or cut into it mid escape sequence
    /// when none had (RA-116). All of it stays seq 0, before anything this process prints.
    /// The modes the old process left behind are switched off after it: the new one starts
    /// on a fresh terminal, not inside its predecessor's alternate screen (RA-242).
    fn push_history(&mut self, history: &[u8], term: &mut ModeTracker) {
        const CHUNK: usize = 16 * 1024;
        let mut rest = history;
        while !rest.is_empty() {
            let mut cut = rest.len().min(CHUNK);
            if cut < rest.len() {
                match rest[..cut].iter().rposition(|byte| *byte == b'\n') {
                    Some(newline) => cut = newline + 1,
                    // One line longer than a chunk: at least never split a character.
                    None => while cut > 1 && rest[cut] & 0xC0 == 0x80 { cut -= 1; },
                }
            }
            let mark = term.mark();
            term.feed(&rest[..cut]);
            self.push(0, &rest[..cut], mark);
            rest = &rest[cut..];
        }
        let left = term.mark();
        if left != Mark::default() {
            // CAN abandons a sequence the old process was cut off in the middle of.
            let mut reset = if left.clean { Vec::new() } else { vec![0x18] };
            TermModes::default().write_screen(Some(left.modes), &mut reset);
            TermModes::default().write_rest(Some(left.modes), &mut reset);
            term.feed(&reset);
            self.push(0, &reset, left);
        }
    }
    /// At most [`ATTACH_CATCHUP_BYTES`] from `from`, starting on a frame boundary, and the
    /// terminal state at that start when it is not `from`: the replay was cut short, the
    /// subscriber never saw how the modes it starts in were set, and it needs them restored.
    /// A cut start also skips a few frames forward to one that is not inside an escape sequence
    /// or a character, so the replay never opens with the tail of one printed as text.
    fn bounded_bytes_from(&self, from: u64) -> (Vec<u8>, Option<Mark>) {
        let earliest = self.end().saturating_sub(ATTACH_CATCHUP_BYTES as u64);
        let requested = from.max(earliest);
        let mut at = self.frames.partition_point(|(_, offset, _)| *offset < requested);
        let Some(&(_, aligned, _)) = self.frames.get(at) else {
            // No boundary at or after it: the request starts inside the last frame or at the end.
            let cut = (requested > from).then(|| at.checked_sub(1).map_or_else(Mark::default, |i| self.frames[i].2));
            return (self.bytes_from(requested), cut);
        };
        if aligned == from {
            return (self.bytes_from(aligned), None);
        }
        if let Some(clean) = self.frames.range(at..).take(64).position(|(_, _, mark)| mark.clean) {
            at += clean;
        }
        let (_, start, mark) = self.frames[at];
        (self.bytes_from(start), Some(mark))
    }
    /// The bytes covering the last `lines` newline-terminated lines, so reading a short tail
    /// out of an 8 MiB ring costs the tail rather than the ring.
    fn tail_lines(&self, lines: usize) -> Vec<u8> {
        let (head, tail) = self.buf.as_slices();
        let mut seen = 0usize;
        let mut start = None;
        // Walk backwards through the two halves, stopping at the newline that opens the
        // (lines)-th line from the end.
        'scan: for (part, base) in [(tail, head.len()), (head, 0)] {
            for (i, byte) in part.iter().enumerate().rev() {
                if *byte == b'\n' {
                    seen += 1;
                    if seen > lines {
                        start = Some(base + i + 1);
                        break 'scan;
                    }
                }
            }
        }
        let start = start.unwrap_or(0);
        self.bytes_from(self.base + start as u64)
    }
}

/// The DEC private modes (DECSET/DECRST, `CSI ? n h|l`) a full-screen program sets once at
/// start-up and a subscriber that joins later must be told about (RA-242). Bit set = mode on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TermModes(u16);

const ALT_SCREEN: u16 = 1;
/// Every tracked mode but the alternate screen: (mode number, bit). The mouse tracking modes
/// ascend, so a terminal that keeps only one of them (xterm) ends on the one VTE would pick.
const MODE_BITS: [(u16, u16); 10] = [
    (1, 1 << 1),     // application cursor keys
    (25, 1 << 2),    // cursor visible
    (9, 1 << 3),     // X10 mouse
    (1000, 1 << 4),  // mouse buttons
    (1002, 1 << 5),  // mouse drag
    (1003, 1 << 6),  // all mouse motion
    (1004, 1 << 7),  // focus events
    (1005, 1 << 8),  // UTF-8 mouse encoding
    (1006, 1 << 9),  // SGR mouse encoding
    (2004, 1 << 10), // bracketed paste
];

impl Default for TermModes {
    /// A terminal as it powers on: the cursor shows, everything else is off.
    fn default() -> Self {
        TermModes(1 << 2)
    }
}

impl TermModes {
    fn set(&mut self, mode: u16, on: bool) {
        let bit = match mode {
            47 | 1047 | 1049 => ALT_SCREEN,
            _ => match MODE_BITS.iter().find(|(number, _)| *number == mode) {
                Some((_, bit)) => *bit,
                None => return,
            },
        };
        if on { self.0 |= bit } else { self.0 &= !bit }
    }
    /// Whether DEC private `mode` is on; false for one not tracked.
    fn on(self, mode: u16) -> bool {
        let mut probe = TermModes(0);
        probe.set(mode, true);
        probe.0 != 0 && self.0 & probe.0 == probe.0
    }
    /// Switch the screen of a terminal in modes `from` (`None`: unknown) to this one's.
    fn write_screen(self, from: Option<TermModes>, out: &mut Vec<u8>) {
        let alt = self.0 & ALT_SCREEN != 0;
        if from.is_none_or(|from| (from.0 & ALT_SCREEN != 0) != alt) {
            out.extend_from_slice(if alt { b"\x1b[?1049h" } else { b"\x1b[?1049l" });
        }
    }
    /// Switch every other mode the same way: all that go off before any that go on, since
    /// switching one mouse mode off turns all of them off in xterm.
    fn write_rest(self, from: Option<TermModes>, out: &mut Vec<u8>) {
        use std::io::Write as _;
        for on in [false, true] {
            let mut sep = "\x1b[?";
            for (number, bit) in MODE_BITS {
                let changed = from.is_none_or(|from| (from.0 ^ self.0) & bit != 0);
                if changed && (self.0 & bit != 0) == on {
                    let _ = write!(out, "{sep}{number}");
                    sep = ";";
                }
            }
            if sep == ";" { out.push(if on { b'h' } else { b'l' }); }
        }
    }
    /// What goes before a replay cut short: these modes, on a cleared screen. The screen goes
    /// first so the clear lands on the one the replay draws on; scroll region and attributes
    /// go back to a fresh terminal's, which is what the replay assumes of both.
    fn prelude(self) -> Vec<u8> {
        let mut out = Vec::with_capacity(96);
        self.write_screen(None, &mut out);
        out.extend_from_slice(b"\x1b[r\x1b[0m\x1b[H\x1b[2J");
        self.write_rest(None, &mut out);
        out
    }
}

/// The terminal state at a frame boundary, recorded with the frame that starts there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mark {
    modes: TermModes,
    /// Not inside an escape sequence or a UTF-8 character: a replay may start here.
    clean: bool,
}

impl Default for Mark {
    fn default() -> Self {
        Mark { modes: TermModes::default(), clean: true }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Scan {
    #[default]
    Ground,
    Esc,
    /// `ESC` + intermediates (a charset designation and the like), up to its final byte.
    EscIntermediate,
    /// `CSI`, parameters being read. `private` is the `?` marker.
    Csi { private: bool },
    /// A CSI that cannot be a DECSET/DECRST, up to its final byte.
    CsiIgnore,
    /// OSC, DCS, SOS, PM or APC payload, up to BEL or `ESC`.
    Str,
}

/// An incremental scanner for [`TermModes`], fed every read as it leaves the PTY. Only `ESC`
/// matters, so text between sequences is skipped with one search per run; a sequence split
/// across reads resumes where it stopped. A DEC VT parser in miniature: CAN and SUB abort,
/// `ESC` anywhere starts over, C0 controls inside a sequence do not end it.
#[derive(Debug, Default)]
struct ModeTracker {
    modes: TermModes,
    state: Scan,
    params: [u16; 16],
    count: usize,
    /// The last read ended inside a UTF-8 character.
    mid_char: bool,
}

impl ModeTracker {
    fn mark(&self) -> Mark {
        Mark { modes: self.modes, clean: self.state == Scan::Ground && !self.mid_char }
    }

    fn feed(&mut self, data: &[u8]) {
        let mut i = 0;
        while i < data.len() {
            match self.state {
                Scan::Ground => match data[i..].iter().position(|b| *b == 0x1b) {
                    Some(at) => { i += at + 1; self.state = Scan::Esc; continue; }
                    None => break,
                },
                // A payload can be long (an image); only its terminators matter.
                Scan::Str => match data[i..].iter().position(|b| matches!(b, 0x07 | 0x18 | 0x1a | 0x1b)) {
                    Some(at) => i += at,
                    None => break,
                },
                _ => {}
            }
            let byte = data[i];
            i += 1;
            match byte {
                0x18 | 0x1a => { self.state = Scan::Ground; continue; }
                0x1b => { self.state = Scan::Esc; continue; }
                _ => {}
            }
            self.state = match (self.state, byte) {
                (Scan::Esc, b'[') => {
                    self.params[0] = 0;
                    self.count = 0;
                    Scan::Csi { private: false }
                }
                (Scan::Esc, b']' | b'P' | b'X' | b'^' | b'_') => Scan::Str,
                (Scan::Esc, b'c') => { self.modes = TermModes::default(); Scan::Ground }
                (Scan::Esc | Scan::EscIntermediate, 0x20..=0x2f) => Scan::EscIntermediate,
                (Scan::Esc | Scan::EscIntermediate, 0x30..=0x7e) => Scan::Ground,
                (Scan::Csi { .. }, b'?') if self.count == 0 && self.params[0] == 0 => Scan::Csi { private: true },
                (Scan::Csi { private }, b'0'..=b'9') => {
                    let param = &mut self.params[self.count.min(15)];
                    *param = param.saturating_mul(10).saturating_add((byte - b'0') as u16);
                    Scan::Csi { private }
                }
                (Scan::Csi { private }, b';') => {
                    self.count += 1;
                    if self.count < 16 { self.params[self.count] = 0; }
                    Scan::Csi { private }
                }
                (Scan::Csi { private: true }, b'h' | b'l') => {
                    for mode in &self.params[..(self.count + 1).min(16)] {
                        self.modes.set(*mode, byte == b'h');
                    }
                    Scan::Ground
                }
                (Scan::Csi { .. } | Scan::CsiIgnore, 0x40..=0x7e) => Scan::Ground,
                // Another marker, a sub-parameter, an intermediate: not a mode switch.
                (Scan::Csi { .. }, 0x20..=0x3f) => Scan::CsiIgnore,
                (Scan::Str, 0x07) => Scan::Ground,
                // C0 controls inside a sequence are carried out without ending it; anything
                // else (DEL, a byte past ASCII) is ignored where it stands.
                (state, _) => state,
            };
        }
        // Only a read that ends in text can end inside a character.
        self.mid_char = self.state == Scan::Ground && ends_mid_char(data);
    }
}

/// Does `data` stop partway through a UTF-8 character? A tail of continuation bytes with no
/// lead in sight counts as yes: a replay may skip a boundary, never split a character.
fn ends_mid_char(data: &[u8]) -> bool {
    for (back, byte) in data.iter().rev().take(4).enumerate() {
        if byte & 0xC0 != 0x80 {
            let len = match byte { 0xC0..=0xDF => 2, 0xE0..=0xEF => 3, 0xF0..=0xF7 => 4, _ => 1 };
            return len > back + 1;
        }
    }
    !data.is_empty()
}

struct Shared {
    epoch: u64,
    seq: AtomicU64,
    /// Unix milliseconds of the last byte the child produced, 0 if it has produced none.
    /// Kept in memory and read on demand: this is how Relay sees a provider that runs no
    /// lifecycle hooks at all, without a timer anywhere (SPEC §15, D108).
    last_output_ms: AtomicU64,
    ring: Mutex<Ring>,
    tx: broadcast::Sender<Arc<Frame>>,
    exited: AtomicBool,
    /// Nothing was left in the child's process group the last time it was looked at after the
    /// child was reaped. Sticky: once the group is empty its id may be reused by a stranger,
    /// so it is never signalled again.
    group_gone: AtomicBool,
    /// Mirror of `sessions.state == 'idle'`, kept here so `session.input` can decide whether a
    /// keystroke needs a database write without reading the database (D148). Only the
    /// idle→running edge does; every other keystroke is pure memory.
    idle: AtomicBool,
    /// The person has typed since this terminal last sounded a "finished" notification (or it
    /// never has): its next finished turn is worth a ping. Memory only, like `idle`.
    ping_owed: AtomicBool,
    /// Set by [`Pty::silence_exit`]: the request that stops this child records the outcome
    /// itself, so the exit callback must not race it to the row.
    silent: AtomicBool,
    /// Set by [`Pty::hang_up_first`]: stopped as a closed terminal stops its shell, with SIGHUP,
    /// which an interactive shell passes on to its jobs. SIGTERM is ignored by one, and SIGKILL
    /// leaves its jobs (each in its own process group, out of reach of the group's signal) running.
    hang_up: AtomicBool,
    /// The [`TermModes`] after the last read, stored by the reader: `session.input` asks it
    /// whether a paste goes in brackets without reading the ring or the store (D148).
    modes: AtomicU16,
}

/// A live PTY. Cheap to clone the handle (`Arc`).
pub struct Pty {
    pid: u32,
    shared: Arc<Shared>,
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    /// The size last set, `cols << 16 | rows`: a client that resizes for itself (a phone)
    /// reads it first so it can hand the terminal back as it found it.
    size: AtomicU32,
}

fn pack_size(cols: u16, rows: u16) -> u32 {
    (cols as u32) << 16 | rows as u32
}

/// What `attach` hands a subscriber: catch-up bytes, where they end, and the live feed.
pub struct Attached {
    pub epoch: u64,
    pub seq: u64,
    pub catch_up: Vec<u8>,
    pub rx: broadcast::Receiver<Arc<Frame>>,
}

impl Pty {
    /// Spawn `spec` on a fresh PTY. `on_exit` runs on a detached thread once the child is
    /// reaped (with its exit code, `None` if killed by signal). A `cwd` that is not a directory
    /// is refused with [`CwdMissing`]: portable-pty would start the child in `$HOME` instead,
    /// where an agent's relative `--settings` and hook files are not Relay's (RA-330).
    pub fn spawn(spec: SpawnSpec, on_exit: impl FnOnce(Option<i32>) + Send + 'static) -> Result<Arc<Pty>> {
        if !spec.cwd.is_dir() {
            return Err(CwdMissing(spec.cwd).into());
        }
        let sys = native_pty_system();
        let (cols, rows) = (spec.cols.max(2), spec.rows.max(2));
        let pair = sys
            .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| anyhow!("openpty: {e}"))?;
        let mut cmd = CommandBuilder::new(&spec.cmd);
        cmd.args(&spec.args);
        cmd.cwd(&spec.cwd);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        // The daemon may have been started by a noninteractive, monochrome tool.
        // Its log preference must not disable colors in an interactive agent PTY.
        cmd.env_remove("NO_COLOR");
        for k in &spec.env_remove {
            cmd.env_remove(k);
        }
        for (k, v) in &spec.env {
            cmd.env(k, v);
        }
        let mut child = pair.slave.spawn_command(cmd).with_context(|| format!("spawning {}", spec.cmd))?;
        drop(pair.slave);
        let pid = child.process_id().ok_or_else(|| anyhow!("spawned child has no pid"))?;
        let mut reader = pair.master.try_clone_reader().map_err(|e| anyhow!("clone reader: {e}"))?;
        let writer = pair.master.take_writer().map_err(|e| anyhow!("take writer: {e}"))?;

        let (tx, _) = broadcast::channel(1024);
        let mut ring = Ring::new();
        let mut term = ModeTracker::default();
        ring.push_history(&spec.initial_scrollback, &mut term);
        let shared = Arc::new(Shared {
            epoch: spec.epoch,
            seq: AtomicU64::new(0),
            last_output_ms: AtomicU64::new(0),
            ring: Mutex::new(ring),
            tx,
            idle: AtomicBool::new(false),
            ping_owed: AtomicBool::new(true),
            silent: AtomicBool::new(false),
            hang_up: AtomicBool::new(false),
            modes: AtomicU16::new(term.modes.0),
            exited: AtomicBool::new(false),
            group_gone: AtomicBool::new(false),
        });

        // reader: detached; ends when the master is dropped (EIO) or the child closes the slave
        let sh = shared.clone();
        std::thread::Builder::new().name(format!("pty-read-{pid}")).spawn(move || {
            let mut buf = vec![0u8; 64 * 1024];
            // Before the ring: what it holds is stored as scrollback and streamed to a phone.
            let mut redactor = crate::redact::Redactor::default();
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        sh.last_output_ms.store(unix_millis(), Ordering::Relaxed);
                        let mut data = buf[..n].to_vec();
                        redactor.redact(&mut data);
                        // Outside the lock: the scan is this thread's alone (RA-242).
                        let mark = term.mark();
                        term.feed(&data);
                        sh.modes.store(term.modes.0, Ordering::Relaxed);
                        // Numbered under the ring lock: `attach` reads the seq under it too, so
                        // a seq it sees is always one whose bytes are already in the ring. Taken
                        // before the lock, an attach in between skipped that frame as caught up
                        // when it was not, or found no boundary for it and replayed 256 KiB (RA-117).
                        let seq = {
                            let mut ring = sh.ring.lock().unwrap();
                            let seq = sh.seq.fetch_add(1, Ordering::SeqCst) + 1;
                            ring.push(seq, &data, mark);
                            seq
                        };
                        let _ = sh.tx.send(Arc::new(Frame { epoch: sh.epoch, seq, data }));
                    }
                }
            }
        })?;

        // waiter: reaps the child and runs the callback. portable-pty reports a signal death
        // as code 1 beside the signal's name; that is no exit code, and recorded as one it read
        // as an ordinary error exit (RA-331).
        let sh = shared.clone();
        std::thread::Builder::new().name(format!("pty-wait-{pid}")).spawn(move || {
            let code = child.wait().ok().filter(|s| s.signal().is_none()).map(|s| s.exit_code() as i32);
            if !group_alive(pid) { sh.group_gone.store(true, Ordering::SeqCst); }
            sh.exited.store(true, Ordering::SeqCst);
            if !sh.silent.load(Ordering::SeqCst) {
                on_exit(code);
            }
        })?;

        Ok(Arc::new(Pty {
            pid,
            shared,
            master: Mutex::new(Some(pair.master)),
            writer: Mutex::new(Some(writer)),
            size: AtomicU32::new(pack_size(cols, rows)),
        }))
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }
    pub fn seq(&self) -> u64 {
        self.shared.seq.load(Ordering::SeqCst)
    }
    /// When the child last wrote anything, as an RFC 3339 timestamp. `None` before its first
    /// byte. True for every provider, hooked or not.
    pub fn last_output_at(&self) -> Option<String> {
        let millis = self.shared.last_output_ms.load(Ordering::Relaxed);
        (millis > 0).then(|| {
            jiff::Timestamp::from_millisecond(millis as i64)
                .unwrap_or(jiff::Timestamp::UNIX_EPOCH)
                .to_string()
        })
    }
    pub fn exited(&self) -> bool {
        self.shared.exited.load(Ordering::SeqCst)
    }
    /// Record what the session row now says, so the input fast path knows whether it owes the
    /// store a state change.
    pub fn set_idle(&self, idle: bool) {
        self.shared.idle.store(idle, Ordering::SeqCst);
    }
    /// Claim the idle→running edge: true for the first caller after [`Pty::set_idle(true)`],
    /// false for every keystroke after it. Exactly one deferred write per idle period.
    pub fn claim_idle_edge(&self) -> bool {
        self.shared.idle.swap(false, Ordering::SeqCst)
    }
    /// The person typed into the terminal: the agent's next finished turn answers them.
    pub fn owe_ping(&self) {
        self.shared.ping_owed.store(true, Ordering::Relaxed);
    }
    /// Claim the one "finished" ping a terminal sounds until the person types into it again:
    /// true for the first finished turn after [`Pty::owe_ping`], false for the rest.
    pub fn claim_ping(&self) -> bool {
        self.shared.ping_owed.swap(false, Ordering::Relaxed)
    }
    /// Skip the `on_exit` callback when the child is reaped. For a caller that kills the child
    /// with the store unlocked and then writes the session's next state itself (park, close
    /// with checkout removal): without this the callback would mark the row `exited` — and a
    /// builder's task `failed` — in the gap between the kill and that write.
    pub fn silence_exit(&self) {
        self.shared.silent.store(true, Ordering::SeqCst);
    }

    /// Stop this child with SIGHUP before SIGTERM: it is the person's shell ([`Shared::hang_up`]).
    pub fn hang_up_first(&self) {
        self.shared.hang_up.store(true, Ordering::SeqCst);
    }

    /// Nobody is writing right now. A keystroke is answered on a runtime thread only then: one
    /// queued behind a paste that is waiting for the child to drain would park that thread.
    pub fn writer_idle(&self) -> bool {
        self.writer.try_lock().is_ok()
    }

    /// The program has asked for bracketed paste (DECSET 2004) and not switched it off since.
    pub fn bracketed_paste(&self) -> bool {
        TermModes(self.shared.modes.load(Ordering::Relaxed)).on(2004)
    }

    pub fn write(&self, data: &[u8]) -> Result<()> {
        let mut w = self.writer.lock().unwrap();
        let w = w.as_mut().ok_or_else(|| anyhow!("pty is closed"))?;
        w.write_all(data)?;
        w.flush()?;
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        let (cols, rows) = (cols.max(2), rows.max(2));
        let m = self.master.lock().unwrap();
        let m = m.as_ref().ok_or_else(|| anyhow!("pty is closed"))?;
        m.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| anyhow!("resize: {e}"))?;
        // Under the master lock, so two resizes cannot record each other's size.
        self.size.store(pack_size(cols, rows), Ordering::SeqCst);
        Ok(())
    }

    /// `(cols, rows)` as last set by spawn or [`Pty::resize`].
    pub fn size(&self) -> (u16, u16) {
        let packed = self.size.load(Ordering::SeqCst);
        ((packed >> 16) as u16, packed as u16)
    }

    /// Subscribe with bounded catch-up since (`from_epoch`, `from_seq`) plus the live feed.
    pub fn attach(&self, from_epoch: Option<u64>, from_seq: Option<u64>) -> Attached {
        let rx = self.shared.tx.subscribe();
        let ring = self.shared.ring.lock().unwrap();
        let seq = self.seq();
        let (bytes, cut) = match (from_epoch, from_seq) {
            (Some(e), Some(s)) if e == self.shared.epoch => {
                if s >= seq { (Vec::new(), None) } else { ring.bounded_bytes_from(ring.offset_after(s).unwrap_or(0)) }
            }
            _ => ring.bounded_bytes_from(0),
        };
        drop(ring);
        // A replay cut short starts on a screen and in modes the subscriber never saw set up:
        // a TUI enables them once at start-up. Re-establish them and clear what is there, so a
        // recreated pane keeps bracketed paste and the mouse, and draws on the right screen.
        let catch_up = match cut {
            Some(mark) => {
                let mut prelude = mark.modes.prelude();
                prelude.extend_from_slice(&bytes);
                prelude
            }
            None => bytes,
        };
        Attached { epoch: self.shared.epoch, seq, catch_up, rx }
    }

    /// The scrollback as text (last `lines` if given) and where it ends. A bounded request
    /// reads only the tail it asks for: the ring holds 8 MiB, and `session.scrollback {lines}`
    /// is a UI-facing read that must not copy all of it to keep twenty lines.
    pub fn scrollback(&self, lines: Option<usize>) -> (String, u64, u64) {
        let ring = self.shared.ring.lock().unwrap();
        let bytes = match lines {
            Some(n) => ring.tail_lines(n),
            None => ring.bytes_from(0),
        };
        drop(ring);
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let text = match lines {
            Some(n) => {
                let v: Vec<&str> = text.lines().collect();
                let start = v.len().saturating_sub(n);
                v[start..].join("\n")
            }
            None => text,
        };
        (text, self.shared.epoch, self.seq())
    }

    /// Kill the child (SIGTERM to its process group, then SIGKILL after `grace`), then drop
    /// the master so the reader ends. Idempotent. [`kill_all`] does the same for many at once.
    pub fn kill(&self, grace: Duration) {
        kill_all(&[self], grace);
    }

    /// Send `signal` to the child's process group, and to the child itself while it lives.
    /// The group is signalled even after the leader is gone: members that outlived it (a
    /// provider that crashed, or was told `/exit`) would otherwise run on (RA-332).
    fn signal(&self, signal: libc::c_int) {
        unsafe {
            // the child is a session leader on its own pty, so -pid reaches its children
            if !self.shared.group_gone.load(Ordering::SeqCst) { libc::kill(-(self.pid as i32), signal); }
            if !self.exited() { libc::kill(self.pid as i32, signal); }
        }
    }

    /// The child is reaped and nothing is left in its process group. A grandchild that left
    /// the group (`setsid`) is out of reach either way; recovery reaps it by `RELAY_SESSION`.
    fn settled(&self) -> bool {
        if !self.exited() { return false; }
        if self.shared.group_gone.load(Ordering::SeqCst) { return true; }
        let gone = !group_alive(self.pid);
        if gone { self.shared.group_gone.store(true, Ordering::SeqCst); }
        gone
    }

    /// Drop writer then master. The reader's own dup of the master ends on EIO once the
    /// group is gone and the slave with it.
    fn close(&self) {
        self.writer.lock().unwrap().take();
        self.master.lock().unwrap().take();
    }
}

/// `data` as one paste (`session.input {paste}`, I-4). `bracketed` (the program set DECSET 2004):
/// between `ESC[200~` and `ESC[201~`, line breaks kept. Otherwise as typed, but with each line
/// break (`\r\n`, `\r` or `\n`) a space, since a program without bracketed paste would read one
/// as Enter. Either way every other control character but a tab (C0, DEL, C1: ESC and the 8-bit
/// CSI among them) is taken out, as terminals do when pasting, so nothing in it can end the paste
/// early or act as a key; the tty would act on a Ctrl-C (`\x03`) even inside the brackets.
/// Nothing to paste is nothing to send.
pub fn paste_bytes(data: &str, bracketed: bool) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(data.len() + 12);
    if bracketed {
        out.extend_from_slice(b"\x1b[200~");
        let mut buf = [0u8; 4];
        for c in data.chars().filter(|c| !c.is_control() || matches!(c, '\t' | '\n' | '\r')) {
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        }
        out.extend_from_slice(b"\x1b[201~");
    } else {
        let mut buf = [0u8; 4];
        let mut chars = data.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\r' => {
                    chars.next_if_eq(&'\n');
                    out.push(b' ');
                }
                '\n' => out.push(b' '),
                '\t' => out.push(b'\t'),
                c if c.is_control() => {}
                c => out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes()),
            }
        }
    }
    out
}

/// [`Pty::kill`] for many PTYs against one shared deadline: SIGTERM to every group, one wait
/// of at most `grace` for all of them, SIGKILL to whatever remains, then at most 2 s more.
/// Killing one at a time cost up to `grace` per stubborn session, which at shutdown summed to
/// more than a desktop logout or a service stop allows (RA-311).
pub fn kill_all(ptys: &[&Pty], grace: Duration) {
    // Each look scans all of /proc once per group still running. A shell that takes SIGTERM is
    // gone within a few ms, so the looks back off from 1 ms to every 50 ms: a fixed 50 ms first
    // wait made every park and close cost at least 50 ms.
    let wait = |deadline: Instant| {
        let mut pause = Duration::from_millis(1);
        while !ptys.iter().all(|pty| pty.settled()) {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            std::thread::sleep(pause.min(deadline - now));
            pause = (pause * 2).min(Duration::from_millis(50));
        }
    };
    let pending: Vec<&&Pty> = ptys.iter().filter(|pty| !pty.settled()).collect();
    // A shell hangs up its jobs and exits on SIGHUP, at once; the SIGTERM after it is for
    // whatever ignored the hangup.
    let shells: Vec<&&&Pty> = pending.iter().filter(|pty| pty.shared.hang_up.load(Ordering::SeqCst)).collect();
    for pty in &shells { pty.signal(libc::SIGHUP); }
    if !shells.is_empty() { wait(Instant::now() + grace.min(Duration::from_millis(500))); }
    for pty in &pending { pty.signal(libc::SIGTERM); }
    if !pending.is_empty() { wait(Instant::now() + grace); }
    let stubborn: Vec<&&Pty> = pending.into_iter().filter(|pty| !pty.settled()).collect();
    for pty in &stubborn { pty.signal(libc::SIGKILL); }
    if !stubborn.is_empty() { wait(Instant::now() + Duration::from_secs(2)); }
    for pty in ptys { pty.close(); }
}

/// [`Pty::spawn`]'s refusal of a working directory that is not one (`anyhow` downcasts to it).
#[derive(Debug)]
pub struct CwdMissing(pub PathBuf);

impl std::fmt::Display for CwdMissing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "working directory {} is gone", self.0.display())
    }
}

impl std::error::Error for CwdMissing {}

fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_millis() as u64)
        .unwrap_or(0)
}



impl Drop for Pty {
    fn drop(&mut self) {
        // last handle gone: make sure nothing lingers
        if !self.exited() {
            unsafe {
                libc::kill(-(self.pid as i32), libc::SIGKILL);
                libc::kill(self.pid as i32, libc::SIGKILL);
            }
        }
    }
}

/// Is anything left running in process group `pgid`? Signal 0 to the group also finds members
/// that are dead and not yet reaped. Orphans go to the engine, a subreaper that reaps them on its
/// own schedule (`peer::spawn_orphan_reaper`, up to a minute), so those zombies are looked past.
/// The group is called gone only when the scan saw every member: a member it cannot read (another
/// user's, under `hidepid`, like `sudo`) or a signal answered by none it found counts as alive.
fn group_alive(pgid: u32) -> bool {
    if unsafe { libc::kill(-(pgid as i32), 0) } != 0 {
        return false;
    }
    if proc_hides_pids() {
        return true;
    }
    let Ok(entries) = std::fs::read_dir("/proc") else { return true };
    let mut found = false;
    for entry in entries.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse::<i32>().ok()) else { continue };
        let Some((state, pgrp, threads)) = std::fs::read_to_string(entry.path().join("stat")).ok().as_deref().and_then(stat_fields) else {
            // Unreadable, or gone since the listing: getpgid needs no access to /proc.
            if unsafe { libc::getpgid(pid) } == pgid as i32 { return true; }
            continue;
        };
        if pgrp != pgid { continue; }
        found = true;
        // A zombie leader whose other threads run (it called pthread_exit) is no zombie.
        if state != 'Z' || threads > 1 { return true; }
    }
    !found
}

/// State, process group and thread count from `/proc/<pid>/stat`:
/// `pid (comm) state ppid pgrp session tty tpgid flags … priority nice num_threads …`.
/// comm may hold spaces and parentheses.
fn stat_fields(stat: &str) -> Option<(char, u32, u32)> {
    let (_, rest) = stat.rsplit_once(')')?;
    let fields: Vec<&str> = rest.split_whitespace().take(18).collect();
    let state = fields.first()?.chars().next()?;
    Some((state, fields.get(2)?.parse().ok()?, fields.get(17)?.parse().ok()?))
}

/// Is /proc mounted with `hidepid`, so other users' processes (a `sudo`) may be missing from it?
/// Then a scan cannot prove a group empty. Read once.
fn proc_hides_pids() -> bool {
    static HIDES: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *HIDES.get_or_init(|| {
        std::fs::read_to_string("/proc/self/mountinfo").map(|text| mount_hides_pids(&text)).unwrap_or(false)
    })
}

/// Whether the `/proc` line of `mountinfo` sets `hidepid` to anything but 0/off.
fn mount_hides_pids(mountinfo: &str) -> bool {
    mountinfo.lines().filter(|line| line.split_whitespace().nth(4) == Some("/proc")).any(|line| {
        line.split([' ', ',']).filter_map(|option| option.strip_prefix("hidepid=")).any(|value| value != "0" && value != "off")
    })
}

/// Is `pid` alive? (signal 0)
pub fn pid_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

/// Environment of a live process, from /proc.
pub fn proc_env(pid: u32) -> Option<Vec<(String, String)>> {
    let raw = std::fs::read(format!("/proc/{pid}/environ")).ok()?;
    Some(raw.split(|b| *b == 0).filter_map(|kv| {
        let s = String::from_utf8_lossy(kv);
        let (k, v) = s.split_once('=')?;
        Some((k.to_string(), v.to_string()))
    }).collect())
}

/// Every live pid whose environment carries `RELAY_SESSION` for `instance` **and** this
/// store (`RELAY_STORE`) — Relay's own children (and theirs), regardless of which engine
/// process spawned them. The store filter keeps a test engine from reaping a real one.
///
/// Every process on the machine is read, so each environment is searched as it came, for the
/// three variables, and nothing is copied out of a process that is not ours: as owned pairs, the
/// environments of a desktop's few hundred processes were most of engine startup.
pub fn relay_children(instance: &str, store: &str) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir("/proc") else { return out };
    let me = std::process::id();
    for e in rd.flatten() {
        let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() else { continue };
        if pid == me { continue; }
        let Ok(raw) = std::fs::read(format!("/proc/{pid}/environ")) else { continue };
        if let Some(session) = relay_child_session(&raw, instance, store) {
            out.push((pid, session));
        }
    }
    out
}

/// The `RELAY_SESSION` of an `/proc/<pid>/environ` whose `RELAY_INSTANCE` and `RELAY_STORE` are
/// `instance` and `store`, the first of each name counting, as [`proc_env`] would read them.
fn relay_child_session(environ: &[u8], instance: &str, store: &str) -> Option<String> {
    let first = |key: &[u8]| environ.split(|b| *b == 0).find_map(|kv| kv.strip_prefix(key));
    if first(b"RELAY_INSTANCE=")? != instance.as_bytes() || first(b"RELAY_STORE=")? != store.as_bytes() {
        return None;
    }
    first(b"RELAY_SESSION=").map(|v| String::from_utf8_lossy(v).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The byte search answers as the owned pairs of [`proc_env`] did: the first of each name,
    /// exact values, no session without both the instance and the store.
    #[test]
    fn relay_child_session_reads_the_environment_as_proc_env_did() {
        let env = |vars: &[&str]| vars.iter().flat_map(|v| v.bytes().chain([0])).collect::<Vec<u8>>();
        let ours = env(&["HOME=/h", "RELAY_INSTANCE=default", "RELAY_STORE=/s/store.db", "RELAY_SESSION=spry-kestrel"]);
        assert_eq!(relay_child_session(&ours, "default", "/s/store.db").as_deref(), Some("spry-kestrel"));
        assert_eq!(relay_child_session(&ours, "default", "/s/store.db.bak"), None, "a value is matched whole");
        assert_eq!(relay_child_session(&ours, "other", "/s/store.db"), None);
        assert_eq!(relay_child_session(&env(&["RELAY_INSTANCE=default", "RELAY_STORE=/s/store.db"]), "default", "/s/store.db"), None);
        assert_eq!(relay_child_session(&env(&["RELAY_STORE=/s/store.db", "RELAY_SESSION=a"]), "default", "/s/store.db"), None);
        let twice = env(&["RELAY_INSTANCE=other", "RELAY_INSTANCE=default", "RELAY_STORE=/s/store.db", "RELAY_SESSION=a"]);
        assert_eq!(relay_child_session(&twice, "default", "/s/store.db"), None, "the first of a name counts");
        assert_eq!(relay_child_session(&env(&["XRELAY_INSTANCE=default", "RELAY_STORE=/s/store.db", "RELAY_SESSION=a"]), "default", "/s/store.db"), None);
        assert_eq!(relay_child_session(b"RELAY_INSTANCE=default\0RELAY_STORE=/s/store.db\0RELAY_SESSION=a", "default", "/s/store.db").as_deref(), Some("a"), "no NUL after the last");
    }

    #[test]
    fn catch_up_is_bounded_and_starts_on_a_frame_boundary() {
        let mut ring = Ring { buf: VecDeque::new(), base: 0, frames: VecDeque::new() };
        for seq in 1..=1100 { ring.push(seq, &vec![(seq % 251) as u8; 1024], Mark::default()); }
        let catch_up = ring.bounded_bytes_from(0).0;
        assert_eq!(catch_up.len(), ATTACH_CATCHUP_BYTES);
        assert_eq!(catch_up.len() % 1024, 0);
    }

    fn ring() -> Ring {
        Ring { buf: VecDeque::new(), base: 0, frames: VecDeque::new() }
    }

    #[test]
    fn bulk_trimming_keeps_the_same_window_and_offsets_as_a_byte_at_a_time_ring() {
        let mut r = ring();
        let chunk = 64 * 1024;
        let pushes = (SCROLLBACK_BYTES / chunk) + 3;
        for seq in 1..=pushes as u64 {
            r.push(seq, &vec![(seq % 251) as u8; chunk], Mark::default());
        }
        assert_eq!(r.buf.len(), SCROLLBACK_BYTES);
        assert_eq!(r.base, (pushes * chunk - SCROLLBACK_BYTES) as u64);
        assert_eq!(r.end(), (pushes * chunk) as u64);
        // Every byte still in the window is the one its absolute offset names.
        let all = r.bytes_from(0);
        assert_eq!(all.len(), SCROLLBACK_BYTES);
        let first_seq = (r.base as usize / chunk) as u64 + 1;
        assert_eq!(all[0], (first_seq % 251) as u8);
        assert_eq!(*all.last().unwrap(), (pushes as u64 % 251) as u8);
        // A partial read lands on the same byte the offset names.
        let tail = r.bytes_from(r.end() - 10);
        assert_eq!(tail, vec![(pushes as u64 % 251) as u8; 10]);
    }

    #[test]
    fn a_single_push_larger_than_the_ring_keeps_its_tail() {
        let mut r = ring();
        let huge = SCROLLBACK_BYTES + 4096;
        r.push(0, &vec![7u8; huge], Mark::default());
        assert_eq!(r.buf.len(), SCROLLBACK_BYTES);
        assert_eq!(r.base, 4096);
        assert_eq!(r.end(), huge as u64);
    }

    #[test]
    fn restored_history_is_caught_up_from_a_line_boundary() {
        let mut history = Vec::new();
        for line in 0..40_000 {
            history.extend_from_slice(format!("\x1b[32mline {line}\x1b[0m\n").as_bytes());
        }
        assert!(history.len() > 2 * ATTACH_CATCHUP_BYTES);
        let mut r = ring();
        r.push_history(&history, &mut ModeTracker::default());
        assert_eq!(r.bytes_from(0), history, "split, not changed");
        // Nothing printed yet: the tail of the history, starting on a line.
        let catch_up = r.bounded_bytes_from(0).0;
        assert!(catch_up.len() > ATTACH_CATCHUP_BYTES - 16 * 1024 && catch_up.len() <= ATTACH_CATCHUP_BYTES);
        assert!(catch_up.starts_with(b"\x1b[32mline "), "cut mid line: {:?}", &catch_up[..16]);
        assert!(history.ends_with(&catch_up));
        // A first live frame does not push the history out of the catch-up.
        r.push(1, b"$ ", Mark::default());
        let catch_up = r.bounded_bytes_from(0).0;
        assert!(catch_up.len() > ATTACH_CATCHUP_BYTES - 16 * 1024, "{}", catch_up.len());
        assert!(catch_up.starts_with(b"\x1b[32mline ") && catch_up.ends_with(b"line 39999\x1b[0m\n$ "));
        // A client that saw the history resumes after it.
        assert_eq!(r.bytes_from(r.offset_after(0).unwrap()), b"$ ");
        // A line longer than a chunk is cut, but never inside a character.
        let mut r = ring();
        let long = "é".repeat(20 * 1024);
        r.push_history(long.as_bytes(), &mut ModeTracker::default());
        assert!(r.frames.len() > 1);
        for (_, offset, _) in r.frames.iter() {
            assert!(std::str::from_utf8(&r.bytes_from(*offset)).is_ok());
        }
    }

    #[test]
    fn frame_lookup_matches_a_linear_scan() {
        let mut r = ring();
        for seq in 1..=500u64 {
            r.push(seq, &[0u8; 16], Mark::default());
        }
        for seq in 0..=501u64 {
            let scanned = r.frames.iter().find(|(s, _, _)| *s > seq).map(|(_, o, _)| *o);
            assert_eq!(r.offset_after(seq), scanned, "seq {seq}");
        }
    }

    #[test]
    fn a_bounded_tail_read_returns_the_same_lines_as_the_whole_ring() {
        let mut r = ring();
        for seq in 1..=400u64 {
            r.push(seq, format!("line {seq}\n").as_bytes(), Mark::default());
        }
        let whole = String::from_utf8(r.bytes_from(0)).unwrap();
        for lines in [1usize, 5, 399, 400, 4000] {
            let tail = String::from_utf8(r.tail_lines(lines)).unwrap();
            let expect: Vec<&str> = whole.lines().collect();
            let expect = expect[expect.len().saturating_sub(lines)..].join("\n");
            let got: Vec<&str> = tail.lines().collect();
            let got = got[got.len().saturating_sub(lines)..].join("\n");
            assert_eq!(got, expect, "{lines} lines");
        }
    }

    #[test]
    fn a_tail_read_survives_a_wrapped_ring_without_splitting_a_line() {
        let mut r = ring();
        let filler = vec![b'z'; 1024];
        let mut seq = 0u64;
        while r.end() < SCROLLBACK_BYTES as u64 + 2048 {
            seq += 1;
            let mut line = filler.clone();
            line.push(b'\n');
            r.push(seq, &line, Mark::default());
        }
        let tail = String::from_utf8(r.tail_lines(3)).unwrap();
        assert_eq!(tail.lines().count(), 3);
        assert!(tail.lines().all(|l| l.len() == 1024));
    }

    fn modes_of(chunks: &[&[u8]]) -> ModeTracker {
        let mut term = ModeTracker::default();
        for chunk in chunks { term.feed(chunk); }
        term
    }

    fn on(term: &ModeTracker, mode: u16) -> bool {
        let mut probe = TermModes(0);
        probe.set(mode, true);
        term.modes.0 & probe.0 != 0
    }

    #[test]
    fn modes_are_tracked_across_any_split_of_the_stream() {
        let stream: &[u8] = "hi \x1b[1;31mred\x1b[0m \x1b[?1049h\x1b[?2004;1006h\x1b]0;tïtle\x07\x1b[?1000h\x1b[?25l é \x1b[?1000l\x1b[?1002h\x1bP+q\x1b\\\x1b[?1h".as_bytes();
        let whole = modes_of(&[stream]);
        for (mode, set) in [(1049, true), (2004, true), (1006, true), (1000, false), (1002, true), (25, false), (1, true), (1003, false)] {
            assert_eq!(on(&whole, mode), set, "mode {mode}");
        }
        assert!(whole.mark().clean);
        // Every cut in two, and one byte at a time, ends in the same state.
        for cut in 0..=stream.len() {
            let split = modes_of(&[&stream[..cut], &stream[cut..]]);
            assert_eq!(split.modes, whole.modes, "cut at {cut}");
        }
        let bytes: Vec<&[u8]> = stream.chunks(1).collect();
        assert_eq!(modes_of(&bytes).modes, whole.modes);
    }

    #[test]
    fn only_a_private_set_or_reset_switches_a_mode() {
        for not_a_switch in [
            "\x1b[2004h",          // ANSI mode, not DEC private
            "\x1b[?2004$p",        // DECRQM asks, does not set
            "\x1b[>2004h",         // another private marker
            "\x1b[?2004:1h",       // sub-parameters
            "\x1b]2;?2004h\x07",   // text inside a title
            "\x1b[?20\x1804h",     // cancelled by CAN
            "\x1bP?2004h\x1b\\",   // a DCS payload
        ] {
            assert_eq!(modes_of(&[not_a_switch.as_bytes()]).modes, TermModes::default(), "{not_a_switch:?}");
        }
        // A control inside a sequence is carried out without ending it.
        assert!(on(&modes_of(&[b"\x1b[?20\n04h"]), 2004));
        // Full reset.
        assert_eq!(modes_of(&[b"\x1b[?1049h\x1b[?2004h\x1bc"]).modes, TermModes::default());
        // 47 and 1047 are the alternate screen too.
        assert!(on(&modes_of(&[b"\x1b[?47h"]), 1049));
        assert!(!on(&modes_of(&[b"\x1b[?1049h\x1b[?1047l"]), 1049));
    }

    #[test]
    fn a_boundary_inside_a_sequence_or_a_character_is_not_clean() {
        let mut term = ModeTracker::default();
        term.feed(b"abc\x1b[?10");
        assert!(!term.mark().clean);
        term.feed(b"49h");
        assert!(term.mark().clean && on(&term, 1049));
        term.feed(b"x\xc3");
        assert!(!term.mark().clean);
        term.feed(b"\xa9y");
        assert!(term.mark().clean);
        term.feed("\x1b]0;tïtle".as_bytes());
        assert!(!term.mark().clean, "inside an OSC");
        term.feed(b"\x1b\\");
        assert!(term.mark().clean);
    }

    /// Feed `data` through `term` and the ring as the reader thread does.
    fn read(r: &mut Ring, term: &mut ModeTracker, seq: u64, data: &[u8]) {
        let mark = term.mark();
        term.feed(data);
        r.push(seq, data, mark);
    }

    #[test]
    fn a_cut_replay_carries_the_modes_it_starts_in_and_never_starts_mid_sequence() {
        let mut r = ring();
        let mut term = ModeTracker::default();
        read(&mut r, &mut term, 1, b"\x1b[?1049h\x1b[?2004h\x1b[?1000;1006h");
        // Every other read ends inside an SGR sequence the next one finishes.
        let mut seq = 1;
        while r.end() < 2 * ATTACH_CATCHUP_BYTES as u64 {
            seq += 1;
            let mut data = vec![b'x'; 1000];
            if seq % 2 == 0 { data.extend_from_slice(b"\x1b[3") } else { data.splice(0..0, *b"1m"); }
            read(&mut r, &mut term, seq, &data);
        }
        let (bytes, cut) = r.bounded_bytes_from(0);
        let mark = cut.expect("the replay was cut short");
        assert!(bytes.len() <= ATTACH_CATCHUP_BYTES && bytes.len() > ATTACH_CATCHUP_BYTES - 4096);
        assert!(bytes.starts_with(b"xxx"), "starts mid sequence: {:?}", &bytes[..8]);
        assert_eq!(mark.modes, term.modes);
        let prelude = String::from_utf8(mark.modes.prelude()).unwrap();
        assert_eq!(prelude, "\x1b[?1049h\x1b[r\x1b[0m\x1b[H\x1b[2J\x1b[?1;9;1002;1003;1004;1005l\x1b[?25;1000;1006;2004h");
        // A replay that is not cut needs nothing: the subscriber saw every byte before it.
        let resume = r.offset_after(seq - 3).unwrap();
        assert_eq!(r.bounded_bytes_from(resume).1, None);
        let mut small = ring();
        read(&mut small, &mut ModeTracker::default(), 1, b"\x1b[?2004hok");
        assert_eq!(small.bounded_bytes_from(0), (b"\x1b[?2004hok".to_vec(), None));
    }

    #[test]
    fn history_leaves_the_new_process_a_fresh_terminal() {
        let mut r = ring();
        let mut term = ModeTracker::default();
        r.push_history(b"$ claude\n\x1b[?1049h\x1b[?2004hdrawn\x1b[?10", &mut term);
        assert_eq!(term.mark(), Mark::default());
        let (_, _, mark) = *r.frames.back().unwrap();
        assert!(!mark.clean && mark.modes != TermModes::default());
        assert!(r.bytes_from(0).ends_with(b"drawn\x1b[?10\x18\x1b[?1049l\x1b[?2004l"));
        // Plain history is left exactly as it was.
        let mut r = ring();
        let mut term = ModeTracker::default();
        r.push_history(b"$ ls\nfile\n", &mut term);
        assert_eq!(r.bytes_from(0), b"$ ls\nfile\n");
    }

    #[test]
    fn attach_prefixes_a_cut_catch_up_with_the_modes_the_program_set() {
        let script = format!(
            "printf '\\033[?2004h\\033[?1049h'; head -c {} /dev/zero | tr '\\000' x; printf END",
            2 * ATTACH_CATCHUP_BYTES,
        );
        let spec = SpawnSpec {
            cmd: "sh".into(), args: vec!["-c".into(), script], env: Vec::new(), env_remove: Vec::new(),
            cwd: std::env::temp_dir(), cols: 80, rows: 24, epoch: 1, initial_scrollback: Vec::new(),
        };
        let pty = Pty::spawn(spec, |_| {}).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let attached = loop {
            let attached = pty.attach(None, None);
            if attached.catch_up.ends_with(b"END") { break attached; }
            assert!(Instant::now() < deadline, "output never finished");
            std::thread::sleep(Duration::from_millis(20));
        };
        let prelude = TermModes(ALT_SCREEN | 1 << 2 | 1 << 10).prelude();
        assert!(attached.catch_up.starts_with(&prelude), "{:?}", String::from_utf8_lossy(&attached.catch_up[..64]));
        assert!(attached.catch_up[prelude.len()..].iter().all(|b| *b == b'x' || b"END\r\n".contains(b)));
        // Caught up from where it left off: no prelude.
        assert!(pty.attach(Some(1), Some(pty.seq())).catch_up.is_empty());
        pty.kill(Duration::from_millis(100));
    }

    /// A shell's background job is in its own process group, out of reach of the group's
    /// signals: closing the shell hangs it up, as closing a terminal does, rather than leaving
    /// it running.
    #[test]
    fn closing_a_shell_hangs_up_its_background_jobs() {
        if !std::path::Path::new("/bin/bash").exists() { return; }
        let dir = tempfile::tempdir().unwrap();
        let pidfile = dir.path().join("job.pid");
        let spec = SpawnSpec {
            cmd: "/bin/bash".into(), args: vec!["--norc".into(), "--noprofile".into(), "-i".into()], env: Vec::new(), env_remove: Vec::new(),
            cwd: dir.path().to_path_buf(), cols: 80, rows: 24, epoch: 1, initial_scrollback: Vec::new(),
        };
        let pty = Pty::spawn(spec, |_| {}).unwrap();
        pty.hang_up_first();
        pty.write(format!("sleep 300 & echo $! > {}\r", pidfile.display()).as_bytes()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let job: i32 = loop {
            if let Some(pid) = std::fs::read_to_string(&pidfile).ok().and_then(|s| s.trim().parse().ok()) { break pid; }
            assert!(Instant::now() < deadline, "the job never started");
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_ne!(unsafe { libc::getpgid(job) }, unsafe { libc::getpgid(pty.pid() as i32) }, "a job of its own, as job control makes it");
        pty.kill(Duration::from_millis(150));
        let deadline = Instant::now() + Duration::from_secs(5);
        while unsafe { libc::kill(job, 0) } == 0 && std::path::Path::new(&format!("/proc/{job}/stat")).exists()
            && !std::fs::read_to_string(format!("/proc/{job}/stat")).is_ok_and(|s| s.contains(") Z "))
        {
            assert!(Instant::now() < deadline, "the background job outlived its shell");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn a_bracketed_paste_keeps_its_lines_and_cannot_end_itself() {
        // I-4: the lines go in as they are; an ESC (or 8-bit CSI) inside would let the text
        // close the paste and act as keys after it.
        let pasted = paste_bytes("Make it\nbigger\x1b[201~\r\n/compact é\u{9b}201~", true);
        assert_eq!(pasted, "\x1b[200~Make it\nbigger[201~\r\n/compact é201~\x1b[201~".as_bytes());
        assert_eq!(pasted.iter().filter(|b| **b == 0x1b).count(), 2, "only the brackets' own");
        assert!(paste_bytes("", true).is_empty());
        // Nor can a control character the tty or the program acts on: Ctrl-C, Ctrl-Z, Ctrl-U, DEL.
        assert_eq!(paste_bytes("ls\x03rm\x1a\x15\x7f\u{85}\tx", true), "\x1b[200~lsrm\tx\x1b[201~".as_bytes());
    }

    #[test]
    fn a_paste_without_brackets_is_typed_on_one_line() {
        // Without DECSET 2004 a line break is Enter: each one is a space, `\r\n` a single one.
        assert_eq!(paste_bytes("one\ntwo\r\nthree\rfour", false), b"one two three four");
        assert_eq!(paste_bytes("tab\there é", false), "tab\there é".as_bytes());
        assert_eq!(paste_bytes("ls\x03 \x1b[A\x15x\x7f", false), b"ls [Ax", "no control character acts as a key");
        assert!(paste_bytes("", false).is_empty());
    }

    #[test]
    fn a_live_pty_knows_whether_its_program_asked_for_bracketed_paste() {
        let pty = Pty::spawn(sh("printf '\\033[?2004h'; echo on; read x; printf '\\033[?2004l'; echo off; sleep 5"), |_| {}).unwrap();
        assert!(!TermModes::default().on(2004) && !TermModes::default().on(7), "off as it powers on; 7 is not tracked");
        wait_for("bracketed paste on", || pty.bracketed_paste());
        pty.write(b"\n").unwrap();
        wait_for("bracketed paste off", || !pty.bracketed_paste());
        pty.kill(Duration::from_millis(100));
    }

    #[test]
    fn a_full_ring_never_grows_past_its_reservation() {
        let mut r = Ring::new();
        let reserved = r.buf.capacity();
        let mut seq = 0;
        // Odd-sized frames, so no push lands exactly on the cap, then one larger than the ring.
        while r.end() < 2 * SCROLLBACK_BYTES as u64 {
            seq += 1;
            r.push(seq, &[b'x'; 70_001], Mark::default());
            assert!(r.buf.len() <= SCROLLBACK_BYTES);
        }
        r.push(seq + 1, &vec![b'y'; SCROLLBACK_BYTES + 3], Mark::default());
        assert_eq!(r.buf.capacity(), reserved, "the ring reallocated");
        assert_eq!(r.buf.len(), SCROLLBACK_BYTES);
        assert_eq!(r.base + SCROLLBACK_BYTES as u64, r.end());
        assert!(r.bytes_from(0).iter().all(|b| *b == b'y'));
    }

    fn sh(script: &str) -> SpawnSpec {
        SpawnSpec {
            cmd: "sh".into(), args: vec!["-c".into(), script.into()], env: Vec::new(), env_remove: Vec::new(),
            cwd: std::env::temp_dir(), cols: 80, rows: 24, epoch: 1, initial_scrollback: Vec::new(),
        }
    }

    fn wait_for(what: &str, done: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !done() {
            assert!(Instant::now() < deadline, "{what} never happened");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// A variable named in `env_remove` is not inherited, and a key the child prints reaches the
    /// ring, so the scrollback and every attach, blanked.
    #[test]
    fn removed_variables_are_not_inherited_and_printed_keys_are_blanked() {
        let mut spec = sh("echo \"home=${HOME:-gone}\"; printf 'k=sk-ant-'; sleep 0.1; printf 'api03-SECRET\\n'; echo done");
        spec.env_remove = vec!["HOME".into()];
        let pty = Pty::spawn(spec, |_| {}).unwrap();
        wait_for("the output", || String::from_utf8_lossy(&pty.attach(None, None).catch_up).contains("done"));
        let out = String::from_utf8_lossy(&pty.attach(None, None).catch_up).into_owned();
        assert!(out.contains("home=gone"), "{out:?}");
        assert!(out.contains("k=sk-ant-************") && !out.contains("SECRET"), "{out:?}");
    }

    #[test]
    fn a_missing_working_directory_is_refused_not_replaced_by_home() {
        let mut spec = sh("true");
        spec.cwd = std::env::temp_dir().join("relay-no-such-worktree-RA-330");
        let error = Pty::spawn(spec, |_| {}).err().expect("spawned in a directory that does not exist");
        assert!(error.downcast_ref::<CwdMissing>().is_some(), "{error:#}");
    }

    #[test]
    fn a_signal_death_has_no_exit_code() {
        for (script, expect) in [("exit 3", Some(3)), ("kill -9 $$", None)] {
            let (tx, rx) = std::sync::mpsc::channel();
            let _pty = Pty::spawn(sh(script), move |code| { let _ = tx.send(code); }).unwrap();
            assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), expect, "{script}");
        }
    }

    #[test]
    fn kill_reaches_group_members_that_outlive_the_leader() {
        // The member ignores TERM and HUP, so neither the leader's exit nor SIGTERM ends it.
        let pty = Pty::spawn(sh("(trap '' TERM HUP; exec sleep 30) & sleep 0.2; exit 0"), |_| {}).unwrap();
        wait_for("the leader's exit", || pty.exited());
        assert!(group_alive(pty.pid()), "the member should still be running");
        pty.kill(Duration::from_millis(100));
        assert!(!group_alive(pty.pid()), "a member survived kill");
    }

    #[test]
    fn kill_all_waits_once_for_every_pty() {
        let ptys: Vec<_> = (0..3).map(|_| Pty::spawn(sh("trap '' TERM; echo ready; sleep 30"), |_| {}).unwrap()).collect();
        for pty in &ptys {
            wait_for("the trap", || String::from_utf8_lossy(&pty.attach(None, None).catch_up).contains("ready"));
        }
        let started = Instant::now();
        kill_all(&ptys.iter().map(|pty| &**pty).collect::<Vec<_>>(), Duration::from_secs(1));
        // One at a time this is at least 3 s of grace; the shared deadline makes it about 1.
        assert!(started.elapsed() < Duration::from_millis(2500), "{:?}", started.elapsed());
        assert!(ptys.iter().all(|pty| pty.exited() && !group_alive(pty.pid())));
    }

    #[test]
    fn a_zombie_leader_with_running_threads_reads_as_alive() {
        // A main thread that called pthread_exit, its other thread still running.
        let stat = "1961551 (pe (x) y) Z 1961549 1960822 1960822 0 -1 4227084 125 0 0 0 0 0 0 0 16 -4 2 0 8832968 0 0";
        assert_eq!(stat_fields(stat), Some(('Z', 1960822, 2)));
        assert_eq!(stat_fields("12 (sleep) S 1 12 12 34816 12 4194560 0 0 0 0 0 0 0 0 20 0 1 0 5"), Some(('S', 12, 1)));
        assert_eq!(stat_fields("12 (sleep) S 1 12"), None);
        assert!(!mount_hides_pids("49 40 0:24 / /proc rw,nosuid,nodev,noexec,relatime shared:11 - proc proc rw"));
        assert!(!mount_hides_pids("49 40 0:24 / /proc rw,relatime - proc proc rw,hidepid=0"));
        assert!(mount_hides_pids("49 40 0:24 / /proc rw,relatime - proc proc rw,hidepid=invisible"));
        assert!(mount_hides_pids("49 40 0:24 / /proc rw,relatime - proc proc rw,hidepid=2,gid=10"));
        assert!(!mount_hides_pids("50 40 0:25 / /mnt/proc rw - proc proc rw,hidepid=2"));
    }

    #[test]
    fn kill_reaches_a_member_whose_main_thread_exited() {
        // Built here when a C compiler is at hand; the parsing above is checked either way.
        let dir = tempfile::tempdir().unwrap();
        let (source, program) = (dir.path().join("pe.c"), dir.path().join("pe"));
        // It ignores the hangup the leader's exit sends, as a provider's helper might.
        std::fs::write(&source, "#include <pthread.h>\n#include <signal.h>\n#include <unistd.h>\n\
            static void *run(void *a) { (void)a; sleep(30); return 0; }\n\
            int main(void) { signal(SIGHUP, SIG_IGN); pthread_t t; pthread_create(&t, 0, run, 0); pthread_exit(0); }\n").unwrap();
        let built = std::process::Command::new("cc").arg("-pthread").arg("-o").arg(&program).arg(&source)
            .stderr(std::process::Stdio::null()).status();
        if !built.is_ok_and(|status| status.success()) {
            eprintln!("no C compiler; skipped");
            return;
        }
        let pty = Pty::spawn(sh(&format!("{} & sleep 0.3; exit 0", program.display())), |_| {}).unwrap();
        wait_for("the leader's exit", || pty.exited());
        assert!(group_alive(pty.pid()), "a process whose main thread exited read as gone");
        pty.kill(Duration::from_millis(100));
        assert!(!group_alive(pty.pid()), "the process outlived kill");
    }
}
