//! The names the canvas gives what it makes (client-text): a node's default name, and a copy's.
//!
//! Also what the canvas makes of names and words it is handed: a caption that is never blank
//! ([`display_name`], [`node_caption`]), a pasted text or a name cut without splitting a drawn
//! character ([`cut_text`]), a picture's file name as a layer name ([`layer_name`]), a layer's
//! name as a file name ([`file_stem`]), and the order names sort in ([`name_key`]). Nothing here
//! needs GTK, so `cargo test` covers what the layers, breadcrumb, inspector, Ask and export show.

use relay_bus::design::{strip_controls, Node, NodeKind, MAX_NAME};
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// The characters past which a cut stops looking for the start of what is drawn as one: a
/// letter carrying more marks than this is cut among them rather than lost whole.
const MAX_CLUSTER: usize = 32;

/// Whether `c` draws nothing: Unicode's default-ignorable characters (zero-width spaces and
/// joiners, the byte order mark, soft hyphens, bidirectional controls, variation selectors,
/// tags), and control characters.
fn invisible(c: char) -> bool {
    c.is_control()
        || matches!(c,
            '\u{00AD}' | '\u{034F}' | '\u{061C}' | '\u{115F}' | '\u{1160}' | '\u{17B4}' | '\u{17B5}'
            | '\u{180B}'..='\u{180F}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}' | '\u{3164}' | '\u{FE00}'..='\u{FE0F}' | '\u{FEFF}' | '\u{FFA0}'
            | '\u{FFF0}'..='\u{FFF8}' | '\u{1BCA0}'..='\u{1BCA3}' | '\u{1D173}'..='\u{1D17A}'
            | '\u{E0000}'..='\u{E0FFF}')
}

/// Whether `s` shows anything: a name or a line of only spaces or zero-width characters
/// (`"\u{200B}"`, which an agent or a paste can write) is no name.
pub fn visible(s: &str) -> bool {
    s.chars().any(|c| !c.is_whitespace() && !invisible(c))
}

/// `s` without the spaces and invisible characters at its start and the spaces at its end. An
/// invisible character at the end can belong to the letter before it (an emoji's variation
/// selector, a flag's tags), so it stays.
fn trimmed(s: &str) -> &str {
    s.trim_start_matches(|c: char| c.is_whitespace() || invisible(c)).trim_end()
}

/// A text's caption: its first line that shows anything, trimmed, cut to `max` characters with
/// "…" when longer. `None` when no line shows anything.
pub fn text_label(text: &str, max: usize) -> Option<String> {
    let line = trimmed(text.lines().find(|l| visible(l))?);
    let (mut label, cut) = cut_text(line, max);
    if cut {
        label.truncate(label.trim_end().len());
        label.push('…');
    }
    Some(label)
}

/// A node's name as the layers, the breadcrumb, the inspector, Ask and export show it, never
/// blank: its name when that shows anything, else its text's first line (40 characters), else
/// what it is ([`node_caption`]).
pub fn display_name(n: &Node) -> String {
    if let Some(name) = n.name.as_deref().filter(|s| visible(s)) {
        return trimmed(name).to_string();
    }
    if let Some(label) = n.text.as_deref().and_then(|t| text_label(t, 40)) {
        return label;
    }
    node_caption(n).to_string()
}

/// What a node of `kind` is called on the canvas ("Rectangle", "Drawing" for a pen path).
pub fn kind_caption(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Page => "Page",
        NodeKind::Frame => "Frame",
        NodeKind::Group => "Group",
        NodeKind::Rect => "Rectangle",
        NodeKind::Ellipse => "Ellipse",
        NodeKind::Line => "Line",
        NodeKind::Text => "Text",
        NodeKind::Sticky => "Sticky",
        NodeKind::Path => "Drawing",
        NodeKind::Image => "Image",
        NodeKind::Polygon => "Polygon",
        NodeKind::Icon => "Icon",
    }
}

