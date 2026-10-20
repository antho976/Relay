//! Text as the canvas edits it (client-text): what a run or a case change does to the characters
//! of a text, kept in step with the words.
//!
//! A node's `runs` count characters of its stored `text` (`start` to `end`, the end left out). The
//! painter draws the text cased (`text_case`), where one letter can become two (`ß` is `SS`), and
//! styles byte ranges of what it draws: [`shown`] and [`run_bytes`] map the one onto the other.
//! The text editor rewrites the words a keystroke at a time, and [`remap_runs`] carries the runs
//! across each rewrite, so a styled stretch keeps its letters.

use relay_bus::design::{Node, TextRun};

/// A node's text as it is drawn (`text_case` applied, else as stored), and for each of its stored
/// characters, and one past the last, the byte where that character starts in the drawn text.
/// No text is `("", [0])`.
pub fn shown(n: &Node) -> (String, Vec<usize>) {
    let text = n.text.as_deref().unwrap_or("");
    match n.text_case {
        Some(case) => case.apply_indexed(text),
        None => {
            let mut starts: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
            starts.push(text.len());
            (text.to_string(), starts)
        }
    }
}

/// Each of `n`'s runs as a byte range of its drawn text (`offsets` from [`shown`]), as the
/// painter's attributes want them. A range is clamped to the text, which the editor's preview can
/// shorten mid-edit, and a run with nothing of it left is left out.
pub fn run_bytes<'a>(n: &'a Node, offsets: &[usize]) -> Vec<(u32, u32, &'a TextRun)> {
    let Some(last) = offsets.len().checked_sub(1) else {
        return Vec::new();
    };
    let byte = |c: u32| offsets[(c as usize).min(last)] as u32;
    n.runs.iter().flatten().filter_map(|r| Some((byte(r.start), byte(r.end), r)).filter(|(s, e, _)| s < e)).collect()
}

