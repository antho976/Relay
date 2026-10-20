//! A design's pictures (MODEL-SPEC §6.4 engine-assets): `design.asset.get` reading a stored
//! picture back in pieces; `design.asset.add` storing one durably (B51), refusing one no node
//! could take (B65), reading what an agent names in its worktree (I-10), saying why a file
//! cannot be read (TM-15), sizing a turned JPEG as shown (TM-24).

mod common;

use base64::Engine as _;
use common::{call, call_as, code, committed_repo, engine_with_project, ok, ok_as, refused};
use relay_bus::Actor;
use relay_core::engine::Engine;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// A 1×1 PNG.
const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

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

fn apply(e: &Engine, design: i64, ops: Value) -> Value {
    ok(e, "design.apply", json!({"design_id": design, "ops": ops}))
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

fn add(e: &Engine, design: i64, bytes: &[u8]) -> Value {
    ok(e, "design.asset.add", json!({"design_id": design, "bytes_b64": b64(bytes)}))
}

fn assets_dir(root: &Path) -> PathBuf {
    root.join("store/design-assets")
}

fn files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).into_iter().flatten().flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

/// 2000-01-01: long before any undo window.
fn long_ago() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(946_684_800)
}

fn date(path: &Path, at: SystemTime) {
    std::fs::File::options().write(true).open(path).unwrap().set_modified(at).unwrap();
}

fn modified(path: &Path) -> SystemTime {
    std::fs::metadata(path).unwrap().modified().unwrap()
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

// ---------------------------------------------------------------- design.asset.add

/// B51: a stored copy cut short (a crash between the add and the disk) is written again by the
/// next add of the same picture, never trusted for its name; a whole copy is left as it is and
/// dated now, which the sweep reads as "just named".
#[test]
fn a_picture_cut_short_is_stored_again() {
    let (e, _root) = project();
    let id = create(&e);
    let bytes = unb64(PNG_1X1);
    let first = add(&e, id, &bytes);
    let path = PathBuf::from(first["path"].as_str().unwrap());
    for torn in [&b""[..], &bytes[..10]] {
        std::fs::write(&path, torn).unwrap();
        assert_eq!(add(&e, id, &bytes)["src"], first["src"]);
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "{} bytes were left", torn.len());
    }
    date(&path, long_ago());
    add(&e, id, &bytes);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(modified(&path) > long_ago() + Duration::from_secs(86_400), "an add dates the picture now");
    let leftovers: Vec<String> = files(path.parent().unwrap()).into_iter().filter(|n| n.starts_with(".tmp")).collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

/// B51 for the phone's pictures to an agent: the same rule in the agent's checkout.
#[test]
fn an_upload_cut_short_is_written_again() {
    let (e, _root) = project();
    let s = ok(&e, "session.create", json!({"project_id": 1, "provider": "claude"}));
    let name = s["name"].as_str().unwrap();
    let out = ok(&e, "session.upload", json!({"session": name, "bytes_b64": PNG_1X1}));
    let path = PathBuf::from(out["absolute"].as_str().unwrap());
    std::fs::write(&path, b"\x89PNG").unwrap();
    assert_eq!(ok(&e, "session.upload", json!({"session": name, "bytes_b64": PNG_1X1}))["absolute"], out["absolute"]);
    assert_eq!(std::fs::read(&path).unwrap(), unb64(PNG_1X1));
}

/// B65: a raster's header may say any size; one with a side no image node can take is refused
/// at the add, where the agent can still pick another picture, and nothing is stored.
#[test]
fn a_picture_larger_than_a_node_can_be_is_refused() {
    let (e, root) = project();
    let id = create(&e);
    let before = files(&assets_dir(root.path()));
    for (w, h) in [(4_000_000_000u32, 1u32), (1, 1_000_001)] {
        let error = refused(call(&e, "design.asset.add", json!({"design_id": id, "bytes_b64": b64(&png(w, h, 0))})));
        assert_eq!(error.code, "design.asset");
        assert!(error.message.contains(&format!("{w}×{h} px")) && error.message.contains("at most 1000000"), "{}", error.message);
        assert_eq!(error.hint.as_deref(), Some("scale it down first"));
    }
    assert_eq!(files(&assets_dir(root.path())), before, "nothing was stored");
    let edge = add(&e, id, &png(1_000_000, 1, 0));
    assert_eq!((edge["w"].as_u64(), edge["h"].as_u64()), (Some(1_000_000), Some(1)));
    apply(&e, id, json!([{"op": "add", "node": {"parent": "page-1", "kind": "image", "src": edge["src"], "w": 1_000_000, "h": 1}}]));
}

/// TM-15: what cannot be read is said as the caller can act on it, the path quoted.
#[test]
fn a_file_that_cannot_be_read_says_why() {
    use std::os::unix::fs::PermissionsExt;
    let (e, root) = project();
    let id = create(&e);
    let add_path = |path: &Path| refused(call(&e, "design.asset.add", json!({"design_id": id, "path": path})));

    let missing = add_path(&root.path().join("gone.png "));
    assert_eq!((missing.code.as_str(), missing.kind), ("design.asset", relay_bus::error::ErrorKind::NotFound));
    assert!(missing.message.ends_with("gone.png \""), "a trailing space shows: {}", missing.message);

    let locked = root.path().join("locked.png");
    std::fs::write(&locked, unb64(PNG_1X1)).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    let shut = root.path().join("shut");
    std::fs::create_dir(&shut).unwrap();
    std::fs::write(shut.join("in.png"), unb64(PNG_1X1)).unwrap();
    std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o000)).unwrap();
    // Root reads whatever the mode says; the case only exists for everyone else.
    if std::fs::read(&locked).is_err() {
        for path in [locked.clone(), shut.join("in.png")] {
            let error = add_path(&path);
            assert_eq!((error.code.as_str(), error.kind), ("design.asset", relay_bus::error::ErrorKind::Refused), "{path:?}");
            assert_eq!(error.message, format!("cannot read {:?}: permission denied", path.display().to_string()));
            assert_eq!(error.hint.as_deref(), Some("make it readable by the Relay engine, or pass bytes_b64"));
        }
    }
    std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o755)).unwrap();

    let dir = add_path(root.path());
    assert_eq!(dir.code, "design.asset");
    assert!(dir.message.contains("is not a regular file"), "{}", dir.message);

    let data = refused(call(&e, "design.asset.add", json!({"design_id": id, "bytes_b64": format!("data:image/png;base64,{PNG_1X1}")})));
    assert_eq!(data.code, "design.asset");
    assert_eq!(data.hint.as_deref(), Some("drop the data:…;base64, prefix"));
}

