//! Recommendations from AniList: "Top picks", "Because you read X", "Collections", "Readers also
//! like" and "Not interested". Made in the background once a day and kept (`kv` "recs"); a page
//! load only reads what was made.
//!
//! - The series you've read whose AniList id is known (`al_id:{work}`, as sync keeps it) are
//!   asked for their recommendations in batches (`Page(media(id_in:[...]))`), through AniList's
//!   own budget (one request every 2 s).
//! - Each title recommended is scored ([`score`]): the ratings of the recommendations pointing at
//!   it from your series (log-damped), half as much again from a series you read to its end, a
//!   small boost for the genres and tags you read most, less for tags you asked to see fewer of.
//!   What is already in your space, what you hid, adult titles and anything but comics
//!   (AniList's `MANGA` format) are left out; Japanese titles weigh less when you read mostly
//!   Korean and Chinese ones.
//! - Only what your sources carry is shown: the top candidates are searched on a few broad
//!   sources (`LEAF_REC_SOURCES`), as background work, the hits kept a week per AniList id; at
//!   most `LEAF_REC_SEARCHES` titles a day (80), the rest tomorrow.

use std::collections::{HashMap, HashSet};

use leaf_proto::{Because, Collection, Hit, Rec, Recs, same_work};
use serde_json::{Value, json};

use crate::autopilot::{Ctx, log, now_ms};
use crate::budget::Lane;

const MEDIA: &str = "id title{romaji english} synonyms format countryOfOrigin isAdult status chapters genres tags{name rank} averageScore popularity coverImage{large}";
/// Series asked about per request: ten, each with ten recommendations.
const BATCH: usize = 10;
const TOP: usize = 12;
const BECAUSE_SERIES: usize = 3;
const BECAUSE_EACH: usize = 8;
const COLLECTIONS: usize = 4;
const COLLECTION_EACH: usize = 10;
/// A tag counts for a title when AniList ranks it this relevant (0 to 100).
const TAG_RANK: u32 = 60;
const DAY: i64 = 86_400_000;
const HITS_KEPT: i64 = 7 * DAY;
pub const NO_COVER: &str = "no cover known";

/// A title as AniList describes it.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Media {
    pub id: i64,
    pub title: String,
    /// Every name it goes by (English, romaji, synonyms), the title first.
    pub names: Vec<String>,
    pub format: String,
    pub country: String,
    pub adult: bool,
    /// AniList's: RELEASING, FINISHED, HIATUS, CANCELLED, NOT_YET_RELEASED.
    pub status: String,
    pub chapters: Option<u32>,
    pub genres: Vec<String>,
    pub tags: Vec<(String, u32)>,
    pub score: Option<u32>,
    pub popularity: u64,
    pub cover: Option<String>,
}

impl Media {
    pub fn of(v: &Value) -> Option<Media> {
        let id = v["id"].as_i64()?;
        let mut names: Vec<String> = Vec::new();
        for t in [&v["title"]["english"], &v["title"]["romaji"]] {
            if let Some(s) = t.as_str().filter(|s| !s.trim().is_empty()) {
                names.push(s.trim().to_string());
            }
        }
        for s in v["synonyms"].as_array().into_iter().flatten().filter_map(|s| s.as_str()) {
            if !names.iter().any(|n| n == s) {
                names.push(s.to_string());
            }
        }
        let strs = |a: &Value| -> Vec<String> { a.as_array().into_iter().flatten().filter_map(|s| s.as_str().map(String::from)).collect() };
        Some(Media {
            id,
            title: names.first().cloned().unwrap_or_default(),
            names,
            format: v["format"].as_str().unwrap_or("").to_string(),
            country: v["countryOfOrigin"].as_str().unwrap_or("").to_string(),
            adult: v["isAdult"].as_bool().unwrap_or(false),
            status: v["status"].as_str().unwrap_or("").to_string(),
            chapters: v["chapters"].as_u64().map(|c| c as u32),
            genres: strs(&v["genres"]),
            tags: v["tags"].as_array().into_iter().flatten().filter_map(|t| Some((t["name"].as_str()?.to_string(), t["rank"].as_u64().unwrap_or(0) as u32))).collect(),
            score: v["averageScore"].as_u64().map(|s| s as u32),
            popularity: v["popularity"].as_u64().unwrap_or(0),
            cover: v["coverImage"]["large"].as_str().map(String::from),
        })
    }

    fn top_tags(&self) -> impl Iterator<Item = &str> {
        self.tags.iter().filter(|(_, r)| *r >= TAG_RANK).map(|(t, _)| t.as_str())
    }

    fn east(&self) -> bool {
        matches!(self.country.as_str(), "KR" | "CN" | "TW")
    }
}

/// One of your series and what AniList recommends from it, highest rated first.
#[derive(Clone, Debug, PartialEq)]
pub struct WithRecs {
    pub media: Media,
    pub recs: Vec<(i64, Media)>,
}

