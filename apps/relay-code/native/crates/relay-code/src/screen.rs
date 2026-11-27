//! What an agent's terminal shows at its foot, read from the rendered rows the PC sends
//! (`session.screen`): Claude Code's live status line ("✻ Crunching… (1m 6s · ↓ 6.3k tokens ·
//! thought for 2s)"), its permission mode ("⏵⏵ auto mode on (shift+tab to cycle)"), and a question
//! it waits on ("Do you want to proceed?" with numbered answers). The conversation log has none of
//! these: they live only on the screen, so the chat reads them from it (the owner: "I have no idea
//! what mode it's on … it just says working").

/// The live status line.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Status {
    /// "Crunching…", as Claude Code words it this time.
    pub verb: String,
    /// What follows in brackets: "1m 6s", "↓ 6.3k tokens", "thought for 2s".
    pub details: Vec<String>,
}

/// Claude Code's permission mode, as its footer says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Asks before edits (the footer names no mode).
    Default,
    AcceptEdits,
    Auto,
    Plan,
    Bypass,
}

impl Mode {
    pub fn words(self) -> &'static str {
        match self {
            Mode::Default => "Asks first",
            Mode::AcceptEdits => "Accept edits",
            Mode::Auto => "Auto",
            Mode::Plan => "Plan",
            Mode::Bypass => "Bypass",
        }
    }
}

/// A question the agent waits on: its words and its numbered answers.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Ask {
    pub question: String,
    /// (the key that picks it, its words).
    pub options: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Screen {
    pub status: Option<Status>,
    pub mode: Option<Mode>,
    pub ask: Option<Ask>,
}

// Not `⏺` or `●`: those are Claude's message bullets ("● Done… mostly" is a reply, not a status).
const SPINNERS: &[char] = &['·', '✢', '✳', '✶', '✻', '✽', '*', '∗'];
const FRAME: &[char] = &['│', '┃', '╭', '╮', '╰', '╯', '─', '━', '┌', '┐', '└', '┘', '├', '┤'];

/// Read the rendered rows (plain text, top to bottom).
pub fn read(text: &str) -> Screen {
    let lines: Vec<&str> = text.lines().collect();
    Screen { status: status(&lines), mode: mode(&lines), ask: ask(&lines) }
}

fn status(lines: &[&str]) -> Option<Status> {
    // The newest status line is the lowest one.
    for l in lines.iter().rev() {
        let t = l.trim();
        let mut chars = t.chars();
        let Some(first) = chars.next() else { continue };
        if !SPINNERS.contains(&first) {
            continue;
        }
        let rest = chars.as_str().trim_start();
        let Some(dots) = rest.find('…') else { continue };
        let verb_end = dots + '…'.len_utf8();
        let verb = rest[..verb_end].trim();
        // A verb is one or two words ("Crunching…", "Thinking hard…"), never a sentence of output.
        if verb.is_empty() || verb.split_whitespace().count() > 3 || verb.chars().any(|c| c.is_ascii_digit()) {
            continue;
        }
        let after = rest[verb_end..].trim();
        let details = after
            .strip_prefix('(')
            .and_then(|s| s.split(')').next())
            .map(|inner| inner.split('·').map(|p| p.trim().to_string()).filter(|p| !p.is_empty() && !p.contains("esc to interrupt")).collect())
            .unwrap_or_default();
        return Some(Status { verb: verb.to_string(), details });
    }
    None
}

fn mode(lines: &[&str]) -> Option<Mode> {
    let foot = lines.iter().rev().filter(|l| !l.trim().is_empty()).take(4).map(|l| l.to_lowercase()).collect::<Vec<_>>().join("\n");
    if foot.contains("plan mode on") {
        Some(Mode::Plan)
    } else if foot.contains("bypass permissions on") {
        Some(Mode::Bypass)
    } else if foot.contains("auto mode on") {
        Some(Mode::Auto)
    } else if foot.contains("accept edits on") {
        Some(Mode::AcceptEdits)
    } else if foot.contains("shift+tab to cycle") || foot.contains("? for shortcuts") {
        Some(Mode::Default)
    } else {
        None
    }
}