/// I-10: an agent names a picture where it is in its own worktree, as file.* takes a path; out
/// of it, it names the file whole. The person always does.
#[test]
fn an_agent_names_a_picture_by_its_place_in_its_worktree() {
    let (e, root) = project();
    let id = create(&e);
    let (bot, worktree) = agent(&e);
    std::fs::create_dir_all(worktree.join("art")).unwrap();
    std::fs::write(worktree.join("art/dot.png"), unb64(PNG_1X1)).unwrap();
    let by_bytes = add(&e, id, &unb64(PNG_1X1));
    let mine = ok_as(&e, bot.clone(), "design.asset.add", json!({"design_id": id, "path": "art/dot.png"}));
    assert_eq!(mine["src"], by_bytes["src"]);
    assert_eq!(ok_as(&e, bot.clone(), "design.asset.add", json!({"design_id": id, "path": "./art/../art/dot.png"}))["src"], by_bytes["src"]);

    std::fs::write(root.path().join("outside.png"), unb64(PNG_1X1)).unwrap();
    let depth = std::fs::canonicalize(&worktree).unwrap().strip_prefix(std::fs::canonicalize(root.path()).unwrap()).unwrap().components().count();
    let up = refused(call_as(&e, bot.clone(), "design.asset.add", json!({"design_id": id, "path": format!("{}outside.png", "../".repeat(depth))})));
    assert_eq!(up.code, "design.asset");
    assert!(up.message.contains("leads out of your worktree"), "{}", up.message);
    // Its absolute path is read as given.
    let whole = root.path().join("outside.png");
    assert_eq!(ok_as(&e, bot.clone(), "design.asset.add", json!({"design_id": id, "path": whole}))["src"], by_bytes["src"]);
    let none = refused(call_as(&e, bot, "design.asset.add", json!({"design_id": id, "path": "art/none.png"})));
    assert_eq!((none.code.as_str(), none.kind), ("design.asset", relay_bus::error::ErrorKind::NotFound));

    let person = refused(call(&e, "design.asset.add", json!({"design_id": id, "path": "art/dot.png"})));
    assert_eq!(person.code, "design.asset");
    assert!(person.message.contains("is not an absolute path"), "{}", person.message);
}

/// TM-24: a phone's portrait photo, stored landscape and tagged turned, is added at the size
/// it is shown at, so the node an agent sizes from it is not squashed.
#[test]
fn a_turned_photo_is_added_at_the_size_it_is_shown() {
    let (e, _root) = project();
    let id = create(&e);
    // SOI; APP1 Exif (little-endian TIFF, IFD0 with Orientation 6); SOF0 of 200 by 100.
    let mut tiff = b"II*\0\x08\0\0\0\x01\0".to_vec();
    tiff.extend([0x12, 0x01, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0]);
    tiff.extend([0u8; 4]);
    let app1 = [b"Exif\0\0".as_slice(), &tiff].concat();
    let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE1];
    jpeg.extend((app1.len() as u16 + 2).to_be_bytes());
    jpeg.extend(app1);
    jpeg.extend([0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 100, 0x00, 200]);
    let added = add(&e, id, &jpeg);
    assert_eq!((added["w"].as_u64(), added["h"].as_u64()), (Some(100), Some(200)));
}
