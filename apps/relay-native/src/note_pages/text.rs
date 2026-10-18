//! Pure text helpers behind the Notes editor: list continuation, line-prefix toggles,
//! go-to-line parsing and document statistics. No GTK here, so they test headlessly.

/// What Enter does on a Markdown list or quote line.
#[derive(Debug, PartialEq, Eq)]
pub enum Enter {
    /// Start the next line with this prefix (indent included).
    Continue(String),
    /// The item is empty: remove its marker and end the list.
    End,
}

/// A recognised line marker: its indent, the marker text, and what it continues with.
struct Marker<'a> {
    indent: &'a str,
    marker: &'a str,
    next: String,
    kind: Kind,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Bullet,
    Number,
    Check,
    Quote,
}

fn marker(line: &str) -> Option<Marker<'_>> {
    let indent_len = line.len() - line.trim_start_matches([' ', '\t']).len();
    let (indent, rest) = line.split_at(indent_len);
    let bytes = rest.as_bytes();
    if let Some(&first) = bytes.first() {
        // `* * *` and `- - -` are thematic breaks, not list items.
        let rule = first != b'+'
            && rest.bytes().all(|b| b == first || b == b' ' || b == b'\t')
            && rest.bytes().filter(|b| *b == first).count() >= 3;
        if !rule && matches!(first, b'-' | b'*' | b'+') && bytes.get(1) == Some(&b' ') {
            let bullet = first as char;
            if rest.len() >= 6
                && bytes[2] == b'['
                && matches!(bytes[3], b' ' | b'x' | b'X')
                && bytes[4] == b']'
                && bytes[5] == b' '
            {
                return Some(Marker { indent, marker: &rest[..6], next: format!("{bullet} [ ] "), kind: Kind::Check });
            }
            return Some(Marker { indent, marker: &rest[..2], next: format!("{bullet} "), kind: Kind::Bullet });
        }
        if first == b'>' {
            let len = if bytes.get(1) == Some(&b' ') { 2 } else { 1 };
            return Some(Marker { indent, marker: &rest[..len], next: "> ".into(), kind: Kind::Quote });
        }
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if (1..=9).contains(&digits)
            && matches!(bytes.get(digits), Some(b'.' | b')'))
            && bytes.get(digits + 1) == Some(&b' ')
        {
            let n: u64 = rest[..digits].parse().ok()?;
            let delimiter = bytes[digits] as char;
            return Some(Marker { indent, marker: &rest[..digits + 2], next: format!("{}{delimiter} ", n + 1), kind: Kind::Number });
        }
    }
    None
}

/// Enter pressed with `before` the cursor and `after` it on the same line.
pub fn list_enter(before: &str, after: &str) -> Option<Enter> {
    let found = marker(before)?;
    let content = &before[found.indent.len() + found.marker.len()..];
    if content.trim().is_empty() && after.trim().is_empty() {
        return Some(Enter::End);
    }
    Some(Enter::Continue(format!("{}{}", found.indent, found.next)))
}

/// Line-level formats applied from the toolbar and the Format menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prefix {
    Heading(usize),
    Bullet,
    Number,
    Check,
    Quote,
}

fn heading(line: &str) -> Option<(usize, &str)> {
    // Up to three spaces may indent a heading; four make a code block.
    let indent = line.bytes().take(4).take_while(|b| *b == b' ').count();
    if indent > 3 {
        return None;
    }
    let line = &line[indent..];
    let hashes = line.bytes().take_while(|b| *b == b'#').count();
    if (1..=6).contains(&hashes) {
        let rest = &line[hashes..];
        if let Some(text) = rest.strip_prefix(' ') {
            return Some((hashes, text));
        }
        if rest.is_empty() {
            return Some((hashes, rest));
        }
    }
    None
}

