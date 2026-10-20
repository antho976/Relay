//! The providers' answers read into [`Item`]s (pictures-SPEC-v2 §7.2): pure, no I/O.
//!
//! Only openly licensed photos come through: CC BY, CC BY-SA, CC0 and public domain from
//! Openverse, the Pexels License from Pexels. A result that is marked mature, has no size, or
//! whose links are not where they should be is dropped. Every provider string is text from
//! another website: control characters go, lengths are capped, and only https links are kept.
//! Openverse's `url` and `thumbnail` are never read: `url` is on the photographer's own host, and
//! every request the engine sends is built from a checked id.

use super::net::{self, PEXELS_IMAGES_HOST};
use super::{is_pexels_key, is_uuid, provider, Id, PictureCredit, PictureHit, PictureSource, MAX_PAGE};
use relay_bus::BusError;
use serde_json::Value;

/// Characters kept of a title or a creator.
pub const MAX_TITLE: usize = 200;
/// Characters an attribution may have; a longer one is written again from its parts.
pub const MAX_ATTRIBUTION: usize = 500;
/// Characters a kept link may have.
pub const MAX_URL: usize = 500;
pub const PEXELS_LICENCE: &str = "Pexels License";
pub const PEXELS_LICENCE_URL: &str = "https://www.pexels.com/license/";

/// A photo as a provider described it, cleaned: what search answers, what `add` credits, and
/// (Pexels) where its preview and download are.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub source: PictureSource,
    /// `openverse:<uuid>` or `pexels:<digits>`.
    pub id: String,
    /// May be empty.
    pub title: String,
    pub creator: Option<String>,
    pub creator_url: Option<String>,
    /// "CC BY 2.0", "CC BY-SA 4.0", "CC0", "Public domain" or "Pexels License".
    pub licence: String,
    pub licence_url: Option<String>,
    /// The photo's own page.
    pub source_url: Option<String>,
    /// One sentence naming title, creator, source and licence.
    pub attribution: String,
    /// The original's size in px.
    pub w: u32,
    pub h: u32,
    /// Pexels only, both on images.pexels.com: the preview (350 px tall) and the download
    /// (fitted to 940 × 650 at twice the density). Openverse's come from its id.
    pub thumb_url: Option<String>,
    pub download_url: Option<String>,
}

impl Item {
    /// What a search answers of it.
    pub fn hit(&self) -> PictureHit {
        PictureHit {
            id: self.id.clone(),
            title: self.title.clone(),
            creator: self.creator.clone(),
            licence: self.licence.clone(),
            w: self.w,
            h: self.h,
        }
    }

    /// What is kept beside the stored picture.
    pub fn credit(&self) -> PictureCredit {
        PictureCredit {
            source: self.source,
            id: self.id.clone(),
            title: self.title.clone(),
            creator: self.creator.clone(),
            creator_url: self.creator_url.clone(),
            licence: self.licence.clone(),
            licence_url: self.licence_url.clone(),
            source_url: self.source_url.clone(),
            attribution: self.attribution.clone(),
            caption: self.caption(),
        }
    }

    /// The line to put under the photo: `"Title" by Creator · CC BY 2.0`, leaving out what is
    /// empty; for Pexels `Photo by Creator on Pexels`, the wording Pexels' own docs use.
    pub fn caption(&self) -> String {
        if self.source == PictureSource::Pexels {
            return match &self.creator {
                Some(creator) => format!("Photo by {creator} on Pexels"),
                None => "Photo on Pexels".to_string(),
            };
        }
        let lead = match (self.title.is_empty(), &self.creator) {
            (false, Some(creator)) => format!("\"{}\" by {creator}", self.title),
            (false, None) => format!("\"{}\"", self.title),
            (true, Some(creator)) => format!("Photo by {creator}"),
            (true, None) => String::new(),
        };
        if lead.is_empty() { self.licence.clone() } else { format!("{lead} · {}", self.licence) }
    }
}

