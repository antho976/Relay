//! The app's rules, kept apart from drawing so they can be tested: the server's address, the words
//! a series or a chapter is shown with, the chapter list with its gaps, where Continue opens, when
//! the reader reports its place, which cached chapters go, and how a page is cut into tiles.

use std::collections::HashMap;

use leaf_proto::{Chapter, Gap, Progress, Series, SeriesDetail};

// The server

/// A server address as typed ("192.168.1.20:4580", "anthobot.bid/leaf") made into the base every
/// request starts with: a scheme, no trailing slash, and no `/v1` (the app adds it). A path stays:
/// the server may sit behind a prefix. Without a scheme, an address on this network (a private,
/// loopback or link-local IP, `localhost`, a name without dots, a `.local` or `.lan` name) is http
/// and any other https; http to any other is refused, since the token would cross the internet in
/// the clear. Errors are words for the person.
pub fn normalize_server(input: &str) -> Result<String, &'static str> {
    let t = input.trim();
    if t.is_empty() {
        return Err("Type your server's address");
    }
    let with_scheme = if t.contains("://") {
        t.to_string()
    } else if is_local_host(t.split('/').next().unwrap_or("")) {
        format!("http://{t}")
    } else {
        format!("https://{t}")
    };
    let (scheme, rest) = with_scheme.split_once("://").ok_or("That doesn't read as an address")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err("Use an http:// or https:// address");
    }
    let mut rest = rest.trim_end_matches('/').to_string();
    if let Some(r) = rest.strip_suffix("/v1") {
        rest = r.trim_end_matches('/').to_string();
    }
    let host = rest.split('/').next().unwrap_or("");
    if host.is_empty() || rest.chars().any(char::is_whitespace) || host.starts_with(':') {
        return Err("That doesn't read as an address");
    }
    if scheme == "http" && !is_local_host(host) {
        return Err("Use https:// for a server outside your network");
    }
    if let Some((_, port)) = host.rsplit_once(':')
        && !host.ends_with(']')
        && (port.is_empty() || port.parse::<u16>().is_err())
    {
        return Err("The port after the colon should be a number");
    }
    Ok(format!("{scheme}://{rest}"))
}

/// A host (with its port, if any) that is reached on the local network, not by a public name: a
/// public IP is not (the token would go to it in the clear).
fn is_local_host(host_port: &str) -> bool {
    if let Some(v6) = host_port.strip_prefix('[') {
        let ip = v6.split(']').next().unwrap_or("");
        return ip.parse::<std::net::Ipv6Addr>().is_ok_and(|a| {
            let first = a.segments()[0];
            a.is_loopback() || first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80
        });
    }
    let host = host_port.rsplit_once(':').map_or(host_port, |(h, _)| h).to_ascii_lowercase();
    if let Ok(a) = host.parse::<std::net::Ipv4Addr>() {
        // 100.64.0.0/10 is Tailscale's (and carrier NAT's): a tailnet's traffic is its own tunnel.
        let [x, y, ..] = a.octets();
        return a.is_private() || a.is_loopback() || a.is_link_local() || (x == 100 && (64..128).contains(&y));
    }
    host == "localhost" || !host.contains('.') || host.ends_with(".local") || host.ends_with(".lan")
}

/// The address as people say it: no scheme ("192.168.1.20:4580"), unless the scheme is not the
/// one the address would get typed bare ("http://server1.example:4580" stays as it is), so that
/// connecting again with the label keeps the same address.
pub fn server_label(base: &str) -> String {
    let base = base.trim_end_matches('/');
    let bare = base.split_once("://").map_or(base, |(_, r)| r);
    if normalize_server(bare).is_ok_and(|b| b == base) { bare.to_string() } else { base.to_string() }
}

// Words

/// A chapter number as written: "12", "12.5".
pub fn num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e9 { format!("{}", n as i64) } else { format!("{n}") }
}

/// The line under a cover in the library.
pub fn library_line(s: &Series) -> String {
    library_lines(s).swap_remove(0)
}

/// The line under a cover, longest first: the first that fits the cover's width is shown.
pub fn library_lines(s: &Series) -> Vec<String> {
    match s.read_upto {
        None if s.chapters == 0 => vec!["No chapters yet".into()],
        None => vec![format!("{} chapters", s.chapters)],
        Some(_) if s.unread == 0 => vec!["Caught up".into()],
        Some(r) => {
            let of = s.latest.map(|l| format!(" of {}", num(l))).unwrap_or_default();
            vec![format!("Ch. {}{of} · {} unread", num(r), s.unread), format!("Ch. {} · {} unread", num(r), s.unread), format!("{} unread", s.unread)]
        }
    }
}

/// How much of a series is read, 0 to 1, for the line on its cover's edge.
pub fn read_fraction(s: &Series) -> f32 {
    if s.chapters == 0 || s.read_upto.is_none() {
        return 0.0;
    }
    (s.chapters.saturating_sub(s.unread) as f32 / s.chapters as f32).clamp(0.0, 1.0)
}

