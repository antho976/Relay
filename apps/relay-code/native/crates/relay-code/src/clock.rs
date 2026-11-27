//! A monotonic clock in nanoseconds since the app started.

use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();

pub fn now_ns() -> u64 {
    START.get_or_init(Instant::now).elapsed().as_nanos() as u64
}
