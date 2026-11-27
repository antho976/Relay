//! `phone.app.*` through the bus: what the PC serves its paired phone for updates (BUS.md
//! §10.21), read from the `phone/` folder beside the store, as `publish-apk.sh` fills it.

mod common;

use base64::Engine as _;
use common::{call, code, committed_repo, engine_with_project, ok};
use serde_json::{json, Value};

#[test]
fn the_phone_reads_the_published_build_and_its_apk_in_pieces() {
    let root = tempfile::tempdir().unwrap();
    let ws = root.path().join("ws");
    let repo = ws.join("app");
    committed_repo(&repo, &[("README.md", "# app\n")]);
    let e = engine_with_project(root.path(), &ws, &repo);

    // Nothing published: no build, and no APK to read.
    assert_eq!(ok(&e, "phone.app.latest", json!({})), Value::Null);
    assert_eq!(code(call(&e, "phone.app.apk", json!({}))), "phone.app_missing");

    let dir = root.path().join("store/phone");
    std::fs::create_dir_all(&dir).unwrap();
    let apk: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
    std::fs::write(dir.join("relay-code.apk"), &apk).unwrap();
    std::fs::write(dir.join("latest.json"), r#"{"build":7,"version":"0.7","size":1000,"sha256":"00","made_at":1}"#).unwrap();

    let l = ok(&e, "phone.app.latest", json!({}));
    assert_eq!((l["build"].as_u64(), l["size"].as_u64()), (Some(7), Some(1000)));
    let mut got = Vec::new();
    loop {
        let p = ok(&e, "phone.app.apk", json!({"offset": got.len(), "len": 300}));
        got.extend(base64::engine::general_purpose::STANDARD.decode(p["bytes_b64"].as_str().unwrap()).unwrap());
        if p["eof"].as_bool().unwrap() {
            break;
        }
    }
    assert_eq!(got, apk, "the pieces make the whole APK");
}