/// "Ongoing" from "ongoing"; nothing for "unknown".
pub fn status_word(status: &str) -> Option<String> {
    match status {
        "" | "unknown" => None,
        s => {
            let mut c = s.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect())
        }
    }
}

/// A chapter's own title, without the number its row already shows: "Overgeared" from
/// "Chapter 1 - Overgeared"; none from "Ch.1" or "Chapter 1".
pub fn chapter_title(name: &str, number: f64) -> Option<String> {
    let n = num(number);
    let mut t = name.trim();
    for lead in ["Chapter", "Chap.", "Ch.", "Ch", "Episode", "Ep."] {
        if let Some(rest) = t.strip_prefix(lead) {
            let rest = rest.trim_start();
            if let Some(after) = rest.strip_prefix(n.as_str())
                && !after.starts_with(|c: char| c.is_ascii_digit())
            {
                t = after;
                break;
            }
        }
    }
    let t = t.trim_start_matches([' ', '-', ':', '–', '—', '.']).trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// `s` with what the faces can't draw taken out: a bracketed part made only of such characters
/// goes whole ("You Ma Wenhua (有马文化)" → "You Ma Wenhua"). Empty when nothing is left.
pub fn showable(s: &str, can: impl Fn(char) -> bool) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(open) = rest.find(['(', '[', '（']) {
        out.push_str(&rest[..open]);
        let after = &rest[open..];
        // By the brackets' own lengths: a full-width one is three bytes, not one.
        let open_len = after.chars().next().map_or(1, char::len_utf8);
        let close = after[open_len..].find([')', ']', '）']).map(|i| {
            let at = open_len + i;
            (at, at + after[at..].chars().next().map_or(1, char::len_utf8))
        });
        match close {
            Some((inner_end, end)) => {
                let inner = &after[open_len..inner_end];
                if inner.chars().any(|c| !can(c) && !c.is_whitespace()) && !inner.chars().any(|c| can(c) && c.is_alphanumeric()) {
                    // Dropped whole.
                } else {
                    out.push_str(&after[..end]);
                }
                rest = &after[end..];
            }
            None => {
                out.push_str(after);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    let kept: String = out.chars().filter(|c| can(*c)).collect();
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A series' description as worth reading: without a source's leading ranking ("Rank: #4 •
/// Rating: 9.25 • Bookmarks: 156.5K") and its trailing list of other titles.
pub fn clean_description(d: &str) -> String {
    let mut t = d.trim();
    if t.starts_with("Rank:")
        && let Some(i) = t.find("Bookmarks:")
    {
        let rest = t[i + "Bookmarks:".len()..].trim_start();
        let n = rest.find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | ',' | 'K' | 'k' | 'M'))).unwrap_or(rest.len());
        t = rest[n..].trim_start();
    }
    if let Some(i) = t.find("Alternative Title") {
        t = t[..i].trim_end();
    }
    t.to_string()
}

/// A day as "Mar 4, 2024", from milliseconds since the epoch (UTC).
pub fn date(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    let (y, m, d) = civil(days);
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    format!("{} {d}, {y}", MONTHS[(m - 1) as usize])
}

/// The local day ("Nov 22, 2026") a moment falls on, in this phone's time zone.
pub fn local_date(ms: i64) -> String {
    date(crate::shelf::day_of(ms) * 86_400_000)
}

/// Year, month and day of a day counted from 1970-01-01 (Howard Hinnant's civil_from_days).
pub fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    (y, m, d)
}

/// How long ago, said shortly: "just now", "12 min ago", "2 h ago", "yesterday", "3 days ago",
/// then the day ("Mar 4, 2024").
pub fn ago(ms: i64, now_ms: i64) -> String {
    let d = (now_ms - ms).max(0);
    let min = d / 60_000;
    if min < 1 {
        "just now".into()
    } else if min < 60 {
        format!("{min} min ago")
    } else if min < 24 * 60 {
        format!("{} h ago", min / 60)
    } else if min < 48 * 60 {
        "yesterday".into()
    } else if min < 7 * 24 * 60 {
        format!("{} days ago", min / (24 * 60))
    } else {
        local_date(ms)
    }
}

/// "1 chapter", "339 chapters".
pub fn chapters_words(n: usize) -> String {
    if n == 1 { "1 chapter".into() } else { format!("{n} chapters") }
}

// What is new, and what is next

/// Chapters that came since the series was last read and are not read yet. With the series'
/// chapters at hand, counted (those published after the last read); without, 1 when the newest
/// arrived after the last read and something is unread (the library says no more). Nothing for
/// a series never read: all of it is new, which is not news.
pub fn new_chapters(s: &Series, d: Option<&SeriesDetail>) -> u32 {
    if s.read_upto.is_none() || s.unread == 0 {
        return 0;
    }
    let since = s.last_read_at.max(s.added_at);
    match d {
        Some(d) if since > 0 => d.chapters.iter().filter(|c| !c.read && c.uploaded_at > since).count() as u32,
        _ if s.updated_at > since && since > 0 => 1,
        _ => 0,
    }
}

/// A time the server sent as ms since the epoch, mended when it came in seconds (the server's
/// `last_read_at` did, 2026-11-22: Suwayomi keeps it in seconds). Anything before 1973 in ms is
/// taken for seconds.
pub fn as_ms(t: i64) -> i64 {
    if t > 0 && t < 100_000_000_000 { t * 1000 } else { t }
}

/// The series' times, mended ([`as_ms`]).
pub fn mend_times(s: &mut Series) {
    s.last_read_at = as_ms(s.last_read_at);
    s.added_at = as_ms(s.added_at);
    s.updated_at = as_ms(s.updated_at);
}

/// The chapter you are on: where Continue opens when the chapters are at hand, else the one after
/// the last read. None when nothing was opened yet.
pub fn at_chapter(s: &Series, d: Option<&SeriesDetail>) -> Option<f64> {
    if let Some(d) = d
        && d.chapters.iter().any(|c| c.read || c.last_page > 0)
    {
        return continue_point(&d.chapters).map(|c| d.chapters[c.index].number);
    }
    next_chapter(s)
}

/// The chapter to read next as the library knows it: the one after the last read to its end
/// (never past the newest). None when nothing is read yet.
pub fn next_chapter(s: &Series) -> Option<f64> {
    let r = s.read_upto?;
    let n = r.floor() + 1.0;
    Some(match s.latest {
        Some(l) if n > l => l,
        _ => n,
    })
}

/// What the "Recently updated" list says a series got: its newest chapter, or the run of new
/// ones ("Chapters 338–339").
pub fn updated_words(s: &Series, new: u32) -> String {
    match s.latest {
        Some(l) if new >= 2 => format!("Chapters {}–{}", num(l - new as f64 + 1.0), num(l)),
        Some(l) => format!("Chapter {}", num(l)),
        None => "No chapters yet".into(),
    }
}

/// Something the home page's spotlight may show.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    /// The series' bell is on and chapters came since it was read.
    pub rings: bool,
    /// When it last changed (a new chapter), ms.
    pub updated_at: i64,
    /// When it was last read (or opened), ms; 0 if never.
    pub read_at: i64,
}

