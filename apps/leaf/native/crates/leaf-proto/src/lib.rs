//! What Leaf's phone app and leaf-server say to each other (JSON over HTTPS, `/v1/...`), and the
//! few rules both sides must read the same way: how titles are compared, which chapters are
//! missing, which copy of a chapter is the one to read.
//!
//! A series is Leaf's own record (`Series::id`), not a source's: when the autopilot moves a series
//! to another source the id stays, the chapters' ids change.

use serde::{Deserialize, Serialize};

/// One followed series, as the library lists it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Series {
    pub id: i64,
    pub title: String,
    pub author: Option<String>,
    /// The source the chapters come from now ("Asura Scans").
    pub source: String,
    /// "ongoing", "completed", "hiatus", "cancelled" or "unknown".
    pub status: String,
    /// Distinct chapter numbers there are to read.
    pub chapters: u32,
    /// The highest chapter number read to its end, if any.
    pub read_upto: Option<f64>,
    pub unread: u32,
    pub latest: Option<f64>,
    /// Runs of whole chapter numbers no source has, between the first and the latest.
    pub gaps: Vec<Gap>,
    /// When a chapter newer than the ones before arrived (ms since the epoch), 0 if never.
    pub updated_at: i64,
    /// Something the person should know, in words ("Asura Scans needs you to pass a check").
    pub note: Option<String>,
    /// The bell is on: new chapters of this series are announced (`/v1/events`, kind "new").
    #[serde(default)]
    pub bell: bool,
    /// When it was added to your space (ms since the epoch).
    #[serde(default)]
    pub added_at: i64,
    /// When a chapter of it was last read (ms since the epoch), 0 if never.
    #[serde(default)]
    pub last_read_at: i64,
    /// The shelf you put it on: "reading", "plan", "hold", "dropped" or "done"; None when you
    /// haven't chosen (the app shows Reading once started, Plan to read before).
    #[serde(default)]
    pub shelf: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Gap {
    pub from: f64,
    pub to: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Chapter {
    pub id: i64,
    pub number: f64,
    pub name: String,
    pub group: Option<String>,
    pub pages: u32,
    pub read: bool,
    pub last_page: u32,
    /// Kept on the server (and so quick to open, converted to WebP).
    pub downloaded: bool,
    /// When the source published it (ms since the epoch).
    pub uploaded_at: i64,
    /// The source it comes from when that is not the series' own: a chapter the series' source
    /// lacks, filled in from another ("Drake Scans").
    #[serde(default)]
    pub from: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SeriesDetail {
    pub series: Series,
    pub description: Option<String>,
    /// One per chapter number, oldest first (see [`pick_copies`]).
    pub chapters: Vec<Chapter>,
}

/// A search result: one work, found on one or more sources.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Candidate {
    pub title: String,
    /// A Suwayomi manga id whose cover stands for the work.
    pub cover_of: Option<i64>,
    pub hits: Vec<Hit>,
    /// Already followed, as this series.
    pub followed: Option<i64>,
    /// Found only on Webtoon, which sells these episodes ahead of the free ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paid: Option<PaidAhead>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Hit {
    pub manga_id: i64,
    pub source: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Follow {
    pub title: String,
    pub manga_ids: Vec<i64>,
    /// Added in bulk (an import): the other sources are looked at by the next hourly pass, not
    /// at once, so a hundred adds don't become thousands of requests to the sites in a minute.
    #[serde(default)]
    pub quiet: bool,
}

/// Every chapter numbered below `before` is marked read: where you are, brought from elsewhere.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct ReadBefore {
    pub before: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Pages {
    pub chapter_id: i64,
    pub count: u32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Bell {
    pub on: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Progress {
    /// The page the reader is on (0-based).
    pub page: u32,
    /// The chapter was read to its end.
    pub done: bool,
}

/// Something the autopilot did or needs, newest first in `/v1/events`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Event {
    pub at: i64,
    pub series: Option<i64>,
    /// "switched", "new", "blocked", "lost", "elsewhere" (a watched series another source has
    /// now) or "info".
    pub kind: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Status {
    pub server: String,
    pub suwayomi: Option<String>,
    pub sources: Vec<String>,
    pub series: u32,
    /// AniList sync is set up (a token is stored on the server).
    #[serde(default)]
    pub anilist: bool,
}

/// A title as compared across sources: lower case, letters and digits only, words single-spaced,
/// a leading "the" dropped, and what follows a colon kept (sources disagree on subtitles, so
/// [`same_work`] also tries without it).
pub fn norm_title(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    let mut space = true;
    for c in t.chars() {
        let c = match c {
            '’' | '\'' | '`' => continue,
            '&' => {
                if !space {
                    out.push(' ');
                }
                out.push_str("and ");
                space = true;
                continue;
            }
            c => c,
        };
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            space = false;
        } else if !space {
            out.push(' ');
            space = true;
        }
    }
    let out = out.trim_end().to_string();
    match out.strip_prefix("the ") {
        Some(rest) => rest.to_string(),
        None => out,
    }
}

/// The title before its subtitle ("Junk to King: I Am A Scavenger" → "Junk to King").
fn main_title(t: &str) -> &str {
    t.split([':', '–', '—', '(']).next().unwrap_or(t).trim()
}

/// The subtitle, what follows the main title ("" when there is none).
fn subtitle(t: &str) -> &str {
    t.find([':', '–', '—', '(']).map_or("", |i| &t[i..])
}

/// Words in a subtitle that make it another work of the same name: "Tower of God: Urek Mazino
/// Side Story" is not "Tower of God".
const SPIN_OFF: [&str; 12] =
    ["side story", "side stories", "spin off", "spinoff", "gaiden", "after story", "another story", "extra", "special", "anthology", "remake", "season"];

/// Whether two sources' titles name the same work: equal once normalised, equal without their
/// subtitles when that main title is at least three words (two is too weak: "Solo Leveling" and
/// "Solo Leveling: Ragnarok" are different works) and the subtitle left out is not a spin-off's
/// ("Side Story"), or nine tenths of their words shared once the
/// small words ("the", "I", "a") are set aside.
pub fn same_work(a: &str, b: &str) -> bool {
    let (na, nb) = (norm_title(a), norm_title(b));
    if na.is_empty() || nb.is_empty() {
        return false;
    }
    if na == nb {
        return true;
    }
    let (ma, mb) = (norm_title(main_title(a)), norm_title(main_title(b)));
    if ma == mb && ma.split(' ').count() >= 3 {
        // One side leaving its subtitle out, unless that subtitle names a spin-off; two
        // different subtitles are left to the shared words below.
        let (sa, sb) = (norm_title(subtitle(a)), norm_title(subtitle(b)));
        let spin_off = |s: &str| SPIN_OFF.iter().any(|w| format!(" {s} ").contains(&format!(" {w} ")));
        match (sa.is_empty(), sb.is_empty()) {
            (false, false) => {}
            (true, false) if spin_off(&sb) => return false,
            (false, true) if spin_off(&sa) => return false,
            _ => return true,
        }
    }
    // Small words come and go between translations ("I Became the King by Scavenging", "Became the
    // King by Scavenging"): compared without them.
    const SMALL: [&str; 6] = ["the", "a", "an", "i", "my", "of"];
    let words = |n: &str| -> std::collections::BTreeSet<String> { n.split(' ').filter(|w| !SMALL.contains(w)).map(String::from).collect() };
    let (wa, wb) = (words(&na), words(&nb));
    let shared = wa.intersection(&wb).count() as f64;
    let union = wa.union(&wb).count() as f64;
    union >= 3.0 && shared / union >= 0.9
}

/// The whole chapter numbers missing between 1 and the highest, as runs (a prologue, chapter 0,
/// is never missed). Part chapters (12.5) count as their whole number being there.
pub fn gaps(numbers: &[f64]) -> Vec<Gap> {
    let mut whole: Vec<i64> = numbers.iter().filter(|n| n.is_finite() && **n >= 0.0).map(|n| n.floor() as i64).collect();
    whole.sort_unstable();
    whole.dedup();
    // Walked number to number, not 1..=last: a source's misread "chapter 20241105" must not
    // cost twenty million steps on every library answer.
    let mut out = Vec::new();
    let mut have = 0;
    for n in whole {
        if n > have + 1 {
            out.push(Gap { from: (have + 1) as f64, to: (n - 1) as f64 });
        }
        have = have.max(n);
    }
    out
}

/// A source's chapter, as `pick_copies` weighs it.
#[derive(Clone, Debug, PartialEq)]
pub struct Copy {
    pub id: i64,
    pub number: f64,
    pub group: Option<String>,
    pub uploaded_at: i64,
}

/// One copy per chapter number, oldest number first. Several groups often translate the same
/// chapter: the copy kept is the one from the group that did the most chapters of this series
/// (reading stays in one translation's voice), then the earliest upload. Negative numbers (a
/// source's "no number") are dropped.
pub fn pick_copies(all: &[Copy]) -> Vec<Copy> {
    let mut count: std::collections::HashMap<Option<&str>, usize> = std::collections::HashMap::new();
    for c in all {
        *count.entry(c.group.as_deref()).or_default() += 1;
    }
    let mut sorted: Vec<&Copy> = all.iter().filter(|c| c.number.is_finite() && c.number >= 0.0).collect();
    sorted.sort_by(|a, b| {
        a.number
            .total_cmp(&b.number)
            .then(count[&b.group.as_deref()].cmp(&count[&a.group.as_deref()]))
            .then(a.uploaded_at.cmp(&b.uploaded_at))
            .then(a.id.cmp(&b.id))
    });
    let mut out: Vec<Copy> = Vec::new();
    for c in sorted {
        if out.last().is_some_and(|l| l.number == c.number) {
            continue;
        }
        out.push(c.clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_compare_across_sources() {
        assert_eq!(norm_title("The Greatest Estate Developer"), "greatest estate developer");
        assert_eq!(norm_title("Swordmaster’s Youngest Son"), "swordmasters youngest son");
        assert_eq!(norm_title("Ash & Ivory!!"), "ash and ivory");
        assert!(same_work("Junk to King: I Am A Scavenger", "Junk to King"));
        assert!(same_work("Swordmaster's Youngest Son", "Swordmaster’s Youngest Son"));
        assert!(same_work("The Greatest Estate Developer", "Greatest Estate Developer"));
        assert!(!same_work("Solo Leveling", "Solo Leveling: Ragnarok"), "a one-word main title is too weak to match on");
        assert!(!same_work("Nano Machine", "Murim Login"));
        assert!(same_work("I Became the King By Scavenging", "Became the King by Scavenging"));
        assert!(!same_work("The King of the Tower", "The Tower of the King's Daughter"));
        assert!(!same_work("", "Anything"));
        assert!(!same_work("Tower of God", "Tower of God: Urek Mazino Side Story"), "a spin-off is another work");
        assert!(!same_work("Tower of God: Side Story", "Tower of God"));
        assert!(!same_work("Junk to King: Season Two", "Junk to King: I Am A Scavenger"), "two subtitles, two works");
        assert!(same_work("Junk to King: I Am A Scavenger", "Junk to King (I Am A Scavenger)"));
    }

    #[test]
    fn gaps_are_runs_of_missing_whole_numbers() {
        let mut n: Vec<f64> = (1..=22).map(f64::from).collect();
        n.extend((45..=54).map(f64::from));
        n.push(12.5);
        assert_eq!(gaps(&n), vec![Gap { from: 23.0, to: 44.0 }]);
        assert_eq!(gaps(&[0.0, 1.0, 2.0, 3.0]), vec![]);
        assert_eq!(gaps(&[2.0, 4.0]), vec![Gap { from: 1.0, to: 1.0 }, Gap { from: 3.0, to: 3.0 }]);
        assert_eq!(gaps(&[]), vec![]);
        assert_eq!(gaps(&[1.0, 2.0, 20241105.0]), vec![Gap { from: 3.0, to: 20241104.0 }], "a misread number is one gap, found at once");
    }

    #[test]
    fn one_copy_per_chapter_from_the_main_group() {
        let c = |id, number, group: &str, at| Copy { id, number, group: Some(group.into()), uploaded_at: at };
        let all = vec![
            c(1, 1.0, "Vortex", 10),
            c(2, 1.0, "Drake", 20),
            c(3, 2.0, "Vortex", 11),
            c(4, 2.0, "Drake", 21),
            c(5, 3.0, "Drake", 22),
            c(6, 4.0, "Manhuaga", 30),
            Copy { id: 7, number: -1.0, group: None, uploaded_at: 0 },
        ];
        let ids: Vec<i64> = pick_copies(&all).iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![2, 4, 5, 6]);
    }
}


/// A chapter that arrived for a series in your space (`GET /v1/updates`), newest first.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Update {
    pub series: i64,
    pub title: String,
    pub chapter: i64,
    pub number: f64,
    pub name: String,
    /// When the server first saw it (ms since the epoch).
    pub at: i64,
    pub read: bool,
    pub bell: bool,
}

/// `POST /v1/chapters/{id}/read`: read or unread (every copy of that chapter number).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct ChapterRead {
    pub read: bool,
}

/// `POST /v1/series/{id}/shelf`; None clears the choice.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ShelfSet {
    pub shelf: Option<String>,
}

/// `POST /v1/series/{id}/repin`: read this series from this Suwayomi manga instead (a wrong
/// match fixed by hand); your place is carried across by chapter number.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Repin {
    pub manga_id: i64,
}

