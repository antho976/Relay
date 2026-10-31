//! RELAY v4 engine (BUS.md §5, §6): the store, the audit log, the request pipeline and the
//! socket door. `relay serve` runs exactly this, with or without a display; the native client
//! and the phone door are clients of its socket.

// `BusError` is ~250 bytes and is the *normal* path for typed refusals; boxing it in every
// handler signature would obscure the API for no measurable gain.
#![allow(clippy::result_large_err)]

pub mod audit;
pub mod awareness;
pub mod branch_cleanup;
pub mod build_seed;
pub mod check;
pub mod device;
pub mod device_lease;
mod design_render;
pub mod disk_sweep;
pub mod engine;
pub mod gaming;
pub mod guardrail;
pub mod github;
pub mod handlers;
pub mod hooks;
pub mod linked;
pub mod local_models;
pub mod mirror;
pub mod paths;
pub mod peer;
pub mod picture;
pub mod pictures;
pub mod plugins;
pub mod private;
pub mod proc;
pub mod providers;
mod provider_updates;
pub mod pty;
pub mod purge;
pub mod figures;
pub mod readings;
pub mod arbiter;
pub mod gym;
pub mod aide;
pub mod recovery;
pub mod redact;
pub mod serve;
pub mod sessions;
mod shell;
pub mod skills;
pub mod socket;
pub mod store;
pub mod threads;
pub mod time;
pub mod transcript;
pub mod usage;
pub mod video;
pub mod watch;
pub mod worktree;

pub use engine::{Door, Engine};
pub use paths::Instance;
pub use store::Store;

/// How far below normal a background worker runs.
const BACKGROUND_NICE: i32 = 10;

/// Drop the calling thread to background priority.
///
/// Relay runs beside whatever else the machine is doing — a game, a compile, a call — and its
/// filesystem walks are the part that will use every core it is handed. On Linux
/// `setpriority(PRIO_PROCESS, 0, …)` applies to the calling thread alone, so a worker can step
/// down without the rest of the process following it. Raising a nice value never requires
/// privilege, and failing to raise it only costs the courtesy, so the result is ignored.
pub fn background_priority() {
    #[cfg(target_os = "linux")]
    unsafe {
        libc::setpriority(libc::PRIO_PROCESS, 0, BACKGROUND_NICE);
    }
}

/// Lowercase hex for a byte string, in one allocation.
///
/// Session tokens and every audit payload hash go through here. `bytes.iter().map(|b|
/// format!("{b:02x}")).collect()` is the same string built out of one heap allocation per byte.
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
/// Before `main`, while this test process has one thread: its git (fixtures and the engine's own
/// calls alike) reads no global or system config (RA-672), so a developer's `core.hooksPath`,
/// signing or templates never reach a test. The real engine keeps reading the user's config;
/// only test binaries carry this.
#[ctor::ctor(unsafe)]
fn hermetic_git() {
    // SAFETY: runs before main, so no other thread can be reading the environment.
    unsafe {
        libc::setenv(c"GIT_CONFIG_GLOBAL".as_ptr(), c"/dev/null".as_ptr(), 1);
        libc::setenv(c"GIT_CONFIG_NOSYSTEM".as_ptr(), c"1".as_ptr(), 1);
    }
}

#[cfg(test)]
mod hex_tests {
    #[test]
    fn hex_matches_the_per_byte_formatter() {
        for case in [vec![], vec![0u8], vec![0x0f, 0xf0, 0xff], (0u8..=255).collect::<Vec<_>>()] {
            let expected: String = case.iter().map(|b| format!("{b:02x}")).collect();
            assert_eq!(super::hex(&case), expected);
        }
    }
}
