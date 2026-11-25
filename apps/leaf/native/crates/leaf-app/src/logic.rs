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
/// Rating: 9.25 • Bookmarks: 156.5K") and its trailing lists of other titles and links
/// ("Associated Name(s):- …", "Tracker(s):- [AniList](https://…)").
pub fn clean_description(d: &str) -> String {
    let mut t = d.trim();
    // A site's numbers in front ("Rank: #4 • Rating: 9.25 • Bookmarks: 156.5K"), glued to the
    // text that follows: each field and its number taken off.
    const FIELDS: [&str; 7] = ["Rank:", "Rating:", "Bookmarks:", "Views:", "Followers:", "Follows:", "Score:"];
    loop {
        let rest = t.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '•' | '·' | '|' | ','));
        let Some(f) = FIELDS.iter().find(|f| rest.starts_with(**f)) else { break };
        let v = rest[f.len()..].trim_start();
        let n = v.find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | ',' | '#' | 'K' | 'k' | 'M' | '%' | '/'))).unwrap_or(v.len());
        t = &v[n..];
    }
    t = t.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '•' | '·' | '|'));
    for tail in ["Alternative Title", "Associated Name", "Tracker(s)", "Official Source", "Links:", "\n---"] {
        if let Some(i) = t.find(tail) {
            t = t[..i].trim_end();
        }
    }
    // A tagline in brackets ("[By the author of <Mount Hua>]") becomes its own sentence, a title in
    // angle brackets is quoted, and the text is one paragraph with single spaces.
    let mut out = String::new();
    let mut rest = t;
    if let Some(inner) = rest.strip_prefix('[')
        && let Some(end) = inner.find(']')
    {
        let tag = inner[..end].trim();
        out.push_str(tag);
        if !tag.ends_with(['.', '!', '?']) {
            out.push('.');
        }
        out.push(' ');
        rest = &inner[end + 1..];
    }
    out.push_str(rest);
    let out = out.replace('<', "\u{201C}").replace('>', "\u{201D}");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `n` of `all`, a different `n` each `turn_ms` (by the wall clock, so it holds across restarts),
/// round the list: six top picks of 24 come round again after four turns.
pub fn rotating<T: Clone>(all: &[T], n: usize, now_ms: i64, turn_ms: i64) -> Vec<T> {
    if all.len() <= n || n == 0 {
        return all.to_vec();
    }
    let sets = all.len().div_ceil(n) as i64;
    let start = (now_ms.div_euclid(turn_ms.max(1)).rem_euclid(sets)) as usize * n;
    (0..n).map(|i| all[(start + i) % all.len()].clone()).collect()
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

/// Everything there is of the series is read: nothing after the furthest chapter read to its end
/// (a stray side chapter left unread below it does not keep it going). With the chapters at hand,
/// none unread past it; without, the library's newest is not past it. A belled series whose new
/// chapter came after you finished is not finished: it has one past it.
pub fn finished(s: &Series, d: Option<&SeriesDetail>) -> bool {
    let Some(upto) = s.read_upto else { return false };
    match d {
        Some(d) if !d.chapters.is_empty() => !d.chapters.iter().any(|c| !c.read && c.number > upto),
        _ => s.unread == 0 || s.latest.is_none_or(|l| l <= upto),
    }
}

/// Chapters left after the furthest one read, as the library knows it: the newest's number past
/// it, never more than the unread count (a gap or a half chapter counts once). None when
/// nothing is read.
pub fn left_after(s: &Series, d: Option<&SeriesDetail>) -> Option<u32> {
    let upto = s.read_upto?;
    if let Some(d) = d
        && !d.chapters.is_empty()
    {
        return Some(to_go(s, Some(d)));
    }
    let by_number = s.latest.map_or(0.0, |l| (l - upto.floor()).max(0.0).ceil()) as u32;
    Some(by_number.min(s.unread))
}

/// Where a series stands in Recent, as a sort key (the owner, 2026-11-25): what you read here,
/// last read first; then what you are partway through but never opened here (its progress came
/// with it), the fewest chapters left first, so one or two from the newest come up top; then what
/// is caught up, last read first; then what is not started, newest added first.
pub fn recent_key(last_read: i64, added: i64, done: bool, left: Option<u32>) -> (u8, i64, i64) {
    match (last_read > 0, done, left) {
        (true, false, _) => (0, -last_read, -added),
        (false, false, Some(n)) => (1, n as i64, -added),
        (_, true, _) => (2, -last_read, -added),
        (false, false, None) => (3, -added, 0),
    }
}


pub fn to_go(s: &Series, d: Option<&SeriesDetail>) -> u32 {
    match (s.read_upto, d) {
        (Some(upto), Some(d)) if !d.chapters.is_empty() => d.chapters.iter().filter(|c| !c.read && c.number > upto).count() as u32,
        _ => s.unread,
    }
}

/// Something "Continue reading" may list (a series or a book).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reading {
    /// Opened at least once (a chapter read or begun, a book opened).
    pub started: bool,
    /// Nothing left to read ([`finished`]).
    pub done: bool,
    /// When it was last read, ms.
    pub read_at: i64,
    /// A belled series' newest chapter came after it was last read: when it came, ms (0 if not).
    pub news_at: i64,
}