/// One offer for "Wrong series?" (`GET /v1/series/{id}/alternatives`): a work found on a source,
/// with how many chapters it has, so the right one is easy to tell.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Alternative {
    pub manga_id: i64,
    pub title: String,
    pub source: String,
    pub chapters: u32,
    pub latest: Option<f64>,
    /// It is the series' current copy.
    pub current: bool,
}

/// A source's health (`GET /v1/health`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SourceHealth {
    pub name: String,
    pub ok: bool,
    /// When it last answered (ms since the epoch), 0 if never.
    pub last_ok_at: i64,
    pub last_error: Option<String>,
    /// The last error was a Cloudflare check that could not be passed.
    pub blocked: bool,
}

/// `POST /v1/settings/anilist`: an AniList access token (None removes it).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AnilistToken {
    pub token: Option<String>,
}

// Round 3 (2026-11-23). New types only: `Series` and `Chapter` are built field by field in
// leaf-server (`autopilot.rs`) and in the phone app's tests, so a new field there breaks both
// builds. The server sends these extra fields in the same JSON (serde ignores what a struct
// lacks), and the phone reads them beside the structs (leaf-app's `net::Extras`):
//
// - every `Series` (`/v1/library`, `/v1/series/{id}`): `"paid": PaidAhead | null`;
// - every `Chapter` (`/v1/series/{id}`): `"thumb": bool`, a picture at `/v1/chapters/{id}/thumb`;
// - every `SourceHealth` (`/v1/health`): `"paused_until": ms` (0 when not paused) and
//   `"requests_today": {"fore": n, "back": n}`.