/// Toggle `prefix` on every line. Applying a format every line already has removes it;
/// otherwise it replaces whatever list marker or heading a line carried.
pub fn toggle_lines(lines: &[&str], prefix: Prefix) -> Vec<String> {
    let all_empty = lines.iter().all(|l| l.trim().is_empty());
    let applies = |line: &&str| all_empty || !line.trim().is_empty();
    let has = |line: &str| match prefix {
        Prefix::Heading(n) => heading(line).is_some_and(|(h, _)| h == n),
        Prefix::Bullet => marker(line).is_some_and(|m| m.kind == Kind::Bullet),
        Prefix::Number => marker(line).is_some_and(|m| m.kind == Kind::Number),
        Prefix::Check => marker(line).is_some_and(|m| m.kind == Kind::Check),
        Prefix::Quote => marker(line).is_some_and(|m| m.kind == Kind::Quote),
    };
    let remove = !all_empty && lines.iter().filter(|l| applies(l)).all(|l| has(l));
    let mut number = 0;
    lines
        .iter()
        .map(|line| {
            if !applies(line) {
                return line.to_string();
            }
            match prefix {
                Prefix::Heading(n) => {
                    let text = heading(line).map_or(line.trim_start(), |(_, text)| text);
                    if remove { text.to_string() } else { format!("{} {text}", "#".repeat(n)) }
                }
                Prefix::Quote => {
                    if remove {
                        let m = marker(line).unwrap();
                        format!("{}{}", m.indent, &line[m.indent.len() + m.marker.len()..])
                    } else {
                        format!("> {line}")
                    }
                }
                _ => {
                    let (indent, text) = match marker(line) {
                        Some(m) if m.kind != Kind::Quote => (m.indent, &line[m.indent.len() + m.marker.len()..]),
                        _ => {
                            let text = line.trim_start_matches([' ', '\t']);
                            (&line[..line.len() - text.len()], text)
                        }
                    };
                    if remove {
                        return format!("{indent}{text}");
                    }
                    number += 1;
                    let mark = match prefix {
                        Prefix::Bullet => "- ".to_string(),
                        Prefix::Check => "- [ ] ".to_string(),
                        _ => format!("{number}. "),
                    };
                    format!("{indent}{mark}{text}")
                }
            }
        })
        .collect()
}

/// What an inline format toggle does to a selection.
#[derive(Debug, PartialEq, Eq)]
pub enum Inline {
    /// The markers sit just outside the selection: remove them.
    Outside,
    /// The selection starts and ends with the markers: remove them.
    Inside,
    /// Add the markers around the selection.
    Wrap,
}

/// Toggle `before`/`after` on `selected`, with `ahead` and `behind` the line's text either side.
/// Markdown's `**` is bold, not `*` twice, so a one-character marker that is both sides' only
/// marker counts only in an odd run: italic on `**bold**` makes it `***bold***`.
pub fn inline_toggle(ahead: &str, selected: &str, behind: &str, before: &str, after: &str) -> Inline {
    let mut chars = before.chars();
    let single = match (chars.next(), chars.next()) {
        (Some(c), None) if before == after => Some(c),
        _ => None,
    };
    let lead = |s: &str, c: char| s.chars().take_while(|x| *x == c).count();
    let tail = |s: &str, c: char| s.chars().rev().take_while(|x| *x == c).count();
    let odd = |left: usize, right: usize| left.min(right) % 2 == 1;
    if ahead.ends_with(before)
        && behind.starts_with(after)
        && single.is_none_or(|c| odd(tail(ahead, c), lead(behind, c)))
    {
        return Inline::Outside;
    }
    if selected.chars().count() >= before.chars().count() + after.chars().count()
        && selected.starts_with(before)
        && selected.ends_with(after)
        && single.is_none_or(|c| odd(lead(selected, c), tail(selected, c)))
    {
        return Inline::Inside;
    }
    Inline::Wrap
}

