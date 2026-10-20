//! A design's pictures (MODEL-SPEC §6.4 engine-assets): `design.asset.get` reading a stored
//! picture back in pieces.

mod common;

use base64::Engine as _;
use common::{call, call_as, committed_repo, engine_with_project, ok, refused};
use relay_bus::Actor;
use relay_core::engine::Engine;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn unb64(text: &str) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD.decode(text).unwrap()
}

/// A store on disk (so pictures land beside it) with project 1.
fn project() -> (Arc<Engine>, tempfile::TempDir) {
    let root = tempfile::tempdir().unwrap();
    let ws = root.path().join("ws");
    let repo = ws.join("app");
    committed_repo(&repo, &[("README.md", "# app\n")]);
    let engine = engine_with_project(root.path(), &ws, &repo);
    (engine, root)
}

fn create(e: &Engine) -> i64 {
    ok(e, "design.create", json!({"project_id": 1, "title": "Home"}))["design"]["id"].as_i64().unwrap()
}

/// A PNG of `w` by `h` as far as its header says: the signature and IHDR, `padding` bytes after
/// (the size is all `design.asset.add` reads).
fn png(w: u32, h: u32, padding: usize) -> Vec<u8> {
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    png.extend(w.to_be_bytes());
    png.extend(h.to_be_bytes());
    png.extend([8, 2, 0, 0, 0, 0, 0, 0, 0]);
    png.extend(std::iter::repeat_n(0x5Au8, padding));
    png
}

fn assets_dir(root: &Path) -> PathBuf {
    root.join("store/design-assets")
}

/// An agent's session in project 1, and its checkout.
fn agent(e: &Engine) -> (Actor, PathBuf) {
    let s = ok(e, "session.create", json!({"project_id": 1, "provider": "claude"}));
    (Actor::agent(s["name"].as_str().unwrap()), PathBuf::from(s["worktree"].as_str().unwrap()))
}

// ---------------------------------------------------------------- design.asset.get

/// The phone draws a design's pictures from these pieces (feature-phone §3.2): 3 MiB at a time,
/// the last one saying so, the whole as stored.
#[test]
fn a_picture_reads_back_in_pieces() {
    let (e, root) = project();
    let id = create(&e);
    let picture = png(640, 480, 3 * 1024 * 1024 + 512 * 1024);
    let file = root.path().join("big.png");
    std::fs::write(&file, &picture).unwrap();
    let added = ok(&e, "design.asset.add", json!({"design_id": id, "path": file}));
    let src = added["src"].as_str().unwrap().to_string();

    let first = ok(&e, "design.asset.get", json!({"design_id": id, "src": src}));
    let piece = 3 * 1024 * 1024u64;
    assert_eq!((first["media_type"].as_str(), first["size"].as_u64(), first["next"].as_u64()), (Some("image/png"), Some(picture.len() as u64), Some(piece)));
    let rest = ok(&e, "design.asset.get", json!({"design_id": id, "src": src, "offset": piece}));
    assert_eq!(rest["next"], Value::Null, "the last piece");
    let whole = [unb64(first["bytes_b64"].as_str().unwrap()), unb64(rest["bytes_b64"].as_str().unwrap())].concat();
    assert_eq!(whole, picture);

    // A small picture is one piece, and an SVG names its type.
    let svg = ok(&e, "design.asset.add", json!({"design_id": id, "bytes_b64": b64(br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 2"/>"#)}));
    let got = ok(&e, "design.asset.get", json!({"design_id": id, "src": svg["src"]}));
    assert_eq!((got["media_type"].as_str(), got["next"].clone()), (Some("image/svg+xml"), Value::Null));

    let refusal = |payload: Value| refused(call(&e, "design.asset.get", payload));
    let past = refusal(json!({"design_id": id, "src": src, "offset": picture.len()}));
    assert_eq!(past.code, "design.asset");
    assert!(past.message.contains(&format!("({} bytes)", picture.len())), "{}", past.message);
    assert_eq!(refusal(json!({"design_id": id, "src": "../x.png"})).code, "design.asset", "not a picture's name");
    let absent = refusal(json!({"design_id": id, "src": format!("{}.png", "0".repeat(32))}));
    assert_eq!((absent.code.as_str(), absent.kind), ("design.asset_not_found", relay_bus::error::ErrorKind::NotFound));
    assert_eq!(refusal(json!({"design_id": 999, "src": src})).code, "design.not_found");
    // Only the person's clients read pictures this way: an agent has assets_dir.
    let (bot, _) = agent(&e);
    let agents = refused(call_as(&e, bot, "design.asset.get", json!({"design_id": id, "src": src})));
    assert!(agents.code.starts_with("actor."), "{agents:?}");

    // A deleted design's pictures still read, as the design does.
    ok(&e, "design.delete", json!({"design_id": id}));
    assert_eq!(ok(&e, "design.asset.get", json!({"design_id": id, "src": src}))["size"], picture.len());
    // A copy a crash left empty is no picture.
    std::fs::write(assets_dir(root.path()).join(&src), b"").unwrap();
    assert_eq!(refusal(json!({"design_id": id, "src": src})).code, "design.asset_not_found");
}
