//! The native client's pure logic, kept out of the GTK crate so a headless `cargo test` (and CI)
//! runs it. `git_view` is the Git panel's reading of `git.status`, `git.diff` and `git.log`
//! results; `mirror` is the device mirror's video gate and the `device.mirror.input` payloads
//! it sends, which the engine's `tests/native_input.rs` builds with these same functions;
//! `thread_view` is how the Threads page reads an agent's Markdown and tool calls, and
//! `assistant_view` the Dev assistant's cards, pages to open and tool lines; `design` is the
//! design canvas's scene, auto layout, hit testing, snapping and the edits its gestures send.
pub mod assistant_view;
pub mod design;
pub mod git_view;
pub mod mirror;
pub mod thread_view;