/// `Page.media` with nested recommendations.
pub fn parse_with_recs(data: &Value) -> Vec<WithRecs> {
    data["Page"]["media"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let media = Media::of(m)?;
            let recs = m["recommendations"]["nodes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|n| Some((n["rating"].as_i64().unwrap_or(0), Media::of(&n["mediaRecommendation"])?)))
                .collect();
            Some(WithRecs { media, recs })
        })
        .collect()
}

pub fn parse_page(data: &Value) -> Vec<Media> {
    data["Page"]["media"].as_array().into_iter().flatten().filter_map(Media::of).collect()
}

/// A series in your space, as recommendations weigh it.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Mine {
    pub work: i64,
    pub title: String,
    /// Its title and the other titles it goes by.
    pub names: Vec<String>,
    pub al: Option<i64>,
    /// Begun (a chapter read).
    pub read: bool,
    /// Finished and read to its end.
    pub finished: bool,
    pub last_read_at: i64,
}

/// What you read most, and what you asked to see less of.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Taste {
    /// Share of your read series with each genre, and each tag (lower case).
    pub genres: HashMap<String, f64>,
    pub tags: HashMap<String, f64>,
    pub less: HashSet<String>,
    /// Most of what you read is Korean or Chinese.
    pub east: bool,
}

pub fn taste(read: &[&Media], less: &HashSet<String>) -> Taste {
    let n = read.len().max(1) as f64;
    let mut t = Taste { less: less.iter().map(|s| s.to_lowercase()).collect(), ..Default::default() };
    for m in read {
        for g in &m.genres {
            *t.genres.entry(g.to_lowercase()).or_default() += 1.0 / n;
        }
        for g in m.top_tags() {
            *t.tags.entry(g.to_lowercase()).or_default() += 1.0 / n;
        }
    }
    t.east = !read.is_empty() && read.iter().filter(|m| m.east()).count() as f64 / n >= 0.6;
    t
}

/// Tags that describe the format or the cast, not what a story is about: nearly every manhwa has
/// them, so a collection of them says nothing ("Full Color", "Male Protagonist").
const PLAIN_TAGS: [&str; 14] = [
    "full color", "male protagonist", "female protagonist", "long strip", "web comic", "primarily male cast",
    "primarily female cast", "primarily adult cast", "primarily teen cast", "ensemble cast", "episodic",
    "adapted from web novel", "coming of age", "heterosexual",
];

/// The tags you read most, most first, leaving out those you asked to see less of and the ones
/// every series carries.
pub fn top_tags(t: &Taste, n: usize) -> Vec<String> {
    let mut v: Vec<(&String, &f64)> = t.tags.iter().filter(|(k, _)| !t.less.contains(*k) && !PLAIN_TAGS.contains(&k.to_lowercase().as_str())).collect();
    v.sort_by(|a, b| b.1.total_cmp(a.1).then(a.0.cmp(b.0)));
    v.into_iter().take(n).map(|(k, _)| k.clone()).collect()
}

/// The series of yours this title is, by AniList id or by any of its names.
pub fn in_space(m: &Media, mine: &[Mine]) -> Option<i64> {
    mine.iter().find(|s| s.al == Some(m.id) || s.names.iter().any(|n| m.names.iter().any(|x| same_work(n, x)))).map(|s| s.work)
}

/// Never recommended: adult titles, what is not a comic, what you hid.
pub fn unwanted(m: &Media, hidden: &HashSet<i64>) -> bool {
    m.adult || m.format != "MANGA" || hidden.contains(&m.id)
}

/// The genres and tags you read most, a small boost; a tag you want fewer of, a large minus.
fn leaning(m: &Media, t: &Taste) -> f64 {
    let mean = |xs: Vec<f64>| if xs.is_empty() { 0.0 } else { xs.iter().sum::<f64>() / xs.len() as f64 };
    let g = mean(m.genres.iter().map(|g| t.genres.get(&g.to_lowercase()).copied().unwrap_or(0.0)).collect());
    let tg = mean(m.top_tags().map(|g| t.tags.get(&g.to_lowercase()).copied().unwrap_or(0.0)).collect());
    let less = m.genres.iter().map(String::as_str).chain(m.tags.iter().filter(|(_, r)| *r >= 50).map(|(n, _)| n.as_str())).filter(|n| t.less.contains(&n.to_lowercase())).count();
    g + tg - 2.0 * less as f64
}

/// A title recommended, with its score and the series of yours that point at it.
#[derive(Clone, Debug, PartialEq)]
pub struct Scored {
    pub media: Media,
    pub score: f64,
    /// (work, title) of your series pointing here, strongest first.
    pub from: Vec<(i64, String)>,
}

impl Scored {
    pub fn reason(&self) -> String {
        match self.from.as_slice() {
            [] => String::new(),
            [(_, one)] => format!("More like {one}"),
            many => format!("{} of yours point here", many.len()),
        }
    }
}