/// Newer episodes the Webtoons.com app sells ahead of the free ones (`Series.paid`): "Chapters
/// 9–23 are on Webtoon (paid)", with a link that opens the Webtoon app or site. Leaf never fetches
/// a paid episode.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PaidAhead {
    pub from: f64,
    pub to: f64,
    pub url: String,
}

/// A series recommended from AniList, carried by at least one of your sources (`hits`, to add
/// it with `POST /v1/follow`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Rec {
    pub anilist_id: i64,
    pub title: String,
    /// Why, in words: "More like Nano Machine", "3 of yours point here".
    pub reason: String,
    #[serde(default)]
    pub genres: Vec<String>,
    pub chapters: Option<u32>,
    /// "ongoing", "completed", "hiatus", "cancelled" or "unknown", as `Series::status`.
    pub status: String,
    #[serde(default)]
    pub hits: Vec<Hit>,
    /// Its cover on this server: `/v1/recs/cover/{anilist_id}`.
    pub cover: String,
    /// Already in your space, as this series (only `/v1/series/{id}/also` lists those).
    #[serde(default)]
    pub in_space: Option<i64>,
}

/// "Because you read X".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Because {
    pub series: i64,
    pub title: String,
    pub recs: Vec<Rec>,
}

/// Popular titles with one of the tags you read most.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Collection {
    pub tag: String,
    pub recs: Vec<Rec>,
}