/// The cursor Claude Code puts on the answer it would pick, and on its empty prompt.
const CURSORS: &[char] = &['❯', '›'];

/// One answer line: `❯ 1. Yes`, `  12) No`. Its number, its words, and whether the cursor is on it.
struct Opt {
    n: u32,
    label: String,
    cursor: bool,
    /// Where its number starts, in characters: words under an answer sit further in.
    indent: usize,
}

fn option(line: &str) -> Option<Opt> {
    let t = line.trim_start();
    let lead = line.chars().count() - t.chars().count();
    let (cursor, t) = match t.strip_prefix(CURSORS) {
        Some(rest) => (true, rest.trim_start()),
        None => (false, t),
    };
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 || digits > 2 {
        return None;
    }
    let rest = &t[digits..];
    let label = rest.strip_prefix(['.', ')'])?;
    if !label.starts_with(' ') || label.trim().is_empty() {
        return None;
    }
    let indent = line.chars().count() - t.chars().count();
    Some(Opt { n: t[..digits].parse().ok()?, label: label.trim().to_string(), cursor, indent: indent.max(lead) })
}

/// A line that is all frame (a card's border, or the prompt box's rule) or nothing.
fn bare(l: &str) -> bool {
    l.chars().all(|c| c.is_whitespace() || FRAME.contains(&c))
}

/// Claude Code's empty prompt (`❯ `): drawn only while no card is up.
fn prompt(l: &str) -> bool {
    let t = l.trim();
    t.chars().next().is_some_and(|c| CURSORS.contains(&c)) && t.chars().skip(1).all(char::is_whitespace)
}