/// Every title your read series point at, best first (see the module's notes).
pub fn score(read: &[(&Mine, &WithRecs)], mine: &[Mine], t: &Taste, hidden: &HashSet<i64>) -> Vec<Scored> {
    // Per title: it, its score so far, and (share, work, title) for each series pointing at it.
    type Pointers = Vec<(f64, i64, String)>;
    let mut by_id: HashMap<i64, (Media, f64, Pointers)> = HashMap::new();
    for (s, wr) in read {
        let weight = if s.finished { 1.5 } else { 1.0 };
        for (rating, m) in &wr.recs {
            if unwanted(m, hidden) || in_space(m, mine).is_some() {
                continue;
            }
            let part = (2.0 + (*rating).max(0) as f64).ln() * weight;
            let e = by_id.entry(m.id).or_insert_with(|| (m.clone(), 0.0, Vec::new()));
            e.1 += part;
            e.2.push((part, s.work, s.title.clone()));
        }
    }
    let mut out: Vec<Scored> = by_id
        .into_values()
        .map(|(media, base, mut from)| {
            from.sort_by(|a, b| b.0.total_cmp(&a.0));
            let mut score = base + leaning(&media, t);
            if t.east && !media.east() {
                score *= 0.7;
            }
            Scored { media, score, from: from.into_iter().map(|(_, w, title)| (w, title)).collect() }
        })
        .collect();
    out.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.media.id.cmp(&b.media.id)));
    out
}

pub fn status_of(s: &str) -> &'static str {
    match s {
        "RELEASING" => "ongoing",
        "FINISHED" => "completed",
        "HIATUS" => "hiatus",
        "CANCELLED" => "cancelled",
        _ => "unknown",
    }
}

pub fn rec_of(m: &Media, reason: String, hits: Vec<Hit>, in_space: Option<i64>) -> Rec {
    Rec {
        anilist_id: m.id,
        title: m.title.clone(),
        reason,
        genres: m.genres.clone(),
        chapters: m.chapters,
        status: status_of(&m.status).to_string(),
        hits,
        cover: format!("/v1/recs/cover/{}", m.id),
        in_space,
    }
}

/// Your `n` most recently read series, for "Because you read X".
pub fn recent<'a>(read: &[(&'a Mine, &'a WithRecs)], n: usize) -> Vec<(&'a Mine, &'a WithRecs)> {
    let mut v = read.to_vec();
    v.sort_by_key(|(s, _)| std::cmp::Reverse(s.last_read_at));
    v.truncate(n);
    v
}

/// A finished series worth reading in one go: well scored, with chapters enough.
fn bingeable(m: &Media) -> bool {
    m.status == "FINISHED" && m.score.is_some_and(|s| s >= 75) && m.chapters.is_none_or(|c| c >= 20)
}

/// The page, from the scored titles, your most recently read series with their recommendations,
/// and the popular titles of your most read tags; only titles with `hits` on your sources.
/// `made_at` is left for the caller.
pub fn assemble(scored: &[Scored], because: &[(&Mine, &WithRecs)], collections: &[(String, Vec<Media>)], mine: &[Mine], t: &Taste, hidden: &HashSet<i64>, hits: &HashMap<i64, Vec<Hit>>) -> Recs {
    let carried = |id: i64| hits.get(&id).filter(|h| !h.is_empty()).cloned();
    let binge = scored.iter().filter(|s| bingeable(&s.media)).find_map(|s| {
        let h = carried(s.media.id)?;
        let words = match s.media.score {
            Some(sc) => format!("Finished, scored {sc}%"),
            None => "Finished".to_string(),
        };
        Some(rec_of(&s.media, words, h, None))
    });
    let top: Vec<Rec> = scored
        .iter()
        .filter(|s| binge.as_ref().is_none_or(|b| b.anilist_id != s.media.id))
        .filter_map(|s| Some(rec_of(&s.media, s.reason(), carried(s.media.id)?, None)))
        .take(TOP)
        .collect();
    let because: Vec<Because> = because
        .iter()
        .filter_map(|(s, wr)| {
            let recs: Vec<Rec> = wr
                .recs
                .iter()
                .filter(|(_, m)| !unwanted(m, hidden) && in_space(m, mine).is_none())
                .filter_map(|(_, m)| Some(rec_of(m, format!("More like {}", s.title), carried(m.id)?, None)))
                .take(BECAUSE_EACH)
                .collect();
            (!recs.is_empty()).then(|| Because { series: s.work, title: s.title.clone(), recs })
        })
        .collect();
    let collections: Vec<Collection> = collections
        .iter()
        .filter_map(|(tag, list)| {
            let mut list: Vec<&Media> = list.iter().filter(|m| !unwanted(m, hidden) && in_space(m, mine).is_none()).collect();
            if t.east {
                // Stable: popularity order kept within each side.
                list.sort_by_key(|m| !m.east());
            }
            let recs: Vec<Rec> = list.into_iter().filter_map(|m| Some(rec_of(m, format!("Popular in {}", title_case(tag)), carried(m.id)?, None))).take(COLLECTION_EACH).collect();
            (!recs.is_empty()).then(|| Collection { tag: title_case(tag), recs })
        })
        .collect();
    Recs { top, because, collections, binge, made_at: 0 }
}

fn title_case(tag: &str) -> String {
    tag.split(' ').map(|w| {
        let mut c = w.chars();
        c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
    }).collect::<Vec<_>>().join(" ")
}

