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
//! | `GET /v1/search?q=` | `[Candidate]`: the broad sources (`LEAF_SEARCH_SOURCES`); `&all=1` every source, `&source=<name>` one; only titles with every word searched, at most 20, each source's answer kept 10 min |
//! | `GET /v1/search/sources` | [`SearchSources`]: the broad sources, asked one by one by the phone |
//! | `GET /v1/search/suggest?q=` | `{"title":…}`: AniList's title the words probably meant ("Did you mean"), or null; kept a day |
//! | `GET /v1/preview?manga_id=a,b` | a title not added yet, as [`SeriesDetail`] (id 0): the first entry that answers, asked now, kept 10 min; nothing is added. An entry already in My space answers its series (its id) |
//! | `POST /v1/follow` [`Follow`] | [`Series`] |
//! | `GET /v1/chapters/{id}/pages` | [`Pages`] |
//! | `GET /v1/chapters/{id}/pages/{n}` | the page image (WebP when Suwayomi converted it); `204` with `x-leaf-skip: 1` for a scanlator's promo page (`promo.rs`) unless `?promo=1` |
//! | `POST` / `DELETE /v1/pages/hide` [`PageHide`] | hides a page and the ones like it, for good / undoes that (`all`: every one); `{"ok":true,"count":n}` |
//! | `GET /v1/pages/hidden` | [`HiddenPages`]: how many pages were hidden by hand |
//! | `POST /v1/chapters/{id}/progress` [`Progress`] | `{"ok":true}` |
//! | `GET /v1/events` | `[Event]`, newest first; `?since=<ms>` the ones after, oldest first |
//! | `POST /v1/series/{id}/bell` [`Bell`] | turns its new-chapter notices on or off |
//! | `POST /v1/series/{id}/watch` [`Bell`] | a series only on Webtoon: one notice ("elsewhere") when another source has it |
//! | `POST /v1/series/{id}/read` [`ReadBefore`] | marks every chapter below a number read (and, asked, every one from it unread) |
//! | `GET /v1/updates?days=14` | `[Update]`, chapters that arrived, newest first |
//! | `POST /v1/chapters/{id}/read` [`ChapterRead`] | marks that chapter number read or unread |
//! | `POST /v1/series/{id}/shelf` [`ShelfSet`] | puts it on a shelf |
//! | `GET /v1/series/{id}/alternatives` | `[Alternative]`: other entries for the work |
//! | `POST /v1/series/{id}/repin` [`Repin`] | reads it from another entry; answers [`Series`] |
//! | `GET /v1/health` | `[SourceHealth]` |
//! | `GET` / `POST /v1/backup` | the latest nightly backup / one made now (JSON) |
//! | `POST /v1/settings/anilist` [`AnilistToken`] | sets or removes the AniList token |
//! | `POST` / `DELETE /v1/chapters/{id}/download` | downloads a chapter on the server now, or removes it |
//! | `POST` / `DELETE /v1/downloads` [`DownloadIds`] | queues chapters to download (drained per source), or removes them; `{"ok":true,"count":n}` |
//! | `POST /v1/series/{id}/download` [`SeriesDownload`] | queues chapters `from`–`to`, or every unread one; `{"ok":true,"count":n}` |
//! | `GET /v1/chapters/{id}/thumb` | the chapter's picture (WebP, 240 × 180); 404 at once while there is none |
//! | `GET /v1/recs` | [`Recs`], made once a day |
//! | `GET /v1/recs/cover/{anilist_id}` | a recommendation's cover |
//! | `GET /v1/recs/tagart/{slug}` | a collection's picture (`<slug>.webp` in `LEAF_TAGART`, 640 × 360; the slug is the tag lower-cased, spaces as `-`); 404 when there is none |
//! | `POST /v1/recs/{anilist_id}/hide` | "Not interested", for good |
//! | `POST /v1/recs/tags` [`RecTag`] | fewer recommendations with a tag, or as before |
//! | `GET /v1/series/{id}/also` | `[Rec]`: "Readers also like", kept a day |
//! | `GET /v1/app/latest` | [`AppLatest`]; 404 when the server holds no app |
//! | `GET /v1/app/apk` | the app (`application/vnd.android.package-archive`) |
//! | `GET /v1/phone/files`, `PUT` / `GET /v1/phone/files/{kind}/{name}` | the phone's backup of its books (`phone.rs`) |
//!
//! `/v1/library` and `/v1/series/{id}` send each series with `paid` ([`leaf_proto::PaidAhead`] or
//! null, less the chapters the series has now), `watch` (null unless only Webtoon has it, then
//! whether `/watch` is on) and each chapter with `thumb`; `/v1/health` each source with `paused_until` and
//! `requests_today`: fields the phone's structs gain when it reads them (see leaf-proto).

