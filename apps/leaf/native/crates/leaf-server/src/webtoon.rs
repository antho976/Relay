//! Paid episodes on Webtoons.com. Its source lists only the free episodes ("I Married the Dragon
//! I Killed": 8 of 23 on 2026-11-23); the series' own page says how many more the app sells
//! ahead ("Read 15 new episodes"). Read once a day per series, with a normal browser's
//! User-Agent and through the source's budget; a paid episode itself is never fetched.
//!
//! What a list page said is kept a day per page (`wt:{url}`), so a search, a preview and the daily
//! look never ask Webtoon twice for the same series. The range sent is cut at send time to what
//! the series has now ([`still_paid`]): a chapter another source brings drops out of it at once.

use std::time::Duration;

use leaf_proto::PaidAhead;

use crate::autopilot::{Ctx, PAUSED, WEBTOONS, log, plausible_latest};
use crate::budget::Lane;

const DAY: i64 = 86_400_000;

/// What a series' list page says.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Listing {
    /// The newest free episode ("#8" on the first entry of the list).
    pub free: f64,
    /// Episodes the app has ahead of it ("Read 15 new episodes").
    pub ahead: u32,
}

impl Listing {
    /// The newest episode there is, free or paid.
    pub fn newest(&self) -> f64 {
        self.free + self.ahead as f64
    }
}

fn digits_after(s: &str, at: usize) -> Option<u32> {
    let rest = s.get(at..)?.trim_start();
    let n: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    n.parse().ok()
}

/// Reads the newest free episode and the paid ones ahead from a list page (the desktop page, or
/// the mobile one's `previewProductEpisodeCount`). None when the page has no episode list.
pub fn parse(html: &str) -> Option<Listing> {
    // The first entry of the list is the newest (the page is sorted newest first).
    let first = html.find("_episodeItem")?;
    let item_end = html[first..].find("</li>").map_or(html.len(), |e| first + e);
    let item = &html[first..item_end];
    let shown = item.find("class=\"tx\">#").and_then(|i| digits_after(item, i + "class=\"tx\">#".len()));
    let numbered = item.find("data-episode-no=\"").and_then(|i| digits_after(item, i + "data-episode-no=\"".len()));
    let free = shown.or(numbered)? as f64;
    let ahead = ahead_of(html).unwrap_or(0);
    Some(Listing { free, ahead })
}

fn ahead_of(html: &str) -> Option<u32> {
    const KEY: &str = "\"previewProductEpisodeCount\":";
    if let Some(i) = html.find(KEY) {
        return digits_after(html, i + KEY.len());
    }
    // "Read 15 new episodes" (desktop), "Read 15 new episodes only on the app!" (mobile).
    let mut from = 0;
    while let Some(i) = html[from..].find("Read ") {
        let at = from + i + "Read ".len();
        if let Some(n) = digits_after(html, at) {
            let tail = html[at..].trim_start().trim_start_matches(|c: char| c.is_ascii_digit()).trim_start();
            if tail.starts_with("new episode") {
                return Some(n);
            }
        }
        from = at;
    }
    None
}

/// The list page's address, if `url` (Suwayomi's realUrl or url) is a Webtoons.com series.
pub fn list_url(url: &str) -> Option<String> {
    let full = if url.starts_with('/') { format!("https://www.webtoons.com{url}") } else { url.to_string() };
    let rest = full.strip_prefix("https://")?;
    let host = rest.split('/').next()?;
    (host == "www.webtoons.com" || host == "webtoons.com" || host == "m.webtoons.com").then_some(())?;
    full.contains("title_no=").then(|| full.replacen("://m.webtoons.com", "://www.webtoons.com", 1))
}

const BROWSER: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// The list page, as a browser would ask for it.
pub fn fetch(url: &str) -> Result<String, String> {
    let config = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).build();
    let agent = ureq::Agent::new_with_config(config);
    let mut resp = agent
        .get(url)
        .header("user-agent", BROWSER)
        .header("accept", "text/html,application/xhtml+xml")
        .header("accept-language", "en-US,en;q=0.9")
        .call()
        .map_err(|e| format!("webtoons: {e}"))?;
    resp.body_mut().with_config().limit(4 << 20).read_to_string().map_err(|e| format!("webtoons: {e}"))
}

