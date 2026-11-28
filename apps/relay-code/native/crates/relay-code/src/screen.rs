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

const SPINNERS: &[char] = &['·', '✢', '✳', '✶', '✻', '✽', '*', '∗', '⏺', '●'];
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

/// `❯ 1. Yes`, `  2) No`: an answer's number and words.
fn option(line: &str) -> Option<(String, String)> {
    let t = line.trim_start();
    let t = t.strip_prefix(['❯', '›', '>']).map_or(t, str::trim_start);
    let mut cs = t.char_indices();
    let (_, d) = cs.next()?;
    if !d.is_ascii_digit() {
        return None;
    }
    let (i, sep) = cs.next()?;
    if sep != '.' && sep != ')' {
        return None;
    }
    let label = t[i + 1..].trim();
    if label.is_empty() || !t[i + 1..].starts_with(' ') {
        return None;
    }
    Some((d.to_string(), label.to_string()))
}

fn ask(lines: &[&str]) -> Option<Ask> {
    let clean: Vec<String> = lines.iter().map(|l| l.chars().map(|c| if FRAME.contains(&c) { ' ' } else { c }).collect::<String>().trim_end().to_string()).collect();
    let first = clean.iter().position(|l| option(l).is_some())?;
    let mut options: Vec<(String, String)> = Vec::new();
    for l in &clean[first..] {
        if let Some(o) = option(l)
            && !options.iter().any(|(n, _)| *n == o.0)
        {
            options.push(o);
        }
    }
    if options.len() < 2 {
        return None;
    }
    let question: Vec<&str> = clean[..first].iter().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    // Line by line, as the terminal shows it: a tool's name, its command, then the question.
    let question = question[question.len().saturating_sub(4)..].join("\n");
    Some(Ask { question, options })
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
    fn output_with_numbers_is_not_a_status_or_a_question() {
        let s = read("● 1. read the file\n  ⏵⏵ accept edits on (shift+tab to cycle)");
        assert_eq!(s.status, None);
        assert_eq!(s.ask, None);
        assert_eq!(s.mode, Some(Mode::AcceptEdits));
    }
}
