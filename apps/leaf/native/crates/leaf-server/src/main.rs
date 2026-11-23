//! leaf-server: Leaf's comics autopilot, beside a Suwayomi it drives.
//!
//! Settings come from the environment:
//!
//! | Variable | Default | What |
//! |---|---|---|
//! | `SUWAYOMI_URL` | `http://127.0.0.1:4567` | Suwayomi, unexposed (leaf-server is the only door) |
//! | `LEAF_BIND` | `127.0.0.1:4580` | where the phone reaches leaf-server |
//! | `LEAF_TOKEN` | none | the phone's bearer token; required unless bound to loopback |
//! | `LEAF_DATA` | `./leaf-data` | leaf-server's own record (`leaf.db`) |
//! | `LEAF_LANG` | `en` | the language of the sources searched |
//! | `LEAF_SOURCES` | `Asura Scans,MangaDex` | preferred sources, best first; their extensions are installed at start |
//! | `LEAF_EXTENSIONS_REPO` | Keiyoushi's `index.min.json` | the extension list added when Suwayomi has none |
//! | `LEAF_FLARESOLVERR` | none | a FlareSolverr's address, turned on in Suwayomi for Cloudflare-checked sources |
//! | `LEAF_AHEAD` | `1` | chapters after the last one read kept downloaded |
//! | `LEAF_DOWNLOADS` | none | Suwayomi's download folder; its WebP pages are re-encoded smaller |
//! | `LEAF_WEBP_QUALITY` | `60` | the quality they are re-encoded at |
//! | `LEAF_TICK_SECS` | `3600` | how often the autopilot looks at what is due |

mod anilist;
mod api;
mod autopilot;
mod squeeze;
mod store;
mod suwa;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use autopilot::{Ctx, log};

fn var(name: &str, default: &str) -> String {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| default.to_string())
}

fn main() {
    let bind = var("LEAF_BIND", "127.0.0.1:4580");
    let token = std::env::var("LEAF_TOKEN").ok().filter(|t| !t.trim().is_empty());
    let loopback = bind.starts_with("127.") || bind.starts_with("localhost") || bind.starts_with("[::1]");
    if token.is_none() && !loopback {
        eprintln!("leaf-server: LEAF_TOKEN must be set to listen on {bind}");
        std::process::exit(2);
    }
    let data = PathBuf::from(var("LEAF_DATA", "leaf-data"));
    if let Err(e) = std::fs::create_dir_all(&data) {
        eprintln!("leaf-server: {}: {e}", data.display());
        std::process::exit(2);
    }
    let store = match store::Store::open(&data.join("leaf.db")) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("leaf-server: {e}");
            std::process::exit(2);
        }
    };
    let ctx = Arc::new(Ctx {
        suwa: suwa::Suwa::new(&var("SUWAYOMI_URL", "http://127.0.0.1:4567")),
        store: Mutex::new(store),
        lang: var("LEAF_LANG", "en"),
        prefs: var("LEAF_SOURCES", "Asura Scans,MangaDex").split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
        ahead: var("LEAF_AHEAD", "1").parse().unwrap_or(1),
        pages: Mutex::new(HashMap::new()),
    });
    let tick = Duration::from_secs(var("LEAF_TICK_SECS", "3600").parse().unwrap_or(3600).max(60));

    // The autopilot: waits for Suwayomi, sets it up once, then a pass every `tick`.
    let pilot = ctx.clone();
    let downloads = std::env::var("LEAF_DOWNLOADS").ok().filter(|v| !v.trim().is_empty()).map(PathBuf::from);
    let quality: f32 = var("LEAF_WEBP_QUALITY", "60").parse().unwrap_or(60.0);
    let flaresolverr = std::env::var("LEAF_FLARESOLVERR").ok().filter(|v| !v.trim().is_empty());
    let repo = var("LEAF_EXTENSIONS_REPO", "https://raw.githubusercontent.com/keiyoushi/extensions/repo/index.min.json");
    std::thread::spawn(move || {
        let mut ready = false;
        let mut extensions_in = false;
        loop {
            if !ready {
                match pilot.suwa.version().and_then(|v| pilot.suwa.ensure_settings(flaresolverr.as_deref()).map(|_| v)) {
                    Ok(v) => {
                        log(&format!("Suwayomi {v} is ready"));
                        ready = true;
                    }
                    Err(e) => {
                        log(&format!("waiting for Suwayomi: {e}"));
                        std::thread::sleep(Duration::from_secs(15));
                        continue;
                    }
                }
            }
            // Until every preferred source's extension is in: a list that failed to load (no
            // network yet) is tried again a minute later rather than an hour.
            if !extensions_in {
                match pilot.suwa.ensure_extensions(&pilot.prefs, &pilot.lang, &repo) {
                    Ok(added) => {
                        if !added.is_empty() {
                            log(&format!("installed {}", added.join(", ")));
                        }
                        extensions_in = true;
                    }
                    Err(e) => log(&format!("extensions: {e}")),
                }
            }
            pilot.tick();
            if let Some(root) = &downloads {
                let st = &pilot.store;
                let lock = || st.lock().unwrap_or_else(|p| p.into_inner());
                let s = squeeze::pass(root, quality, &mut |k| lock().get(&format!("sq:{k}")).is_some(), &mut |k| lock().set(&format!("sq:{k}"), "1"));
                if s.pages > 0 {
                    log(&format!("pages re-encoded: {} ({} MB to {} MB)", s.pages, s.before >> 20, s.after >> 20));
                }
            }
            std::thread::sleep(if extensions_in { tick } else { Duration::from_secs(60) });
        }
    });

    let server = match tiny_http::Server::http(&bind) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("leaf-server: {bind}: {e}");
            std::process::exit(2);
        }
    };
    log(&format!("listening on {bind}"));
    let api = Arc::new(api::Api { ctx, token });
    // A few workers: a page fetched from a slow source must not hold up the library.
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let (server, api) = (server.clone(), api.clone());
            std::thread::spawn(move || {
                for req in server.incoming_requests() {
                    api.handle(req);
                }
            })
        })
        .collect();
    for w in workers {
        let _ = w.join();
    }
}