/// `12`, `12:5`, `12,5` or `12 5` → line and optional column, both 1-based.
pub fn parse_goto(value: &str) -> Option<(i32, Option<i32>)> {
    let mut parts = value
        .trim()
        .split(|c: char| c == ':' || c == ',' || c.is_whitespace())
        .filter(|part| !part.is_empty());
    let line = parts.next()?.parse::<i32>().ok().filter(|n| *n > 0)?;
    let column = match parts.next() {
        Some(part) => Some(part.parse::<i32>().ok().filter(|n| *n > 0)?),
        None => None,
    };
    if parts.next().is_some() {
        return None;
    }
    Some((line, column))
}

/// Lines, words and characters, as a text editor counts them.
pub fn stats(value: &str) -> (usize, usize, usize) {
    (value.split('\n').count(), value.split_whitespace().count(), value.chars().count())
}

pub fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `n` of a thing with thousands separated, in the singular for one: "1 line", "2,048 lines".
pub fn counted(n: usize, one: &str, many: &str) -> String {
    format!("{} {}", thousands(n), if n == 1 { one } else { many })
}

fn plain(line: &str) -> &str {
    let mut line = line.trim();
    if let Some((_, text)) = heading(line) {
        line = text.trim();
    }
    if let Some(m) = marker(line) {
        line = line[m.indent.len() + m.marker.len()..].trim();
    }
    line
}

fn clip(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        value.to_string()
    } else {
        format!("{}…", value.chars().take(limit).collect::<String>().trim_end())
    }
}

/// The name a note goes by: its title, else its first line of text.
pub fn display_title(title: &str, body: &str) -> String {
    let title = title.trim();
    if !title.is_empty() {
        return title.to_string();
    }
    body.lines()
        .map(plain)
        .find(|line| !line.is_empty())
        .map(|line| clip(line, 60))
        .unwrap_or_else(|| "Untitled".into())
}

/// The title a copy of a note is stored under. An untitled note's copy stays untitled, so its
/// name keeps following the body instead of freezing a clipped first line.
pub fn copy_title(title: &str) -> Option<String> {
    let title = title.trim();
    (!title.is_empty()).then(|| format!("{title} (copy)"))
}

/// A one-line preview of the text that is not already the title.
pub fn preview(title: &str, body: &str) -> String {
    let name = display_title(title, body);
    body.lines()
        .map(plain)
        .filter(|line| !line.is_empty())
        .find(|line| clip(line, 60) != name)
        .map(|line| clip(line, 90))
        .unwrap_or_default()
}

/// GTK takes C strings, so engine text with a NUL in it aborts the client in a debug build and
/// leaves the widget empty in a release one. U+FFFD stands in for each NUL on screen.
pub fn clean(value: &str) -> std::borrow::Cow<'_, str> {
    if value.contains('\0') {
        value.replace('\0', "\u{FFFD}").into()
    } else {
        value.into()
    }
}

pub const ZOOMS: [u16; 12] = [50, 67, 80, 90, 100, 110, 125, 150, 175, 200, 250, 300];

pub fn zoom_step(current: u16, direction: i32) -> u16 {
    if direction > 0 {
        ZOOMS.iter().copied().find(|z| *z > current).unwrap_or(300)
    } else {
        ZOOMS.iter().rev().copied().find(|z| *z < current).unwrap_or(50)
    }
}

/// Ctrl+scroll travel `acc` plus a scroll of `dy` → zoom steps (positive zooms in, as scrolling
/// up does) and the travel left over. A notch is one wheel click or 24 touchpad pixels.
pub fn zoom_notches(acc: f64, dy: f64, pixels: bool) -> (i32, f64) {
    let travel = acc + dy / if pixels { 24.0 } else { 1.0 };
    let notches = travel.trunc();
    (-(notches as i32), travel - notches)
}

/// A checklist item on `line`: the character column of its `[`, and whether it is ticked.
pub fn check_item(line: &str) -> Option<(usize, bool)> {
    let found = marker(line).filter(|m| m.kind == Kind::Check)?;
    let ticked = matches!(found.marker.as_bytes()[3], b'x' | b'X');
    Some((found.indent.chars().count() + 2, ticked))
}