/// `GET /v1/recs`: made once a day on the server; `made_at` 0 when not made yet.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Recs {
    #[serde(default)]
    pub top: Vec<Rec>,
    #[serde(default)]
    pub because: Vec<Because>,
    #[serde(default)]
    pub collections: Vec<Collection>,
    /// A finished series with high scores, to read in one go.
    #[serde(default)]
    pub binge: Option<Rec>,
    #[serde(default)]
    pub made_at: i64,
}

/// `POST /v1/recs/tags`: fewer recommendations with this tag (`less`), or as before.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RecTag {
    pub tag: String,
    pub less: bool,
}

/// `POST` / `DELETE /v1/downloads`: chapters to download on the server, or to remove.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DownloadIds {
    pub ids: Vec<i64>,
}

/// `POST /v1/series/{id}/download`: the chapters numbered `from` to `to`, or every unread one.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct SeriesDownload {
    #[serde(default)]
    pub from: Option<f64>,
    #[serde(default)]
    pub to: Option<f64>,
    #[serde(default)]
    pub unread: bool,
}

/// `GET /v1/app/latest`: the newest phone app the server holds (`GET /v1/app/apk`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AppLatest {
    pub build: u32,
    pub version: String,
    pub size: u64,
    pub sha256: String,
    pub made_at: i64,
}

