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
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Pages {
    pub chapter_id: i64,
    pub count: u32,
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
    /// "switched", "new", "blocked", "lost" or "info".
    pub kind: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Status {
    pub server: String,
    pub suwayomi: Option<String>,
    pub sources: Vec<String>,
    pub series: u32,
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

/// Whether two sources' titles name the same work: equal once normalised, equal without their
/// subtitles when that main title is at least three words (two is too weak: "Solo Leveling" and
/// "Solo Leveling: Ragnarok" are different works), or nine tenths of their words shared once the
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
        return true;
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
    let Some(&last) = whole.last() else { return Vec::new() };
    let mut out = Vec::new();
    let mut run: Option<(i64, i64)> = None;
    for n in 1..=last {
        if whole.binary_search(&n).is_ok() {
            if let Some((a, b)) = run.take() {
                out.push(Gap { from: a as f64, to: b as f64 });
            }
        } else {
            run = Some(match run {
                Some((a, _)) => (a, n),
                None => (n, n),
            });
        }
    }
    if let Some((a, b)) = run {
        out.push(Gap { from: a as f64, to: b as f64 });
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