/// The spotlight's order, as indexes into `spots`: belled series with new chapters first (newest
/// first), then what was read most recently; at most `max`. Never-read things lead nothing.
pub fn spotlight(spots: &[Spot], max: usize) -> Vec<usize> {
    let mut ringing: Vec<usize> = (0..spots.len()).filter(|&i| spots[i].rings).collect();
    ringing.sort_by_key(|&i| std::cmp::Reverse(spots[i].updated_at));
    let mut read: Vec<usize> = (0..spots.len()).filter(|&i| !spots[i].rings && spots[i].read_at > 0).collect();
    read.sort_by_key(|&i| std::cmp::Reverse(spots[i].read_at));
    ringing.into_iter().chain(read).take(max).collect()
}

/// My space's Following, sorted: 0 recently read, 1 recently updated, 2 recently followed (when
/// added). Never-read series go last in the first, by when they were added.
pub fn sort_following(list: &mut [Series], by: u8) {
    match by {
        1 => list.sort_by_key(|s| std::cmp::Reverse(s.updated_at)),
        2 => list.sort_by_key(|s| std::cmp::Reverse(s.added_at)),
        _ => list.sort_by_key(|s| (std::cmp::Reverse(s.last_read_at), std::cmp::Reverse(s.added_at))),
    }
}

pub const FOLLOWING_SORTS: [&str; 3] = ["Recently read", "Recently updated", "Recently followed"];

// The chapter list

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Row {
    /// An index into the series' chapters.
    Chapter(usize),
    Gap(Gap),
}

/// The series page's list, newest first, with a row in place of each run of missing chapters.
pub fn chapter_rows(chapters: &[Chapter], gaps: &[Gap]) -> Vec<Row> {
    let mut keyed: Vec<(f64, u8, Row)> = chapters.iter().enumerate().map(|(i, c)| (c.number, 1, Row::Chapter(i))).collect();
    // A gap sits just above its last missing number: under the chapter after it, over the one before.
    keyed.extend(gaps.iter().map(|g| (g.to + 0.5, 0, Row::Gap(*g))));
    keyed.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    keyed.into_iter().map(|(_, _, r)| r).collect()
}

/// The next `n` chapters from where reading stopped (the Continue point on) that the server has
/// not downloaded: what "Download next 5" asks for. Chapter ids.
pub fn next_to_download(chapters: &[Chapter], n: usize) -> Vec<i64> {
    let from = continue_point(chapters).map_or(0, |c| c.index);
    chapters[from..].iter().filter(|c| !c.read && !c.downloaded).take(n).map(|c| c.id).collect()
}

/// What a gap row says.
pub fn gap_words(g: &Gap) -> String {
    if g.from == g.to {
        format!("Chapter {} isn't available in English", num(g.from))
    } else {
        format!("Chapters {}–{} aren't available in English", num(g.from), num(g.to))
    }
}

