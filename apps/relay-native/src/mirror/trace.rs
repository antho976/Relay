//! The client's half of the mirror trace: `RELAY_MIRROR_TRACE=1` (anything but empty or `0`),
//! the engine's switch and format (`docs/engine/BUS.md`, *Mirror trace*). Each event is an INFO
//! line at the `mirror_trace` target, so the subscriber must let it through:
//!
//! ```text
//! RELAY_MIRROR_TRACE=1 RUST_LOG=mirror_trace=info relay-native > native.log
//! ```
//!
//! Off, a trace point costs one load and formats nothing. Stamps are `CLOCK_MONOTONIC`
//! nanoseconds (`mono_ns`, `*_ns`) — the engine's clock and GTK's frame clock — and
//! `(mirror_id, seq)` is the engine packet's, so the client's lines join the engine's `read`,
//! `serialize` and `flush` and subtract directly. `scripts/mirror-latency/analyze.py` does that.
//!
//! | stage     | when (`mono_ns`)                          | also                                         |
//! |-----------|-------------------------------------------|----------------------------------------------|
//! | `recv`    | the frame's JSON is parsed                | `line_ns` (its line complete), `bytes`       |
//! | `in`      | base64 decoded, admitted, sent to decode  | `flags`, `len`                               |
//! | `gate`    | the gate turned the packet away           | `verdict` (`skip`, `reset`)                  |
//! | `dec`     | the picture came out of libavcodec        | `in_ns`, `width`, `height`, `matrix`, `full_range`, `probe`, `field` |
//! | `nopic`   | the unit decoded to no picture            | `in_ns`                                      |
//! | `damaged` | libavcodec refused the unit               | `in_ns`, `code`                              |
//! | `stale`   | a newer picture replaced it unpainted     |                                              |
//! | `paint`   | its texture was set on the picture widget | `got_ns` (the GTK thread took it)            |
//! | `shown`   | the first frame clock paint showing it    | `frame` (the counter), `paint_ns` (it began) |
//! | `present` | the compositor presented that frame       | `frame`                                      |
//!
//! From `dec` on, `seq` is the picture packet that completed the access unit. `probe` and
//! `field` read the latency probe's barcode off the luma plane (`scripts/mirror-latency/`): the
//! device's `CLOCK_MONOTONIC` when it drew the frame, low 24 bits in 0.1 ms, and its touch field
//! (0 black, 1 white); -1 when the picture is not the probe's.
use std::sync::LazyLock;

static ON: LazyLock<bool> = LazyLock::new(|| std::env::var_os("RELAY_MIRROR_TRACE").is_some_and(|value| !value.is_empty() && value != "0"));

/// Whether tracing is on.
#[inline]
pub fn on() -> bool {
    *ON
}

/// `CLOCK_MONOTONIC` in nanoseconds; 0 with tracing off, so a timestamp taken for the trace costs
/// nothing when nobody reads it.
#[inline]
pub fn now() -> u64 {
    if on() {
        monotonic()
    } else {
        0
    }
}

pub fn monotonic() -> u64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: `ts` is a valid, writable timespec; CLOCK_MONOTONIC always exists on Linux.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

/// `trace!(stage = "dec", mirror_id, seq, …)`: one `mirror_trace` event when tracing is on.
macro_rules! trace {
    ($($field:tt)*) => {
        if $crate::mirror::trace::on() {
            tracing::info!(target: "mirror_trace", $($field)*);
        }
    };
}
pub(crate) use trace;

/// The latency probe's barcode and touch field, read off a picture's luma plane (and its Cr
/// plane, for the magenta marker that says the picture is the probe's). The probe draws, over the
/// left third, a marker stripe then 24 bit stripes (white = 1, most significant first) each
/// 1/26 of the height; the right two thirds are the touch field.
pub fn probe(width: usize, height: usize, luma: &[u8], luma_stride: usize, cr: &[u8], cr_stride: usize) -> Option<(u32, bool)> {
    let stripe = height as f64 / 26.0;
    let x = width / 6;
    let y = |row: f64| (row * stripe) as usize;
    let at = |plane: &[u8], stride: usize, x: usize, y: usize| plane.get(y * stride + x).copied();
    let (marker_y, marker_cr) = (at(luma, luma_stride, x, y(0.5))?, at(cr, cr_stride, x / 2, y(0.5) / 2)?);
    // Magenta: mid luma, Cr far above neutral.
    if !(50..170).contains(&marker_y) || marker_cr <= 170 {
        return None;
    }
    let mut bits = 0u32;
    for bit in 0..24 {
        bits = (bits << 1) | u32::from(at(luma, luma_stride, x, y(bit as f64 + 1.5))? > 128);
    }
    Some((bits, at(luma, luma_stride, width * 2 / 3, height / 2)? > 128))
}