/// "Continue reading" lists at least this many when it can: finished things fill it up to here.
pub const CONTINUE_FILL: usize = 15;

/// Continue reading's order, as (index, finished): what you have started and not finished, most
/// recently read first (a belled series' new chapter counts as of when it came, so it comes back
/// near the front); then, only while there are fewer than [`CONTINUE_FILL`], finished ones, most
/// recently read first. Never what was never opened.
pub fn continue_order(list: &[Reading]) -> Vec<(usize, bool)> {
    let key = |r: &Reading| r.read_at.max(r.news_at);
    let mut going: Vec<usize> = (0..list.len()).filter(|&i| list[i].started && !list[i].done).collect();
    going.sort_by_key(|&i| std::cmp::Reverse(key(&list[i])));
    let mut done: Vec<usize> = (0..list.len()).filter(|&i| list[i].started && list[i].done).collect();
    done.sort_by_key(|&i| std::cmp::Reverse(list[i].read_at));
    let room = CONTINUE_FILL.saturating_sub(going.len());
    going.into_iter().map(|i| (i, false)).chain(done.into_iter().take(room).map(|i| (i, true))).collect()
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
    /// Everything of it is read ([`finished`]): it comes after what is still being read.
    pub done: bool,
}

/// The spotlight's order, as indexes into `spots`: belled series with new chapters first (newest
/// first), then what is being read, most recently first; at most `max`. Finished things are left
/// out (the caller adds them last, after what was added lately); never-read things lead nothing.
pub fn spotlight(spots: &[Spot], max: usize) -> Vec<usize> {
    let mut ringing: Vec<usize> = (0..spots.len()).filter(|&i| spots[i].rings).collect();
    ringing.sort_by_key(|&i| std::cmp::Reverse(spots[i].updated_at));
    let mut read: Vec<usize> = (0..spots.len()).filter(|&i| !spots[i].rings && !spots[i].done && spots[i].read_at > 0).collect();
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

/// The next `n` chapters from where reading stopped (the Continue point on) that this phone
/// doesn't hold or wait for (`kept`): what "Download next 5" asks for. Chapter ids.
pub fn next_to_download(chapters: &[Chapter], n: usize, kept: impl Fn(i64) -> bool) -> Vec<i64> {
    let from = continue_point(chapters).map_or(0, |c| c.index);
    chapters[from..].iter().filter(|c| !c.read && !kept(c.id)).take(n).map(|c| c.id).collect()
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

/// A series shows in My space (Recent, Following, Updates) and on Home only once you have done
/// something with it: read a chapter, turned its bell on, or downloaded one to this phone
/// (`downloads`: it has chapters on the phone or in the line). One added and left alone (a
/// preview opened, "Wrong series?" elsewhere) stays out (the owner, 2026-11-23).
pub fn in_space(s: &Series, downloads: bool) -> bool {
    s.bell || s.last_read_at > 0 || s.read_upto.is_some() || downloads
}

/// Read into a chapter, not just opened: past its second page, or a fifth of it (chapters cut
/// into a few tall pages). A chapter opened by mistake and left at once is not.
pub fn read_into(page: u32, pages: u32) -> bool {
    page >= 2 || (pages > 0 && page >= 1 && page * 5 >= pages)
}

/// Continue where the person stopped: a chapter left part way (after the last one finished),
/// else the one after the last finished, else the first. When everything is read, the newest.
pub fn continue_point(chapters: &[Chapter]) -> Option<Continue> {
    if chapters.is_empty() {
        return None;
    }
    let last_read = chapters.iter().rposition(|c| c.read);
    // A chapter left part way counts only once it was read into, or when it is the one after
    // the last finished anyway: one opened by mistake and left at once does not move Continue.
    let next = last_read.map_or(0, |r| r + 1);
    let part_way = chapters.iter().enumerate().rposition(|(i, c)| !c.read && c.last_page > 0 && (i == next || read_into(c.last_page, c.pages)));
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

    /// The latest word on a chapter: held back, else sent, else what the server held.
    pub fn place(&self, chapter: i64) -> Option<Progress> {
        self.pending.filter(|(c, _)| *c == chapter).map(|(_, p)| p).or_else(|| self.sent.get(&chapter).copied())
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

/// When a backup file was made, from its name ("leaf-backup-1795398420.json": seconds since the
/// epoch); 0 when the name doesn't say.
pub fn backup_made_ms(name: &str) -> i64 {
    let digits: String = name.rsplit('-').next().unwrap_or("").chars().take_while(char::is_ascii_digit).collect();
    digits.parse::<i64>().ok().filter(|s| *s > 1_000_000_000).map_or(0, |s| s * 1000)
}

/// A file name from a `Content-Disposition` header (`attachment; filename="x.json"`).
pub fn disposition_name(h: &str) -> Option<String> {
    let (_, rest) = h.split_once("filename=")?;
    let name = rest.split(';').next()?.trim().trim_matches('"').trim();
    let name: String = name.chars().filter(|c| !matches!(c, '/' | '\\')).collect();
    (!name.is_empty()).then_some(name)
}

// Finding a title as you type

/// A letter without its accent, lower case: "é" and "É" are "e", "ß" is "ss", "æ" is "ae".
fn fold_char(c: char, out: &mut String) {
    let base = match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'Ā' | 'Ă' | 'Ą' => 'a',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' | 'Ç' | 'Ć' | 'Ĉ' | 'Ċ' | 'Č' => 'c',
        'ď' | 'đ' | 'Ď' | 'Đ' | 'ð' | 'Ð' => 'd',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' | 'È' | 'É' | 'Ê' | 'Ë' | 'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => 'e',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' | 'Ĝ' | 'Ğ' | 'Ġ' | 'Ģ' => 'g',
        'ĥ' | 'ħ' | 'Ĥ' | 'Ħ' => 'h',
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' | 'Ì' | 'Í' | 'Î' | 'Ï' | 'Ĩ' | 'Ī' | 'Ĭ' | 'Į' | 'İ' => 'i',
        'ĵ' | 'Ĵ' => 'j',
        'ķ' | 'Ķ' => 'k',
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' | 'Ĺ' | 'Ļ' | 'Ľ' | 'Ŀ' | 'Ł' => 'l',
        'ñ' | 'ń' | 'ņ' | 'ň' | 'Ñ' | 'Ń' | 'Ņ' | 'Ň' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'Ō' | 'Ŏ' | 'Ő' => 'o',
        'ŕ' | 'ŗ' | 'ř' | 'Ŕ' | 'Ŗ' | 'Ř' => 'r',
        'ś' | 'ŝ' | 'ş' | 'š' | 'Ś' | 'Ŝ' | 'Ş' | 'Š' => 's',
        'ţ' | 'ť' | 'ŧ' | 'Ţ' | 'Ť' | 'Ŧ' => 't',
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' | 'Ù' | 'Ú' | 'Û' | 'Ü' | 'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => 'u',
        'ŵ' | 'Ŵ' => 'w',
        'ý' | 'ÿ' | 'ŷ' | 'Ý' | 'Ÿ' | 'Ŷ' => 'y',
        'ź' | 'ż' | 'ž' | 'Ź' | 'Ż' | 'Ž' => 'z',
        'ß' => return out.push_str("ss"),
        'æ' | 'Æ' => return out.push_str("ae"),
        'œ' | 'Œ' => return out.push_str("oe"),
        'þ' | 'Þ' => return out.push_str("th"),
        // Curly quotes and dashes read as the plain ones.
        '\u{2018}' | '\u{2019}' => '\'',
        '\u{2013}' | '\u{2014}' => '-',
        c => return out.extend(c.to_lowercase()),
    };
    out.push(base);
}

/// `s` lower case, without accents, its runs of spaces made one.
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_whitespace() {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
        } else {
            fold_char(c, &mut out);
        }
    }
    out.trim_end().to_string()
}

/// The title holds every word typed, anywhere in it (inside words too), whatever the case and
/// accents: "lev god" finds "Level Up with the Gods". An empty query holds everything.
pub fn title_matches(title: &str, query: &str) -> bool {
    let t = fold(title);
    fold(query).split(' ').filter(|w| !w.is_empty()).all(|w| t.contains(w))
}

// Shelves

/// Where a series sits in My space: what you choose, or Reading once started and Plan to read
/// before.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shelf {
    Reading,
    Plan,
    Hold,
    Dropped,
    Done,
}

