//! Threads' reading of an agent's reply: Markdown cut into the blocks the thread page draws,
//! each block's text already Pango markup, and plain captions for the tools the agent called.
//!
//! Only what agents actually write is read: paragraphs, headings, bullet and numbered lists,
//! fenced code, tables, rules, and inline bold, italics, code and links. Anything else stays text.

/// One block of a reply. Text fields are Pango markup, escaped.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Paragraph(String),
    Heading { level: u8, text: String },
    /// Each item's markup; `ordered` lists number from `start`.
    List { ordered: bool, start: u32, items: Vec<String> },
    /// Fenced code, as written: plain text, not markup.
    Code { lang: String, text: String },
    /// The header row first; every cell is markup.
    Table(Vec<Vec<String>>),
    Rule,
}

/// Escape `text` for Pango markup.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// Inline Markdown to Pango markup: `**bold**`, `*italic*` or `_italic_`, `` `code` ``,
/// `[label](https://…)`. Unclosed markers stay as written.
pub fn inline(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let find = |from: usize, pat: &[char]| -> Option<usize> {
        (from..chars.len().saturating_sub(pat.len() - 1)).find(|&j| chars[j..j + pat.len()] == *pat)
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '`' {
            if let Some(end) = find(i + 1, &['`']) {
                let code: String = chars[i + 1..end].iter().collect();
                out.push_str(&format!("<tt>{}</tt>", escape(&code)));
                i = end + 1;
                continue;
            }
        }
        if c == '*' && chars.get(i + 1) == Some(&'*') {
            if let Some(end) = find(i + 2, &['*', '*']).filter(|&e| e > i + 2) {
                let inner: String = chars[i + 2..end].iter().collect();
                out.push_str(&format!("<b>{}</b>", inline(&inner)));
                i = end + 2;
                continue;
            }
        }
        if (c == '*' || c == '_') && chars.get(i + 1).is_some_and(|n| !n.is_whitespace()) {
            // `_` inside a word (snake_case) is not emphasis.
            let word_before = i > 0 && chars[i - 1].is_alphanumeric();
            if !(c == '_' && word_before) {
                if let Some(end) = find(i + 1, &[c]).filter(|&e| e > i + 1 && !chars[e - 1].is_whitespace()) {
                    let word_after = chars.get(end + 1).is_some_and(|n| n.is_alphanumeric());
                    if !(c == '_' && word_after) {
                        let inner: String = chars[i + 1..end].iter().collect();
                        out.push_str(&format!("<i>{}</i>", inline(&inner)));
                        i = end + 1;
                        continue;
                    }
                }
            }
        }
        if c == '[' {
            if let Some(close) = find(i + 1, &[']', '(']) {
                if let Some(end) = find(close + 2, &[')']) {
                    let label: String = chars[i + 1..close].iter().collect();
                    let url: String = chars[close + 2..end].iter().collect();
                    if url.starts_with("https://") || url.starts_with("http://") {
                        out.push_str(&format!("<a href=\"{}\">{}</a>", escape(&url), inline(&label)));
                        i = end + 1;
                        continue;
                    }
                }
            }
        }
        out.push_str(&escape(&c.to_string()));
        i += 1;
    }
    out
}

fn bullet(line: &str) -> Option<&str> {
    let t = line.trim_start();
    ["- ", "* ", "+ ", "• "].iter().find_map(|m| t.strip_prefix(m))
}

fn numbered(line: &str) -> Option<(u32, &str)> {
    let t = line.trim_start();
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 || digits > 4 {
        return None;
    }
    let rest = &t[digits..];
    let rest = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") "))?;
    Some((t[..digits].parse().ok()?, rest))
}

fn cells(line: &str) -> Vec<String> {
    let t = line.trim().trim_start_matches('|').trim_end_matches('|');
    t.split('|').map(|c| c.trim().to_string()).collect()
}

