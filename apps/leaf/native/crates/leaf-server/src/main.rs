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
//! | `LEAF_DOWNLOADS` | none | Suwayomi's download folder (`mangas/`); its WebP pages are re-encoded smaller, and chapter pictures cut from them |
//! | `LEAF_WEBP_QUALITY` | `60` | the quality they are re-encoded at |
//! | `LEAF_TICK_SECS` | `3600` | how often the autopilot looks at what is due |
//! | `LEAF_SOURCE_GAP_MS` | `3000` (MangaDex `1500`, AniList `2000`) | background requests' spacing per source: `ms`, `Name=ms`, or both, comma-separated |
//! | `LEAF_REC_SOURCES` | `MangaDex,Weeb Central,MangaFire` | the sources searched to tell whether a recommendation can be read |
//! | `LEAF_REC_SEARCHES` | `80` | recommended titles looked for on them per day at most (each kept a week) |
//! | `LEAF_SEARCH_SOURCES` | `MangaDex,Asura Scans,Weeb Central,MangaFire` | the broad sources a search asks; the others only on "Search all sources" |
//! | `LEAF_TAGART` | `<LEAF_DATA>/tagart` | the collections' pictures, `<slug>.webp` (`apps/leaf/tools/tagart.sh` makes them) |
//!
//! Data (`LEAF_DATA`): `leaf.db`, `backups/`, `thumbs/<chapter>.webp`, `recs/covers/`, `tagart/`, and
//! `app/` (`leaf.apk` and `latest.json`, put there by `apps/leaf/tools/publish-apk.sh`).

mod anilist;
mod api;
mod autopilot;
mod budget;
mod disk;
mod lists;
mod phone;
mod promo;
mod recs;
mod squeeze;
mod store;
mod suwa;
mod thumbs;
mod webtoon;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use autopilot::{Ctx, log};

fn var(name: &str, default: &str) -> String {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| default.to_string())
}

/// Whether `bind` ("127.0.0.1:4580", "[::1]:4580", "localhost:4580") listens only on this machine.
fn is_loopback(bind: &str) -> bool {
    if let Ok(a) = bind.parse::<std::net::SocketAddr>() {
        return a.ip().is_loopback();
    }
    bind.rsplit_once(':').is_some_and(|(host, _)| host.eq_ignore_ascii_case("localhost"))
}

fn main() {
    let bind = var("LEAF_BIND", "127.0.0.1:4580");
    // Trimmed as the phone's is: a token pasted with a newline into .env still matches.
    let token = std::env::var("LEAF_TOKEN").ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    let loopback = is_loopback(&bind);
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
            eprintln!("leaf-server: {e} (is {} writable by this user? In Docker, the host folder must be uid 1000's)", data.display());
            std::process::exit(2);
        }
    };
    let downloads = std::env::var("LEAF_DOWNLOADS").ok().filter(|v| !v.trim().is_empty()).map(PathBuf::from);
    let ctx = Arc::new(Ctx {
        suwa: suwa::Suwa::new(&var("SUWAYOMI_URL", "http://127.0.0.1:4567")),
        store: Mutex::new(store),
        lang: var("LEAF_LANG", "en"),
        prefs: var("LEAF_SOURCES", "Asura Scans,MangaDex").split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
        ahead: var("LEAF_AHEAD", "1").parse().unwrap_or(1),
        pages: Mutex::new(HashMap::new()),
        data: data.clone(),
        budget: budget::Budget::new(std::env::var("LEAF_SOURCE_GAP_MS").ok().as_deref()),
        downloads: downloads.clone(),
        thumbs: Mutex::new(HashSet::new()),
        thumbs_wanted: Mutex::new(Vec::new()),
        kick: (Mutex::new(false), Condvar::new()),
        rec_source_names: var("LEAF_REC_SOURCES", "MangaDex,Weeb Central,MangaFire").split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
        rec_searches: var("LEAF_REC_SEARCHES", "80").parse().unwrap_or(80),
        search_sources: var("LEAF_SEARCH_SOURCES", "MangaDex,Asura Scans,Weeb Central,MangaFire").split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
        search_cache: Mutex::new(HashMap::new()),
        promo: Mutex::new(promo::Seen::default()),
        preview_cache: Mutex::new(HashMap::new()),
        suggest_cache: Mutex::new(HashMap::new()),
        tagart: std::env::var("LEAF_TAGART").ok().filter(|v| !v.trim().is_empty()).map(PathBuf::from).unwrap_or_else(|| data.join("tagart")),
    });
    ctx.restore_pauses();
    ctx.load_thumbs();
    ctx.load_promo();
    let tick = Duration::from_secs(var("LEAF_TICK_SECS", "3600").parse().unwrap_or(3600).max(60));

    // The autopilot: waits for Suwayomi, sets it up once, then a pass every `tick`.
    let pilot = ctx.clone();
    let quality: f32 = var("LEAF_WEBP_QUALITY", "60").parse().unwrap_or(60.0);
    let flaresolverr = std::env::var("LEAF_FLARESOLVERR").ok().filter(|v| !v.trim().is_empty());
    let repo = var("LEAF_EXTENSIONS_REPO", "https://raw.githubusercontent.com/keiyoushi/extensions/repo/index.min.json");
    std::thread::spawn(move || {
        let mut ready = false;
        let mut extensions_in = false;
        let mut next_pass = std::time::Instant::now();
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
                    Ok((added, missing)) => {
                        if !added.is_empty() {
                            log(&format!("installed {}", added.join(", ")));
                        }
                        // A name the list lacks is a setting to fix, not something a retry mends.
                        if !missing.is_empty() {
                            log(&format!("LEAF_SOURCES: no extension called {} in the extension list", missing.join(", ")));
                        }
                        extensions_in = true;
                    }
                    Err(e) => log(&format!("extensions: {e}")),
                }
            }
            // The pass keeps its own pace while the extensions are retried every minute: a source
            // down for a while must not count three failures in three minutes.
            if std::time::Instant::now() >= next_pass {
                pilot.tick();
                if let Some(root) = &downloads {
                    let st = &pilot.store;
                    let lock = || st.lock().unwrap_or_else(|p| p.into_inner());
                    let s = squeeze::pass(root, quality, &mut |k| lock().get(&format!("sq:{k}")).is_some(), &mut |k| lock().set(&format!("sq:{k}"), "1"));
                    if s.pages > 0 {
                        log(&format!("pages re-encoded: {} ({} MB to {} MB)", s.pages, s.before >> 20, s.after >> 20));
                    }
                }
                next_pass = std::time::Instant::now() + tick;
            }
            let wait = next_pass.saturating_duration_since(std::time::Instant::now());
            std::thread::sleep(if extensions_in { wait } else { wait.min(Duration::from_secs(60)) });
        }
    });

    // The download line and the pictures asked for: every minute, or at once when woken.
    let line = ctx.clone();
    std::thread::spawn(move || {
        loop {
            line.wait_for_work(Duration::from_secs(60));
            line.drain();
            line.wanted_thumbs();
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

#[cfg(test)]
mod tests {
    use super::is_loopback;

    #[test]
    fn only_this_machine_counts_as_loopback() {
        assert!(is_loopback("127.0.0.1:4580"));
        assert!(is_loopback("[::1]:4580"));
        assert!(is_loopback("localhost:4580"));
        assert!(!is_loopback("0.0.0.0:4580"));
        assert!(!is_loopback("127.example.com:4580"), "a name that starts like an address is not one");
        assert!(!is_loopback("localhost.example.com:4580"));
    }
}