impl Shelf {
    pub const ALL: [Shelf; 5] = [Shelf::Reading, Shelf::Plan, Shelf::Hold, Shelf::Dropped, Shelf::Done];

    /// The name the server keeps.
    pub fn key(self) -> &'static str {
        match self {
            Shelf::Reading => "reading",
            Shelf::Plan => "plan",
            Shelf::Hold => "hold",
            Shelf::Dropped => "dropped",
            Shelf::Done => "done",
        }
    }

    pub fn words(self) -> &'static str {
        match self {
            Shelf::Reading => "Reading",
            Shelf::Plan => "Plan to read",
            Shelf::Hold => "On hold",
            Shelf::Dropped => "Dropped",
            Shelf::Done => "Done",
        }
    }

    pub fn from_key(k: &str) -> Option<Shelf> {
        Shelf::ALL.into_iter().find(|s| s.key() == k)
    }
}

/// The shelf a series shows on: the one chosen, else Reading once a chapter was read or opened,
/// else Plan to read.
pub fn shelf_of(s: &Series) -> Shelf {
    if let Some(sh) = s.shelf.as_deref().and_then(Shelf::from_key) {
        return sh;
    }
    if s.read_upto.is_some() || s.last_read_at > 0 { Shelf::Reading } else { Shelf::Plan }
}