/// What a node is called on the canvas, by what it draws: a line is a "Shape" when closed, a
/// "Vector" when smooth or bent (more than two points), an "Arrow" with an arrowhead, and every
/// other node its kind's caption.
pub fn node_caption(n: &Node) -> &'static str {
    if n.kind == NodeKind::Line {
        let on = |v: Option<bool>| v == Some(true);
        if on(n.closed) {
            return "Shape";
        }
        if on(n.smooth) || n.points.as_ref().is_some_and(|p| p.len() > 2) {
            return "Vector";
        }
        if on(n.arrow_end) || on(n.arrow_start) {
            return "Arrow";
        }
    }
    kind_caption(n.kind)
}

/// A regional indicator: two of them draw one flag.
fn regional(c: char) -> bool {
    ('\u{1F1E6}'..='\u{1F1FF}').contains(&c)
}

/// The Hangul jamo classes a decomposed syllable is built of (leading, vowel, trailing), and the
/// precomposed syllables (with or without a trailing consonant).
enum Jamo {
    L,
    V,
    T,
    Lv,
    Lvt,
}

fn jamo(c: char) -> Option<Jamo> {
    Some(match c {
        '\u{1100}'..='\u{115F}' | '\u{A960}'..='\u{A97C}' => Jamo::L,
        '\u{1160}'..='\u{11A7}' | '\u{D7B0}'..='\u{D7C6}' => Jamo::V,
        '\u{11A8}'..='\u{11FF}' | '\u{D7CB}'..='\u{D7FB}' => Jamo::T,
        '\u{AC00}'..='\u{D7A3}' if (c as u32 - 0xAC00).is_multiple_of(28) => Jamo::Lv,
        '\u{AC00}'..='\u{D7A3}' => Jamo::Lvt,
        _ => return None,
    })
}

/// Whether `after` belongs to the character drawn with `before` (Unicode's grapheme cluster
/// rules, as far as a cut needs them): a mark, joiner, variation selector, skin tone or tag after
/// anything; anything after a zero-width joiner (an emoji sequence); the second letter of a flag
/// (`ris_before` regional indicators run up to `after`); CR LF; and Hangul jamo.
fn joined(before: char, after: char, ris_before: usize) -> bool {
    let extends = is_combining_mark(after)
        || matches!(after, '\u{200C}' | '\u{200D}' | '\u{FE00}'..='\u{FE0F}' | '\u{1F3FB}'..='\u{1F3FF}' | '\u{E0020}'..='\u{E007F}');
    let hangul = matches!(
        (jamo(before), jamo(after)),
        (Some(Jamo::L), Some(Jamo::L | Jamo::V | Jamo::Lv | Jamo::Lvt))
            | (Some(Jamo::Lv | Jamo::V), Some(Jamo::V | Jamo::T))
            | (Some(Jamo::Lvt | Jamo::T), Some(Jamo::T))
    );
    extends || hangul || before == '\u{200D}' || (before == '\r' && after == '\n') || (regional(after) && ris_before % 2 == 1)
}

/// The byte index at or before `at` (a character boundary of `text`) where a cut leaves every
/// drawn character whole. Past [`MAX_CLUSTER`] characters it gives up and cuts at `at`.
fn cluster_start(text: &str, at: usize) -> usize {
    let mut cut = at;
    for _ in 0..=MAX_CLUSTER {
        let (Some(before), Some(after)) = (text[..cut].chars().next_back(), text[cut..].chars().next()) else {
            return cut;
        };
        let ris = if regional(after) { text[..cut].chars().rev().take_while(|c| regional(*c)).count() } else { 0 };
        if !joined(before, after, ris) {
            return cut;
        }
        cut -= before.len_utf8();
    }
    at
}

/// `text` cut to at most `max` characters without splitting what is drawn as one character (an
/// emoji sequence, a letter and its accents, a flag), and whether anything was cut: a paste past
/// `MAX_TEXT` characters, or a name past `MAX_NAME`.
pub fn cut_text(text: &str, max: usize) -> (String, bool) {
    match text.char_indices().nth(max) {
        None => (text.to_string(), false),
        Some((at, _)) => (text[..cluster_start(text, at)].to_string(), true),
    }
}