/// `POST` / `DELETE /v1/pages/hide`: a page the owner hid by hand ("Hide this page and ones like
/// it"), or that hiding undone; `all` on a DELETE forgets every page hidden.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct PageHide {
    #[serde(default)]
    pub chapter: i64,
    #[serde(default)]
    pub page: u32,
    #[serde(default)]
    pub all: bool,
}

/// `GET /v1/pages/hidden`: how many pages the owner hid by hand.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct HiddenPages {
    #[serde(default)]
    pub count: u32,
}

/// `GET /v1/search/sources`: the broad sources a search asks first (one request each, so the
/// phone shows each one's results as they come), and how many sources there are in all.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct SearchSources {
    #[serde(default)]
    pub broad: Vec<String>,
    #[serde(default)]
    pub all: u32,
}

/// The edit distance between two words (Levenshtein, over characters): letters added, taken
/// away or changed to turn one into the other.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diag = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let next = (diag + usize::from(ca != cb)).min(row[j] + 1).min(row[j + 1] + 1);
            diag = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

/// A word of a search found in a title's words, typos allowed: inside the title, the start of one
/// of its words, or within one letter of one (for words of three to five letters) or two (longer).
/// Words of one or two letters must be there as typed.
pub fn word_in_title(word: &str, title_words: &[&str], title: &str) -> bool {
    if title.contains(word) {
        return true;
    }
    let n = word.chars().count();
    let k = match n {
        0..=2 => return false,
        3..=5 => 1,
        _ => 2,
    };
    title_words.iter().any(|t| t.starts_with(word) || edit_distance(word, t) <= k)
}

/// A search's words found in a title: every word of the query in the title, typos allowed
/// ([`word_in_title`], both normalised, [`norm_title`]), or the two naming the same work
/// ([`same_work`]). What a source answers beyond that ("Solo Leveling" bringing back every title
/// with "level" in its synopsis) is not shown.
pub fn title_has_words(title: &str, query: &str) -> bool {
    let (t, q) = (norm_title(title), norm_title(query));
    if q.is_empty() || t.is_empty() {
        return false;
    }
    let words: Vec<&str> = t.split(' ').collect();
    q.split(' ').all(|w| word_in_title(w, &words, &t)) || same_work(title, query)
}

#[cfg(test)]
mod search_tests {
    use super::title_has_words;

    #[test]
    fn a_result_keeps_every_word_of_the_search() {
        assert!(title_has_words("Solo Leveling", "solo leveling"));
        assert!(title_has_words("Solo Leveling: Ragnarok", "solo lev"), "a word typed part way");
        assert!(title_has_words("The Greatest Estate Developer", "estate developer"));
        assert!(title_has_words("Omniscient Reader’s Viewpoint", "omniscient readers"));
        assert!(!title_has_words("Leveling With the Gods", "solo leveling"), "one word missing");
        assert!(!title_has_words("Return of the Mount Hua Sect", "solo"));
        assert!(title_has_words("I Became the King by Scavenging", "Became the King by Scavenging"));
        assert!(!title_has_words("", "solo"));
        assert!(!title_has_words("Solo Leveling", "  "));
    }

    #[test]
    fn a_typo_or_two_still_finds_the_title() {
        assert!(title_has_words("Solo Leveling", "solo levleing"), "two letters swapped in a long word");
        assert!(title_has_words("Nano Machine", "nano machin"), "typed part way");
        assert!(title_has_words("Nano Machine", "nanp machine"), "one wrong in a short word");
        assert!(title_has_words("The Awakening Contract", "awakenng contract"));
        assert!(!title_has_words("The Awakening Contract", "aweking contract"), "three letters off: that is for Did you mean");
        assert!(!title_has_words("Nano Machine", "nnao mechanic"));
        assert!(title_has_words("Tower of God", "tower of good"), "one letter off in a short word");
        assert!(!title_has_words("Tower of God", "tower of gaad"), "but not two");
        assert!(title_has_words("Tower of God", "tower of gods"));
        assert!(!title_has_words("Tower of God", "tower on god"), "two-letter words exactly");
        assert_eq!(super::edit_distance("aweking", "awakening"), 3);
        assert_eq!(super::edit_distance("", "abc"), 3);
        assert_eq!(super::edit_distance("kitten", "sitting"), 3);
    }
}