use std::io::Read;
use std::sync::Arc;

use leaf_proto::{AnilistToken, AppLatest, Bell, ChapterRead, DownloadIds, Follow, HiddenPages, PageHide, Pages, Progress, ReadBefore, RecTag, Repin, SearchSources, SeriesDownload, ShelfSet, Status};
use serde::Serialize;
use tiny_http::{Header, Method, Request, Response};

use crate::autopilot::{Ctx, Scope, log};
use crate::promo::PageOut;

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

/// A collection's picture: `<slug>.webp` in `dir`, for a slug of lower-case letters, digits and
/// `-` only (nothing that climbs out of the folder).
fn tagart_file(dir: &std::path::Path, slug: &str) -> Option<std::path::PathBuf> {
    let ok = !slug.is_empty() && slug.len() <= 64 && slug.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') && !slug.starts_with('-');
    ok.then(|| dir.join(format!("{slug}.webp")))
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
        if authorised && matches!(method, Method::Get) && parts.as_slice() == ["v1", "app", "apk"] {
            // Streamed from disk with its length (never chunked: the phone shows progress), not
            // read into memory.
            let path = self.ctx.data.join("app").join("leaf.apk");
            let _ = match std::fs::File::open(&path) {
                Ok(f) => req.respond(
                    Response::from_file(f)
                        .with_chunked_threshold(usize::MAX)
                        .with_header(header("content-type", "application/vnd.android.package-archive"))
                        .with_header(header("content-disposition", "attachment; filename=\"leaf.apk\"")),
                ),
                Err(_) => req.respond(fail(404, "no app on this server")),
            };
            return;
        }
        // The phone's backup (`phone.rs`): files in and out as bytes, not JSON.
        if authorised && parts.len() >= 3 && parts[..3] == ["v1", "phone", "files"] {
            let data = &self.ctx.data;
            let reply = match (&method, &parts[3..]) {
                (Method::Get, []) => json(&crate::phone::list(data)),
                (Method::Put, [kind, name]) => match crate::phone::put(data, kind, &percent_decode(name), req.as_reader()) {
                    Ok(n) => json(&serde_json::json!({ "ok": true, "size": n })),
                    Err((code, why)) => fail(code, &why),
                },
                (Method::Get, [kind, name]) => match crate::phone::file(data, kind, &percent_decode(name)).and_then(|p| std::fs::File::open(p).ok()) {
                    Some(f) => {
                        let _ = req.respond(Response::from_file(f).with_chunked_threshold(usize::MAX).with_header(header("content-type", "application/octet-stream")));
                        return;
                    }
                    None => fail(404, "not kept"),
                },
                _ => fail(404, "no such request"),
            };
            let _ = req.respond(reply);
            return;
        }
        let reply = if parts.as_slice() == ["v1", "ping"] {
            json(&serde_json::json!({ "ok": true }))
        } else if !authorised {
            fail(401, "this server needs its token")
        } else {
            let mut body = String::new();
            if matches!(method, Method::Post | Method::Delete) {
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
            (Method::Get, ["v1", "library"]) => Ok(json(&ctx.library_json())),
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
            (Method::Post, ["v1", "series", _, "watch"]) => {
                let Some(i) = id(2) else { return fail(400, "bad id") };
                serde_json::from_str::<Bell>(body).map_err(|e| format!("watch: {e}")).and_then(|b| ctx.set_watch(i, b.on)).map(|_| json(&serde_json::json!({ "ok": true })))
            }
            (Method::Post, ["v1", "chapters", _, "download"]) => {
                id(2).ok_or("bad id".to_string()).and_then(|c| ctx.download(c, true)).map(|_| json(&serde_json::json!({ "ok": true })))
            }
            (Method::Delete, ["v1", "chapters", _, "download"]) => {
                id(2).ok_or("bad id".to_string()).and_then(|c| ctx.download(c, false)).map(|_| json(&serde_json::json!({ "ok": true })))
            }
            (Method::Get, ["v1", "series", _]) => id(2).ok_or("bad id".to_string()).and_then(|i| ctx.detail_json(i)).map(|d| json(&d)),
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
                Some(q) => {
                    let scope = match (query_param(url, "source").filter(|s| !s.trim().is_empty()), query_param(url, "all").as_deref()) {
                        (Some(s), _) => Scope::One(s),
                        (None, Some("1")) => Scope::All,
                        _ => Scope::Broad,
                    };
                    ctx.search(q.trim(), &scope).map(|c| json(&c))
                }
                None => Err("search for what?".into()),
            },
            (Method::Get, ["v1", "search", "sources"]) => ctx.search_names(&Scope::Broad).map(|(broad, all)| json(&SearchSources { broad, all })),
            (Method::Get, ["v1", "search", "suggest"]) => match query_param(url, "q").filter(|q| !q.trim().is_empty()) {
                Some(q) => ctx.suggest(q.trim()).map(|t| json(&serde_json::json!({ "title": t }))),
                None => Err("suggest for what?".into()),
            },
            (Method::Get, ["v1", "preview"]) => {
                let ids: Vec<i64> = query_param(url, "manga_id").unwrap_or_default().split(',').filter_map(|v| v.trim().parse().ok()).collect();
                if ids.is_empty() {
                    return fail(400, "preview what?");
                }
                ctx.preview_json(&ids).map(|v| json(&v))
            }
            (Method::Get, ["v1", "pages", "hidden"]) => Ok(json(&HiddenPages { count: ctx.hidden_count() })),
            (Method::Post, ["v1", "pages", "hide"]) => serde_json::from_str::<PageHide>(body)
                .map_err(|e| format!("hide: {e}"))
                .and_then(|h| ctx.hide_page(h.chapter, h.page))
                .map(|n| json(&serde_json::json!({ "ok": true, "count": n }))),
            (Method::Delete, ["v1", "pages", "hide"]) => serde_json::from_str::<PageHide>(body).map_err(|e| format!("hide: {e}")).map(|h| {
                let n = ctx.unhide_page(if h.all { None } else { Some((h.chapter, h.page)) });
                json(&serde_json::json!({ "ok": true, "count": n }))
            }),
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
                    .and_then(|r| ctx.read_before(i, r.before, r.unread_after))
                    .map(|n| json(&serde_json::json!({ "ok": true, "marked": n })))
            }
            (Method::Get, ["v1", "chapters", _, "pages"]) => {
                id(2).ok_or("bad id".to_string()).and_then(|c| ctx.pages(c).map(|p| json(&Pages { chapter_id: c, count: p.urls.len() as u32 })))
            }
            (Method::Get, ["v1", "chapters", _, "pages", _]) => {
                let (Some(c), Some(n)) = (id(2), id(4)) else { return fail(400, "bad page") };
                let skip = query_param(url, "promo").as_deref() != Some("1");
                let got = ctx.page_out(c, n.max(0) as u32, skip);
                if got.is_err() {
                    // The phone asks again; the list is fetched afresh, from another source if
                    // this one keeps refusing.
                    ctx.forget_pages(c);
                }
                got.map(|p| match p {
                    PageOut::Bytes(b, k) => image(b, &k),
                    PageOut::Skip => Response::from_data(Vec::new()).with_status_code(204).with_header(header("x-leaf-skip", "1")),
                })
            }
            (Method::Post, ["v1", "chapters", _, "progress"]) => {
                let Some(c) = id(2) else { return fail(400, "bad id") };
                serde_json::from_str::<Progress>(body)
                    .map_err(|e| format!("progress: {e}"))
                    .and_then(|p| ctx.progress(c, p.page, p.done))
                    .map(|_| json(&serde_json::json!({ "ok": true })))
            }
            (Method::Get, ["v1", "chapters", _, "thumb"]) => {
                let Some(c) = id(2) else { return fail(400, "bad id") };
                match ctx.thumb(c) {
                    Some(b) => Ok(image(b, "image/webp")),
                    None => return fail(404, "no picture yet"),
                }
            }
            (Method::Post, ["v1", "downloads"]) | (Method::Delete, ["v1", "downloads"]) => serde_json::from_str::<DownloadIds>(body)
                .map_err(|e| format!("downloads: {e}"))
                .and_then(|d| ctx.download_many(&d.ids, matches!(method, Method::Post)))
                .map(|n| json(&serde_json::json!({ "ok": true, "count": n }))),
            (Method::Post, ["v1", "series", _, "download"]) => {
                let Some(i) = id(2) else { return fail(400, "bad id") };
                serde_json::from_str::<SeriesDownload>(body)
                    .map_err(|e| format!("download: {e}"))
                    .and_then(|r| ctx.download_series(i, r))
                    .map(|n| json(&serde_json::json!({ "ok": true, "count": n })))
            }
            (Method::Get, ["v1", "recs"]) => Ok(json(&ctx.recs())),
            (Method::Get, ["v1", "recs", "cover", _]) => match id(3).ok_or("bad id".to_string()).and_then(|a| ctx.rec_cover(a)) {
                Err(e) if e == crate::recs::NO_COVER => return fail(404, &e),
                r => r.map(|(b, k)| image(b, &k)),
            },
            (Method::Get, ["v1", "recs", "tagart", slug]) => match tagart_file(&ctx.tagart, slug).and_then(|p| std::fs::read(p).ok()) {
                Some(b) => Ok(image(b, "image/webp")),
                None => return fail(404, "no picture for that tag"),
            },
            (Method::Post, ["v1", "recs", "tags"]) => serde_json::from_str::<RecTag>(body).map_err(|e| format!("tags: {e}")).map(|t| {
                ctx.rec_tag(&t.tag, t.less);
                json(&serde_json::json!({ "ok": true }))
            }),
            (Method::Post, ["v1", "recs", _, "hide"]) => {
                let Some(a) = id(2) else { return fail(400, "bad id") };
                ctx.hide_rec(a);
                Ok(json(&serde_json::json!({ "ok": true })))
            }
            (Method::Get, ["v1", "series", _, "also"]) => id(2).ok_or("bad id".to_string()).and_then(|i| ctx.also(i)).map(|r| json(&r)),
            (Method::Get, ["v1", "app", "latest"]) => {
                let kept = std::fs::read_to_string(ctx.data.join("app").join("latest.json")).ok().and_then(|t| serde_json::from_str::<AppLatest>(&t).ok());
                match kept {
                    Some(a) => Ok(json(&a)),
                    None => return fail(404, "no app on this server"),
                }
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
    fn tag_pictures_stay_in_their_folder() {
        let d = std::path::Path::new("/data/tagart");
        assert_eq!(tagart_file(d, "martial-arts"), Some(d.join("martial-arts.webp")));
        assert_eq!(tagart_file(d, "../leaf"), None);
        assert_eq!(tagart_file(d, "Murim"), None, "slugs are lower-cased by the phone");
        assert_eq!(tagart_file(d, ""), None);
        assert_eq!(tagart_file(d, "a/b"), None);
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