/// How many series sit on each shelf, in [`Shelf::ALL`]'s order.
pub fn shelf_counts(list: &[Series]) -> [usize; 5] {
    let mut n = [0; 5];
    for s in list {
        let i = Shelf::ALL.iter().position(|x| *x == shelf_of(s)).unwrap_or(0);
        n[i] += 1;
    }
    n
}

// Updates

/// Updates grouped by the local day they came, newest first: (`day` counted from 1970-01-01 as
/// [`crate::shelf::day_of`] counts, indexes into `list`). `belled_only` leaves out series whose
/// bell is off; `query` filters by title.
pub fn group_updates(list: &[leaf_proto::Update], belled_only: bool, query: &str, day_of: impl Fn(i64) -> i64) -> Vec<(i64, Vec<usize>)> {
    let mut idx: Vec<usize> = (0..list.len()).filter(|&i| (!belled_only || list[i].bell) && title_matches(&list[i].title, query)).collect();
    idx.sort_by_key(|&i| (std::cmp::Reverse(list[i].at), std::cmp::Reverse(list[i].number.to_bits())));
    let mut out: Vec<(i64, Vec<usize>)> = Vec::new();
    for i in idx {
        let d = day_of(list[i].at);
        match out.last_mut() {
            Some((day, v)) if *day == d => v.push(i),
            _ => out.push((d, vec![i])),
        }
    }
    out
}

/// A day's updates as rows: a series with more than two chapters that day is one row (its
/// chapters, lowest number first), the others one row per chapter; in the day's order, a series'
/// row where its newest chapter was.
pub fn fold_updates(list: &[leaf_proto::Update], day: &[usize]) -> Vec<Vec<usize>> {
    let mut of: Vec<(i64, Vec<usize>)> = Vec::new();
    for &i in day {
        match of.iter_mut().find(|(s, _)| *s == list[i].series) {
            Some((_, v)) => v.push(i),
            None => of.push((list[i].series, vec![i])),
        }
    }
    let mut out = Vec::new();
    for (_, mut v) in of {
        if v.len() > 2 {
            v.sort_by(|&a, &b| list[a].number.total_cmp(&list[b].number));
            out.push(v);
        } else {
            out.extend(v.into_iter().map(|i| vec![i]));
        }
    }
    out
}

/// A folded row's line: "Chapters 1–86 · 86 new", "· 3 new of 86", or "· read".
pub fn run_words(list: &[leaf_proto::Update], run: &[usize]) -> String {
    let (lo, hi) = (list[run[0]].number, list[run[run.len() - 1]].number);
    let unread = run.iter().filter(|&&i| !list[i].read).count();
    let tail = match unread {
        0 => "read".to_string(),
        u if u == run.len() => format!("{u} new"),
        u => format!("{u} new of {}", run.len()),
    };
    format!("Chapters {}\u{2013}{} · {tail}", num(lo), num(hi))
}

/// A day's heading: "Today", "Yesterday", else its date.
pub fn day_words(day: i64, today: i64) -> String {
    match today - day {
        0 => "Today".into(),
        1 => "Yesterday".into(),
        _ => date(day * 86_400_000),
    }
}

