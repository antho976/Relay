//! `session.screen`: the bottom of a live terminal as a person would see it, rendered from the
//! PTY's raw output by a terminal emulator (`vt100`). A TUI that redraws with cursor moves
//! (Claude Code's Ink) writes megabytes with hardly a newline, so the raw scrollback says little
//! about what is on screen; the emulator's grid says exactly that.
//!
//! One emulator per live PTY is kept between calls, with the ring offset it has consumed: a call
//! feeds it only what the PTY wrote since. It starts over from the whole ring (up to 8 MiB) on
//! the first call, after the ring dropped the bytes it stopped at, when the PTY is resized or
//! replaced (a wake is a new PTY and a new epoch). Memory only; never the store.

use crate::pty::Pty;
use relay_bus::types::Id;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError, Weak};

/// Rows `session.screen` returns when asked for no number.
pub const DEFAULT_ROWS: u16 = 12;

/// The emulators, by session id. Each sits behind its own lock, so a long first parse holds up
/// only reads of that one terminal.
#[derive(Default)]
pub struct Screens {
    live: Mutex<BTreeMap<Id, (Weak<Pty>, Slot)>>,
}

/// One terminal's emulator, empty until its first read (or after a parse that panicked).
type Slot = Arc<Mutex<Option<Emulator>>>;

struct Emulator {
    epoch: u64,
    /// Absolute ring offset of the next byte to feed.
    end: u64,
    seq: u64,
    /// `(cols, rows)` it was built at.
    size: (u16, u16),
    parser: vt100::Parser,
}

/// What `session.screen` answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub text: String,
    pub cols: u16,
    pub rows: u16,
    pub alternate: bool,
    pub epoch: u64,
    pub seq: u64,
}

impl Screens {
    /// The last `rows` rows of session `id`'s screen, drawn by `pty`.
    pub fn render(&self, id: Id, pty: &Arc<Pty>, rows: Option<u16>) -> Rendered {
        let slot = {
            let mut live = self.live.lock().unwrap_or_else(PoisonError::into_inner);
            // Bounded by the live PTYs: an emulator whose PTY is gone goes with it.
            live.retain(|_, (owner, _)| owner.strong_count() > 0);
            let current = Arc::downgrade(pty);
            match live.get(&id) {
                Some((owner, slot)) if Weak::ptr_eq(owner, &current) => slot.clone(),
                _ => {
                    let slot = Arc::new(Mutex::new(None));
                    live.insert(id, (current, slot.clone()));
                    slot
                }
            }
        };
        let mut slot = slot.lock().unwrap_or_else(PoisonError::into_inner);
        // Taken out while it parses: a parse that panics leaves nothing half-fed behind.
        let emulator = advance(slot.take(), pty);
        let rendered = render(&emulator, rows);
        *slot = Some(emulator);
        rendered
    }

    /// Drop session `id`'s emulator: its PTY is gone.
    pub fn forget(&self, id: Id) {
        self.live.lock().unwrap_or_else(PoisonError::into_inner).remove(&id);
    }
}

/// `cached` brought up to what `pty` has written, or a new emulator fed the whole ring when
/// the cached one cannot continue.
fn advance(cached: Option<Emulator>, pty: &Pty) -> Emulator {
    let size = pty.size();
    let cached = cached.filter(|e| e.size == size);
    let output = pty.output_since(cached.as_ref().map(|e| e.end));
    let mut emulator = match cached {
        Some(e) if !output.whole && e.epoch == output.epoch => e,
        _ => Emulator { epoch: output.epoch, end: 0, seq: 0, size, parser: vt100::Parser::new(size.1.max(1), size.0.max(1), 0) },
    };
    emulator.parser.process(&output.bytes);
    emulator.end = output.end;
    emulator.seq = output.seq;
    emulator
}

fn render(emulator: &Emulator, rows: Option<u16>) -> Rendered {
    let screen = emulator.parser.screen();
    let (height, width) = screen.size();
    let want = rows.unwrap_or(DEFAULT_ROWS).clamp(1, height) as usize;
    Rendered {
        text: bottom(screen.rows(0, width), want),
        cols: width,
        rows: height,
        alternate: screen.alternate_screen(),
        epoch: emulator.epoch,
        seq: emulator.seq,
    }
}