fn is_table_rule(line: &str) -> bool {
    let t = line.trim();
    t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

/// A reply's Markdown as blocks.
pub fn markdown(text: &str) -> Vec<Block> {
    let lines: Vec<&str> = text.lines().collect();
    let mut blocks = Vec::new();
    let mut paragraph: Vec<&str> = Vec::new();
    let flush = |paragraph: &mut Vec<&str>, blocks: &mut Vec<Block>| {
        if !paragraph.is_empty() {
            let joined = paragraph.iter().map(|l| l.trim()).collect::<Vec<_>>().join("\n");
            blocks.push(Block::Paragraph(inline(&joined)));
            paragraph.clear();
        }
    };
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        if let Some(fence) = trimmed.strip_prefix("```") {
            flush(&mut paragraph, &mut blocks);
            let lang = fence.trim().to_string();
            let mut code = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                code.push(lines[i]);
                i += 1;
            }
            blocks.push(Block::Code { lang, text: code.join("\n") });
            i += 1;
            continue;
        }
        if trimmed.is_empty() {
            flush(&mut paragraph, &mut blocks);
            i += 1;
            continue;
        }
        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|c| *c == '#').count();
            if level <= 6 && trimmed[level..].starts_with(' ') {
                flush(&mut paragraph, &mut blocks);
                blocks.push(Block::Heading { level: level as u8, text: inline(trimmed[level..].trim()) });
                i += 1;
                continue;
            }
        }
        if matches!(trimmed, "---" | "***" | "___") {
            flush(&mut paragraph, &mut blocks);
            blocks.push(Block::Rule);
            i += 1;
            continue;
        }
        if trimmed.starts_with('|') && lines.get(i + 1).is_some_and(|next| is_table_rule(next)) {
            flush(&mut paragraph, &mut blocks);
            let mut rows = vec![cells(line).iter().map(|c| inline(c)).collect::<Vec<_>>()];
            i += 2;
            while i < lines.len() && lines[i].trim().starts_with('|') {
                rows.push(cells(lines[i]).iter().map(|c| inline(c)).collect());
                i += 1;
            }
            blocks.push(Block::Table(rows));
            continue;
        }
        if bullet(line).is_some() || numbered(line).is_some() {
            flush(&mut paragraph, &mut blocks);
            let ordered = numbered(line).is_some();
            let start = numbered(line).map_or(1, |(n, _)| n);
            let mut items: Vec<String> = Vec::new();
            while i < lines.len() {
                let l = lines[i];
                let item = if ordered { numbered(l).map(|(_, t)| t) } else { bullet(l) };
                match item {
                    Some(t) => items.push(t.trim().to_string()),
                    // A continuation line, indented under the item.
                    None if !l.trim().is_empty() && l.starts_with("  ") && !items.is_empty() => {
                        let last = items.last_mut().expect("not empty");
                        last.push(' ');
                        last.push_str(l.trim());
                    }
                    None => break,
                }
                i += 1;
            }
            blocks.push(Block::List { ordered, start, items: items.iter().map(|t| inline(t)).collect() });
            continue;
        }
        paragraph.push(line);
        i += 1;
    }
    flush(&mut paragraph, &mut blocks);
    blocks
}

/// The op an agent's tool call ran: `mcp__relay__money_tx_add` is `money.tx.add`.
pub fn tool_op(name: &str) -> Option<String> {
    let rest = name.strip_prefix("mcp__relay__")?;
    Some(rest.replace('_', "."))
}

/// What a tool call did, in words: "Read the budget", "Added an entry".
pub fn tool_caption(name: &str) -> String {
    let op = tool_op(name).unwrap_or_default();
    match op.as_str() {
        "money.summary" => "Read the budget".into(),
        "money.lists" => "Read accounts and categories".into(),
        "money.tx.list" => "Read entries".into(),
        "money.series" => "Added up spending".into(),
        "money.tx.add" => "Added an entry".into(),
        "money.tx.update" => "Changed an entry".into(),
        "money.tx.restore" => "Restored an entry".into(),
        "money.invest.summary" => "Read your investments".into(),
        "money.invest.list" => "Listed investment activity".into(),
        "money.invest.add" => "Recorded an investment activity".into(),
        "thread.propose" => "Asked you first".into(),
        "gym.summary" => "Read your training".into(),
        "gym.sessions" => "Listed workouts".into(),
        "gym.session.get" => "Read a workout".into(),
        "gym.lifts" => "Read your lifts".into(),
        "gym.lift.get" => "Read a lift's history".into(),
        "gym.cardio" => "Read cardio".into(),
        "gym.series" => "Added up training".into(),
        "bus.schema" => "Checked how a tool works".into(),
        "" => format!("Used {name}"),
        other => format!("Used {other}"),
    }
}

/// Whether a tool call changed the ledger, so its card offers Undo.
pub fn tool_writes(name: &str) -> bool {
    matches!(tool_op(name).as_deref(), Some("money.tx.add" | "money.tx.update" | "money.tx.restore" | "money.invest.add"))
}

/// An ```` ```import ```` block: the agent asking for a file (docs/INVESTMENTS.md, "The thread agent").
/// The card it draws lets the person choose the file, previews it and imports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSpec {
    /// Where the file comes from: only `wealthsimple` today.
    pub source: String,
    /// `holdings`, `activities` or `statement`.
    pub expects: String,
    pub title: String,
}