/// A name as a file name's stem, before its suffix and extension: slashes, backslashes and
/// control characters (a text's line breaks) become "-", without spaces at either end or dots in
/// front (no hidden file), at most `max_bytes` long (a file name holds 255 bytes, not
/// characters) without splitting a drawn character; "Design" when nothing is left to show.
pub fn file_stem(name: &str, max_bytes: usize) -> String {
    let plain: String = name.trim().chars().map(|c| if matches!(c, '/' | '\\') || c.is_control() { '-' } else { c }).collect();
    let plain = plain.trim_start_matches('.').trim_start();
    let mut end = plain.len();
    if end > max_bytes {
        end = (0..=max_bytes).rev().find(|i| plain.is_char_boundary(*i)).unwrap_or(0);
        end = cluster_start(plain, end);
    }
    let stem = plain[..end].trim_end();
    if visible(stem) { stem.to_string() } else { "Design".into() }
}

/// A picture's file name (its stem) as the layer name of the image it is dropped as: composed
/// characters, no control characters (B43), trimmed, cut to `MAX_NAME` characters; `None` when
/// nothing is left to show. A longer name would refuse the whole drop.
pub fn layer_name(raw: &str) -> Option<String> {
    let composed: String = raw.nfc().collect();
    let plain = strip_controls(&composed, false);
    let (name, _) = cut_text(trimmed(&plain), MAX_NAME);
    let name = name.trim_end();
    visible(name).then(|| name.to_string())
}

/// The name of a copy: "Hero copy", the original cut so the whole stays within `MAX_NAME`
/// characters (as the engine names a duplicated design); "Copy" for a blank one.
pub fn copy_name(name: &str) -> String {
    let (kept, _) = cut_text(trimmed(name), MAX_NAME - " copy".len());
    match kept.trim_end() {
        "" => "Copy".into(),
        kept => format!("{kept} copy"),
    }
}