/// `runs` of the text `old`, carried over to the text `new` (the editor's next state).
///
/// The change is the stretch between the words both texts start and end with. Text typed where
/// nothing was replaced takes the style of the character before it (the first character's at
/// the start), so typing at a run's end extends it; text that replaces characters takes the style
/// of the first one it replaces, so selecting everything and typing keeps the first character's
/// look. A run whose letters are all gone is dropped; the rest keep their order.
pub fn remap_runs(old: &str, new: &str, runs: &[TextRun]) -> Vec<TextRun> {
    if runs.is_empty() {
        return Vec::new();
    }
    let (old_len, new_len) = (old.chars().count(), new.chars().count());
    // The common prefix, then the common suffix of what is left (the two never overlap).
    let p = old.chars().zip(new.chars()).take_while(|(a, b)| a == b).count();
    let room = old_len.min(new_len) - p;
    let s = old.chars().rev().zip(new.chars().rev()).take(room).take_while(|(a, b)| a == b).count();
    // Old characters p..oe were replaced by new characters p..ne.
    let (oe, ne) = (old_len - s, new_len - s);
    let holding = |c: usize| runs.iter().position(|r| (r.start as usize) <= c && c < r.end as usize);
    let owner = match (oe > p, p) {
        (true, _) => holding(p),
        (false, 0) => holding(0),
        (false, _) => holding(p - 1),
    };
    let mapped = |b: u32, owned: bool, start: bool| -> u32 {
        let b = b as usize;
        let to = if b < p {
            b
        } else if b > oe {
            b - oe + ne
        } else if owned == start {
            // The owner's start, or another run's end, closes up on the left of the change.
            p
        } else {
            ne
        };
        to.min(new_len) as u32
    };
    runs.iter()
        .enumerate()
        .map(|(i, r)| {
            let owned = owner == Some(i);
            TextRun { start: mapped(r.start, owned, true), end: mapped(r.end, owned, false), ..r.clone() }
        })
        .filter(|r| r.start < r.end)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_bus::design::{NodeKind, TextCase};

    fn run(start: u32, end: u32, color: &str) -> TextRun {
        TextRun {
            start,
            end,
            color: Some(color.into()),
            font: None,
            font_size: None,
            font_weight: None,
            italic: None,
            underline: None,
            strike: None,
            rise: None,
        }
    }

    fn spans(runs: &[TextRun]) -> Vec<(u32, u32, &str)> {
        runs.iter().map(|r| (r.start, r.end, r.color.as_deref().unwrap_or(""))).collect()
    }

    fn text(words: &str, case: Option<TextCase>, runs: Vec<TextRun>) -> Node {
        let mut n = Node::new(NodeKind::Text);
        (n.id, n.parent) = ("t".into(), Some("p1".into()));
        n.text = Some(words.into());
        n.text_case = case;
        n.runs = Some(runs);
        n
    }

    #[test]
    fn shown_text_is_cased_and_indexed_by_character() {
        let (plain, at) = shown(&text("añb", None, vec![]));
        assert_eq!((plain.as_str(), at), ("añb", vec![0, 1, 3, 4]));
        let (upper, at) = shown(&text("straße", Some(TextCase::Upper), vec![]));
        assert_eq!((upper.as_str(), at), ("STRASSE", vec![0, 1, 2, 3, 4, 6, 7]));
        assert_eq!(shown(&Node::new(NodeKind::Rect)), (String::new(), vec![0]));
    }

    #[test]
    fn run_bytes_follow_the_cased_text() {
        // "ße" is characters 4 to 6, bytes 4 to 7 of "STRASSE".
        let n = text("straße", Some(TextCase::Upper), vec![run(0, 1, "#f00"), run(4, 6, "#0f0")]);
        let (drawn, at) = shown(&n);
        let bytes: Vec<(u32, u32)> = run_bytes(&n, &at).iter().map(|(s, e, _)| (*s, *e)).collect();
        assert_eq!(bytes, vec![(0, 1), (4, 7)]);
        assert_eq!(&drawn[4..7], "SSE");
        // Uncased, the same characters are their UTF-8 bytes.
        let n = text("straße", None, vec![run(4, 6, "#0f0")]);
        let (_, at) = shown(&n);
        assert_eq!(run_bytes(&n, &at).iter().map(|(s, e, _)| (*s, *e)).collect::<Vec<_>>(), vec![(4, 7)]);
    }

    #[test]
    fn a_run_past_a_shortened_text_is_clamped_or_dropped() {
        // The editor's preview holds three characters while the runs still name more.
        let n = text("abc", None, vec![run(1, 10, "#f00"), run(5, 8, "#0f0")]);
        let (_, at) = shown(&n);
        let bytes = run_bytes(&n, &at);
        assert_eq!(bytes.len(), 1);
        assert_eq!((bytes[0].0, bytes[0].1, bytes[0].2.color.as_deref()), (1, 3, Some("#f00")));
        assert!(run_bytes(&Node::new(NodeKind::Text), &[]).is_empty());
    }

    #[test]
    fn typing_inside_or_at_the_end_of_a_run_extends_it() {
        let runs = [run(2, 6, "#f00")];
        assert_eq!(spans(&remap_runs("abcdefgh", "abcdXefgh", &runs)), vec![(2, 7, "#f00")]);
        // Left-sticky: the run ends at 6, and "X" typed at 6 joins it.
        assert_eq!(spans(&remap_runs("abcdefgh", "abcdefXgh", &runs)), vec![(2, 7, "#f00")]);
    }

    #[test]
    fn typing_at_a_run_start_shifts_it() {
        let runs = [run(2, 4, "#f00")];
        assert_eq!(spans(&remap_runs("abcdef", "abXcdef", &runs)), vec![(3, 5, "#f00")]);
    }

    #[test]
    fn typing_at_the_start_takes_the_first_run() {
        assert_eq!(spans(&remap_runs("abc", "Xabc", &[run(0, 3, "#f00")])), vec![(0, 4, "#f00")]);
        // A run that does not start at 0 moves along.
        assert_eq!(spans(&remap_runs("abcd", "Xabcd", &[run(2, 4, "#f00")])), vec![(3, 5, "#f00")]);
    }

    #[test]
    fn replaced_figures_keep_the_unit_run() {
        // "8.4 km" with a small " km" becomes "12.1 km": the run still covers " km".
        let out = remap_runs("8.4 km", "12.1 km", &[run(3, 6, "#8C877F")]);
        assert_eq!(spans(&out), vec![(4, 7, "#8C877F")]);
        let words: Vec<char> = "12.1 km".chars().collect();
        assert_eq!(words[4..7].iter().collect::<String>(), " km");
    }

    #[test]
    fn deleting_across_two_runs_trims_both() {
        let runs = [run(0, 4, "#f00"), run(4, 8, "#00f")];
        assert_eq!(spans(&remap_runs("abcdefgh", "abgh", &runs)), vec![(0, 2, "#f00"), (2, 4, "#00f")]);
    }

    #[test]
    fn select_all_and_retype_keeps_the_first_characters_style() {
        let runs = [run(0, 3, "#f00"), run(3, 6, "#00f")];
        assert_eq!(spans(&remap_runs("abcdef", "xyz", &runs)), vec![(0, 3, "#f00")]);
        // Unstyled first character: the new words are plain.
        assert!(remap_runs("abcdef", "xyz", &[run(3, 6, "#00f")]).is_empty());
        // Clearing the text drops every run.
        assert!(remap_runs("abcdef", "", &runs).is_empty());
    }

    #[test]
    fn runs_count_characters_not_bytes() {
        // "é" and "😀" are one character each.
        let runs = [run(1, 2, "#f00")];
        assert_eq!(spans(&remap_runs("é😀z", "éé😀z", &runs)), vec![(2, 3, "#f00")]);
        assert_eq!(spans(&remap_runs("é😀z", "é😀😀z", &runs)), vec![(1, 3, "#f00")]);
    }

    #[test]
    fn remapped_runs_stay_inside_the_new_text_and_keep_their_style() {
        // A stale run past the old text is held to the new one.
        let mut bold = run(1, 9, "#f00");
        bold.font_weight = Some(700);
        let out = remap_runs("abc", "abcd", &[bold]);
        assert_eq!((out[0].start, out[0].end, out[0].font_weight), (1, 4, Some(700)));
        // Every remapped list validates against its new text.
        let runs = [run(0, 2, "#f00"), run(3, 5, "#0f0"), run(6, 8, "#00f")];
        for next in ["a", "abXcdefgh", "abgh", "Xabcdefgh", "abcdefghX", "zzz", "abcd", "efgh", "ab😀😀efgh"] {
            let kept = remap_runs("abcdefgh", next, &runs);
            let after = text(next, None, kept.clone());
            let after = Node { runs: (!kept.is_empty()).then_some(kept), ..after };
            assert_eq!(after.validate(), Ok(()), "{next:?}");
        }
    }
}