/// Read an import block. `None` when it is not one Relay can act on, so it shows as code instead.
pub fn import_spec(text: &str) -> Option<ImportSpec> {
    let v: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
    let source = v["source"].as_str()?.trim().to_ascii_lowercase();
    let expects = v["expects"].as_str()?.trim().to_ascii_lowercase();
    if source != "wealthsimple" || !matches!(expects.as_str(), "holdings" | "activities" | "statement") {
        return None;
    }
    Some(ImportSpec { source, expects, title: v["title"].as_str().unwrap_or("").trim().to_string() })
}

/// How a chart is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartKind {
    Bar,
    Line,
    Donut,
}

/// A ```` ```chart ```` block: what to draw and the question its numbers come from (`money.series`,
/// or `gym.series` for Avex's history), or, failing a question, numbers written in it.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartSpec {
    pub kind: ChartKind,
    pub title: String,
    /// A `money.series` payload.
    pub query: Option<serde_json::Value>,
    /// A `gym.series` payload: Avex's training history instead of the ledger.
    pub gym: Option<serde_json::Value>,
    /// `{"labels", "series": [{"name", "values"}]}`, minor units, when there is no query.
    pub data: Option<serde_json::Value>,
}

/// Read a chart block. `None` when it is not one Relay can draw, so it shows as code instead.
pub fn chart_spec(text: &str) -> Option<ChartSpec> {
    let v: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
    let kind = match v["type"].as_str().unwrap_or("bar") {
        "bar" | "column" => ChartKind::Bar,
        "line" | "area" => ChartKind::Line,
        "donut" | "pie" => ChartKind::Donut,
        _ => return None,
    };
    let query = v.get("query").filter(|q| q.is_object()).cloned();
    let gym = v.get("gym").filter(|q| q.is_object()).cloned();
    let data = v.get("data").filter(|d| d["labels"].is_array() && d["series"].is_array()).cloned();
    if query.is_none() && gym.is_none() && data.is_none() {
        return None;
    }
    Some(ChartSpec { kind, title: v["title"].as_str().unwrap_or("").to_string(), query, gym, data })
}