/// The paid episodes ahead, if the newest on Webtoon is beyond the highest chapter any of your
/// sources has.
pub fn paid_ahead(l: &Listing, highest: Option<f64>, url: &str) -> Option<PaidAhead> {
    let newest = l.newest();
    let have = highest.unwrap_or(0.0).floor();
    (newest > have).then(|| PaidAhead { from: have + 1.0, to: newest, url: url.to_string() })
}

/// The paid range as it stands now: what the series has (`latest`) taken off its start, none
/// once it has them all.
pub fn still_paid(p: PaidAhead, latest: Option<f64>) -> Option<PaidAhead> {
    let have = latest.map_or(0.0, |l| l.floor() + 1.0);
    let from = p.from.max(have);
    (from <= p.to).then_some(PaidAhead { from, ..p })
}

/// At most this many series looked at for the first time in one pass, beside the day's share,
/// so a newly linked copy shows its paid episodes within the hour.
const FIRST_LOOKS: usize = 12;

#[derive(serde::Serialize, serde::Deserialize)]
struct Kept {
    at: i64,
    listing: Option<Listing>,
}

impl Ctx {
    /// What a Webtoon list page says, kept a day: asked now (through Webtoon's budget, in `lane`)
    /// only when not kept. None when the page has no episode list.
    pub fn listing(&self, url: &str, lane: Lane, now: i64) -> Result<Option<Listing>, String> {
        let key = format!("wt:{url}");
        let kept = self.store().get(&key).and_then(|v| serde_json::from_str::<Kept>(&v).ok()).filter(|k| now - k.at < DAY);
        if let Some(k) = kept {
            return Ok(k.listing);
        }
        let listing = parse(&self.ask(WEBTOONS, lane, || fetch(url))?);
        self.store().set(&key, &serde_json::to_string(&Kept { at: now, listing }).unwrap_or_default());
        Ok(listing)
    }

    /// The list page of a Webtoon entry, if it is one.
    pub fn webtoon_url(&self, manga_id: i64) -> Option<String> {
        self.suwa.manga_url(manga_id).ok().and_then(|u| list_url(&u))
    }