/// The last `want` of `rows` once blank rows at the bottom are dropped, each without its
/// trailing spaces. A TUI draws from the top of a cleared screen, so its last line is often far
/// above the screen's bottom.
fn bottom(rows: impl Iterator<Item = String>, want: usize) -> String {
    let mut lines: Vec<String> = rows.map(|row| row.trim_end().to_string()).collect();
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines[lines.len().saturating_sub(want)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pty::{Pty, SpawnSpec};
    use std::time::{Duration, Instant};

    /// A PTY whose child writes `script`'s output, then waits for a line on its stdin.
    fn spawn(script: &str, cols: u16, rows: u16) -> Arc<Pty> {
        Pty::spawn(SpawnSpec {
            cmd: "sh".into(), args: vec!["-c".into(), script.into()], env: vec![], env_remove: vec![],
            cwd: std::env::temp_dir(), cols, rows, epoch: 1, initial_scrollback: Vec::new(),
        }, |_| {}).unwrap()
    }

    fn wait_for(pty: &Pty, what: &str) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !pty.scrollback(None).0.contains(what) {
            assert!(Instant::now() < deadline, "the PTY never wrote {what:?}: {:?}", pty.scrollback(None).0);
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// A status line drawn, erased with a cursor move and drawn again, as Ink redraws: the raw
    /// bytes hold both versions, the screen only the last. A second read after more output feeds
    /// only what came since, and shows it.
    #[test]
    fn renders_redraws_and_feeds_only_the_delta() {
        // No echo: the Enter typed below would move the cursor the script then counts from.
        let script = r#"stty -echo; printf 'first line\r\nworking 1s\r\n'; printf '\033[1A\033[2Kworking 2s\r\n'; printf 'footer-a\r\n'; read _; printf '\033[2A\033[2Kdone\r\n\033[2Kfooter-b\r\n'; read _"#;
        let pty = spawn(script, 40, 10);
        wait_for(&pty, "footer-a");
        let screens = Screens::default();
        let first = screens.render(7, &pty, None);
        assert_eq!(first.text, "first line\nworking 2s\nfooter-a");
        assert_eq!((first.cols, first.rows, first.alternate, first.epoch), (40, 10, false, 1));
        assert_eq!(screens.render(7, &pty, Some(2)).text, "working 2s\nfooter-a");

        let fed = {
            let live = screens.live.lock().unwrap();
            let slot = live[&7].1.lock().unwrap();
            slot.as_ref().unwrap().end
        };
        pty.write(b"\n").unwrap();
        wait_for(&pty, "footer-b");
        // What the emulator is about to be fed: only the bytes after `fed`.
        let delta = pty.output_since(Some(fed));
        assert!(!delta.whole, "the ring still holds where the emulator stopped");
        assert!(String::from_utf8_lossy(&delta.bytes).contains("footer-b"));
        assert!(!String::from_utf8_lossy(&delta.bytes).contains("first line"), "the delta repeats nothing already fed");

        let second = screens.render(7, &pty, None);
        assert_eq!(second.text, "first line\ndone\nfooter-b");
        assert!(second.seq > first.seq);
        let (end, size) = {
            let live = screens.live.lock().unwrap();
            let slot = live[&7].1.lock().unwrap();
            let e = slot.as_ref().unwrap();
            (e.end, e.size)
        };
        assert_eq!(end, delta.end);
        assert_eq!(size, (40, 10));

        // A resize starts over at the new size.
        pty.resize(30, 6).unwrap();
        let resized = screens.render(7, &pty, None);
        assert_eq!((resized.cols, resized.rows), (30, 6));
        assert_eq!(resized.text, "first line\ndone\nfooter-b");

        // A PTY that is gone takes its emulator with it on the next read.
        let other = spawn("printf 'x\\r\\n'; read _", 20, 5);
        wait_for(&other, "x");
        screens.render(8, &other, None);
        pty.kill(Duration::from_secs(1));
        drop(pty);
        screens.render(8, &other, None);
        assert_eq!(screens.live.lock().unwrap().keys().copied().collect::<Vec<_>>(), vec![8]);
        other.kill(Duration::from_secs(1));
    }

    #[test]
    fn bottom_drops_blank_rows_then_keeps_the_last() {
        let rows = ["a  ", "", "b", "c ", "", "   "].map(String::from);
        assert_eq!(bottom(rows.clone().into_iter(), 2), "b\nc");
        assert_eq!(bottom(rows.clone().into_iter(), 12), "a\n\nb\nc");
        assert_eq!(bottom(["", ""].map(String::from).into_iter(), 3), "");
    }

    #[test]
    fn the_alternate_screen_is_reported() {
        let pty = spawn(r#"printf 'main\r\n\033[?1049h\033[Halt-view\r\n'; read _"#, 20, 5);
        wait_for(&pty, "alt-view");
        let shown = Screens::default().render(1, &pty, None);
        assert!(shown.alternate);
        assert_eq!(shown.text, "alt-view");
        pty.kill(Duration::from_secs(1));
    }

    /// The cost of the first read: the whole 8 MiB ring parsed at once. Run with
    /// `cargo test -p relay-core --lib screen::tests::first_parse -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn first_parse_of_a_full_ring() {
        // Ink-like output: a status line redrawn in place, again and again, plus some text.
        let mut chunk = Vec::new();
        for i in 0..200 {
            chunk.extend_from_slice(format!("\x1b[2A\x1b[2K\x1b[38;5;208m· Crunching… ({i}s · ↓ 24.1k tokens)\x1b[0m\r\n\x1b[2K⏵⏵ auto mode on (shift+tab to cycle) · esc to interrupt\r\n").as_bytes());
        }
        let mut bytes = Vec::with_capacity(crate::pty::SCROLLBACK_BYTES);
        while bytes.len() + chunk.len() <= crate::pty::SCROLLBACK_BYTES {
            bytes.extend_from_slice(&chunk);
        }
        let started = Instant::now();
        let mut parser = vt100::Parser::new(50, 200, 0);
        parser.process(&bytes);
        let took = started.elapsed();
        let text = bottom(parser.screen().rows(0, 200), 2);
        assert!(text.contains("auto mode on"), "{text}");
        eprintln!("first parse of {} bytes: {took:?}", bytes.len());
    }
}