/// Where the series page's primary key opens.
#[derive(Clone, Debug, PartialEq)]
pub struct Continue {
    /// Index into the chapters (oldest first).
    pub index: usize,
    /// The page to open at (0-based).
    pub page: u32,
    pub label: String,
}

/// The page a chapter left part way opens at: its last page, kept inside the chapter when its
/// length is known (an unknown length, 0, is no reason to start it over).
pub fn resume_page(c: &Chapter) -> u32 {
    if c.pages == 0 { c.last_page } else { c.last_page.min(c.pages - 1) }
}

/// Continue where the person stopped: a chapter left part way (after the last one finished),
/// else the one after the last finished, else the first. When everything is read, the newest.
pub fn continue_point(chapters: &[Chapter]) -> Option<Continue> {
    if chapters.is_empty() {
        return None;
    }
    let last_read = chapters.iter().rposition(|c| c.read);
    let part_way = chapters.iter().rposition(|c| !c.read && c.last_page > 0);
    let at = |i: usize, page: u32, verb: &str| Continue { index: i, page, label: format!("{verb} ch. {}", num(chapters[i].number)) };
    match (part_way, last_read) {
        (Some(p), r) if r.is_none_or(|r| p > r) => Some(at(p, resume_page(&chapters[p]), "Continue")),
        (_, Some(r)) if r + 1 < chapters.len() => Some(at(r + 1, 0, "Continue")),
        (_, Some(r)) => Some(Continue { index: r, page: 0, label: format!("Read ch. {} again", num(chapters[r].number)) }),
        (_, None) => Some(Continue { index: 0, page: 0, label: "Start reading".into() }),
    }
}

// Reporting the place

/// How often the reader may tell the server its page, at most.
pub const PROGRESS_EVERY_NS: u64 = 3_000_000_000;

/// Decides when the reader posts `POST /v1/chapters/{id}/progress`: a finished chapter at once,
/// a page change at most every few seconds, the last word when the reader closes. A chapter once
/// finished is never reported unfinished again.
#[derive(Default, Debug)]
pub struct ProgressGate {
    sent: HashMap<i64, Progress>,
    pending: Option<(i64, Progress)>,
    last_ns: Option<u64>,
}

impl ProgressGate {
    /// What the server already holds for a chapter (as the series page read it): no report needed
    /// to say it again.
    pub fn known(&mut self, chapter: i64, page: u32, done: bool) {
        self.sent.insert(chapter, Progress { page, done });
    }

    /// The reader is on `page` of `chapter` (`done` once its last page has scrolled past). Returns
    /// the reports to send now.
    pub fn see(&mut self, chapter: i64, page: u32, done: bool, now_ns: u64) -> Vec<(i64, Progress)> {
        let was = self.sent.get(&chapter).copied();
        let done_before = was.is_some_and(|p| p.done);
        let want = Progress { page, done: done || done_before };
        let mut out = Vec::new();
        if was == Some(want) || (done_before && was.is_some_and(|p| p.page >= page)) {
            // Nothing new; a pending report for this chapter is moot.
            if self.pending.is_some_and(|(c, _)| c == chapter) {
                self.pending = None;
            }
            return out;
        }
        // Moving to another chapter: the last word on the one before goes first.
        if let Some((c, p)) = self.pending
            && c != chapter
        {
            out.push(self.mark(c, p, now_ns));
            self.pending = None;
        }
        let urgent = want.done && !done_before;
        let due = self.last_ns.is_none_or(|t| now_ns.saturating_sub(t) >= PROGRESS_EVERY_NS);
        if urgent || due {
            self.pending = None;
            out.push(self.mark(chapter, want, now_ns));
        } else {
            self.pending = Some((chapter, want));
        }
        out
    }

    /// A report held back by the debounce, now due (call each frame).
    pub fn tick(&mut self, now_ns: u64) -> Option<(i64, Progress)> {
        let (c, p) = self.pending?;
        if self.last_ns.is_none_or(|t| now_ns.saturating_sub(t) >= PROGRESS_EVERY_NS) {
            self.pending = None;
            return Some(self.mark(c, p, now_ns));
        }
        None
    }

    /// The reader closes: whatever is held back goes now.
    pub fn flush(&mut self, now_ns: u64) -> Option<(i64, Progress)> {
        let (c, p) = self.pending.take()?;
        Some(self.mark(c, p, now_ns))
    }

    /// When the held-back report is due, if one is.
    pub fn due_at(&self) -> Option<u64> {
        self.pending.map(|_| self.last_ns.unwrap_or(0) + PROGRESS_EVERY_NS)
    }

    fn mark(&mut self, c: i64, p: Progress, now_ns: u64) -> (i64, Progress) {
        self.sent.insert(c, p);
        self.last_ns = Some(now_ns);
        (c, p)
    }
}

// The page cache on disk

