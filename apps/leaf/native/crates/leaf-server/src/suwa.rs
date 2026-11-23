//! Suwayomi, as leaf-server uses it: its GraphQL API at `/api/graphql` and its page and cover
//! URLs. Suwayomi runs the Mihon extensions (one per site), downloads chapters and converts them
//! to WebP; everything here is a thin typed call on top of that.

use std::time::Duration;

use serde_json::{Value, json};

pub struct Suwa {
    base: String,
    agent: ureq::Agent,
}

#[derive(Clone, Debug)]
pub struct Source {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct Found {
    pub id: i64,
    pub title: String,
}

#[derive(Clone, Debug, Default)]
pub struct Manga {
    pub id: i64,
    pub title: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub status: String,
    pub source_id: String,
    pub source_name: String,
    pub chapters: Vec<Ch>,
}

#[derive(Clone, Debug)]
pub struct Ch {
    pub id: i64,
    pub number: f64,
    pub name: String,
    pub group: Option<String>,
    pub pages: u32,
    pub read: bool,
    pub last_page: u32,
    pub downloaded: bool,
    pub uploaded_at: i64,
    /// When it was last read (ms since the epoch), 0 if never.
    pub last_read_at: i64,
}

const MANGA_FIELDS: &str = "id title author description status source { id displayName }";
const CHAPTER_FIELDS: &str = "id chapterNumber name scanlator pageCount isRead lastPageRead isDownloaded uploadDate lastReadAt";

/// Images a page conversion turns to WebP. No quality is given: Suwayomi 2.4 refuses its own
/// `compressionLevel` ("No compression type set!") and then keeps the original, while its default
/// quality took a 46.5 MB chapter to 6.3 MB (2026-11-22).
const TO_WEBP: [&str; 2] = ["image/jpeg", "image/png"];

impl Suwa {
    pub fn new(base: &str) -> Suwa {
        let config = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(180))).build();
        Suwa { base: base.trim_end_matches('/').to_string(), agent: ureq::Agent::new_with_config(config) }
    }

    fn gql(&self, query: &str, variables: Value) -> Result<Value, String> {
        let body = json!({ "query": query, "variables": variables }).to_string();
        let mut resp = self
            .agent
            .post(format!("{}/api/graphql", self.base))
            .header("content-type", "application/json")
            .send(&body)
            .map_err(|e| format!("suwayomi: {e}"))?;
        let text = resp.body_mut().with_config().limit(64 << 20).read_to_string().map_err(|e| format!("suwayomi: {e}"))?;
        let mut v: Value = serde_json::from_str(&text).map_err(|e| format!("suwayomi answered something else: {e}"))?;
        if let Some(errors) = v.get("errors").and_then(|e| e.as_array()).filter(|e| !e.is_empty()) {
            let words: Vec<&str> = errors.iter().filter_map(|e| e.get("message").and_then(|m| m.as_str())).collect();
            return Err(words.join("; "));
        }
        Ok(v["data"].take())
    }

    pub fn version(&self) -> Result<String, String> {
        let d = self.gql("{ aboutServer { version } }", json!({}))?;
        Ok(d["aboutServer"]["version"].as_str().unwrap_or("?").to_string())
    }

    /// What Leaf needs Suwayomi to do on its own: store (and serve) pages as WebP, and download
    /// nothing by itself (leaf-server asks for the chapter after the one you read, and for what
    /// you tap download on).
    /// `flaresolverr` is the address of a FlareSolverr beside Suwayomi, which passes the
    /// Cloudflare checks some sites put in front of their pages; without one those sources answer
    /// "Cloudflare bypass currently disabled".
    pub fn ensure_settings(&self, flaresolverr: Option<&str>) -> Result<(), String> {
        let conv: Vec<Value> = TO_WEBP.iter().map(|m| json!({ "mimeType": m, "target": "image/webp" })).collect();
        self.gql(
            "mutation($c:[SettingsDownloadConversionTypeInput!]){ setSettings(input:{settings:{downloadConversions:$c, serveConversions:$c, downloadAsCbz:false, autoDownloadNewChapters:false}}){ clientMutationId } }",
            json!({ "c": conv }),
        )?;
        if let Some(url) = flaresolverr {
            self.gql(
                "mutation($u:String!){ setSettings(input:{settings:{flareSolverrEnabled:true, flareSolverrUrl:$u, flareSolverrAsResponseFallback:true}}){ clientMutationId } }",
                json!({ "u": url }),
            )?;
        }
        Ok(())
    }

    /// The installed sources in this language ("en"); MangaDex's "all" twin is left out, as it
    /// would only find the same series again.
    pub fn sources(&self, lang: &str) -> Result<Vec<Source>, String> {
        let d = self.gql("{ sources { nodes { id displayName name lang } } }", json!({}))?;
        let mut out = Vec::new();
        for s in d["sources"]["nodes"].as_array().into_iter().flatten() {
            if s["lang"].as_str() == Some(lang) {
                out.push(Source {
                    id: s["id"].as_str().unwrap_or_default().to_string(),
                    name: s["name"].as_str().unwrap_or_default().to_string(),
                });
            }
        }
        Ok(out)
    }

    pub fn search(&self, source_id: &str, query: &str) -> Result<Vec<Found>, String> {
        let d = self.gql(
            "mutation($s:LongString!,$q:String){ fetchSourceManga(input:{source:$s,type:SEARCH,page:1,query:$q}){ mangas { id title } } }",
            json!({ "s": source_id, "q": query }),
        )?;
        Ok(d["fetchSourceManga"]["mangas"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|m| Found { id: m["id"].as_i64().unwrap_or(0), title: m["title"].as_str().unwrap_or_default().to_string() })
            .collect())
    }

    /// The series and its chapters, fetched from its source now (slow: the source is asked).
    pub fn refresh(&self, manga_id: i64) -> Result<Manga, String> {
        let q = format!(
            "mutation($id:Int!){{ fetchMangaAndChapters(input:{{id:$id,fetchManga:true,fetchChapters:true}}){{ manga {{ {MANGA_FIELDS} }} chapters {{ {CHAPTER_FIELDS} }} }} }}"
        );
        let d = self.gql(&q, json!({ "id": manga_id }))?;
        let p = &d["fetchMangaAndChapters"];
        Ok(manga_of(&p["manga"], &p["chapters"]))
    }

    /// The series and its chapters as Suwayomi last stored them (quick: no source is asked).
    pub fn manga(&self, manga_id: i64) -> Result<Manga, String> {
        let q = format!("query($id:Int!){{ manga(id:$id){{ {MANGA_FIELDS} chapters {{ nodes {{ {CHAPTER_FIELDS} }} }} }} }}");
        let d = self.gql(&q, json!({ "id": manga_id }))?;
        Ok(manga_of(&d["manga"], &d["manga"]["chapters"]["nodes"]))
    }

    pub fn set_in_library(&self, manga_id: i64, on: bool) -> Result<(), String> {
        self.gql("mutation($id:Int!,$on:Boolean!){ updateManga(input:{id:$id,patch:{inLibrary:$on}}){ clientMutationId } }", json!({ "id": manga_id, "on": on }))?;
        Ok(())
    }

    pub fn mark(&self, ids: &[i64], read: Option<bool>, last_page: Option<u32>) -> Result<(), String> {
        if ids.is_empty() {
            return Ok(());
        }
        self.gql(
            "mutation($ids:[Int!]!,$r:Boolean,$p:Int){ updateChapters(input:{ids:$ids,patch:{isRead:$r,lastPageRead:$p}}){ clientMutationId } }",
            json!({ "ids": ids, "r": read, "p": last_page }),
        )?;
        Ok(())
    }

    /// Where a chapter is: its series and its number.
    pub fn chapter_place(&self, chapter_id: i64) -> Result<(i64, f64), String> {
        let d = self.gql("query($id:Int!){ chapter(id:$id){ mangaId chapterNumber } }", json!({ "id": chapter_id }))?;
        Ok((d["chapter"]["mangaId"].as_i64().unwrap_or(0), d["chapter"]["chapterNumber"].as_f64().unwrap_or(-1.0)))
    }

    /// The chapter's page addresses on Suwayomi (each one fetched with [`Suwa::bytes`]).
    pub fn pages(&self, chapter_id: i64) -> Result<Vec<String>, String> {
        let d = self.gql("mutation($id:Int!){ fetchChapterPages(input:{chapterId:$id}){ pages } }", json!({ "id": chapter_id }))?;
        Ok(d["fetchChapterPages"]["pages"].as_array().into_iter().flatten().filter_map(|p| p.as_str().map(String::from)).collect())
    }

    pub fn enqueue(&self, chapter_ids: &[i64]) -> Result<(), String> {
        if chapter_ids.is_empty() {
            return Ok(());
        }
        self.gql("mutation($i:[Int!]!){ enqueueChapterDownloads(input:{ids:$i}){ clientMutationId } }", json!({ "i": chapter_ids }))?;
        self.gql("mutation{ startDownloader(input:{}){ clientMutationId } }", json!({}))?;
        Ok(())
    }

    /// Installs the extensions named (as the extension lists name them: "MangaDex", "Asura
    /// Scans") that are not installed yet, in `lang` or for all languages; answers those installed
    /// and those the list has no extension for. A name the list lacks does not stop the others.
    pub fn ensure_extensions(&self, names: &[String], lang: &str, repo: &str) -> Result<(Vec<String>, Vec<String>), String> {
        if names.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }
        const LIST: &str = "mutation{ fetchExtensions(input:{}){ extensions { pkgName name lang isInstalled } } }";
        let mut d = self.gql(LIST, json!({}))?;
        if d["fetchExtensions"]["extensions"].as_array().is_none_or(|a| a.is_empty()) {
            // Suwayomi ships with no extension list (and its Docker image's EXTENSION_REPOS did not
            // take in 2.4): add the one Mihon's community uses, once.
            self.gql("mutation($u:String!){ addExtensionStore(input:{indexUrl:$u}){ clientMutationId } }", json!({ "u": repo }))?;
            d = self.gql(LIST, json!({}))?;
        }
        let (mut done, mut missing) = (Vec::new(), Vec::new());
        for want in names {
            let found = d["fetchExtensions"]["extensions"].as_array().into_iter().flatten().find(|e| {
                e["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(want)) && matches!(e["lang"].as_str(), Some(l) if l == lang || l == "all")
            });
            let Some(e) = found else {
                missing.push(want.clone());
                continue;
            };
            if e["isInstalled"].as_bool() == Some(true) {
                continue;
            }
            self.gql("mutation($id:String!){ updateExtension(input:{id:$id,patch:{install:true}}){ clientMutationId } }", json!({ "id": e["pkgName"] }))?;
            done.push(want.clone());
        }
        // An empty list is one that has not loaded yet (no network): asked again, not given up on.
        if d["fetchExtensions"]["extensions"].as_array().is_none_or(|a| a.is_empty()) {
            return Err("the extension list is empty".into());
        }
        Ok((done, missing))
    }

    pub fn delete_downloads(&self, chapter_ids: &[i64]) -> Result<(), String> {
        if chapter_ids.is_empty() {
            return Ok(());
        }
        self.gql("mutation($i:[Int!]!){ deleteDownloadedChapters(input:{ids:$i}){ clientMutationId } }", json!({ "i": chapter_ids }))?;
        Ok(())
    }

    /// Fetches the extension lists again and updates every installed extension that has a newer
    /// version; answers the names updated.
    pub fn update_extensions(&self) -> Result<Vec<String>, String> {
        let d = self.gql("mutation{ fetchExtensions(input:{}){ extensions { pkgName name isInstalled hasUpdate } } }", json!({}))?;
        let (mut ids, mut names) = (Vec::new(), Vec::new());
        for e in d["fetchExtensions"]["extensions"].as_array().into_iter().flatten() {
            if e["isInstalled"].as_bool() == Some(true) && e["hasUpdate"].as_bool() == Some(true) {
                ids.push(e["pkgName"].as_str().unwrap_or_default().to_string());
                names.push(e["name"].as_str().unwrap_or_default().to_string());
            }
        }
        if !ids.is_empty() {
            self.gql("mutation($i:[String!]!){ updateExtensions(input:{ids:$i,patch:{update:true}}){ clientMutationId } }", json!({ "i": ids }))?;
        }
        Ok(names)
    }

    /// A page or a cover: its bytes and their type. `path` is Suwayomi's own (`/api/v1/...`).
    pub fn bytes(&self, path: &str) -> Result<(Vec<u8>, String), String> {
        if !path.starts_with("/api/") {
            return Err(format!("not a Suwayomi address: {path}"));
        }
        let mut resp = self.agent.get(format!("{}{path}", self.base)).call().map_err(|e| format!("suwayomi: {e}"))?;
        let kind = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("application/octet-stream").to_string();
        let body = resp.body_mut().with_config().limit(128 << 20).read_to_vec().map_err(|e| format!("suwayomi: {e}"))?;
        Ok((body, kind))
    }
}