/// The local time of day, "14:05", of a moment `offset_s` seconds from UTC.
pub fn clock_words(ms: i64, offset_s: i64) -> String {
    let m = (ms.div_euclid(1000) + offset_s).div_euclid(60).rem_euclid(1440);
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// Minutes into the day as "23:00".
pub fn minute_words(m: u16) -> String {
    format!("{:02}:{:02}", (m / 60) % 24, m % 60)
}

// Auto-scroll

/// Auto-scroll's speeds, slowest first.
pub const SPEEDS: u8 = 10;
/// The speed a series starts at.
pub const DEFAULT_SPEED: u8 = 4;

/// Units per second at speed `level` (1 to [`SPEEDS`]): each a quarter faster than the one before,
/// from a slow read of dense pages to a skim.
pub fn scroll_speed(level: u8) -> f32 {
    40.0 * 1.25f32.powi(level.clamp(1, SPEEDS) as i32 - 1)
}

// White and black gutters

/// A run of near-uniform rows at a page's top or bottom longer than this (page pixels) is cut.
pub const GUTTER_MIN: u32 = 40;
/// It is cut down to this.
pub const GUTTER_KEEP: u32 = 16;

/// A row of RGBA pixels is near-uniform white (Some(true)) or black (Some(false)): all but one in
/// a hundred of the pixels looked at are within a small distance of it.
fn row_kind(row: &[u8]) -> Option<bool> {
    let px = row.len() / 4;
    if px == 0 {
        return None;
    }
    let step = (px / 200).max(1);
    let (mut white, mut black, mut seen) = (0usize, 0usize, 0usize);
    for i in (0..px).step_by(step) {
        let p = &row[i * 4..i * 4 + 3];
        let (lo, hi) = (p.iter().copied().min().unwrap_or(0), p.iter().copied().max().unwrap_or(0));
        if lo >= 232 {
            white += 1;
        } else if hi <= 24 {
            black += 1;
        }
        seen += 1;
    }
    let need = seen - seen / 100;
    if white >= need {
        Some(true)
    } else if black >= need {
        Some(false)
    } else {
        None
    }
}

/// How many rows to cut from a page's top and bottom (RGBA, `w` × `h`): a run of near-white or
/// near-black rows longer than [`GUTTER_MIN`] is cut down to [`GUTTER_KEEP`]. A page that is all
/// one blank keeps twice that.
pub fn gutter_cuts(px: &[u8], w: u32, h: u32) -> (u32, u32) {
    let row = (w * 4) as usize;
    if h == 0 || px.len() < row * h as usize {
        return (0, 0);
    }
    let kind = |y: u32| row_kind(&px[y as usize * row..(y as usize + 1) * row]);
    let run = |rows: &mut dyn Iterator<Item = u32>| -> u32 {
        let mut first = None;
        let mut n = 0;
        for y in rows {
            match (kind(y), first) {
                (Some(k), None) => first = Some(k),
                (Some(k), Some(f)) if k == f => {}
                _ => break,
            }
            n += 1;
        }
        n
    };
    let top = run(&mut (0..h));
    if top >= h {
        return (h.saturating_sub(2 * GUTTER_KEEP), 0);
    }
    let bottom = run(&mut (0..h).rev());
    let cut = |n: u32| if n > GUTTER_MIN { n - GUTTER_KEEP } else { 0 };
    (cut(top), cut(bottom))
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
            shelf: None,
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
            shelf: None,
        }
    }

    #[test]
    fn recent_puts_series_one_or_two_chapters_from_the_end_first_after_what_you_read_here() {
        // Progress that came with a series (read to 9 of 10), never opened here.
        let near = series(Some(9.0), 1, 0, 0);
        assert_eq!(left_after(&near, None), Some(1));
        // A half chapter and a gap leave more unread than the numbers say: the numbers win.
        let gappy = series(Some(9.0), 5, 0, 0);
        assert_eq!(left_after(&gappy, None), Some(1));
        let far = series(Some(2.0), 8, 0, 0);
        assert_eq!(left_after(&far, None), Some(8));
        assert_eq!(left_after(&series(None, 10, 0, 0), None), None, "nothing read");
        let mut keys = [
            ("not started", recent_key(0, 50, false, None)),
            ("far", recent_key(0, 40, false, Some(8))),
            ("caught up", recent_key(0, 30, true, Some(0))),
            ("read here", recent_key(1_000, 1, false, Some(30))),
            ("near", recent_key(0, 20, false, Some(1))),
            ("two left", recent_key(0, 60, false, Some(2))),
            ("read here lately", recent_key(2_000, 1, false, Some(40))),
        ];
        keys.sort_by_key(|k| k.1);
        let order: Vec<&str> = keys.iter().map(|k| k.0).collect();
        assert_eq!(order, ["read here lately", "read here", "near", "two left", "far", "caught up", "not started"]);
    }
    #[test]
    fn only_what_you_did_something_with_is_in_your_space() {
        let mut s = series(None, 10, 0, 0);
        assert!(!in_space(&s, false), "added and left alone");
        assert!(in_space(&s, true), "a chapter downloaded");
        s.bell = true;
        assert!(in_space(&s, false), "followed");
        assert!(in_space(&series(Some(1.0), 9, 0, 0), false), "a chapter read");
        assert!(in_space(&series(None, 10, 0, 5), false), "opened in the reader");
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
        let sp = |rings, updated_at, read_at| Spot { rings, updated_at, read_at, done: false };
        let mut spots = vec![sp(false, 9, 50), sp(true, 30, 10), sp(false, 0, 0), sp(true, 40, 5), sp(false, 1, 70)];
        assert_eq!(spotlight(&spots, 5), vec![3, 1, 4, 0]);
        assert_eq!(spotlight(&spots, 2), vec![3, 1]);
        assert!(spotlight(&[], 5).is_empty());
        // Finished, read last: left to the caller to add at the end.
        spots.push(Spot { done: true, ..sp(false, 0, 90) });
        assert_eq!(spotlight(&spots, 9), vec![3, 1, 4, 0]);
    }

    #[test]
    fn finished_means_nothing_left_past_where_you_read() {
        // Read to 10 of 10: finished, though a side chapter 4.5 was never read.
        let mut s = series(Some(10.0), 1, 0, 5);
        assert!(finished(&s, None));
        let mut cs: Vec<Chapter> = (1..=10).map(|n| ch(n, n as f64, true, 0)).collect();
        cs.insert(4, ch(99, 4.5, false, 0));
        let d = SeriesDetail { series: s.clone(), description: None, chapters: cs.clone() };
        assert!(finished(&s, Some(&d)));
        assert_eq!(to_go(&s, Some(&d)), 0);
        // A new chapter 11 came after: not finished, one to go.
        cs.push(ch(11, 11.0, false, 0));
        s.latest = Some(11.0);
        let d = SeriesDetail { series: s.clone(), description: None, chapters: cs };
        assert!(!finished(&s, Some(&d)));
        assert!(!finished(&s, None));
        assert_eq!(to_go(&s, Some(&d)), 1);
        // Never read, or part way: not finished.
        assert!(!finished(&series(None, 10, 0, 0), None));
        assert!(!finished(&series(Some(6.0), 4, 0, 5), None));
    }

    #[test]
    fn continue_lists_what_is_going_then_fills_with_what_is_finished() {
        let r = |started, done, read_at, news_at| Reading { started, done, read_at, news_at };
        let list = [r(true, false, 50, 0), r(true, true, 90, 0), r(false, false, 0, 0), r(true, false, 10, 80), r(true, false, 70, 0), r(true, true, 60, 0)];
        // Going, most recent first (3's new chapter came at 80); then finished ones, labelled.
        assert_eq!(continue_order(&list), vec![(3, false), (4, false), (0, false), (1, true), (5, true)]);
        // Fifteen going: no finished ones.
        let mut many: Vec<Reading> = (0..15).map(|i| r(true, false, 100 + i, 0)).collect();
        many.push(r(true, true, 1000, 0));
        let order = continue_order(&many);
        assert_eq!(order.len(), 15);
        assert!(order.iter().all(|(_, done)| !done));
        assert_eq!(order[0].0, 14);
        // Fourteen going: one finished fills the last place.
        many.remove(0);
        assert_eq!(continue_order(&many).last(), Some(&(14, true)));
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
        let chapters: Vec<Chapter> = (1..=10).map(|n| ch(n, n as f64, n <= 3, 0)).collect();
        // Chapter 5 is on this phone already; what the server keeps doesn't count.
        let kept = |id: i64| id == 5;
        assert_eq!(next_to_download(&chapters, 5, kept), vec![4, 6, 7, 8, 9]);
        assert_eq!(next_to_download(&chapters[..5], 5, kept), vec![4]);
    }

    #[test]
    fn descriptions_lose_the_sources_furniture() {
        assert_eq!(clean_description("Rank: #4 • Rating: 9.25 • Bookmarks: 156.5KNanotechnology meets martial arts.Alternative Titles:- NM"), "Nanotechnology meets martial arts.");
        assert_eq!(clean_description("  A plain one. "), "A plain one.");
        let picks: Vec<u32> = (0..14).collect();
        let h = 3_600_000;
        assert_eq!(rotating(&picks, 6, 0, 4 * h), vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(rotating(&picks, 6, 3 * h, 4 * h), vec![0, 1, 2, 3, 4, 5], "the same until the turn");
        assert_eq!(rotating(&picks, 6, 4 * h, 4 * h), vec![6, 7, 8, 9, 10, 11]);
        assert_eq!(rotating(&picks, 6, 8 * h, 4 * h), vec![12, 13, 0, 1, 2, 3], "round the list, always six");
        assert_eq!(rotating(&picks, 6, 12 * h, 4 * h), vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(rotating(&picks[..4], 6, 4 * h, 4 * h), vec![0, 1, 2, 3], "fewer than six: all of them");
        assert_eq!(
            clean_description("Rating: 9.29 • Bookmarks: 40.9K[By the Author that brought you <Return of the Mount Hua Sect>] The great warrior who ended the chaos."),
            "By the Author that brought you \u{201C}Return of the Mount Hua Sect\u{201D}. The great warrior who ended the chaos."
        );
        assert_eq!(clean_description("Views: 1.2M | A story.\n\nIn two parts."), "A story. In two parts.");
        assert_eq!(clean_description("Hajin begins to uncover the truth behind his death...Associated Name(s):- Reborn As The Heavenly DemonTracker(s):- [Official Source](https://www.webtoons.com/en/x)"), "Hajin begins to uncover the truth behind his death...");
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
        // A chapter far ahead opened by mistake (left on its first pages) does not move Continue;
        // one read into does.
        let slip = vec![ch(1, 1.0, true, 29), ch(2, 2.0, false, 0), ch(3, 3.0, false, 0), ch(4, 4.0, false, 1)];
        assert_eq!(continue_point(&slip).unwrap().index, 1);
        let real = vec![ch(1, 1.0, true, 29), ch(2, 2.0, false, 0), ch(3, 3.0, false, 0), ch(4, 4.0, false, 6)];
        assert_eq!(continue_point(&real).unwrap().index, 3);
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

    #[test]
    fn a_title_is_found_by_any_part_of_it_whatever_its_accents() {
        assert!(title_matches("Level Up with the Gods", "lev god"));
        assert!(title_matches("Level Up with the Gods", ""));
        assert!(title_matches("Pokémon Adventures", "POKEMON"));
        assert!(title_matches("Çağ", "cag"));
        assert!(title_matches("Solo  Leveling", "o lev"));
        assert!(!title_matches("Nano Machine", "nano king"));
        assert_eq!(fold("  Straße  Œuvre "), "strasse oeuvre");
    }

    #[test]
    fn a_series_sits_on_the_shelf_chosen_else_where_its_reading_puts_it() {
        let mut s = series(None, 10, 0, 0);
        assert_eq!(shelf_of(&s), Shelf::Plan);
        s.last_read_at = 5;
        assert_eq!(shelf_of(&s), Shelf::Reading);
        s.shelf = Some("dropped".into());
        assert_eq!(shelf_of(&s), Shelf::Dropped);
        s.shelf = Some("nonsense".into());
        assert_eq!(shelf_of(&s), Shelf::Reading);
        let mut d = series(Some(1.0), 9, 0, 0);
        d.shelf = Some("done".into());
        assert_eq!(shelf_counts(&[s, d, series(None, 1, 0, 0)]), [1, 1, 0, 0, 1]);
        for sh in Shelf::ALL {
            assert_eq!(Shelf::from_key(sh.key()), Some(sh));
        }
    }

    #[test]
    fn updates_group_by_day_newest_first() {
        let day = 86_400_000i64;
        let u = |series: i64, title: &str, number: f64, at: i64, bell: bool| leaf_proto::Update { series, title: title.into(), chapter: series * 1000 + number as i64, number, name: String::new(), at, read: false, bell };
        let list = vec![u(1, "Nano Machine", 280.0, 10 * day + 5, true), u(2, "Level Up", 90.0, 11 * day + 7, false), u(1, "Nano Machine", 281.0, 10 * day + 5, true), u(3, "Junk", 5.0, 9 * day, true)];
        let g = group_updates(&list, false, "", |ms| ms.div_euclid(day));
        assert_eq!(g, vec![(11, vec![1]), (10, vec![2, 0]), (9, vec![3])]);
        assert_eq!(group_updates(&list, true, "", |ms| ms.div_euclid(day)), vec![(10, vec![2, 0]), (9, vec![3])]);
        assert_eq!(group_updates(&list, false, "nano", |ms| ms.div_euclid(day)), vec![(10, vec![2, 0])]);
        assert_eq!(day_words(11, 11), "Today");
        assert_eq!(day_words(10, 11), "Yesterday");
        assert_eq!(day_words(0, 11), "Jan 1, 1970");
        assert_eq!(clock_words(14 * 3_600_000 + 5 * 60_000, 0), "14:05");
        assert_eq!(clock_words(23 * 3_600_000, 3600), "00:00");
        assert_eq!(minute_words(1380), "23:00");
    }

    #[test]
    fn auto_scroll_speeds_climb_evenly() {
        assert_eq!(scroll_speed(1), 40.0);
        assert!(scroll_speed(0) == scroll_speed(1) && scroll_speed(99) == scroll_speed(SPEEDS));
        for l in 1..SPEEDS {
            assert!((scroll_speed(l + 1) / scroll_speed(l) - 1.25).abs() < 1e-4);
        }
    }

    /// A page `w` wide of rows given as (colour, count) from the top.
    fn page(w: u32, bands: &[([u8; 3], u32)]) -> (Vec<u8>, u32) {
        let mut px = Vec::new();
        let mut h = 0;
        for (c, n) in bands {
            for _ in 0..*n {
                for _ in 0..w {
                    px.extend_from_slice(&[c[0], c[1], c[2], 255]);
                }
            }
            h += n;
        }
        (px, h)
    }

    #[test]
    fn white_and_black_gutters_are_cut_down() {
        let (white, black, art) = ([255, 255, 255], [3, 3, 3], [120, 60, 200]);
        // 100 white rows over the art, 60 black under it: cut to 16 each.
        let (px, h) = page(50, &[(white, 100), (art, 300), (black, 60)]);
        assert_eq!(gutter_cuts(&px, 50, h), (84, 44));
        // 40 rows is not more than 40: kept whole.
        let (px, h) = page(50, &[(white, 40), (art, 300), ([250, 248, 245], 41)]);
        assert_eq!(gutter_cuts(&px, 50, h), (0, 25), "near-white counts as white");
        // White then black is two runs: only the first counts.
        let (px, h) = page(50, &[(white, 30), (black, 30), (art, 100)]);
        assert_eq!(gutter_cuts(&px, 50, h), (0, 0));
        // A speck or two in a row does not stop it being blank; art does.
        let (mut px, h) = page(400, &[(white, 80), (art, 100)]);
        px[10 * 400 * 4..10 * 400 * 4 + 4].copy_from_slice(&[0, 0, 0, 255]);
        assert_eq!(gutter_cuts(&px, 400, h), (64, 0));
        // A blank page keeps a sliver.
        let (px, h) = page(50, &[(white, 500)]);
        assert_eq!(gutter_cuts(&px, 50, h), (500 - 32, 0));
        assert_eq!(gutter_cuts(&[], 50, 0), (0, 0));
    }

    #[test]
    fn a_backup_says_when_it_was_made() {
        assert_eq!(backup_made_ms("leaf-backup-1795398420.json"), 1_795_398_420_000);
        assert_eq!(backup_made_ms("backup.json"), 0);
        assert_eq!(disposition_name("attachment; filename=\"leaf-backup-1795398420.json\""), Some("leaf-backup-1795398420.json".into()));
        assert_eq!(disposition_name("attachment; filename=../x.json"), Some("..x.json".into()));
        assert_eq!(disposition_name("inline"), None);
    }
}

#[cfg(test)]
mod fold_tests {
    use super::*;
    fn u(series: i64, number: f64, read: bool) -> leaf_proto::Update {
        leaf_proto::Update { series, title: format!("S{series}"), chapter: (series * 1000) + number as i64, number, name: String::new(), at: 0, read, bell: true }
    }
    #[test]
    fn more_than_two_of_a_series_fold_into_one_row() {
        let mut list: Vec<_> = (1..=86).rev().map(|n| u(1, n as f64, false)).collect();
        list.push(u(2, 5.0, false));
        list.push(u(2, 4.0, true));
        let day: Vec<usize> = (0..list.len()).collect();
        let rows = fold_updates(&list, &day);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].len(), 86);
        assert_eq!(run_words(&list, &rows[0]), "Chapters 1\u{2013}86 · 86 new");
        assert_eq!((rows[1].len(), rows[2].len()), (1, 1));
        list[0].read = true;
        assert_eq!(run_words(&list, &rows[0]), "Chapters 1\u{2013}86 · 85 new of 86");
    }
}