/// The titles to look for on the sources, most wanted first: the top scored, then each "because"
/// list's, then each collection's; once each, `max` at most.
pub fn wanted(scored: &[Scored], because: &[(&Mine, &WithRecs)], collections: &[(String, Vec<Media>)], mine: &[Mine], hidden: &HashSet<i64>, max: usize) -> Vec<Media> {
    let mut out: Vec<Media> = Vec::new();
    let mut seen: HashSet<i64> = HashSet::new();
    let ok = |m: &Media| !unwanted(m, hidden) && in_space(m, mine).is_none();
    let firsts = scored.iter().take(30).map(|s| &s.media);
    let from_because = because.iter().flat_map(|(_, wr)| wr.recs.iter().map(|(_, m)| m).filter(|m| ok(m)).take(12));
    let from_tags = collections.iter().flat_map(|(_, l)| l.iter().filter(|m| ok(m)).take(15));
    for m in firsts.chain(from_because).chain(from_tags) {
        if out.len() >= max {
            break;
        }
        if seen.insert(m.id) {
            out.push(m.clone());
        }
    }
    out
}

/// Recommendations still worth showing now: what you hid or followed since they were made, and
/// collections of tags you asked fewer of, left out.
pub fn still_wanted(mut r: Recs, hidden: &HashSet<i64>, less: &HashSet<String>, mine: &[Mine]) -> Recs {
    let keep = |x: &Rec| !hidden.contains(&x.anilist_id) && !mine.iter().any(|s| s.al == Some(x.anilist_id) || s.names.iter().any(|n| same_work(n, &x.title)));
    r.top.retain(keep);
    for b in &mut r.because {
        b.recs.retain(keep);
    }
    r.because.retain(|b| !b.recs.is_empty());
    r.collections.retain(|c| !less.contains(&c.tag.to_lowercase()));
    for c in &mut r.collections {
        c.recs.retain(keep);
    }
    r.collections.retain(|c| !c.recs.is_empty());
    r.binge = r.binge.filter(keep);
    r
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Kept<T> {
    at: i64,
    v: T,
}

impl Ctx {
    /// An AniList query through its budget.
    fn al(&self, q: &str, vars: Value, lane: Lane) -> Result<Value, String> {
        self.ask("AniList", lane, || crate::anilist::query(q, vars))
    }

    /// The work's AniList id: kept (`al_id:{work}`, "none" when AniList has no entry), else asked.
    pub(crate) fn anilist_id(&self, work: i64, title: &str, lane: Lane) -> Option<i64> {
        let key = format!("al_id:{work}");
        let known = self.store().get(&key);
        if let Some(v) = known {
            return v.parse().ok();
        }
        match self.ask("AniList", lane, || crate::anilist::media_id(title)) {
            Ok(found) => {
                self.store().set(&key, &found.map(|i| i.to_string()).unwrap_or_else(|| "none".into()));
                found
            }
            Err(e) => {
                // Paused: said once by `ask`, not once per series.
                if !e.starts_with(crate::autopilot::PAUSED) {
                    log(&format!("anilist id of {title}: {e}"));
                }
                None
            }
        }
    }

    /// Your space as recommendations see it; AniList ids looked up for the series you've read.
    fn mine(&self) -> Vec<Mine> {
        let works = self.store().works();
        let mut out = Vec::new();
        for w in works {
            let Ok(m) = self.suwa.manga(w.manga_id) else { continue };
            let d = self.detail_now(&w, &m);
            let read = d.series.read_upto.is_some();
            let al = if read {
                self.anilist_id(w.id, &w.title, Lane::Back)
            } else {
                let known = self.store().get(&format!("al_id:{}", w.id));
                known.and_then(|v| v.parse().ok())
            };
            let mut names = vec![w.title.clone()];
            names.extend(self.store().aliases(w.id));
            out.push(Mine {
                work: w.id,
                title: w.title.clone(),
                names,
                al,
                read,
                finished: d.series.status == "completed" && d.series.unread == 0,
                last_read_at: d.series.last_read_at,
            });
        }
        out
    }

    /// The sources searched for availability: `LEAF_REC_SOURCES` that are installed, else the
    /// first three installed.
    fn rec_sources(&self) -> Vec<crate::suwa::Source> {
        let all = self.suwa.sources(&self.lang).unwrap_or_default();
        let picked: Vec<_> = all.iter().filter(|s| self.rec_source_names.iter().any(|r| r.eq_ignore_ascii_case(&s.name))).cloned().collect();
        if picked.is_empty() { all.into_iter().take(3).collect() } else { picked }
    }

    /// Hits for each title on the broad sources, kept a week (`rec_hits:{id}`); a title a
    /// source could not be asked about (paused, down) is asked again tomorrow.
    fn availability(&self, want: &[Media], now: i64) -> HashMap<i64, Vec<Hit>> {
        let mut out: HashMap<i64, Vec<Hit>> = HashMap::new();
        let mut ask: Vec<&Media> = Vec::new();
        for m in want {
            let kept = self.store().get(&format!("rec_hits:{}", m.id));
            match kept.and_then(|v| serde_json::from_str::<Kept<Vec<Hit>>>(&v).ok()) {
                Some(k) if now - k.at < HITS_KEPT => {
                    out.insert(m.id, k.v);
                }
                _ => ask.push(m),
            }
        }
        let ask: Vec<&Media> = ask.into_iter().take(self.rec_searches).collect();
        if ask.is_empty() {
            return out;
        }
        let sources = self.rec_sources();
        let ask = &ask;
        // One thread per source, each asking about every title in turn at its own pace.
        let found: Vec<HashMap<i64, Result<Vec<Hit>, ()>>> = std::thread::scope(|s| {
            let hs: Vec<_> = sources
                .iter()
                .map(|src| {
                    s.spawn(move || {
                        let mut got: HashMap<i64, Result<Vec<Hit>, ()>> = HashMap::new();
                        for m in ask.iter() {
                            let mut result: Result<Vec<Hit>, ()> = Ok(Vec::new());
                            for q in m.names.iter().take(2) {
                                match self.ask(&src.name, Lane::Back, || self.suwa.search(&src.id, q)) {
                                    Ok(list) => {
                                        if let Some(f) = list.into_iter().find(|f| m.names.iter().any(|n| same_work(&f.title, n))) {
                                            result = Ok(vec![Hit { manga_id: f.id, source: src.name.clone() }]);
                                            break;
                                        }
                                    }
                                    Err(_) => {
                                        result = Err(());
                                        break;
                                    }
                                }
                            }
                            got.insert(m.id, result);
                        }
                        got
                    })
                })
                .collect();
            hs.into_iter().filter_map(|h| h.join().ok()).collect()
        });
        for m in ask.iter() {
            let mut hits: Vec<Hit> = Vec::new();
            let mut whole = true;
            for f in &found {
                match f.get(&m.id) {
                    Some(Ok(h)) => hits.extend(h.iter().cloned()),
                    _ => whole = false,
                }
            }
            if whole {
                self.store().set(&format!("rec_hits:{}", m.id), &serde_json::to_string(&Kept { at: now, v: hits.clone() }).unwrap_or_default());
            }
            out.insert(m.id, hits);
        }
        out
    }

    fn remember_covers(&self, media: &[&Media]) {
        for m in media {
            if let Some(c) = &m.cover {
                self.store().set(&format!("rec_cover:{}", m.id), c);
            }
        }
    }

    /// Makes the day's recommendations (slow: AniList and the sources are asked, in turn).
    pub fn make_recs(&self) -> Result<Recs, String> {
        let now = now_ms();
        let mine = self.mine();
        let hidden = self.store().hidden_recs();
        let less = self.store().less_tags();
        let ids: Vec<i64> = mine.iter().filter(|s| s.read).filter_map(|s| s.al).collect();
        let mut asked: Vec<WithRecs> = Vec::new();
        let q = format!("query($ids:[Int]){{ Page(perPage:{BATCH}){{ media(id_in:$ids,type:MANGA){{ {MEDIA} recommendations(sort:RATING_DESC,perPage:10){{ nodes{{ rating mediaRecommendation{{ {MEDIA} }} }} }} }} }} }}");
        for part in ids.chunks(BATCH) {
            match self.al(&q, json!({ "ids": part }), Lane::Back) {
                Ok(d) => asked.extend(parse_with_recs(&d)),
                Err(e) => {
                    log(&format!("recommendations: {e}"));
                    if e.starts_with(crate::autopilot::PAUSED) {
                        break;
                    }
                }
            }
        }
        let read: Vec<(&Mine, &WithRecs)> = mine.iter().filter(|s| s.read).filter_map(|s| Some((s, asked.iter().find(|w| Some(w.media.id) == s.al)?))).collect();
        if read.is_empty() {
            return Err("no series read that AniList knows yet".into());
        }
        let t = taste(&read.iter().map(|(_, w)| &w.media).collect::<Vec<_>>(), &less);
        let scored = score(&read, &mine, &t, &hidden);
        let recent = recent(&read, BECAUSE_SERIES);
        let tq = format!("query($t:String){{ Page(perPage:30){{ media(tag:$t,type:MANGA,format:MANGA,isAdult:false,sort:POPULARITY_DESC){{ {MEDIA} }} }} }}");
        let mut collections: Vec<(String, Vec<Media>)> = Vec::new();
        for tag in top_tags(&t, COLLECTIONS) {
            // AniList's tag names are title case ("Time Manipulation"); the taste keeps them lower.
            let name = read.iter().flat_map(|(_, w)| w.media.tags.iter()).find(|(n, _)| n.to_lowercase() == tag).map(|(n, _)| n.clone()).unwrap_or(tag.clone());
            match self.al(&tq, json!({ "t": name }), Lane::Back) {
                Ok(d) => collections.push((name, parse_page(&d))),
                Err(e) => log(&format!("recommendations, {name}: {e}")),
            }
        }
        let want = wanted(&scored, &recent, &collections, &mine, &hidden, usize::MAX);
        let hits = self.availability(&want, now);
        let r = Recs { made_at: now, ..assemble(&scored, &recent, &collections, &mine, &t, &hidden, &hits) };
        let shown: Vec<&Media> = want.iter().collect();
        self.remember_covers(&shown);
        self.store().set("recs", &serde_json::to_string(&r).unwrap_or_default());
        Ok(r)
    }

    /// `GET /v1/recs`: the day's, less what you hid or added since.
    pub fn recs(&self) -> Recs {
        let kept = self.store().get("recs");
        let r: Recs = kept.and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default();
        let hidden = self.store().hidden_recs();
        let less = self.store().less_tags();
        let works = self.store().works();
        let mine: Vec<Mine> = works
            .iter()
            .map(|w| {
                let al = self.store().get(&format!("al_id:{}", w.id)).and_then(|v| v.parse().ok());
                Mine { work: w.id, title: w.title.clone(), names: vec![w.title.clone()], al, ..Default::default() }
            })
            .collect();
        still_wanted(r, &hidden, &less, &mine)
    }

    pub fn hide_rec(&self, anilist_id: i64) {
        self.store().hide_rec(anilist_id, now_ms());
    }

    pub fn rec_tag(&self, tag: &str, less: bool) {
        self.store().set_rec_tag(tag, less);
    }

    /// "Readers also like" for one series: AniList's recommendations from it, kept a day; hits
    /// only as already known (no source is searched for this), yours marked.
    pub fn also(&self, id: i64) -> Result<Vec<Rec>, String> {
        let w = self.store().work(id).ok_or("no such series")?;
        let key = format!("also:{id}");
        let now = now_ms();
        let hidden = self.store().hidden_recs();
        let kept = self.store().get(&key);
        if let Some(k) = kept.and_then(|v| serde_json::from_str::<Kept<Vec<Rec>>>(&v).ok()).filter(|k| now - k.at < DAY) {
            return Ok(k.v.into_iter().filter(|r| !hidden.contains(&r.anilist_id)).collect());
        }
        let al = self.anilist_id(w.id, &w.title, Lane::Fore).ok_or("AniList doesn't know this series")?;
        let q = format!("query($ids:[Int]){{ Page(perPage:1){{ media(id_in:$ids,type:MANGA){{ {MEDIA} recommendations(sort:RATING_DESC,perPage:15){{ nodes{{ rating mediaRecommendation{{ {MEDIA} }} }} }} }} }} }}");
        let d = self.al(&q, json!({ "ids": [al] }), Lane::Fore)?;
        let list = parse_with_recs(&d);
        let works = self.store().works();
        let mut mine: Vec<Mine> = Vec::new();
        for x in &works {
            let al = self.store().get(&format!("al_id:{}", x.id)).and_then(|v| v.parse().ok());
            let mut names = vec![x.title.clone()];
            names.extend(self.store().aliases(x.id));
            mine.push(Mine { work: x.id, title: x.title.clone(), names, al, ..Default::default() });
        }
        let mut out: Vec<Rec> = Vec::new();
        let mut covers: Vec<&Media> = Vec::new();
        for (_, m) in list.iter().flat_map(|w| w.recs.iter()) {
            if unwanted(m, &hidden) {
                continue;
            }
            let kept = self.store().get(&format!("rec_hits:{}", m.id));
            let hits = kept.and_then(|v| serde_json::from_str::<Kept<Vec<Hit>>>(&v).ok()).map(|k| k.v).unwrap_or_default();
            out.push(rec_of(m, format!("Readers of {} also like it", w.title), hits, in_space(m, &mine)));
            covers.push(m);
        }
        self.remember_covers(&covers);
        self.store().set(&key, &serde_json::to_string(&Kept { at: now, v: out.clone() }).unwrap_or_default());
        Ok(out)
    }

    /// A recommendation's cover: AniList's picture, fetched once and kept in `recs/covers/`.
    pub fn rec_cover(&self, anilist_id: i64) -> Result<(Vec<u8>, String), String> {
        let dir = self.data.join("recs").join("covers");
        let path = dir.join(format!("{anilist_id}.img"));
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => {
                let url = self.store().get(&format!("rec_cover:{anilist_id}")).ok_or(NO_COVER)?;
                // Only AniList's own pictures: the address came from AniList, but is checked.
                let host = url.strip_prefix("https://").and_then(|r| r.split('/').next()).unwrap_or("");
                if !(host == "anilist.co" || host.ends_with(".anilist.co")) {
                    return Err("not an AniList picture".into());
                }
                let b = crate::anilist::picture(&url)?;
                let _ = std::fs::create_dir_all(&dir);
                let tmp = path.with_extension("tmp");
                if std::fs::write(&tmp, &b).is_ok() {
                    let _ = std::fs::rename(&tmp, &path);
                }
                b
            }
        };
        let kind = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
            "image/png"
        } else if bytes.starts_with(b"RIFF") {
            "image/webp"
        } else {
            "image/jpeg"
        };
        Ok((bytes, kind.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shaped as AniList answers `Page(media(id_in:...)){ ... recommendations }` (trimmed).
    fn fixture() -> Value {
        json!({ "Page": { "media": [
            { "id": 1, "title": { "english": "Nano Machine", "romaji": "Nano Mashin" }, "synonyms": [], "format": "MANGA", "countryOfOrigin": "KR",
              "isAdult": false, "status": "RELEASING", "chapters": null, "genres": ["Action", "Martial Arts"], "tags": [{ "name": "Murim", "rank": 90 }, { "name": "Cultivation", "rank": 70 }],
              "averageScore": 82, "popularity": 50000, "coverImage": { "large": "https://s4.anilist.co/file/1.jpg" },
              "recommendations": { "nodes": [
                { "rating": 120, "mediaRecommendation": { "id": 10, "title": { "english": "Return of the Mount Hua Sect" }, "format": "MANGA", "countryOfOrigin": "KR", "isAdult": false, "status": "RELEASING", "genres": ["Action", "Martial Arts"], "tags": [{ "name": "Murim", "rank": 95 }], "averageScore": 85 } },
                { "rating": 40, "mediaRecommendation": { "id": 11, "title": { "english": "The Breaker" }, "format": "MANGA", "countryOfOrigin": "KR", "isAdult": false, "status": "FINISHED", "chapters": 72, "genres": ["Action"], "tags": [], "averageScore": 80 } },
                { "rating": 300, "mediaRecommendation": { "id": 12, "title": { "english": "Adult One" }, "format": "MANGA", "countryOfOrigin": "KR", "isAdult": true, "genres": ["Action"] } },
                { "rating": 200, "mediaRecommendation": { "id": 13, "title": { "english": "A Novel" }, "format": "NOVEL", "countryOfOrigin": "KR", "isAdult": false, "genres": ["Action"] } },
                { "rating": 90, "mediaRecommendation": { "id": 2, "title": { "english": "Murim Login" }, "format": "MANGA", "countryOfOrigin": "KR", "isAdult": false, "genres": ["Action"] } }
              ] } },
            { "id": 2, "title": { "english": "Murim Login" }, "format": "MANGA", "countryOfOrigin": "KR", "isAdult": false, "status": "FINISHED",
              "genres": ["Action", "Fantasy"], "tags": [{ "name": "Murim", "rank": 80 }, { "name": "Isekai", "rank": 75 }],
              "recommendations": { "nodes": [
                { "rating": 30, "mediaRecommendation": { "id": 10, "title": { "english": "Return of the Mount Hua Sect" }, "format": "MANGA", "countryOfOrigin": "KR", "isAdult": false, "status": "RELEASING", "genres": ["Action", "Martial Arts"], "tags": [{ "name": "Murim", "rank": 95 }] } },
                { "rating": 40, "mediaRecommendation": { "id": 14, "title": { "english": "Isekai Thing", "romaji": "Isekai Mono" }, "format": "MANGA", "countryOfOrigin": "JP", "isAdult": false, "status": "RELEASING", "genres": ["Fantasy"], "tags": [{ "name": "Isekai", "rank": 90 }] } },
                { "rating": 20, "mediaRecommendation": { "id": 15, "title": { "english": "Hidden One" }, "format": "MANGA", "countryOfOrigin": "KR", "isAdult": false, "genres": ["Action"] } },
                { "rating": 25, "mediaRecommendation": { "id": 16, "title": { "english": "Nano Machine: Side Story" }, "synonyms": ["Junk to King"], "format": "MANGA", "countryOfOrigin": "CN", "isAdult": false, "genres": ["Action"] } }
              ] } }
        ] } })
    }

    fn mine() -> Vec<Mine> {
        vec![
            Mine { work: 101, title: "Nano Machine".into(), names: vec!["Nano Machine".into()], al: Some(1), read: true, finished: false, last_read_at: 50 },
            Mine { work: 102, title: "Murim Login".into(), names: vec!["Murim Login".into()], al: Some(2), read: true, finished: true, last_read_at: 90 },
            Mine { work: 103, title: "Junk to King".into(), names: vec!["Junk to King".into(), "Became the King by Scavenging".into()], al: None, read: false, finished: false, last_read_at: 0 },
        ]
    }

    #[test]
    fn titles_parse_with_all_their_names() {
        let w = parse_with_recs(&fixture());
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].media.title, "Nano Machine");
        assert_eq!(w[0].media.names, vec!["Nano Machine", "Nano Mashin"]);
        assert_eq!(w[0].media.top_tags().collect::<Vec<_>>(), vec!["Murim", "Cultivation"]);
        assert_eq!(w[0].recs.len(), 5);
        assert_eq!(w[0].recs[1].1.chapters, Some(72));
        assert_eq!(status_of("FINISHED"), "completed");
    }

    #[test]
    fn scores_add_up_across_your_series_and_leave_out_what_should_be() {
        let w = parse_with_recs(&fixture());
        let m = mine();
        let read: Vec<(&Mine, &WithRecs)> = vec![(&m[0], &w[0]), (&m[1], &w[1])];
        let t = taste(&[&w[0].media, &w[1].media], &HashSet::new());
        assert!(t.east, "all Korean");
        assert_eq!(t.tags.get("murim"), Some(&1.0));
        let hidden: HashSet<i64> = [15].into();
        let s = score(&read, &m, &t, &hidden);
        let ids: Vec<i64> = s.iter().map(|x| x.media.id).collect();
        assert_eq!(ids[0], 10, "two of yours point at Mount Hua: {ids:?}");
        assert_eq!(s[0].reason(), "2 of yours point here");
        assert_eq!(s[0].from[0].0, 102, "the stronger pointer first: a finished series counts half as much again");
        for gone in [12, 13, 2, 15, 16] {
            assert!(!ids.contains(&gone), "{gone} left out (adult, a novel, yours, hidden, yours by another name): {ids:?}");
        }
        let breaker = s.iter().find(|x| x.media.id == 11).unwrap();
        assert_eq!(breaker.reason(), "More like Nano Machine");
        // A finished series of yours counts half as much again: Murim Login's 40 for the Isekai
        // title beats Nano Machine's 40 for The Breaker, until the Japanese title is weighed less.
        let isekai = s.iter().find(|x| x.media.id == 14).unwrap();
        assert!(isekai.score < breaker.score, "Korean reading: a Japanese title weighs less");
        let t2 = Taste { east: false, ..t.clone() };
        let s2 = score(&read, &m, &t2, &hidden);
        let (i2, b2) = (s2.iter().find(|x| x.media.id == 14).unwrap(), s2.iter().find(|x| x.media.id == 11).unwrap());
        assert!(i2.score > b2.score, "without that, the finished series' pointer wins: {} vs {}", i2.score, b2.score);
        // "Fewer like Isekai" pushes it down hard.
        let less: HashSet<String> = ["isekai".to_string()].into();
        let t3 = taste(&[&w[0].media, &w[1].media], &less);
        let s3 = score(&read, &m, &t3, &hidden);
        assert_eq!(s3.last().unwrap().media.id, 14);
        assert_eq!(top_tags(&t3, 4), vec!["murim", "cultivation"], "isekai left out of the collections");
    }

    #[test]
    fn the_page_keeps_only_what_the_sources_carry() {
        let w = parse_with_recs(&fixture());
        let m = mine();
        let read: Vec<(&Mine, &WithRecs)> = vec![(&m[0], &w[0]), (&m[1], &w[1])];
        let t = taste(&[&w[0].media, &w[1].media], &HashSet::new());
        let hidden = HashSet::new();
        let s = score(&read, &m, &t, &hidden);
        let popular = vec![
            Media { id: 20, title: "Popular Murim".into(), names: vec!["Popular Murim".into()], format: "MANGA".into(), country: "JP".into(), ..Default::default() },
            Media { id: 21, title: "Korean Murim".into(), names: vec!["Korean Murim".into()], format: "MANGA".into(), country: "KR".into(), ..Default::default() },
            Media { id: 1, title: "Nano Machine".into(), names: vec!["Nano Machine".into()], format: "MANGA".into(), country: "KR".into(), ..Default::default() },
        ];
        let collections = vec![("Murim".to_string(), popular)];
        let hit = |id| (id, vec![Hit { manga_id: id * 100, source: "MangaDex".into() }]);
        let hits: HashMap<i64, Vec<Hit>> = [hit(10), hit(11), (14, vec![]), hit(20), hit(21)].into_iter().collect();
        let last = recent(&read, 3);
        assert_eq!(recent(&read, 1).len(), 1);
        let want = wanted(&s, &last, &collections, &m, &hidden, 99);
        assert!(want.iter().any(|x| x.id == 20) && !want.iter().any(|x| x.id == 1), "yours are not looked for");
        assert_eq!(wanted(&s, &last, &collections, &m, &hidden, 2).len(), 2);
        let r = assemble(&s, &last, &collections, &m, &t, &hidden, &hits);
        assert_eq!(r.binge.as_ref().map(|b| b.anilist_id), Some(11), "The Breaker: finished, 80%");
        assert_eq!(r.binge.as_ref().unwrap().reason, "Finished, scored 80%");
        assert_eq!(r.top.iter().map(|x| x.anilist_id).collect::<Vec<_>>(), vec![10], "14 has no hits, 11 is the binge");
        assert_eq!(r.top[0].cover, "/v1/recs/cover/10");
        assert_eq!(r.because.len(), 2);
        assert_eq!(r.because[0].series, 102, "the one read last first");
        assert_eq!(r.because[0].recs[0].reason, "More like Murim Login");
        assert_eq!(r.because[1].recs.iter().map(|x| x.anilist_id).collect::<Vec<_>>(), vec![10, 11]);
        assert_eq!(r.collections[0].tag, "Murim");
        assert_eq!(r.collections[0].recs.iter().map(|x| x.anilist_id).collect::<Vec<_>>(), vec![21, 20], "Korean first for a Korean reader");
        // Hidden or followed since: gone at once, without a new day's run.
        let mut m2 = m.clone();
        m2.push(Mine { work: 104, title: "Korean Murim".into(), names: vec!["Korean Murim".into()], ..Default::default() });
        let later = still_wanted(r.clone(), &[10].into(), &HashSet::new(), &m2);
        assert!(later.top.is_empty());
        assert_eq!(later.collections[0].recs.len(), 1);
        let fewer = still_wanted(r, &HashSet::new(), &["murim".to_string()].into(), &m);
        assert!(fewer.collections.is_empty());
    }
}