fn status_of(s: &str) -> &'static str {
    match s {
        "ONGOING" => "ongoing",
        "COMPLETED" | "PUBLISHING_FINISHED" => "completed",
        "ON_HIATUS" => "hiatus",
        "CANCELLED" => "cancelled",
        _ => "unknown",
    }
}

fn manga_of(m: &Value, chapters: &Value) -> Manga {
    let text = |v: &Value| v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from);
    Manga {
        id: m["id"].as_i64().unwrap_or(0),
        title: m["title"].as_str().unwrap_or_default().to_string(),
        author: text(&m["author"]),
        description: text(&m["description"]),
        status: status_of(m["status"].as_str().unwrap_or("")).to_string(),
        source_id: m["source"]["id"].as_str().unwrap_or_default().to_string(),
        source_name: m["source"]["displayName"].as_str().unwrap_or_default().trim_end_matches(" (EN)").to_string(),
        chapters: chapters
            .as_array()
            .into_iter()
            .flatten()
            .map(|c| Ch {
                id: c["id"].as_i64().unwrap_or(0),
                number: c["chapterNumber"].as_f64().unwrap_or(-1.0),
                name: c["name"].as_str().unwrap_or_default().to_string(),
                group: text(&c["scanlator"]),
                pages: c["pageCount"].as_i64().unwrap_or(0).max(0) as u32,
                read: c["isRead"].as_bool().unwrap_or(false),
                last_page: c["lastPageRead"].as_i64().unwrap_or(0).max(0) as u32,
                downloaded: c["isDownloaded"].as_bool().unwrap_or(false),
                uploaded_at: c["uploadDate"].as_str().and_then(|s| s.parse().ok()).or_else(|| c["uploadDate"].as_i64()).unwrap_or(0),
                // Suwayomi keeps lastReadAt in seconds (uploadDate is in ms): ms, like the rest.
                last_read_at: c["lastReadAt"].as_str().and_then(|s| s.parse::<i64>().ok()).or_else(|| c["lastReadAt"].as_i64()).unwrap_or(0) * 1000,
            })
            .collect(),
    }
}