/// Why a result was left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dropped {
    /// No id of the provider's form.
    Id,
    Mature,
    /// A width or height missing, null or zero.
    Size,
    /// A licence outside the allowlist: the provider's code, cleaned.
    Licence(String),
    /// A Pexels picture link that is not https on images.pexels.com.
    Host,
}

/// A licence code and version as a label: `by` is `CC BY {v}`, `by-sa` `CC BY-SA {v}`, `cc0`
/// `CC0`, `pdm` `Public domain`; any other code is not one Relay keeps (`None`).
pub fn licence_label(code: &str, version: &str) -> Option<String> {
    let version = Some(version.trim()).filter(|v| (1..=8).contains(&v.len()) && v.bytes().all(|b| b.is_ascii_digit() || b == b'.'));
    let versioned = |name: &str| version.map_or_else(|| name.to_string(), |v| format!("{name} {v}"));
    match code.trim().to_ascii_lowercase().as_str() {
        "by" => Some(versioned("CC BY")),
        "by-sa" => Some(versioned("CC BY-SA")),
        "cc0" => Some("CC0".to_string()),
        "pdm" => Some("Public domain".to_string()),
        _ => None,
    }
}

/// Provider text made safe to show and keep: control and bidi-control characters gone (a line
/// break or tab becomes a space), whitespace runs one space, at most `max` characters.
pub fn clean(text: &str, max: usize) -> String {
    let spaced: String = text
        .chars()
        .filter(|c| !is_bidi_control(*c))
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .filter(|c| !c.is_control())
        .collect();
    let words = spaced.split_whitespace().collect::<Vec<_>>().join(" ");
    match words.char_indices().nth(max) {
        Some((cut, _)) => words[..cut].trim_end().to_string(),
        None => words,
    }
}

fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// A provider link kept only if it is https, at most [`MAX_URL`] characters, and reads as a URL
/// with a host.
pub fn https_url(value: &Value) -> Option<String> {
    let url = value.as_str()?.trim();
    let ok = url.len() <= MAX_URL
        && url.starts_with("https://")
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
        && url.parse::<ureq::http::Uri>().is_ok_and(|uri| uri.host().is_some_and(|h| !h.is_empty()));
    ok.then(|| url.to_string())
}

/// A width and height, each 1 to `u32::MAX`.
fn size(w: &Value, h: &Value) -> Option<(u32, u32)> {
    let side = |v: &Value| v.as_u64().filter(|n| *n > 0).and_then(|n| u32::try_from(n).ok());
    Some((side(w)?, side(h)?))
}

fn text(value: &Value, max: usize) -> String {
    clean(value.as_str().unwrap_or(""), max)
}

fn some_text(value: &Value, max: usize) -> Option<String> {
    Some(text(value, max)).filter(|t| !t.is_empty())
}

fn malformed(source: PictureSource) -> BusError {
    provider(source, None, "answered in a form Relay cannot read")
}

/// A detail read's drop as the refusal `add` and `thumb` answer.
fn refused(source: PictureSource, dropped: Dropped) -> BusError {
    match dropped {
        Dropped::Licence(code) => super::licence(if code.is_empty() { "none" } else { &code }),
        Dropped::Mature => super::licence("mature"),
        Dropped::Id | Dropped::Size | Dropped::Host => malformed(source),
    }
}

/// A detail read must describe the photo asked for.
fn the_one(id: &Id, item: Item) -> Result<Item, BusError> {
    if item.id == id.to_string() { Ok(item) } else { Err(provider(id.source, None, "described another photo than the one asked for")) }
}

// ------------------------------------------------------------------ Openverse