/// A card the agent waits on, and only that: answers numbered 1, 2, 3… with the cursor on exactly
/// one of them, a question above, and nothing after them but the card's foot (its border, a hint
/// such as "Esc to cancel"). A reply that ends in a numbered list has no cursor in it and the empty
/// prompt under it, so it is never read as a question.
fn ask(lines: &[&str]) -> Option<Ask> {
    let clean: Vec<String> = lines.iter().map(|l| l.chars().map(|c| if FRAME.contains(&c) { ' ' } else { c }).collect::<String>().trim_end().to_string()).collect();
    // The last block of answers on screen, read upwards from its last answer.
    let last = clean.iter().rposition(|l| option(l).is_some())?;
    let mut options: Vec<Opt> = Vec::new();
    let mut first = last;
    for i in (0..=last).rev() {
        let l = &clean[i];
        if let Some(o) = option(l) {
            if options.last().is_some_and(|after| after.n != o.n + 1) {
                break;
            }
            first = i;
            options.push(o);
            if options.last().is_some_and(|o| o.n == 1) {
                break;
            }
        } else if l.trim().is_empty() || options.last().is_some_and(|o| l.chars().count() - l.trim_start().chars().count() > o.indent) {
            // A blank line, or an answer's description under it (further in).
            continue;
        } else {
            break;
        }
    }
    options.reverse();
    if options.len() < 2 || options[0].n != 1 || options.iter().filter(|o| o.cursor).count() != 1 {
        return None;
    }
    // Under the answers: the card's foot only. The empty prompt there means no card is up.
    let foot: Vec<&String> = clean[last + 1..].iter().filter(|l| !bare(l)).collect();
    if foot.len() > 3 || foot.iter().any(|l| prompt(l) || option(l).is_some()) {
        return None;
    }
    if lines[last + 1..].iter().any(|l| prompt(l)) {
        return None;
    }
    // Above them, the question: the words that end just before the first answer.
    let above: Vec<&str> = clean[..first].iter().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    if above.is_empty() {
        return None;
    }
    // Line by line, as the terminal shows it: a tool's name, its command, then the question.
    let question = above[above.len().saturating_sub(4)..].join("\n");
    Some(Ask { question, options: options.into_iter().map(|o| (o.n.to_string(), o.label)).collect() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_line_and_mode_read_as_the_terminal_shows_them() {
        let s = read("● Rebuilding the screen\n· Crunching… (3m 59s · ↓ 24.1k tokens · thought for 2s)\n──── bold-puffin ─\n❯ \n────\n  ⏵⏵ auto mode on (shift+tab to cycle) · esc to interrupt");
        let st = s.status.unwrap();
        assert_eq!(st.verb, "Crunching…");
        assert_eq!(st.details, ["3m 59s", "↓ 24.1k tokens", "thought for 2s"]);
        assert_eq!(s.mode, Some(Mode::Auto));
        assert_eq!(s.ask, None);
    }

    #[test]
    fn a_question_reads_with_its_answers() {
        let s = read("╭──────────╮\n│ Bash command │\n│   git push -u origin relay/x │\n│ Do you want to proceed? │\n│ ❯ 1. Yes │\n│   2. Yes, and don't ask again for git push │\n│   3. No, and tell Claude what to do differently (esc) │\n╰──────────╯");
        let a = s.ask.unwrap();
        assert_eq!(a.options.len(), 3);
        assert_eq!(a.options[0], ("1".into(), "Yes".into()));
        assert_eq!(a.question, "Bash command\ngit push -u origin relay/x\nDo you want to proceed?");
    }

    #[test]
    fn a_reply_ending_in_a_numbered_list_is_not_a_question() {
        let s = read("● Three things are left:\n  1. Fix the test\n  2. Push the branch\n  3. Open the PR\n\n──────────\n❯ \n──────────\n  ⏵⏵ auto mode on (shift+tab to cycle)");
        assert_eq!(s.ask, None);
        assert_eq!(s.status, None);
        // Even with a stray cursor in the words: the empty prompt under it says no card is up.
        let s = read("Pick one:\n❯ 1. Fix the test\n  2. Push\n────\n❯ \n────");
        assert_eq!(s.ask, None);
    }

    #[test]
    fn a_reply_bullet_is_not_a_status() {
        let s = read("● Done… mostly\n⏺ Done… mostly\n────\n❯ \n────\n  ? for shortcuts");
        assert_eq!(s.status, None);
        assert_eq!(s.mode, Some(Mode::Default));
    }

    #[test]
    fn a_long_menu_reads_every_answer() {
        let mut t = String::from("Which test should run?\n");
        for i in 1..=12 {
            t.push_str(&format!("{}{i}. suite {i}\n", if i == 3 { "❯ " } else { "  " }));
            t.push_str("     runs in about a minute\n");
        }
        t.push_str("\nEnter to select · ↑/↓ to navigate · Esc to cancel");
        let a = read(&t).ask.unwrap();
        assert_eq!(a.options.len(), 12);
        assert_eq!(a.options[9], ("10".into(), "suite 10".into()));
        assert_eq!(a.options[11], ("12".into(), "suite 12".into()));
        assert_eq!(a.question, "Which test should run?");
    }

    #[test]
    fn answers_need_the_cursor_and_a_question() {
        // No cursor on any answer.
        assert_eq!(read("Do you want to proceed?\n  1. Yes\n  2. No").ask, None);
        // No question above.
        assert_eq!(read("❯ 1. Yes\n  2. No").ask, None);
        // Not numbered from one.
        assert_eq!(read("Next?\n❯ 2. Yes\n  3. No").ask, None);
    }

    #[test]
    fn output_with_numbers_is_not_a_status_or_a_question() {
        let s = read("● 1. read the file\n  ⏵⏵ accept edits on (shift+tab to cycle)");
        assert_eq!(s.status, None);
        assert_eq!(s.ask, None);
        assert_eq!(s.mode, Some(Mode::AcceptEdits));
    }
}
