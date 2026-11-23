//! The phone's API, `/v1/...`, JSON in and out, pages and covers as images. Every request but
//! `/v1/ping` carries `Authorization: Bearer <token>` when a token is set.
//!
//! | Request | Answer |
//! |---|---|
//! | `GET /v1/ping` | `{"ok":true}` |
//! | `GET /v1/status` | [`Status`] |
//! | `GET /v1/library` | `[Series]` |
//! | `GET /v1/series/{id}` | [`SeriesDetail`] |
//! | `DELETE /v1/series/{id}` | unfollows |
//! | `GET /v1/series/{id}/cover` | the cover image |
//! | `GET /v1/search?q=` | `[Candidate]` |
//! | `POST /v1/follow` [`Follow`] | [`Series`] |
//! | `GET /v1/chapters/{id}/pages` | [`Pages`] |
//! | `GET /v1/chapters/{id}/pages/{n}` | the page image (WebP when Suwayomi converted it) |
//! | `POST /v1/chapters/{id}/progress` [`Progress`] | `{"ok":true}` |
//! | `GET /v1/events` | `[Event]`, newest first; `?since=<ms>` the ones after, oldest first |
//! | `POST /v1/series/{id}/bell` [`Bell`] | turns its new-chapter notices on or off |
//! | `POST /v1/series/{id}/read` [`ReadBefore`] | marks every chapter below a number read |
//! | `GET /v1/updates?days=14` | `[Update]`, chapters that arrived, newest first |
//! | `POST /v1/chapters/{id}/read` [`ChapterRead`] | marks that chapter number read or unread |
//! | `POST /v1/series/{id}/shelf` [`ShelfSet`] | puts it on a shelf |
//! | `GET /v1/series/{id}/alternatives` | `[Alternative]`: other entries for the work |
//! | `POST /v1/series/{id}/repin` [`Repin`] | reads it from another entry; answers [`Series`] |
//! | `GET /v1/health` | `[SourceHealth]` |
//! | `GET` / `POST /v1/backup` | the latest nightly backup / one made now (JSON) |
//! | `POST /v1/settings/anilist` [`AnilistToken`] | sets or removes the AniList token |
//! | `POST` / `DELETE /v1/chapters/{id}/download` | downloads a chapter on the server, or removes it |

use std::io::Read;
use std::sync::Arc;

use leaf_proto::{AnilistToken, Bell, ChapterRead, Follow, Pages, Progress, ReadBefore, Repin, ShelfSet, Status};
use serde::Serialize;
use tiny_http::{Header, Method, Request, Response};

use crate::autopilot::{Ctx, log};

pub struct Api {
    pub ctx: Arc<Ctx>,
    pub token: Option<String>,
}

type Reply = Response<std::io::Cursor<Vec<u8>>>;

fn header(k: &str, v: &str) -> Header {
    Header::from_bytes(k.as_bytes(), v.as_bytes()).expect("a header")
}

fn json<T: Serialize>(v: &T) -> Reply {
    let body = serde_json::to_vec(v).unwrap_or_default();
    Response::from_data(body).with_header(header("content-type", "application/json"))
}

fn fail(code: u16, words: &str) -> Reply {
    let body = serde_json::to_vec(&serde_json::json!({ "error": words })).unwrap_or_default();
    Response::from_data(body).with_status_code(code).with_header(header("content-type", "application/json"))
}

fn image(bytes: Vec<u8>, kind: &str) -> Reply {
    Response::from_data(bytes).with_header(header("content-type", kind)).with_header(header("cache-control", "private, max-age=86400"))
}

fn query_param(url: &str, name: &str) -> Option<String> {
    let q = url.split_once('?')?.1;
    q.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == name).then(|| percent_decode(v))
    })
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Same length and same bytes, compared without stopping at the first difference, and over the
/// longer of the two so the time taken does not tell the token's length either.
fn same_secret(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let diff = (0..a.len().max(b.len())).fold(0u8, |acc, i| acc | (a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0xff)));
    diff == 0 && a.len() == b.len()
}

impl Api {
    pub fn handle(&self, mut req: Request) {
        let url = req.url().to_string();
        let path = url.split('?').next().unwrap_or("").to_string();
        let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
        let method = req.method().clone();
        let authorised = match &self.token {
            None => true,
            Some(t) => req
                .headers()
                .iter()
                .find(|h| h.field.equiv("authorization"))
                .and_then(|h| h.value.as_str().strip_prefix("Bearer "))
                .is_some_and(|given| same_secret(given.trim(), t)),
        };
        let reply = if parts.as_slice() == ["v1", "ping"] {
            json(&serde_json::json!({ "ok": true }))
        } else if !authorised {
            fail(401, "this server needs its token")
        } else {
            let mut body = String::new();
            if matches!(method, Method::Post) {
                let _ = req.as_reader().take(1 << 20).read_to_string(&mut body);
            }
            self.route(&method, &parts, &url, &body)
        };
        let _ = req.respond(reply);
    }