/// One Openverse result (`/v1/images/` or `/v1/images/<uuid>/`).
pub fn openverse_item(v: &Value) -> Result<Item, Dropped> {
    let uuid = v["id"].as_str().filter(|id| is_uuid(id)).ok_or(Dropped::Id)?;
    if v["mature"].as_bool() == Some(true) {
        return Err(Dropped::Mature);
    }
    let (w, h) = size(&v["width"], &v["height"]).ok_or(Dropped::Size)?;
    let code = v["license"].as_str().unwrap_or("");
    let licence = licence_label(code, v["license_version"].as_str().unwrap_or("")).ok_or_else(|| Dropped::Licence(clean(code, 20)))?;
    let title = text(&v["title"], MAX_TITLE);
    let creator = some_text(&v["creator"], MAX_TITLE);
    let attribution = some_text(&v["attribution"], usize::MAX)
        .filter(|a| a.chars().count() <= MAX_ATTRIBUTION)
        .unwrap_or_else(|| openverse_attribution(&title, creator.as_deref(), &licence, code.trim().eq_ignore_ascii_case("pdm")));
    Ok(Item {
        source: PictureSource::Openverse,
        id: format!("openverse:{uuid}"),
        title,
        creator,
        creator_url: https_url(&v["creator_url"]),
        licence,
        licence_url: https_url(&v["license_url"]),
        source_url: https_url(&v["foreign_landing_url"]),
        attribution,
        w,
        h,
        thumb_url: None,
        download_url: None,
    })
}

/// The sentence Openverse would have written, for a result that came without one (or with one
/// too long to keep whole).
fn openverse_attribution(title: &str, creator: Option<&str>, licence: &str, public_domain: bool) -> String {
    let subject = if title.is_empty() { "This photo".to_string() } else { format!("\"{title}\"") };
    let by = creator.map(|c| format!(" by {c}")).unwrap_or_default();
    if public_domain {
        format!("{subject}{by} is in the public domain.")
    } else {
        format!("{subject}{by} is licensed under {licence}.")
    }
}

/// An Openverse search answer: the results kept, and whether `page + 1` would answer. Openverse
/// refuses pages past `page_count`, and Relay stops at page 10, so `has_more` is
/// `page < min(page_count, 10)`.
pub fn openverse_page(body: &[u8], page: u32) -> Result<(Vec<Item>, bool), BusError> {
    let v: Value = serde_json::from_slice(body).map_err(|_| malformed(PictureSource::Openverse))?;
    let results = v["results"].as_array().ok_or_else(|| malformed(PictureSource::Openverse))?;
    let items = results.iter().filter_map(|r| openverse_item(r).ok()).collect();
    let pages = v["page_count"].as_u64().unwrap_or(0).min(u64::from(MAX_PAGE));
    Ok((items, u64::from(page) < pages))
}

/// An Openverse detail read of `id`: its item, or `design.picture.licence` when its licence is
/// not one Relay keeps or it is marked mature.
pub fn openverse_detail(body: &[u8], id: &Id) -> Result<Item, BusError> {
    let v: Value = serde_json::from_slice(body).map_err(|_| malformed(PictureSource::Openverse))?;
    the_one(id, openverse_item(&v).map_err(|d| refused(PictureSource::Openverse, d))?)
}

// ------------------------------------------------------------------ Pexels

/// One Pexels photo (`/v1/search` or `/v1/photos/<id>`). `alt` is its title.
pub fn pexels_item(v: &Value) -> Result<Item, Dropped> {
    let key = match &v["id"] {
        Value::Number(n) => n.as_u64().map(|n| n.to_string()),
        Value::String(s) => Some(s.clone()),
        _ => None,
    };
    let key = key.filter(|k| is_pexels_key(k)).ok_or(Dropped::Id)?;
    let (w, h) = size(&v["width"], &v["height"]).ok_or(Dropped::Size)?;
    let picture = |size: &str| {
        let url = v["src"][size].as_str()?;
        (url.len() <= MAX_URL && net::allowed_host(url) == Some(PEXELS_IMAGES_HOST)).then(|| url.to_string())
    };
    let (Some(thumb_url), Some(download_url)) = (picture("medium"), picture("large2x")) else {
        return Err(Dropped::Host);
    };
    let creator = some_text(&v["photographer"], MAX_TITLE);
    let attribution = match &creator {
        Some(creator) => format!("Photo by {creator} on Pexels."),
        None => "Photo on Pexels.".to_string(),
    };
    Ok(Item {
        source: PictureSource::Pexels,
        id: format!("pexels:{key}"),
        title: text(&v["alt"], MAX_TITLE),
        creator,
        creator_url: https_url(&v["photographer_url"]),
        licence: PEXELS_LICENCE.to_string(),
        licence_url: Some(PEXELS_LICENCE_URL.to_string()),
        source_url: https_url(&v["url"]),
        attribution,
        w,
        h,
        thumb_url: Some(thumb_url),
        download_url: Some(download_url),
    })
}