    /// Once a day per series with a Webtoons.com copy (its own or a linked one), spread over the
    /// day's passes: its list page read for the paid episodes ahead, kept as `paid:{work}`.
    pub fn paid_pass(&self, now: i64) {
        let works = self.store().works();
        let mut due: Vec<(i64, crate::store::Work, i64)> = Vec::new();
        let mut with_copy: usize = 0;
        for w in works {
            let links = self.store().link_sources(w.id);
            let copy = if w.source.eq_ignore_ascii_case(WEBTOONS) { Some(w.manga_id) } else { links.iter().find(|(_, s)| s.eq_ignore_ascii_case(WEBTOONS)).map(|(id, _)| *id) };
            let Some(copy) = copy else {
                self.store().del(&format!("paid:{}", w.id));
                continue;
            };
            with_copy += 1;
            let at = self.store().get(&format!("paid_at:{}", w.id)).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
            if now - at >= DAY {
                due.push((at, w, copy));
            }
        }
        due.sort_by_key(|(at, w, _)| (*at, w.id));
        let first = due.iter().filter(|(at, _, _)| *at == 0).count().min(FIRST_LOOKS);
        for (_, w, copy) in due.into_iter().take(with_copy.div_ceil(24).max(first)) {
            let Some(url) = self.webtoon_url(copy) else {
                self.store().set(&format!("paid_at:{}", w.id), &now.to_string());
                continue;
            };
            let listing = match self.listing(&url, Lane::Back, now) {
                Ok(l) => l,
                Err(e) if e.starts_with(PAUSED) => return,
                Err(e) => {
                    log(&format!("paid episodes of {}: {e}", w.title));
                    None
                }
            };
            self.store().set(&format!("paid_at:{}", w.id), &now.to_string());
            let Some(l) = listing else { continue };
            let Ok(m) = self.suwa.manga(w.manga_id) else { continue };
            let d = self.detail_now(&w, &m);
            let numbers: Vec<f64> = d.chapters.iter().map(|c| c.number).collect();
            match paid_ahead(&l, plausible_latest(&numbers), &url) {
                Some(p) => self.store().set(&format!("paid:{}", w.id), &serde_json::to_string(&p).unwrap_or_default()),
                None => self.store().del(&format!("paid:{}", w.id)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cut from the desktop list page of "I Married the Dragon I Killed" (2026-11-23).
    const DESKTOP: &str = r#"
<span class="episode_count">8 Episodes</span>
<ul id="_listUl" class="detail_list">
<li class="_episodeItem detail_list_item" id="episode_8" data-episode-no="8">
  <a href="https://www.webtoons.com/en/fantasy/i-married-the-dragon-i-killed/ep-8-in-search-of-talent/viewer?title_no=11353&episode_no=8" class="detail_list_link">
    <span class="subj"><span>Ep. 8 - In Search of Talent</span><em class="tx_up">UP</em></span>
    <span class="date">Nov 22, 2026</span>
    <span class="like_area _likeitArea"><em class="ico_like _btnLike _likeMark">like</em>2,840</span>
    <span class="tx">#8</span>
  </a>
</li>
<li class="_episodeItem detail_list_item" id="episode_7" data-episode-no="7">
    <span class="tx">#7</span>
</li>
</ul>
<div class="aside_install_app">
  <div class="txt_area">
    <strong class="tit">

        Read 15 new episodes

    </strong>
    <p class="desc">Scan the QR code to download the WEBTOON app.</p>
"#;

    #[test]
    fn the_desktop_page_tells_the_paid_episodes_ahead() {
        let l = parse(DESKTOP).unwrap();
        assert_eq!(l, Listing { free: 8.0, ahead: 15 });
        assert_eq!(l.newest(), 23.0);
    }

    #[test]
    fn the_mobile_page_and_a_series_with_none_paid() {
        let mobile = r#"<li class="_episodeItem" data-episode-no="41"><span class="tx">#40</span></li> dto: {"title":{"previewProductEpisodeCount":3,"x":1}}"#;
        assert_eq!(parse(mobile), Some(Listing { free: 40.0, ahead: 3 }), "the number shown, not the episode's id");
        let free = r#"<li class="_episodeItem" data-episode-no="120"></li><p>Read Episode 1</p>"#;
        assert_eq!(parse(free), Some(Listing { free: 120.0, ahead: 0 }), "'Read Episode 1' is not a count");
        assert_eq!(parse("<html>Access denied</html>"), None);
    }

    #[test]
    fn a_chapter_another_source_brings_leaves_the_paid_range() {
        let p = PaidAhead { from: 9.0, to: 23.0, url: "u".into() };
        assert_eq!(still_paid(p.clone(), Some(8.0)).map(|p| (p.from, p.to)), Some((9.0, 23.0)));
        assert_eq!(still_paid(p.clone(), Some(12.0)).map(|p| p.from), Some(13.0), "chapters 9 to 12 came");
        assert_eq!(still_paid(p.clone(), Some(22.5)).map(|p| p.from), Some(23.0));
        assert_eq!(still_paid(p.clone(), Some(23.0)), None, "every one is here now");
        assert_eq!(still_paid(p.clone(), None).map(|p| p.from), Some(9.0));
    }

    #[test]
    fn paid_is_what_none_of_your_sources_has() {
        let l = Listing { free: 8.0, ahead: 15 };
        assert_eq!(paid_ahead(&l, Some(8.0), "u"), Some(PaidAhead { from: 9.0, to: 23.0, url: "u".into() }));
        assert_eq!(paid_ahead(&l, Some(15.5), "u").map(|p| p.from), Some(16.0), "another source has up to 15");
        assert_eq!(paid_ahead(&l, Some(23.0), "u"), None, "another source has them all");
        assert_eq!(paid_ahead(&Listing { free: 8.0, ahead: 0 }, Some(8.0), "u"), None);
    }

    #[test]
    fn only_webtoons_pages_are_fetched() {
        assert_eq!(list_url("/en/fantasy/x/list?title_no=11353").as_deref(), Some("https://www.webtoons.com/en/fantasy/x/list?title_no=11353"));
        assert_eq!(list_url("https://m.webtoons.com/en/fantasy/x/list?title_no=1").as_deref(), Some("https://www.webtoons.com/en/fantasy/x/list?title_no=1"));
        assert_eq!(list_url("https://www.webtoons.com.evil.example/list?title_no=1"), None);
        assert_eq!(list_url("http://www.webtoons.com/en/x/list?title_no=1"), None);
        assert_eq!(list_url("https://www.webtoons.com/en/x/"), None, "not a series page");
    }
}