/// The most the page cache keeps, in bytes.
pub const CACHE_CAP: u64 = 1_500_000_000;
/// A chapter read longer ago than this leaves the cache.
pub const CACHE_KEEP_MS: i64 = 7 * 86_400_000;

#[derive(Clone, Debug, PartialEq)]
pub struct CachedChapter {
    pub chapter: i64,
    pub bytes: u64,
    /// When it was last opened (or fetched ahead), ms since the epoch.
    pub used_ms: i64,
}

/// The cached chapters to remove: those last used more than a week ago, then the oldest until
/// the rest fit under `cap`. Chapters in `keep` (open now) stay whatever their age.
pub fn prune_choice(cached: &[CachedChapter], now_ms: i64, cap: u64, keep: &[i64]) -> Vec<i64> {
    let mut by_age: Vec<&CachedChapter> = cached.iter().filter(|c| !keep.contains(&c.chapter)).collect();
    by_age.sort_by_key(|c| (c.used_ms, c.chapter));
    let mut total: u64 = cached.iter().map(|c| c.bytes).sum();
    let mut out = Vec::new();
    for c in by_age {
        if now_ms - c.used_ms > CACHE_KEEP_MS || total > cap {
            out.push(c.chapter);
            total = total.saturating_sub(c.bytes);
        }
    }
    out
}

// Pages into tiles

/// Pages are kept at most this wide: they are drawn at the screen's width, and the picture atlas
/// holds three across.
pub const PAGE_W: u32 = 800;
/// Tiles are at most this tall: three rows of them fill the 3072 atlas.
pub const TILE_H: u32 = 1020;

// Three rows and three columns of tiles, with their gaps, fit the picture atlas.
const _: () = assert!(3 * (TILE_H + 1) < crate::gfx::IMAGES_SIZE && 3 * (PAGE_W + 1) < crate::gfx::IMAGES_SIZE);

/// A page's size once scaled down to at most [`PAGE_W`] wide (never up).
pub fn scaled_size(w: u32, h: u32) -> (u32, u32) {
    if w <= PAGE_W || w == 0 {
        return (w.max(1), h.max(1));
    }
    let nh = ((h as u64 * PAGE_W as u64 + w as u64 / 2) / w as u64).max(1) as u32;
    (PAGE_W, nh)
}

