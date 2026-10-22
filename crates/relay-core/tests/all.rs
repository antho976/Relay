//! Most of the engine's integration tests, linked as one executable. Each file under `tests/`
//! was its own crate, and a change to `relay-core` linked all of them again: 67 executables,
//! where one does the same work (docs/perf/FIXES-2026-10-20.md). A file still keeps its own
//! `mod common;`, which resolves to `tests/common/` through the `#[path]` below, so it builds
//! alone or here unchanged.
//!
//! A file that changes process-wide state — `std::env::set_var`, `set_current_dir` — would
//! change it under every other test in this executable, so it stays its own `[[test]]` in
//! `Cargo.toml` instead. `every_test_file_is_built` fails for a file in `tests/` that is in
//! neither place, since `autotests = false` would otherwise skip it without a word.

// Each file's `mod common;` is its own copy of `tests/common/`, as when it was a crate of its own.
#![allow(clippy::duplicate_mod)]

#[path = "agent_features.rs"]
mod agent_features;
#[path = "agent_surface.rs"]
mod agent_surface;
#[path = "arbiter.rs"]
mod arbiter;
#[path = "audit_low_g.rs"]
mod audit_low_g;
#[path = "awareness.rs"]
mod awareness;
#[path = "board.rs"]
mod board;
#[path = "bounded_payloads.rs"]
mod bounded_payloads;
#[path = "branch_cleanup.rs"]
mod branch_cleanup;
#[path = "branch_recheck.rs"]
mod branch_recheck;
#[path = "build_seed.rs"]
mod build_seed;
#[path = "bus.rs"]
mod bus;
#[path = "design_assets.rs"]
mod design_assets;
#[path = "design_chart_fit.rs"]
mod design_chart_fit;
#[path = "design_charts.rs"]
mod design_charts;
#[path = "design_compat.rs"]
mod design_compat;
#[path = "design_components.rs"]
mod design_components;
#[path = "design_contrast.rs"]
mod design_contrast;
#[path = "design_diff.rs"]
mod design_diff;
#[path = "design_errors.rs"]
mod design_errors;
#[path = "design_expect.rs"]
mod design_expect;
#[path = "design_fields.rs"]
mod design_fields;
#[path = "design_find.rs"]
mod design_find;
#[path = "design_icons.rs"]
mod design_icons;
#[path = "design_integrity.rs"]
mod design_integrity;
#[path = "design_pages.rs"]
mod design_pages;
#[path = "design_phone.rs"]
mod design_phone;
#[path = "design_pins.rs"]
mod design_pins;
#[path = "design_render.rs"]
mod design_render;
#[path = "design_reuse.rs"]
mod design_reuse;
#[path = "design_revs.rs"]
mod design_revs;
#[path = "design.rs"]
mod design;
#[path = "design_store.rs"]
mod design_store;
#[path = "device_lease.rs"]
mod device_lease;
#[path = "device_run_worker.rs"]
mod device_run_worker;
#[path = "device_signing.rs"]
mod device_signing;
#[path = "file_handlers.rs"]
mod file_handlers;
#[path = "file_task_audit.rs"]
mod file_task_audit;
#[path = "git_branch_switch.rs"]
mod git_branch_switch;
#[path = "git_handlers.rs"]
mod git_handlers;
#[path = "guardrail_bypasses.rs"]
mod guardrail_bypasses;
#[path = "guardrails.rs"]
mod guardrails;
#[path = "gym.rs"]
mod gym;
#[path = "held_text.rs"]
mod held_text;
#[path = "linked_repos.rs"]
mod linked_repos;
#[path = "mirror.rs"]
mod mirror;
#[path = "native_input.rs"]
mod native_input;
#[path = "phase11.rs"]
mod phase11;
#[path = "phase7.rs"]
mod phase7;
#[path = "phase8.rs"]
mod phase8;
#[path = "phase9.rs"]
mod phase9;
#[path = "pictures.rs"]
mod pictures;
#[path = "project_relink_restore.rs"]
mod project_relink_restore;
#[path = "push_check.rs"]
mod push_check;
#[path = "registry_remove.rs"]
mod registry_remove;
#[path = "reorder.rs"]
mod reorder;
#[path = "retention.rs"]
mod retention;
#[path = "session_lifecycle.rs"]
mod session_lifecycle;
#[path = "session_lows.rs"]
mod session_lows;
#[path = "sessions.rs"]
mod sessions;
#[path = "task_comments.rs"]
mod task_comments;
#[path = "threads.rs"]
mod threads;
#[path = "upload.rs"]
mod upload;
#[path = "video.rs"]
mod video;
#[path = "video_e2e.rs"]
mod video_e2e;
#[path = "worktree_dirty.rs"]
mod worktree_dirty;

#[test]
fn every_test_file_is_built() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let here = include_str!("all.rs");
    let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
    let mut missing = Vec::new();
    for entry in std::fs::read_dir(dir.join("tests")).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        if !name.ends_with(".rs") || name == "all.rs" {
            continue;
        }
        let merged = here.contains(&format!("#[path = \"{name}\"]"));
        let alone = manifest.contains(&format!("path = \"tests/{name}\""));
        if !merged && !alone {
            missing.push(name);
        }
    }
    assert!(
        missing.is_empty(),
        "{missing:?} in tests/ would never run: add `#[path = \"<file>\"] mod <file>;` to tests/all.rs, \
         or a `[[test]]` in Cargo.toml if it changes process-wide state"
    );
}