/// The first Markdown image on `line`: its alt text, its path, and the character columns the
/// reference spans. A path with spaces or parentheses is written `<like this>`.
pub fn image_ref(line: &str) -> Option<(String, String, usize, usize)> {
    let start = line.find("![")?;
    let close = start + 2 + line[start + 2..].find("](")?;
    let alt = &line[start + 2..close];
    if alt.contains(']') {
        return None;
    }
    let rest = &line[close + 2..];
    let (path, length) = match rest.strip_prefix('<') {
        Some(inner) => {
            let end = inner.find('>')?;
            inner[end + 1..].starts_with(')').then_some(())?;
            (&inner[..end], end + 3)
        }
        None => {
            let end = rest.find(')')?;
            (&rest[..end], end + 1)
        }
    };
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    let from = line[..start].chars().count();
    let to = line[..close + 2 + length].chars().count();
    Some((alt.to_string(), path.to_string(), from, to))
}

/// Whether `text` holds nothing but image references (one per line) and blank space: only
/// then does a copy put the pictures on the clipboard. With words around them, another app
/// would take the picture and drop the words, so the copy stays text.
pub fn only_images(text: &str) -> bool {
    let mut any = false;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Some((_, _, from, to)) = image_ref(line) else { return false };
        let mut outside = line.chars().take(from).chain(line.chars().skip(to));
        if !outside.all(char::is_whitespace) {
            return false;
        }
        any = true;
    }
    any
}

/// A Markdown image reference to `path`, bracketed when the path would end it early.
pub fn image_markdown(alt: &str, path: &str) -> String {
    if path.contains([' ', '(', ')', '<', '>']) {
        format!("![{alt}](<{path}>)")
    } else {
        format!("![{alt}]({path})")
    }
}

/// The number the next pasted image takes: one past the highest "Image N" already in `body`.
pub fn next_image_number(body: &str) -> usize {
    body.lines()
        .filter_map(image_ref)
        .filter_map(|(alt, ..)| alt.strip_prefix("Image ").and_then(|n| n.trim().parse::<usize>().ok()))
        .max()
        .unwrap_or(0)
        + 1
}