/// A Pexels search answer: the photos kept, and whether `page + 1` would answer (`next_page`
/// given, and never past page 10).
pub fn pexels_page(body: &[u8], page: u32) -> Result<(Vec<Item>, bool), BusError> {
    let v: Value = serde_json::from_slice(body).map_err(|_| malformed(PictureSource::Pexels))?;
    let photos = v["photos"].as_array().ok_or_else(|| malformed(PictureSource::Pexels))?;
    let items = photos.iter().filter_map(|p| pexels_item(p).ok()).collect();
    let next = v["next_page"].as_str().is_some_and(|next| !next.trim().is_empty());
    Ok((items, next && page < MAX_PAGE))
}

/// A Pexels detail read of `id`.
pub fn pexels_detail(body: &[u8], id: &Id) -> Result<Item, BusError> {
    let v: Value = serde_json::from_slice(body).map_err(|_| malformed(PictureSource::Pexels))?;
    the_one(id, pexels_item(&v).map_err(|d| refused(PictureSource::Pexels, d))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pictures::parse_id;
    use serde_json::json;

    const OPENVERSE_SEARCH: &str = include_str!("../../tests/fixtures/pictures/openverse-search.json");
    const OPENVERSE_DETAIL: &str = include_str!("../../tests/fixtures/pictures/openverse-detail.json");
    const OPENVERSE_DETAIL_ND: &str = include_str!("../../tests/fixtures/pictures/openverse-detail-nd.json");
    const PEXELS_SEARCH: &str = include_str!("../../tests/fixtures/pictures/pexels-search.json");
    const PEXELS_PHOTO: &str = include_str!("../../tests/fixtures/pictures/pexels-photo.json");

    fn openverse(over: Value) -> Value {
        let mut v = json!({
            "id": "48e4625b-0930-43db-990c-22858b1d633e", "title": "Kitchen", "creator": "someone",
            "creator_url": "https://www.flickr.com/photos/someone", "license": "by", "license_version": "2.0",
            "license_url": "https://creativecommons.org/licenses/by/2.0/", "foreign_landing_url": "https://www.flickr.com/photos/someone/1",
            "attribution": "\"Kitchen\" by someone is licensed under CC BY 2.0.", "mature": false, "width": 1024, "height": 768,
        });
        for (k, value) in over.as_object().expect("an object") {
            v[k] = value.clone();
        }
        v
    }

    #[test]
    fn the_openverse_search_keeps_exactly_the_allowed_licences_with_their_labels() {
        let (items, more) = openverse_page(OPENVERSE_SEARCH.as_bytes(), 1).unwrap();
        let got: Vec<(&str, &str)> = items.iter().map(|i| (i.id.as_str(), i.licence.as_str())).collect();
        assert_eq!(got, [
            ("openverse:48e4625b-0930-43db-990c-22858b1d633e", "CC BY 2.0"),
            ("openverse:6f31796e-019b-4888-948c-882a30a648dd", "CC BY 2.0"),
            ("openverse:71e28cde-167a-4dfa-86c4-e2fd9959d446", "CC BY 2.0"),
            ("openverse:ca32bd72-5e2b-4949-b641-8a9a13a1b6ad", "CC BY-SA 2.0"),
            ("openverse:726bc020-7982-45a7-b52c-140c8dbf8b94", "CC BY-SA 2.0"),
            ("openverse:7ec91bdf-1d54-4abc-98bd-5f9bac943554", "Public domain"),
        ], "by-nd, by-nc, the mature one and the one with no width are dropped");
        assert!(more, "page 1 of 12");
        let first = &items[0];
        assert_eq!((first.w, first.h), (1024, 768));
        assert_eq!(first.title, "Designed by Williams Kitchen & Bath");
        assert_eq!(first.creator.as_deref(), Some("williamskitchenbath"));
        assert_eq!(first.source_url.as_deref(), Some("https://www.flickr.com/photos/56473047@N05/5410474045"));
        assert_eq!(first.licence_url.as_deref(), Some("https://creativecommons.org/licenses/by/2.0/"));
        assert!(first.attribution.starts_with("\"Designed by Williams Kitchen & Bath\" by williamskitchenbath is licensed under CC BY 2.0."));
        assert_eq!(first.credit().caption, "\"Designed by Williams Kitchen & Bath\" by williamskitchenbath · CC BY 2.0");
        let hit = serde_json::to_value(first.hit()).unwrap();
        let keys: Vec<&str> = hit.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["id", "title", "creator", "licence", "w", "h"]);
    }

    #[test]
    fn openverse_url_and_thumbnail_are_never_read() {
        let v = openverse(json!({"url": "https://evil.example/u.jpg", "thumbnail": "https://evil.example/t.jpg"}));
        let item = openverse_item(&v).unwrap();
        assert!(!format!("{item:?}").contains("evil.example"));
        assert_eq!((item.thumb_url, item.download_url), (None, None));
    }

    #[test]
    fn every_drop_has_its_reason() {
        let drop = |over: Value| openverse_item(&openverse(over)).unwrap_err();
        assert_eq!(drop(json!({"license": "by-nd"})), Dropped::Licence("by-nd".into()));
        assert_eq!(drop(json!({"license": "by-nc-sa"})), Dropped::Licence("by-nc-sa".into()));
        assert_eq!(drop(json!({"license": null})), Dropped::Licence(String::new()));
        assert_eq!(drop(json!({"mature": true})), Dropped::Mature);
        assert_eq!(drop(json!({"width": null})), Dropped::Size);
        assert_eq!(drop(json!({"height": 0})), Dropped::Size);
        assert_eq!(drop(json!({"width": 1024.5})), Dropped::Size);
        assert_eq!(drop(json!({"id": "48E4625B-0930-43DB-990C-22858B1D633E"})), Dropped::Id, "upper-case hex");
        assert_eq!(drop(json!({"id": "../../etc"})), Dropped::Id);
        assert!(openverse_item(&openverse(json!({"mature": null}))).is_ok(), "mature defaults to false");
    }

    #[test]
    fn licences_are_labelled() {
        assert_eq!(licence_label("by", "4.0").as_deref(), Some("CC BY 4.0"));
        assert_eq!(licence_label("by-sa", "2.0").as_deref(), Some("CC BY-SA 2.0"));
        assert_eq!(licence_label("BY", "").as_deref(), Some("CC BY"));
        assert_eq!(licence_label("by", "4.0; drop table").as_deref(), Some("CC BY"), "an odd version is left out");
        assert_eq!(licence_label("cc0", "1.0").as_deref(), Some("CC0"));
        assert_eq!(licence_label("pdm", "1.0").as_deref(), Some("Public domain"));
        for code in ["by-nd", "by-nc", "by-nc-sa", "by-nc-nd", "sampling+", "nc-sampling+", ""] {
            assert_eq!(licence_label(code, "2.0"), None, "{code}");
        }
        let cc0 = openverse_item(&openverse(json!({"license": "cc0", "license_version": "1.0"}))).unwrap();
        assert_eq!(cc0.licence, "CC0");
    }

    #[test]
    fn provider_text_is_cleaned_and_capped() {
        assert_eq!(clean("  a\tb\n\nc\u{0007}d\u{202E}e  ", 200), "a b cde");
        let long = "x".repeat(300);
        assert_eq!(clean(&long, MAX_TITLE).chars().count(), 200);
        assert_eq!(clean(&"é".repeat(250), 200), "é".repeat(200), "characters, not bytes");
        let v = openverse(json!({
            "title": format!("Kit\u{0000}chen{}", "y".repeat(400)), "creator": "a\u{001b}[31mb",
            "creator_url": "http://www.flickr.com/photos/someone", "license_url": format!("https://x.example/{}", "z".repeat(600)),
            "foreign_landing_url": "javascript:alert(1)",
        }));
        let item = openverse_item(&v).unwrap();
        assert!(item.title.starts_with("Kitchenyyy") && item.title.chars().count() == 200);
        assert_eq!(item.creator.as_deref(), Some("a[31mb"));
        assert_eq!((item.creator_url, item.licence_url, item.source_url), (None, None, None), "https only, at most 500 characters");
    }

    #[test]
    fn a_missing_creator_leaves_by_out_and_a_missing_attribution_is_written() {
        let v = openverse(json!({"creator": null, "attribution": null}));
        let item = openverse_item(&v).unwrap();
        assert_eq!(item.creator, None);
        assert_eq!(item.attribution, "\"Kitchen\" is licensed under CC BY 2.0.");
        assert_eq!(item.caption(), "\"Kitchen\" · CC BY 2.0");
        let untitled = openverse_item(&openverse(json!({"title": "", "creator": "someone", "attribution": ""}))).unwrap();
        assert_eq!(untitled.attribution, "This photo by someone is licensed under CC BY 2.0.");
        assert_eq!(untitled.caption(), "Photo by someone · CC BY 2.0");
        let bare = openverse_item(&openverse(json!({"title": null, "creator": null, "license": "pdm", "attribution": null}))).unwrap();
        assert_eq!(bare.attribution, "This photo is in the public domain.");
        assert_eq!(bare.caption(), "Public domain");
        let wordy = openverse_item(&openverse(json!({"attribution": "w".repeat(501)}))).unwrap();
        assert_eq!(wordy.attribution, "\"Kitchen\" by someone is licensed under CC BY 2.0.", "too long to keep whole");
    }

    #[test]
    fn has_more_follows_page_count_and_stops_at_page_ten() {
        let body = |count: u64| json!({"page_count": count, "results": []}).to_string();
        assert!(openverse_page(body(12).as_bytes(), 9).unwrap().1);
        assert!(!openverse_page(body(12).as_bytes(), 10).unwrap().1, "page 10 is the last, though Openverse has 12");
        assert!(!openverse_page(body(3).as_bytes(), 3).unwrap().1);
        assert!(!openverse_page(br#"{"results": []}"#, 1).unwrap().1);
        let next = |next: Value| json!({"photos": [], "next_page": next}).to_string();
        assert!(pexels_page(next(json!("https://api.pexels.com/v1/search/?page=2")).as_bytes(), 1).unwrap().1);
        assert!(!pexels_page(next(json!("https://api.pexels.com/v1/search/?page=11")).as_bytes(), 10).unwrap().1);
        assert!(!pexels_page(next(Value::Null).as_bytes(), 1).unwrap().1);
        assert!(!pexels_page(br#"{"photos": []}"#, 1).unwrap().1);
    }

    #[test]
    fn an_answer_that_does_not_read_is_the_providers_fault() {
        for body in [&b"<html>"[..], b"{}", br#"{"results": {}}"#, b"[]"] {
            let e = openverse_page(body, 1).unwrap_err();
            assert_eq!((e.code.as_str(), e.details), ("design.picture.provider", Some(json!({"source": "openverse"}))));
        }
        assert_eq!(pexels_page(b"{\"photos\": 3}", 1).unwrap_err().code, "design.picture.provider");
    }

    #[test]
    fn a_detail_read_refuses_what_relay_does_not_keep() {
        let id = parse_id("openverse:48e4625b-0930-43db-990c-22858b1d633e").unwrap();
        let item = openverse_detail(OPENVERSE_DETAIL.as_bytes(), &id).unwrap();
        assert_eq!(item.id, id.to_string());
        assert_eq!(item.credit().source, PictureSource::Openverse);
        let nd_id = parse_id("openverse:0b5c3a8e-5a8e-4c31-9d5a-111111111111").unwrap();
        let e = openverse_detail(OPENVERSE_DETAIL_ND.as_bytes(), &nd_id).unwrap_err();
        assert_eq!((e.code.as_str(), e.details), ("design.picture.licence", Some(json!({"licence": "by-nd"}))));
        let mature = openverse(json!({"mature": true})).to_string();
        let e = openverse_detail(mature.as_bytes(), &id).unwrap_err();
        assert_eq!((e.code.as_str(), e.details), ("design.picture.licence", Some(json!({"licence": "mature"}))));
        let other = parse_id("openverse:6f31796e-019b-4888-948c-882a30a648dd").unwrap();
        assert_eq!(openverse_detail(OPENVERSE_DETAIL.as_bytes(), &other).unwrap_err().code, "design.picture.provider");
    }

    #[test]
    fn the_pexels_search_uses_alt_as_the_title_and_drops_a_photo_off_its_host() {
        let (items, more) = pexels_page(PEXELS_SEARCH.as_bytes(), 1).unwrap();
        let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["pexels:2014422", "pexels:1080721", "pexels:2062426"], "3214751's large2x is on another host");
        assert!(more);
        let first = &items[0];
        assert_eq!(first.title, "Green plants on a kitchen shelf");
        assert_eq!((first.w, first.h), (3024, 3780));
        assert_eq!(first.licence, "Pexels License");
        assert_eq!(first.licence_url.as_deref(), Some(PEXELS_LICENCE_URL));
        assert_eq!(first.creator.as_deref(), Some("Ana Sample"));
        assert_eq!(first.attribution, "Photo by Ana Sample on Pexels.");
        assert_eq!(first.caption(), "Photo by Ana Sample on Pexels");
        assert!(first.thumb_url.as_deref().unwrap().starts_with("https://images.pexels.com/photos/2014422/") && first.thumb_url.as_deref().unwrap().ends_with("h=350"));
        assert!(first.download_url.as_deref().unwrap().contains("dpr=2&h=650&w=940"));
        assert_eq!(items[2].title, "", "no alt");
        let photo = pexels_detail(PEXELS_PHOTO.as_bytes(), &parse_id("pexels:2014422").unwrap()).unwrap();
        assert_eq!(&photo, first);
    }

    #[test]
    fn a_pexels_picture_link_must_be_https_on_its_image_host() {
        let photo: Value = serde_json::from_str(PEXELS_PHOTO).unwrap();
        for bad in ["http://images.pexels.com/p.jpeg", "https://images.pexels.com.evil.example/p.jpeg", "https://api.pexels.com/p.jpeg",
            "https://user@images.pexels.com/p.jpeg", "https://images.pexels.com:8443/p.jpeg"] {
            for size in ["medium", "large2x"] {
                let mut v = photo.clone();
                v["src"][size] = json!(bad);
                assert_eq!(pexels_item(&v), Err(Dropped::Host), "{size} {bad}");
            }
        }
        let mut v = photo.clone();
        v["photographer"] = Value::Null;
        let item = pexels_item(&v).unwrap();
        assert_eq!((item.attribution.as_str(), item.caption().as_str()), ("Photo on Pexels.", "Photo on Pexels"));
        v["id"] = json!(1_234_567_890_123u64);
        assert_eq!(pexels_item(&v), Err(Dropped::Id), "13 digits");
    }
}