/// The `shown` and `present` events: which frame clock paint first had a picture on screen, and
/// when the compositor presented that frame. Exists only while tracing; disconnects on drop.
pub struct Frames {
    widget: glib::WeakRef<gtk4::Widget>,
    state: std::rc::Rc<FrameState>,
}

#[derive(Default)]
struct FrameState {
    clock: std::cell::RefCell<Option<(gtk4::gdk::FrameClock, [glib::SignalHandlerId; 2])>>,
    /// When the frame clock's current paint began.
    paint_began: std::cell::Cell<u64>,
    /// The picture the view holds now, and the one the last paint showed.
    current: std::cell::Cell<(i64, u64)>,
    shown: std::cell::Cell<(i64, u64)>,
    /// Frames painted with a new picture whose presentation time is not known yet.
    waiting: std::cell::RefCell<std::collections::VecDeque<(i64, i64, u64)>>,
}

impl Frames {
    pub fn watch(widget: &impl glib::prelude::IsA<gtk4::Widget>) -> Self {
        use gtk4::prelude::*;
        Self { widget: widget.upcast_ref::<gtk4::Widget>().downgrade(), state: Default::default() }
    }

    /// The view set this picture. Follows the widget to whichever window's clock it is in now
    /// (docking and undocking move it).
    pub fn painted(&self, mirror: i64, seq: u64) {
        use gtk4::prelude::*;
        self.state.current.set((mirror, seq));
        let Some(clock) = self.widget.upgrade().and_then(|widget| widget.frame_clock()) else { return };
        if self.state.clock.borrow().as_ref().is_some_and(|(watched, _)| *watched == clock) {
            return;
        }
        self.disconnect();
        let state = std::rc::Rc::downgrade(&self.state);
        let began = clock.connect_before_paint(move |_| {
            if let Some(state) = state.upgrade() {
                state.paint_began.set(monotonic());
            }
        });
        let state = std::rc::Rc::downgrade(&self.state);
        let painted = clock.connect_after_paint(move |clock| {
            let Some(state) = state.upgrade() else { return };
            let frame = clock.frame_counter();
            let current = state.current.get();
            if current != state.shown.get() {
                state.shown.set(current);
                trace!(stage = "shown", mono_ns = monotonic(), mirror_id = current.0, seq = current.1, frame, paint_ns = state.paint_began.get());
                state.waiting.borrow_mut().push_back((frame, current.0, current.1));
            }
            let mut waiting = state.waiting.borrow_mut();
            while let Some(&(painted, mirror_id, seq)) = waiting.front() {
                match clock.timings(painted).filter(|timings| timings.is_complete()) {
                    // GDK's clock is g_get_monotonic_time: CLOCK_MONOTONIC in microseconds; 0
                    // when the compositor did not say.
                    Some(timings) => {
                        let presented = timings.presentation_time().max(0) as u64 * 1000;
                        trace!(stage = "present", mono_ns = presented, mirror_id, seq, frame = painted);
                    }
                    // GDK keeps a short history; a frame that never completes is given up on.
                    None if frame - painted < 16 => break,
                    None => {}
                }
                waiting.pop_front();
            }
        });
        *self.state.clock.borrow_mut() = Some((clock, [began, painted]));
    }

    fn disconnect(&self) {
        use gtk4::prelude::*;
        if let Some((clock, handlers)) = self.state.clock.borrow_mut().take() {
            for handler in handlers {
                clock.disconnect(handler);
            }
        }
    }
}

impl Drop for Frames {
    fn drop(&mut self) {
        self.disconnect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_probe_barcode_reads_back_from_a_420_picture() {
        let (w, h) = (96usize, 260usize);
        let (mut luma, mut cr) = (vec![16u8; w * h], vec![128u8; (w / 2) * (h / 2)]);
        let value = 0b1010_0000_1111_0000_1100_0011u32;
        let stripe = h / 26;
        for y in 0..h {
            let row = y / stripe;
            for x in 0..w {
                let (yv, crv) = if x >= w / 3 {
                    (235, 128) // a white touch field
                } else if row == 0 || row >= 25 {
                    (106, 222) // magenta marker
                } else if value >> (24 - row) & 1 == 1 {
                    (235, 128)
                } else {
                    (16, 128)
                };
                luma[y * w + x] = yv;
                cr[(y / 2) * (w / 2) + x / 2] = crv;
            }
        }
        assert_eq!(probe(w, h, &luma, w, &cr, w / 2), Some((value, true)));
        // Anything that is not the probe's screen has no marker.
        assert_eq!(probe(w, h, &vec![200; w * h], w, &cr, w / 2), None);
        assert_eq!(probe(w, h, &[], w, &[], w / 2), None);
    }
}