/// A name's sort key: its letters without accents, in lower case ("École" sorts with "ecole",
/// before "Zebra"), the same in any locale. Sort by `(name_key(s), s)` so names that differ only
/// in accents or case keep one order.
pub fn name_key(s: &str) -> String {
    s.nfd().filter(|c| !is_combining_mark(*c)).flat_map(char::to_lowercase).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::Scene;
    use relay_bus::design::{NodeOp, MAX_TEXT};

    fn named(kind: NodeKind, name: Option<&str>, text: Option<&str>) -> Node {
        let mut n = Node::new(kind);
        n.name = name.map(str::to_string);
        n.text = text.map(str::to_string);
        n
    }

    #[test]
    fn invisible_characters_are_not_a_name() {
        assert!(!visible(""));
        assert!(!visible(" \u{200B} "));
        assert!(!visible("\u{FEFF}\u{2060}\u{00AD}\t\n"));
        assert!(!visible("\u{1}"));
        assert!(visible("a"));
        assert!(visible("\u{200B}a"));
        assert!(visible("😀"));
    }

    #[test]
    fn a_text_label_is_its_first_visible_line() {
        assert_eq!(text_label("\n\n  Hello\nworld", 40).as_deref(), Some("Hello"));
        assert_eq!(text_label("\u{200B}\u{FEFF}", 40), None);
        assert_eq!(text_label("\n\u{200B}\nTitle", 40).as_deref(), Some("Title"));
        assert_eq!(text_label("\u{FEFF}Title  ", 40).as_deref(), Some("Title"));
        assert_eq!(text_label("", 40), None);
        // Cut at 40 characters, counted as characters, with an ellipsis.
        let long = "x".repeat(50);
        assert_eq!(text_label(&long, 40), Some(format!("{}…", "x".repeat(40))));
        assert_eq!(text_label(&"é".repeat(40), 40), Some("é".repeat(40)));
        assert_eq!(text_label(&"é".repeat(41), 40), Some(format!("{}…", "é".repeat(40))));
    }

    #[test]
    fn a_display_name_is_never_blank() {
        assert_eq!(display_name(&named(NodeKind::Rect, Some("  Card "), None)), "Card");
        assert_eq!(display_name(&named(NodeKind::Rect, Some("   "), None)), "Rectangle");
        assert_eq!(display_name(&named(NodeKind::Text, Some("\u{200B}"), Some("\nHello"))), "Hello");
        assert_eq!(display_name(&named(NodeKind::Text, None, Some("\u{200B}\u{200B}"))), "Text");
        assert_eq!(display_name(&named(NodeKind::Sticky, None, Some(" \n Note"))), "Note");
    }

    #[test]
    fn a_line_is_captioned_by_what_it_draws() {
        let mut line = Node::new(NodeKind::Line);
        line.points = Some(vec![[0.0, 0.0], [10.0, 0.0]]);
        assert_eq!(node_caption(&line), "Line");
        let arrow = Node { arrow_end: Some(true), ..line.clone() };
        assert_eq!(node_caption(&arrow), "Arrow");
        let back = Node { arrow_start: Some(true), ..line.clone() };
        assert_eq!(node_caption(&back), "Arrow");
        let smooth = Node { smooth: Some(true), ..line.clone() };
        assert_eq!(node_caption(&smooth), "Vector");
        let bent = Node { points: Some(vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]), ..line.clone() };
        assert_eq!(node_caption(&bent), "Vector");
        let shape = Node { closed: Some(true), ..bent.clone() };
        assert_eq!(node_caption(&shape), "Shape");
        // A closed smooth line with an arrow is still a shape: its outline is what is drawn.
        let all = Node { closed: Some(true), smooth: Some(true), arrow_end: Some(true), ..bent };
        assert_eq!(node_caption(&all), "Shape");
        // `false` is the same as left out.
        let off = Node { closed: Some(false), smooth: Some(false), arrow_end: Some(false), ..line };
        assert_eq!(node_caption(&off), "Line");
        // Every other kind reads its kind's caption, a path with `closed` too.
        let path = Node { closed: Some(true), ..Node::new(NodeKind::Path) };
        assert_eq!(node_caption(&path), "Drawing");
        for (kind, caption) in [
            (NodeKind::Page, "Page"),
            (NodeKind::Frame, "Frame"),
            (NodeKind::Group, "Group"),
            (NodeKind::Rect, "Rectangle"),
            (NodeKind::Ellipse, "Ellipse"),
            (NodeKind::Text, "Text"),
            (NodeKind::Sticky, "Sticky"),
            (NodeKind::Image, "Image"),
            (NodeKind::Polygon, "Polygon"),
            (NodeKind::Icon, "Icon"),
        ] {
            assert_eq!((node_caption(&Node::new(kind)), kind_caption(kind)), (caption, caption));
        }
    }

    #[test]
    fn a_cut_keeps_each_drawn_character_whole() {
        // A family emoji straddling the cut is left out whole, not cut after a joiner.
        let family = "👨\u{200D}👩\u{200D}👧";
        let (cut, was_cut) = cut_text(&format!("{}{family}", "a".repeat(9_998)), MAX_TEXT);
        assert!(was_cut);
        assert!(cut.ends_with('a') && cut.chars().count() == 9_998, "{:?}", cut.chars().last());
        // A letter keeps its accent.
        let (cut, _) = cut_text(&format!("{}e\u{301}x", "a".repeat(9_999)), MAX_TEXT);
        assert_eq!((cut.chars().count(), cut.chars().last()), (9_999, Some('a')));
        // A skin tone, a flag's two letters, a keycap and a CRLF stay together.
        assert_eq!(cut_text("ab👍🏽", 3), ("ab".to_string(), true));
        assert_eq!(cut_text("a🇫🇷🇩🇪", 4), ("a🇫🇷".to_string(), true));
        assert_eq!(cut_text("a🇫🇷🇩🇪", 3), ("a🇫🇷".to_string(), true));
        assert_eq!(cut_text("a1\u{FE0F}\u{20E3}", 3), ("a".to_string(), true));
        assert_eq!(cut_text("ab\r\nc", 3), ("ab".to_string(), true));
        // Decomposed Hangul: a syllable's jamo stay together.
        assert_eq!(cut_text("a\u{1100}\u{1161}\u{11A8}", 3), ("a".to_string(), true));
        // What fits comes back as it is.
        let whole = "a".repeat(MAX_TEXT);
        assert_eq!(cut_text(&whole, MAX_TEXT), (whole.clone(), false));
        assert_eq!(cut_text("", 0), (String::new(), false));
    }

    #[test]
    fn a_cut_never_passes_its_length_nor_eats_a_long_run_of_marks() {
        let samples = [
            "plain words".to_string(),
            "e\u{301}e\u{301}e\u{301}".to_string(),
            "👨\u{200D}👩\u{200D}👧👍🏽🇫🇷x".to_string(),
            format!("a{}", "\u{301}".repeat(100)),
        ];
        for text in &samples {
            for max in 0..text.chars().count() + 2 {
                let (cut, was_cut) = cut_text(text, max);
                assert!(cut.chars().count() <= max, "{text:?} at {max}");
                assert!(text.starts_with(&cut));
                assert_eq!(was_cut, cut.len() < text.len());
            }
        }
        // A hundred marks on one letter: the cut gives up on keeping them whole rather than
        // return nothing.
        let (cut, _) = cut_text(&samples[3], 50);
        assert_eq!(cut.chars().count(), 50);
    }

    #[test]
    fn a_file_stem_fits_the_filesystem() {
        let stem = file_stem(&"字".repeat(200), 200);
        assert!(stem.len() <= 200 && stem.chars().all(|c| c == '字'), "{} bytes", stem.len());
        assert!(format!("{stem}@2x.png").len() <= 255);
        assert_eq!(file_stem(&format!("{} selection", "字".repeat(200)), 200).len(), 198);
        assert_eq!(file_stem("a/b\\c\nd", 200), "a-b-c-d");
        assert_eq!(file_stem("", 200), "Design");
        assert_eq!(file_stem(" \t ", 200), "Design");
        assert_eq!(file_stem("\u{200B}", 200), "Design");
        assert_eq!(file_stem("...", 200), "Design");
        // A hidden file is never proposed.
        assert_eq!(file_stem(".hidden", 200), "hidden");
        assert_eq!(file_stem("  Hero card  ", 200), "Hero card");
        let ascii = "x".repeat(200);
        assert_eq!(file_stem(&ascii, 200), ascii);
        // An emoji sequence at the limit is left out whole.
        let stem = file_stem(&format!("{}👨\u{200D}👩", "a".repeat(196)), 200);
        assert_eq!(stem, "a".repeat(196));
    }

    #[test]
    fn a_long_file_stem_becomes_a_valid_layer_name() {
        let mut scene = Scene::new(vec![Node { id: "p1".into(), ..Node::new(NodeKind::Page) }]);
        for raw in ["é".repeat(210), "x".repeat(250), "e\u{301}".repeat(210)] {
            let name = layer_name(&raw).unwrap();
            assert_eq!(name.chars().count(), MAX_NAME, "{raw:?}");
            let mut n = Node::new(NodeKind::Image);
            n.id = format!("i{}", raw.len());
            n.parent = Some("p1".into());
            n.src = Some("0123456789abcdef.png".into());
            (n.w, n.h) = (40.0, 30.0);
            n.name = Some(name);
            assert_eq!(n.validate(), Ok(()));
            assert_eq!(scene.apply_all(&[NodeOp::Add { node: n, index: None }]), Ok(()));
        }
        // A stem with nothing to show is no name; control characters go (B43).
        assert_eq!(layer_name("  \u{200B} "), None);
        assert_eq!(layer_name(" photo\u{7}1 ").as_deref(), Some("photo1"));
        assert_eq!(layer_name("IMG_0042").as_deref(), Some("IMG_0042"));
    }

    #[test]
    fn a_copy_name_stays_within_the_name_limit() {
        assert_eq!(copy_name("Page 1"), "Page 1 copy");
        let long = copy_name(&"x".repeat(MAX_NAME));
        assert_eq!((long.chars().count(), long.ends_with("x copy")), (MAX_NAME, true));
        let wide = copy_name(&"字".repeat(MAX_NAME));
        assert!(wide.chars().count() <= MAX_NAME);
        assert_eq!(copy_name("  "), "Copy");
    }

    #[test]
    fn name_key_folds_accents_and_case() {
        let mut titles = vec!["Zebra", "École", "apple", "Été", "banana"];
        titles.sort_by_cached_key(|t| (name_key(t), t.to_string()));
        assert_eq!(titles, ["apple", "banana", "École", "Été", "Zebra"]);
        assert!(name_key("ä") < name_key("b"));
        assert_eq!(name_key("Crème Brûlée"), "creme brulee");
        // Composed and decomposed spellings sort as one.
        assert_eq!(name_key("e\u{301}cole"), name_key("\u{E9}cole"));
    }
}