/// Round axis steps for values up to `max` (minor units, `digits` fraction digits): the step, and
/// how many steps reach `max`. Steps are 1, 2, 2.5 or 5 times a power of ten in major units.
pub fn ticks(max: i64, digits: u32) -> (i64, u32) {
    let unit = 10_i64.pow(digits);
    let max = max.max(1);
    let rough = max as f64 / 3.0 / unit as f64;
    let power = 10_f64.powf(rough.max(1e-9).log10().floor());
    // A step must be a whole number of minor units: 2.5 yen is not one.
    let whole = |s: &f64| ((s * unit as f64) - (s * unit as f64).round()).abs() < 1e-6;
    let step = [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|m| m * power).find(|s| *s >= rough && whole(s)).unwrap_or(10.0 * power);
    let step = ((step * unit as f64).round() as i64).max(1);
    let count = ((max + step - 1) / step).max(1) as u32;
    (step, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_marks_become_pango_and_text_is_escaped() {
        assert_eq!(inline("You spent **$520** & *more*"), "You spent <b>$520</b> &amp; <i>more</i>");
        assert_eq!(inline("run `a<b`"), "run <tt>a&lt;b</tt>");
        assert_eq!(inline("money_tx_add stays"), "money_tx_add stays");
        assert_eq!(inline("a * b * c"), "a * b * c");
        assert_eq!(inline("**unclosed"), "**unclosed");
        assert_eq!(inline("[Tally](https://x.y/z?a=1&b=2)"), "<a href=\"https://x.y/z?a=1&amp;b=2\">Tally</a>");
        assert_eq!(inline("[no](javascript:alert)"), "[no](javascript:alert)");
    }

    #[test]
    fn blocks_are_read_as_agents_write_them() {
        let reply = "## September\n\nYou spent **$520**.\nOver by $20.\n\n- Groceries: $520\n- Dining: $96\n  out twice\n\n1. Cut takeout\n2. Shop once\n\n| Week | Spent |\n|---|---:|\n| W1 | $88 |\n\n```json\n{\"a\": 1}\n```\n---";
        let blocks = markdown(reply);
        assert_eq!(blocks[0], Block::Heading { level: 2, text: "September".into() });
        assert_eq!(blocks[1], Block::Paragraph("You spent <b>$520</b>.\nOver by $20.".into()));
        assert_eq!(blocks[2], Block::List { ordered: false, start: 1, items: vec!["Groceries: $520".into(), "Dining: $96 out twice".into()] });
        assert_eq!(blocks[3], Block::List { ordered: true, start: 1, items: vec!["Cut takeout".into(), "Shop once".into()] });
        assert_eq!(blocks[4], Block::Table(vec![vec!["Week".into(), "Spent".into()], vec!["W1".into(), "$88".into()]]));
        assert_eq!(blocks[5], Block::Code { lang: "json".into(), text: "{\"a\": 1}".into() });
        assert_eq!(blocks[6], Block::Rule);
        assert_eq!(blocks.len(), 7);
    }

    #[test]
    fn chart_blocks_carry_their_question() {
        let spec = chart_spec(r#"{"type":"line","title":"Pace","query":{"by":"day","cumulative":true}}"#).unwrap();
        assert_eq!(spec.kind, ChartKind::Line);
        assert_eq!(spec.query.unwrap()["by"], "day");
        let fixed = chart_spec(r#"{"type":"pie","data":{"labels":["A"],"series":[{"name":"x","values":[5]}]}}"#).unwrap();
        assert_eq!(fixed.kind, ChartKind::Donut);
        let lift = chart_spec(r#"{"type":"line","title":"Bench","gym":{"measure":"e1rm","lift":"bench"}}"#).unwrap();
        assert_eq!(lift.gym.unwrap()["lift"], "bench");
        assert!(lift.query.is_none());
        assert!(chart_spec(r#"{"type":"radar","query":{}}"#).is_none());
        assert!(chart_spec(r#"{"type":"bar"}"#).is_none(), "no question and no numbers");
        assert!(chart_spec("not json").is_none());
    }

    #[test]
    fn axis_steps_are_round_and_reach_the_top() {
        assert_eq!(ticks(14_100, 2), (5_000, 3));
        assert_eq!(ticks(52_000, 2), (20_000, 3));
        assert_eq!(ticks(250_000, 2), (100_000, 3));
        assert_eq!(ticks(7, 0), (5, 2));
        let (step, count) = ticks(0, 2);
        assert!(step > 0 && count >= 1);
    }

    #[test]
    fn tool_calls_read_as_what_they_did() {
        assert_eq!(tool_op("mcp__relay__money_tx_add").as_deref(), Some("money.tx.add"));
        assert_eq!(tool_caption("mcp__relay__money_summary"), "Read the budget");
        assert_eq!(tool_caption("mcp__relay__money_tx_add"), "Added an entry");
        assert_eq!(tool_caption("mcp__relay__money_series"), "Added up spending");
        assert!(tool_writes("mcp__relay__money_tx_update"));
        assert!(!tool_writes("mcp__relay__money_tx_list"));
        assert_eq!(tool_caption("WebSearch"), "Used WebSearch");
        assert_eq!(tool_op("mcp__relay__money_invest_summary").as_deref(), Some("money.invest.summary"));
        assert_eq!(tool_caption("mcp__relay__money_invest_summary"), "Read your investments");
        assert_eq!(tool_caption("mcp__relay__money_invest_list"), "Listed investment activity");
        assert_eq!(tool_caption("mcp__relay__money_invest_add"), "Recorded an investment activity");
        assert_eq!(tool_caption("mcp__relay__gym_lift_get"), "Read a lift's history");
        assert!(tool_writes("mcp__relay__money_invest_add"));
        assert!(!tool_writes("mcp__relay__money_invest_summary") && !tool_writes("mcp__relay__money_invest_list"));
    }

    #[test]
    fn import_blocks_ask_for_a_wealthsimple_file() {
        let reply = "Download it, then choose it here:\n\n```import\n{\"source\": \"wealthsimple\", \"expects\": \"holdings\", \"title\": \"Your holdings report\"}\n```";
        let Block::Code { lang, text } = &markdown(reply)[1] else { panic!("a code block") };
        assert_eq!(lang, "import");
        let spec = import_spec(text).unwrap();
        assert_eq!(spec, ImportSpec { source: "wealthsimple".into(), expects: "holdings".into(), title: "Your holdings report".into() });
        let untitled = import_spec(r#"{"source": "Wealthsimple", "expects": "Activities"}"#).unwrap();
        assert_eq!((untitled.expects.as_str(), untitled.title.as_str()), ("activities", ""));
        assert!(import_spec(r#"{"source": "questrade", "expects": "holdings"}"#).is_none(), "only Wealthsimple is read");
        assert!(import_spec(r#"{"source": "wealthsimple", "expects": "passwords"}"#).is_none());
        assert!(import_spec(r#"{"source": "wealthsimple"}"#).is_none());
        assert!(import_spec("not json").is_none());
    }
}