/// A task title from `text`: its first line with any heading, list or checkbox marker taken off.
pub fn task_title(text: &str) -> Option<String> {
    text.lines().map(plain).find(|line| !line.is_empty()).map(|line| clip(line, 120))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_say_one_in_the_singular() {
        assert_eq!(counted(1, "line", "lines"), "1 line");
        assert_eq!(counted(0, "word", "words"), "0 words");
        assert_eq!(counted(2048, "character", "characters"), "2,048 characters");
    }

    #[test]
    fn lists_continue_and_end() {
        assert_eq!(list_enter("- milk", ""), Some(Enter::Continue("- ".into())));
        assert_eq!(list_enter("  * eggs", ""), Some(Enter::Continue("  * ".into())));
        assert_eq!(list_enter("9. nine", ""), Some(Enter::Continue("10. ".into())));
        assert_eq!(list_enter("3) three", ""), Some(Enter::Continue("4) ".into())));
        assert_eq!(list_enter("- [x] done", ""), Some(Enter::Continue("- [ ] ".into())));
        assert_eq!(list_enter("> quoted", ""), Some(Enter::Continue("> ".into())));
        assert_eq!(list_enter("- ", ""), Some(Enter::End));
        assert_eq!(list_enter("- [ ] ", ""), Some(Enter::End));
        assert_eq!(list_enter("- ", "tail"), Some(Enter::Continue("- ".into())));
        assert_eq!(list_enter("plain text", ""), None);
        assert_eq!(list_enter("---", ""), None);
        assert_eq!(list_enter("* * *", ""), None);
        assert_eq!(list_enter("- - -", ""), None);
        assert_eq!(list_enter("  -  -  - ", ""), None);
        assert_eq!(list_enter("- - ", ""), Some(Enter::Continue("- ".into())));
        assert_eq!(list_enter("+ + +", ""), Some(Enter::Continue("+ ".into())));
        assert_eq!(list_enter("**bold**", ""), None);
        assert_eq!(list_enter("2024. was a year", ""), Some(Enter::Continue("2025. ".into())));
    }

    #[test]
    fn prefixes_toggle() {
        assert_eq!(toggle_lines(&["a", "", "b"], Prefix::Bullet), ["- a", "", "- b"]);
        assert_eq!(toggle_lines(&["- a", "- b"], Prefix::Bullet), ["a", "b"]);
        assert_eq!(toggle_lines(&["- a", "b"], Prefix::Number), ["1. a", "2. b"]);
        assert_eq!(toggle_lines(&["1. a", "2. b"], Prefix::Number), ["a", "b"]);
        assert_eq!(toggle_lines(&["- [x] a"], Prefix::Check), ["a"]);
        assert_eq!(toggle_lines(&["- a"], Prefix::Check), ["- [ ] a"]);
        assert_eq!(toggle_lines(&["Title"], Prefix::Heading(2)), ["## Title"]);
        assert_eq!(toggle_lines(&["# Title"], Prefix::Heading(2)), ["## Title"]);
        assert_eq!(toggle_lines(&["## Title"], Prefix::Heading(2)), ["Title"]);
        assert_eq!(toggle_lines(&["  ## Title"], Prefix::Heading(2)), ["Title"]);
        assert_eq!(toggle_lines(&["   # Title"], Prefix::Heading(2)), ["## Title"]);
        assert_eq!(toggle_lines(&["a", "> b"], Prefix::Quote), ["> a", "> > b"]);
        assert_eq!(toggle_lines(&["> a", "> b"], Prefix::Quote), ["a", "b"]);
        assert_eq!(toggle_lines(&[""], Prefix::Bullet), ["- "]);
        assert_eq!(toggle_lines(&["  - a"], Prefix::Number), ["  1. a"]);
    }

    #[test]
    fn inline_markers_toggle() {
        assert_eq!(inline_toggle("a *", "it", "* b", "*", "*"), Inline::Outside);
        assert_eq!(inline_toggle("a ", "*it*", " b", "*", "*"), Inline::Inside);
        assert_eq!(inline_toggle("a ", "it", " b", "*", "*"), Inline::Wrap);
        // Italic on bold adds to it; italic on bold italic leaves the bold.
        assert_eq!(inline_toggle("**", "bold", "**", "*", "*"), Inline::Wrap);
        assert_eq!(inline_toggle("", "**bold**", "", "*", "*"), Inline::Wrap);
        assert_eq!(inline_toggle("***", "both", "***", "*", "*"), Inline::Outside);
        assert_eq!(inline_toggle("", "***both***", "", "*", "*"), Inline::Inside);
        assert_eq!(inline_toggle("**", "bold", "**", "**", "**"), Inline::Outside);
        assert_eq!(inline_toggle("***", "both", "***", "**", "**"), Inline::Outside);
        assert_eq!(inline_toggle("", "**bold**", "", "**", "**"), Inline::Inside);
        assert_eq!(inline_toggle("*", "it", "*", "**", "**"), Inline::Wrap);
        assert_eq!(inline_toggle("", "*", "", "*", "*"), Inline::Wrap);
        assert_eq!(inline_toggle("[", "x", "](url)", "[", "](url)"), Inline::Outside);
    }

    #[test]
    fn goto_parses_line_and_column() {
        assert_eq!(parse_goto("12"), Some((12, None)));
        assert_eq!(parse_goto(" 12:5 "), Some((12, Some(5))));
        assert_eq!(parse_goto("3,4"), Some((3, Some(4))));
        assert_eq!(parse_goto("3 4"), Some((3, Some(4))));
        assert_eq!(parse_goto("0"), None);
        assert_eq!(parse_goto("x"), None);
        assert_eq!(parse_goto("1:2:3"), None);
        assert_eq!(parse_goto(""), None);
    }

    #[test]
    fn stats_titles_and_zoom() {
        assert_eq!(stats(""), (1, 0, 0));
        assert_eq!(stats("one two\nthree\n"), (3, 3, 14));
        assert_eq!(thousands(1234567), "1,234,567");
        assert_eq!(thousands(999), "999");
        assert_eq!(display_title("  Plan ", "body"), "Plan");
        assert_eq!(display_title("", "\n# Groceries\n- milk"), "Groceries");
        assert_eq!(display_title("", ""), "Untitled");
        assert_eq!(copy_title(" Plan "), Some("Plan (copy)".into()));
        assert_eq!(copy_title("  "), None);
        assert_eq!(preview("", "# Groceries\n- milk"), "milk");
        assert_eq!(preview("Plan", "first line"), "first line");
        assert_eq!(zoom_step(100, 1), 110);
        assert_eq!(zoom_step(100, -1), 90);
        assert_eq!(zoom_step(300, 1), 300);
        assert_eq!(zoom_step(105, -1), 100);
        assert_eq!(zoom_notches(0.0, -1.0, false), (1, 0.0));
        assert_eq!(zoom_notches(0.0, 1.0, false), (-1, 0.0));
        assert_eq!(zoom_notches(0.0, 0.0, false), (0, 0.0));
        assert_eq!(zoom_notches(0.0, -0.25, false), (0, -0.25));
        assert_eq!(zoom_notches(-0.875, -0.125, false), (1, 0.0));
        assert_eq!(zoom_notches(0.0, 12.0, true), (0, 0.5));
        assert_eq!(zoom_notches(0.5, 12.0, true), (-1, 0.0));
        assert_eq!(clean("plain"), "plain");
        assert_eq!(clean("a\0b"), "a\u{FFFD}b");
    }

    #[test]
    fn checklist_images_and_task_titles() {
        assert_eq!(check_item("- [ ] milk"), Some((2, false)));
        assert_eq!(check_item("  * [x] eggs"), Some((4, true)));
        assert_eq!(check_item("- milk"), None);
        assert_eq!(
            image_ref("see ![Image 2](/tmp/a.png) here"),
            Some(("Image 2".into(), "/tmp/a.png".into(), 4, 26))
        );
        assert_eq!(
            image_ref("![shot](</tmp/my shot (1).png>)"),
            Some(("shot".into(), "/tmp/my shot (1).png".into(), 0, 31))
        );
        assert_eq!(image_ref("![x]()"), None);
        assert_eq!(image_ref("[link](url)"), None);
        assert!(only_images("![Image 1](/a.png)\n\n  ![Image 2](/b.png) "));
        assert!(!only_images("Notes on it\n![Image 1](/a.png)"));
        assert!(!only_images("see ![Image 1](/a.png) here"));
        assert!(!only_images("  \n"));
        assert_eq!(image_markdown("Image 1", "/a b.png"), "![Image 1](</a b.png>)");
        assert_eq!(image_markdown("Image 1", "/a.png"), "![Image 1](/a.png)");
        let markdown = image_markdown("Image 3", "/a (b).png");
        assert_eq!(image_ref(&markdown).map(|r| r.1), Some("/a (b).png".into()));
        assert_eq!(next_image_number("![Image 1](/a.png)\n![Image 4](/b.png)\n![logo](/c.png)"), 5);
        assert_eq!(next_image_number("no images"), 1);
        assert_eq!(task_title("\n## Ship it\nmore").as_deref(), Some("Ship it"));
        assert_eq!(task_title("- [ ] call Sam").as_deref(), Some("call Sam"));
        assert_eq!(task_title("  \n "), None);
    }
}