/// The rows a page of height `h` is cut into: (top, height), each at most [`TILE_H`].
pub fn tile_rows(h: u32) -> Vec<(u32, u32)> {
    (0..h.div_ceil(TILE_H)).map(|i| (i * TILE_H, TILE_H.min(h - i * TILE_H))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(id: i64, number: f64, read: bool, last_page: u32) -> Chapter {
        Chapter { id, number, name: format!("Ch.{}", num(number)), group: None, pages: 30, read, last_page, downloaded: true, uploaded_at: 0, from: None }
    }

    #[test]
    fn server_addresses_are_normalised() {
        assert_eq!(normalize_server("192.168.1.20:4580").unwrap(), "http://192.168.1.20:4580");
        assert_eq!(normalize_server(" https://leaf.example.org/ ").unwrap(), "https://leaf.example.org");
        assert_eq!(normalize_server("anthobot.bid/leaf").unwrap(), "https://anthobot.bid/leaf");
        assert_eq!(normalize_server("https://anthobot.bid/leaf/").unwrap(), "https://anthobot.bid/leaf");
        assert_eq!(normalize_server("anthobot.bid/leaf/v1").unwrap(), "https://anthobot.bid/leaf");
        assert_eq!(normalize_server("localhost:4580").unwrap(), "http://localhost:4580");
        assert_eq!(normalize_server("nas.local:4580").unwrap(), "http://nas.local:4580");
        assert_eq!(normalize_server("http://127.0.0.1:4580/v1/").unwrap(), "http://127.0.0.1:4580");
        assert_eq!(normalize_server("HTTPS://h.example/leaf").unwrap(), "https://h.example/leaf");
        assert_eq!(normalize_server("http://[::1]:4580").unwrap(), "http://[::1]:4580");
        assert!(normalize_server("").is_err());
        assert!(normalize_server("ftp://x").is_err());
        assert!(normalize_server("host:port").is_err());
        assert!(normalize_server("a b").is_err());
        assert_eq!(server_label("http://192.168.1.5:4580"), "192.168.1.5:4580");
        assert_eq!(server_label("https://anthobot.bid/leaf"), "anthobot.bid/leaf");
        assert_eq!(normalize_server("server1:4580").unwrap(), "http://server1:4580", "a name without dots is on this network");
        assert_eq!(normalize_server("100.101.102.103:4580").unwrap(), "http://100.101.102.103:4580", "a tailnet address");
        assert_eq!(normalize_server("203.0.113.7:4580").unwrap(), "https://203.0.113.7:4580", "a public IP gets https");
        assert!(normalize_server("http://203.0.113.7:4580").is_err(), "and no token goes to it in the clear");
        assert!(normalize_server("http://leaf.example.org").is_err());
        assert_eq!(normalize_server("[fd00::5]:4580").unwrap(), "http://[fd00::5]:4580");
        assert_eq!(normalize_server("[2001:db8::1]:4580").unwrap(), "https://[2001:db8::1]:4580");
        for base in ["http://server1:4580", "https://server1:4580", "http://192.168.1.5:4580", "https://anthobot.bid/leaf", "https://192.168.1.5"] {
            assert_eq!(normalize_server(&server_label(base)).unwrap(), base, "connecting again with {} keeps it", server_label(base));
        }
    }

    #[test]
    fn words_for_a_series() {
        let mut s = Series {
            id: 1,
            title: "T".into(),
            author: None,
            source: "MangaDex".into(),
            status: "ongoing".into(),
            chapters: 350,
            read_upto: None,
            unread: 350,
            latest: Some(349.0),
            gaps: vec![],
            updated_at: 0,
            note: None,
            bell: false,
            added_at: 0,
            last_read_at: 0,
        };
        assert_eq!(library_line(&s), "350 chapters");
        assert_eq!(read_fraction(&s), 0.0);
        s.read_upto = Some(12.0);
        s.unread = 337;
        assert_eq!(library_line(&s), "Ch. 12 of 349 · 337 unread");
        assert_eq!(library_lines(&s)[1..], ["Ch. 12 · 337 unread".to_string(), "337 unread".to_string()]);
        assert!((read_fraction(&s) - 13.0 / 350.0).abs() < 1e-6);
        s.unread = 0;
        assert_eq!(library_line(&s), "Caught up");
        assert_eq!(num(12.5), "12.5");
        assert_eq!(status_word("completed").as_deref(), Some("Completed"));
        assert_eq!(status_word("unknown"), None);
    }

    #[test]
    fn chapter_titles_drop_their_number() {
        assert_eq!(chapter_title("Chapter 1 - Overgeared", 1.0).as_deref(), Some("Overgeared"));
        assert_eq!(chapter_title("Ch.1", 1.0), None);
        assert_eq!(chapter_title("Chapter 0 - Prologue", 0.0).as_deref(), Some("Prologue"));
        assert_eq!(chapter_title("Vol.2 Ch.12", 12.0).as_deref(), Some("Vol.2 Ch.12"));
        assert_eq!(chapter_title("Chapter 12.5: Side story", 12.5).as_deref(), Some("Side story"));
    }

    #[test]
    fn what_the_faces_cannot_draw_is_left_out() {
        let latin = |c: char| c.is_ascii() || c.is_whitespace();
        assert_eq!(showable("You Ma Wenhua (有马文化)", latin), "You Ma Wenhua");
        assert_eq!(showable("LEE Dong Wook/ PARK Saenal", latin), "LEE Dong Wook/ PARK Saenal");
        assert_eq!(showable("Solo (Redice) 我", latin), "Solo (Redice)");
        assert_eq!(showable("有马文化", latin), "");
        assert_eq!(showable("Mo Xiang Tong Xiu（墨香铜臭）", latin), "Mo Xiang Tong Xiu", "full-width brackets");
        assert_eq!(showable("Mo Xiang (墨香铜臭）", latin), "Mo Xiang");
        assert_eq!(showable("（Mo）", latin), "Mo");
    }

    fn series(read_upto: Option<f64>, unread: u32, updated_at: i64, last_read_at: i64) -> Series {
        Series {
            id: 1,
            title: "T".into(),
            author: None,
            source: "S".into(),
            status: "ongoing".into(),
            chapters: 10,
            read_upto,
            unread,
            latest: Some(10.0),
            gaps: vec![],
            updated_at,
            note: None,
            bell: false,
            added_at: 1,
            last_read_at,
        }
    }

    #[test]
    fn new_means_since_you_last_read() {
        let day = 86_400_000;
        // Never read: nothing is news.
        assert_eq!(new_chapters(&series(None, 10, 5 * day, 0), None), 0);
        // Read on day 3, a chapter came on day 5: from the library alone, one.
        let s = series(Some(8.0), 2, 5 * day, 3 * day);
        assert_eq!(new_chapters(&s, None), 1);
        // With the chapters: those published after day 3 and unread.
        let mut chapters: Vec<Chapter> = (1..=10).map(|n| ch(n, n as f64, n <= 8, 0)).collect();
        chapters[8].uploaded_at = 4 * day;
        chapters[9].uploaded_at = 5 * day;
        let d = SeriesDetail { series: s.clone(), description: None, chapters };
        assert_eq!(new_chapters(&s, Some(&d)), 2);
        // Caught up, or nothing came since: none.
        assert_eq!(new_chapters(&series(Some(10.0), 0, 5 * day, 3 * day), None), 0);
        assert_eq!(new_chapters(&series(Some(8.0), 2, 2 * day, 3 * day), None), 0);
        assert_eq!(next_chapter(&s), Some(9.0));
        assert_eq!(next_chapter(&series(Some(10.0), 0, 0, 0)), Some(10.0));
        assert_eq!(next_chapter(&series(None, 10, 0, 0)), None);
        assert_eq!(updated_words(&s, 2), "Chapters 9–10");
        assert_eq!(updated_words(&s, 0), "Chapter 10");
        assert_eq!(as_ms(1_795_398_420), 1_795_398_420_000);
        assert_eq!(as_ms(1_795_398_420_000), 1_795_398_420_000);
        assert_eq!(as_ms(0), 0);
        // Opened part way, nothing finished: the library can't say, the chapters can.
        let mut fresh = series(None, 10, 0, 5);
        let mut cs: Vec<Chapter> = (1..=10).map(|n| ch(n, n as f64, false, 0)).collect();
        assert_eq!(at_chapter(&fresh, None), None);
        cs[0].last_page = 2;
        fresh.last_read_at = 5;
        let d = SeriesDetail { series: fresh.clone(), description: None, chapters: cs };
        assert_eq!(at_chapter(&fresh, Some(&d)), Some(1.0));
    }

    #[test]
    fn the_spotlight_leads_with_belled_news_then_reading() {
        let sp = |rings, updated_at, read_at| Spot { rings, updated_at, read_at };
        let spots = [sp(false, 9, 50), sp(true, 30, 10), sp(false, 0, 0), sp(true, 40, 5), sp(false, 1, 70)];
        assert_eq!(spotlight(&spots, 5), vec![3, 1, 4, 0]);
        assert_eq!(spotlight(&spots, 2), vec![3, 1]);
        assert!(spotlight(&[], 5).is_empty());
    }

    #[test]
    fn following_sorts_three_ways() {
        let mut a = series(None, 0, 10, 0);
        a.id = 1;
        a.added_at = 3;
        let mut b = series(None, 0, 30, 20);
        b.id = 2;
        b.added_at = 1;
        let mut c = series(None, 0, 20, 5);
        c.id = 3;
        c.added_at = 2;
        let mut l = vec![a, b, c];
        let ids = |l: &[Series]| l.iter().map(|s| s.id).collect::<Vec<_>>();
        sort_following(&mut l, 0);
        assert_eq!(ids(&l), vec![2, 3, 1]);
        sort_following(&mut l, 1);
        assert_eq!(ids(&l), vec![2, 3, 1]);
        sort_following(&mut l, 2);
        assert_eq!(ids(&l), vec![1, 3, 2]);
    }

    #[test]
    fn times_read_shortly() {
        let now = 100 * 86_400_000i64;
        assert_eq!(ago(now - 20_000, now), "just now");
        assert_eq!(ago(now - 12 * 60_000, now), "12 min ago");
        assert_eq!(ago(now - 2 * 3_600_000, now), "2 h ago");
        assert_eq!(ago(now - 30 * 3_600_000, now), "yesterday");
        assert_eq!(ago(now - 3 * 86_400_000, now), "3 days ago");
        assert!(ago(0, now).ends_with("1970") || ago(0, now).ends_with("1969"));
        assert_eq!(chapters_words(1), "1 chapter");
    }

    #[test]
    fn download_next_starts_where_reading_stopped() {
        let mut chapters: Vec<Chapter> = (1..=10).map(|n| ch(n, n as f64, n <= 3, 0)).collect();
        for c in &mut chapters {
            c.downloaded = false;
        }
        chapters[4].downloaded = true;
        assert_eq!(next_to_download(&chapters, 5), vec![4, 6, 7, 8, 9]);
        assert_eq!(next_to_download(&chapters[..5], 5), vec![4]);
    }

    #[test]
    fn descriptions_lose_the_sources_furniture() {
        assert_eq!(clean_description("Rank: #4 • Rating: 9.25 • Bookmarks: 156.5KNanotechnology meets martial arts.Alternative Titles:- NM"), "Nanotechnology meets martial arts.");
        assert_eq!(clean_description("  A plain one. "), "A plain one.");
    }

    #[test]
    fn dates_read_as_days() {
        assert_eq!(date(0), "Jan 1, 1970");
        assert_eq!(date(1_708_875_403_000), "Feb 25, 2024");
        assert_eq!(date(951_782_400_000), "Feb 29, 2000");
    }

    #[test]
    fn the_list_runs_newest_first_with_its_gaps() {
        let mut chapters: Vec<Chapter> = (1..=22).map(|n| ch(n, n as f64, false, 0)).collect();
        chapters.extend((45..=47).map(|n| ch(n, n as f64, false, 0)));
        let gap = Gap { from: 23.0, to: 44.0 };
        let rows = chapter_rows(&chapters, &[gap]);
        assert_eq!(rows.len(), 26);
        let at = |i: usize| match rows[i] {
            Row::Chapter(c) => chapters[c].number,
            Row::Gap(g) => -g.from,
        };
        assert_eq!((at(0), at(2), at(3), at(4), at(25)), (47.0, 45.0, -23.0, 22.0, 1.0));
        assert_eq!(gap_words(&gap), "Chapters 23–44 aren't available in English");
        assert_eq!(gap_words(&Gap { from: 3.0, to: 3.0 }), "Chapter 3 isn't available in English");
    }

    #[test]
    fn continue_opens_where_reading_stopped() {
        assert_eq!(continue_point(&[]), None);
        let fresh = vec![ch(1, 0.0, false, 0), ch(2, 1.0, false, 0)];
        assert_eq!(continue_point(&fresh).unwrap(), Continue { index: 0, page: 0, label: "Start reading".into() });
        let after = vec![ch(1, 1.0, true, 29), ch(2, 2.0, false, 0), ch(3, 3.0, false, 0)];
        assert_eq!(continue_point(&after).unwrap(), Continue { index: 1, page: 0, label: "Continue ch. 2".into() });
        let part = vec![ch(1, 1.0, true, 29), ch(2, 2.0, false, 7), ch(3, 3.0, false, 0)];
        assert_eq!(continue_point(&part).unwrap(), Continue { index: 1, page: 7, label: "Continue ch. 2".into() });
        // A page left part way in an old chapter, since read past, does not pull the reader back.
        let old = vec![ch(1, 1.0, false, 4), ch(2, 2.0, true, 29), ch(3, 3.0, false, 0)];
        assert_eq!(continue_point(&old).unwrap().index, 2);
        let all = vec![ch(1, 1.0, true, 29), ch(2, 2.0, true, 29)];
        assert_eq!(continue_point(&all).unwrap(), Continue { index: 1, page: 0, label: "Read ch. 2 again".into() });
        let mut unknown = vec![ch(1, 1.0, true, 0), ch(2, 2.0, false, 15)];
        unknown[1].pages = 0;
        assert_eq!(continue_point(&unknown).unwrap().page, 15, "a length not known yet does not start the chapter over");
    }

    #[test]
    fn progress_is_debounced_and_never_unfinished() {
        let s = 1_000_000_000u64;
        let mut g = ProgressGate::default();
        g.known(10, 0, false);
        assert!(g.see(10, 0, false, 0).is_empty(), "what the server holds is not said again");
        assert_eq!(g.see(10, 1, false, s), vec![(10, Progress { page: 1, done: false })]);
        // Two more pages within the debounce: held, then sent as the last one.
        assert!(g.see(10, 2, false, 2 * s).is_empty());
        assert!(g.see(10, 3, false, 3 * s).is_empty());
        assert_eq!(g.due_at(), Some(4 * s));
        assert_eq!(g.tick(3 * s), None);
        assert_eq!(g.tick(4 * s), Some((10, Progress { page: 3, done: false })));
        // Finishing goes at once.
        assert_eq!(g.see(10, 29, true, 4 * s + 1), vec![(10, Progress { page: 29, done: true })]);
        // Scrolling back up a finished chapter says nothing.
        assert!(g.see(10, 12, false, 20 * s).is_empty());
        // A new chapter while one is held back: the held one goes first.
        assert_eq!(g.see(11, 1, false, 21 * s), vec![(11, Progress { page: 1, done: false })]);
        assert!(g.see(11, 2, false, 22 * s).is_empty());
        assert_eq!(g.see(12, 1, false, 22 * s + 5), vec![(11, Progress { page: 2, done: false })]);
        assert_eq!(g.flush(22 * s + 6), Some((12, Progress { page: 1, done: false })));
        assert_eq!(g.flush(22 * s + 7), None);
    }

    #[test]
    fn the_cache_lets_go_of_old_chapters_then_the_oldest() {
        let day = 86_400_000i64;
        let now = 100 * day;
        let c = |chapter, gb: f64, days_ago: i64| CachedChapter { chapter, bytes: (gb * 1e9) as u64, used_ms: now - days_ago * day };
        let cached = vec![c(1, 0.1, 9), c(2, 0.6, 3), c(3, 0.6, 2), c(4, 0.6, 1), c(5, 0.2, 30)];
        // 1 and 5 are past a week; the rest (1.8 GB) is over the cap, so 2 (the oldest) goes.
        assert_eq!(prune_choice(&cached, now, CACHE_CAP, &[]), vec![5, 1, 2]);
        // Open now: kept, though over a week old.
        assert_eq!(prune_choice(&cached, now, CACHE_CAP, &[5, 2]), vec![1, 3]);
        assert!(prune_choice(&[c(7, 0.1, 1)], now, CACHE_CAP, &[]).is_empty());
    }

    #[test]
    fn pages_scale_down_and_cut_into_tiles() {
        assert_eq!(scaled_size(1000, 8000), (800, 6400));
        assert_eq!(scaled_size(720, 3000), (720, 3000));
        assert_eq!(scaled_size(801, 1), (800, 1));
        assert_eq!(tile_rows(2500), vec![(0, 1020), (1020, 1020), (2040, 460)]);
        assert_eq!(tile_rows(1020), vec![(0, 1020)]);
        assert!(tile_rows(0).is_empty());
    }
}