    fn route(&self, method: &Method, parts: &[&str], url: &str, body: &str) -> Reply {
        let ctx = &self.ctx;
        let id = |i: usize| parts.get(i).and_then(|s| s.parse::<i64>().ok());
        let result: Result<Reply, String> = match (method, parts) {
            (Method::Get, ["v1", "status"]) => {
                let sources = ctx.suwa.sources(&ctx.lang).map(|v| v.into_iter().map(|s| s.name).collect()).unwrap_or_default();
                let series = ctx.store.lock().map(|s| s.works().len() as u32).unwrap_or(0);
                Ok(json(&Status { server: env!("CARGO_PKG_VERSION").into(), suwayomi: ctx.suwa.version().ok(), sources, series, anilist: ctx.anilist_on() }))
            }
            (Method::Get, ["v1", "library"]) => Ok(json(&ctx.library())),
            (Method::Get, ["v1", "events"]) => Ok(json(&match query_param(url, "since").and_then(|v| v.parse::<i64>().ok()) {
                Some(since) => ctx.store.lock().map(|s| s.events_since(since)).unwrap_or_default(),
                None => ctx.store.lock().map(|s| s.events(100)).unwrap_or_default(),
            })),
            (Method::Get, ["v1", "updates"]) => {
                let days = query_param(url, "days").and_then(|v| v.parse::<i64>().ok()).unwrap_or(14);
                Ok(json(&ctx.updates(days)))
            }
            (Method::Get, ["v1", "health"]) => Ok(json(&ctx.health())),
            (Method::Post, ["v1", "chapters", _, "read"]) => {
                let Some(c) = id(2) else { return fail(400, "bad id") };
                serde_json::from_str::<ChapterRead>(body).map_err(|e| format!("read: {e}")).and_then(|r| ctx.mark_chapter(c, r.read)).map(|_| json(&serde_json::json!({ "ok": true })))
            }
            (Method::Post, ["v1", "series", _, "shelf"]) => {
                let Some(i) = id(2) else { return fail(400, "bad id") };
                serde_json::from_str::<ShelfSet>(body).map_err(|e| format!("shelf: {e}")).and_then(|s| ctx.set_shelf(i, s.shelf.as_deref())).map(|_| json(&serde_json::json!({ "ok": true })))
            }
            (Method::Get, ["v1", "series", _, "alternatives"]) => id(2).ok_or("bad id".to_string()).and_then(|i| ctx.alternatives(i)).map(|a| json(&a)),
            (Method::Post, ["v1", "series", _, "repin"]) => {
                let Some(i) = id(2) else { return fail(400, "bad id") };
                serde_json::from_str::<Repin>(body).map_err(|e| format!("repin: {e}")).and_then(|r| ctx.repin(i, r.manga_id)).map(|s| json(&s))
            }
            (Method::Get, ["v1", "backup"]) | (Method::Post, ["v1", "backup"]) => {
                let path = if matches!(method, Method::Post) { ctx.backup_now().map(Some) } else { Ok(ctx.latest_backup()) };
                match path {
                    Ok(Some(p)) => std::fs::read(&p).map_err(|e| e.to_string()).map(|bytes| {
                        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "leaf-backup.json".into());
                        Response::from_data(bytes)
                            .with_header(header("content-type", "application/json"))
                            .with_header(header("content-disposition", &format!("attachment; filename=\"{name}\"")))
                    }),
                    Ok(None) => Err("no backup yet".to_string()),
                    Err(e) => Err(e),
                }
            }
            (Method::Post, ["v1", "settings", "anilist"]) => serde_json::from_str::<AnilistToken>(body)
                .map_err(|e| format!("anilist: {e}"))
                .map(|t| {
                    ctx.set_anilist(t.token.as_deref());
                    json(&serde_json::json!({ "ok": true, "anilist": ctx.anilist_on() }))
                }),
            (Method::Post, ["v1", "series", _, "bell"]) => {
                let Some(i) = id(2) else { return fail(400, "bad id") };
                serde_json::from_str::<Bell>(body).map_err(|e| format!("bell: {e}")).and_then(|b| ctx.set_bell(i, b.on)).map(|_| json(&serde_json::json!({ "ok": true })))
            }
            (Method::Post, ["v1", "chapters", _, "download"]) => {
                id(2).ok_or("bad id".to_string()).and_then(|c| ctx.download(c, true)).map(|_| json(&serde_json::json!({ "ok": true })))
            }
            (Method::Delete, ["v1", "chapters", _, "download"]) => {
                id(2).ok_or("bad id".to_string()).and_then(|c| ctx.download(c, false)).map(|_| json(&serde_json::json!({ "ok": true })))
            }
            (Method::Get, ["v1", "series", _]) => id(2).ok_or("bad id".to_string()).and_then(|i| ctx.detail(i)).map(|d| json(&d)),
            (Method::Delete, ["v1", "series", _]) => id(2).ok_or("bad id".to_string()).and_then(|i| ctx.unfollow(i)).map(|_| json(&serde_json::json!({ "ok": true }))),
            (Method::Get, ["v1", "series", _, "cover"]) => {
                let manga = id(2).and_then(|i| ctx.store.lock().ok()?.work(i)).map(|w| w.manga_id);
                manga.ok_or("no such series".to_string()).and_then(|m| ctx.suwa.bytes(&format!("/api/v1/manga/{m}/thumbnail"))).map(|(b, k)| image(b, &k))
            }
            (Method::Get, ["v1", "covers", _]) => id(2)
                .ok_or("bad id".to_string())
                .and_then(|m| ctx.suwa.bytes(&format!("/api/v1/manga/{m}/thumbnail")))
                .map(|(b, k)| image(b, &k)),
            (Method::Get, ["v1", "search"]) => match query_param(url, "q").filter(|q| !q.trim().is_empty()) {
                Some(q) => ctx.search(q.trim()).map(|c| json(&c)),
                None => Err("search for what?".into()),
            },
            (Method::Post, ["v1", "follow"]) => serde_json::from_str::<Follow>(body)
                .map_err(|e| format!("follow: {e}"))
                .and_then(|f| ctx.follow(&f.title, &f.manga_ids).map(|s| (s, f.quiet)))
                .map(|(s, quiet)| {
                    if quiet {
                        ctx.compare_soon(s.id);
                    } else {
                        // The other sources are looked at right away, off this request.
                        let (c, id) = (self.ctx.clone(), s.id);
                        std::thread::spawn(move || c.compare_now(id));
                    }
                    json(&s)
                }),
            (Method::Post, ["v1", "series", _, "read"]) => {
                let Some(i) = id(2) else { return fail(400, "bad id") };
                serde_json::from_str::<ReadBefore>(body)
                    .map_err(|e| format!("read: {e}"))
                    .and_then(|r| ctx.read_before(i, r.before))
                    .map(|n| json(&serde_json::json!({ "ok": true, "marked": n })))
            }
            (Method::Get, ["v1", "chapters", _, "pages"]) => {
                id(2).ok_or("bad id".to_string()).and_then(|c| ctx.pages(c).map(|p| json(&Pages { chapter_id: c, count: p.len() as u32 })))
            }
            (Method::Get, ["v1", "chapters", _, "pages", _]) => {
                let (Some(c), Some(n)) = (id(2), id(4)) else { return fail(400, "bad page") };
                let got = ctx.pages(c).and_then(|p| p.get(n as usize).cloned().ok_or("no such page".to_string())).and_then(|u| ctx.suwa.bytes(&u));
                if got.is_err() {
                    // The phone asks again; the list is fetched afresh, from another source if
                    // this one keeps refusing.
                    ctx.forget_pages(c);
                }
                got.map(|(b, k)| image(b, &k))
            }
            (Method::Post, ["v1", "chapters", _, "progress"]) => {
                let Some(c) = id(2) else { return fail(400, "bad id") };
                serde_json::from_str::<Progress>(body)
                    .map_err(|e| format!("progress: {e}"))
                    .and_then(|p| ctx.progress(c, p.page, p.done))
                    .map(|_| json(&serde_json::json!({ "ok": true })))
            }
            _ => return fail(404, "no such thing"),
        };
        match result {
            Ok(r) => r,
            Err(e) => {
                log(&format!("{} {url}: {e}", method));
                fail(502, &e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_words_decode() {
        assert_eq!(query_param("/v1/search?q=junk%20to+king", "q").as_deref(), Some("junk to king"));
        assert_eq!(query_param("/v1/search?x=1&q=caf%C3%A9", "q").as_deref(), Some("café"));
        assert_eq!(query_param("/v1/search?q=100%", "q").as_deref(), Some("100%"));
        assert_eq!(query_param("/v1/search", "q"), None);
    }

    #[test]
    fn tokens_compare_whole() {
        assert!(same_secret("abc", "abc"));
        assert!(!same_secret("abc", "abd"));
        assert!(!same_secret("abc", "abcd"));
        assert!(!same_secret("abcd", "abc"));
        assert!(!same_secret("", "a"));
    }
}
