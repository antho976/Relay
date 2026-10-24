//! The native board's pure logic, kept out of the GTK crate so a headless `cargo test` (and CI)
//! runs it: which tasks a filter and search keep, which group a card sits in and how groups
//! sort, the order a lane shows its cards in, and the final index a drop or a Shift+J/K nudge
//! hands `task.move`. Every function reads `task.list` rows as JSON, exactly as the board does.
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Least urgent first: the bus's priority values in the order a picker lists them.
pub const PRIORITIES: [&str; 4] = ["low", "medium", "high", "urgent"];
/// `""` first: a task need not have a size.
pub const SIZES: [&str; 4] = ["", "S", "M", "L"];
/// [`PRIORITIES`] most urgent first, the order the board groups and offers them in.
pub const URGENT_FIRST: [&str; 4] = {
    let mut urgent_first = PRIORITIES;
    let mut i = 0;
    while i < PRIORITIES.len() {
        urgent_first[i] = PRIORITIES[PRIORITIES.len() - 1 - i];
        i += 1;
    }
    urgent_first
};

/// The board's filters: per key, the values any one of which a task must have.
pub type Filters = BTreeMap<String, BTreeSet<String>>;

fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Zone {
    Before,
    Into,
    After,
}
/// Where a drop lands on a card: its top and bottom 30% insert beside it, the middle nests.
pub fn zone(y: f64, height: f64) -> Zone {
    if height <= 0. || y < height * 0.3 {
        Zone::Before
    } else if y > height * 0.7 {
        Zone::After
    } else {
        Zone::Into
    }
}

/// The index `moving` should end at when dropped beside `onto`, in a column's board order
/// (`task.move` positions are final indices among the column's other tasks).
pub fn drop_index(order: &[i64], moving: i64, onto: i64, after: bool) -> usize {
    let others: Vec<_> = order.iter().filter(|id| **id != moving).collect();
    others
        .iter()
        .position(|id| **id == onto)
        .map(|at| at + after as usize)
        .unwrap_or(others.len())
}

/// Every task of `column` in board order, filters and grouping ignored: the order `task.move`
/// positions index.
pub fn column_order(tasks: &[Value], column: &str) -> Vec<i64> {
    let mut order: Vec<_> = tasks
        .iter()
        .filter(|t| text(t, "column") == column)
        .map(|t| (t["position"].as_i64().unwrap_or(0), t["id"].as_i64().unwrap_or(0)))
        .collect();
    order.sort();
    order.into_iter().map(|(_, id)| id).collect()
}

pub fn task_matches(task: &Value, filters: &Filters, query: &str) -> bool {
    filters.iter().all(|(key, values)| {
        values.is_empty()
            || values.iter().any(|value| match key.as_str() {
                "parent" if value == "roots" => task["parent_id"].is_null(),
                "parent" | "module" => task[if key == "parent" { "parent_id" } else { "module_id" }]
                    .as_i64()
                    .is_some_and(|id| id.to_string() == *value),
                "label" | "session" => task[if key == "label" { "labels" } else { "sessions" }]
                    .as_array()
                    .is_some_and(|list| list.iter().any(|v| v.as_str() == Some(value))),
                key => text(task, key) == value,
            })
    }) && (query.is_empty() || haystack(task).contains(query))
}
/// The tasks blocking `task` that are not done yet, looked up in `all` (every task by id).
pub fn open_blockers<'a>(task: &Value, all: &BTreeMap<i64, &'a Value>) -> Vec<&'a Value> {
    task["blocked_by"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|other| other.as_i64().and_then(|other| all.get(&other)).copied())
        .filter(|t| text(t, "column") != "done")
        .collect()
}
/// Blocked as the board's Blocked tile counts it and a card's "Blocked by #n" says it: open,
/// with a blocker that is not done. The board marks such a task `_blocked` for `is:blocked`.
pub fn is_blocked(task: &Value, all: &BTreeMap<i64, &Value>) -> bool {
    text(task, "column") != "done" && !open_blockers(task, all).is_empty()
}

/// The search box's own language, as GitHub's "filter by keyword or by field": `key:value`
/// terms narrow by a field (several values comma-separated, any one matching), every other
/// word must appear in the task's [`haystack`]. Keys: `status`/`is`, `priority`, `size`,
/// `type`, `label`, `module`, `agent` and `state` (the execution state, so `state:blocked` is an
/// agent's report); `is:blocked` reads the `_blocked` mark of [`is_blocked`], and
/// `no:size`/`no:module`/`no:label` test for absence. Matching ignores case; an unknown key reads
/// as a plain word.
pub fn query_matches(task: &Value, query: &str) -> bool {
    let hay = haystack(task);
    query.split_whitespace().all(|term| {
        let term = term.to_lowercase();
        let Some((key, values)) = term.split_once(':').filter(|(k, v)| !k.is_empty() && !v.is_empty()) else {
            // A task key as the board shows it (`REL-12`) finds that task, whatever the prefix.
            if let Some(id) = key_number(&term) {
                return task["id"].as_i64() == Some(id) || hay.contains(&term);
            }
            return hay.contains(&term);
        };
        let values: Vec<&str> = values.split(',').filter(|v| !v.is_empty()).collect();
        let field = |name: &str| text(task, name).to_lowercase();
        let listed = |name: &str, value: &str| {
            task[name].as_array().is_some_and(|list| list.iter().filter_map(Value::as_str).any(|v| v.to_lowercase() == value))
        };
        values.iter().any(|value| match key {
            // What the Blocked tile counts, not an agent's report (that is `state:blocked`).
            "status" | "column" | "is" if *value == "blocked" => task["_blocked"].as_bool() == Some(true),
            "status" | "column" | "is" => {
                let column = field("column");
                column == *value || column_title(&column).to_lowercase().replace(' ', "") == value.replace(['_', '-'], "")
                    || (*value == "open" && column != "done") || (*value == "closed" && column == "done")
            }
            "priority" | "p" => field("priority") == *value,
            "size" => field("size") == *value,
            "type" => field("type") == *value,
            "state" => field("state") == value.replace('-', "_"),
            "label" => listed("labels", value),
            "agent" | "session" => listed("sessions", value),
            "module" => text(task, "module_name").to_lowercase().contains(value),
            "no" => match *value {
                "size" => text(task, "size").is_empty(),
                "module" => task["module_id"].is_null(),
                "label" | "labels" => task["labels"].as_array().is_none_or(|l| l.is_empty()),
                "agent" => task["sessions"].as_array().is_none_or(|l| l.is_empty()),
                _ => false,
            },
            _ => hay.contains(&term),
        })
    })
}

/// A project's short prefix for its task keys, as lific and Linear show them: the initials of
/// its name's first three words ("Native verification work" → `NVW`), or the first three letters
/// of a one-word name ("RelayV4" → `REL`). Dashes and underscores separate words, as in a folder
/// name; a name with no letters gives `T`.
pub fn task_key_prefix(project: &str) -> String {
    let words: Vec<&str> = project
        .split(|c: char| c.is_whitespace() || c == '-' || c == '_' || c == '.' || c == '/')
        .filter(|w| w.chars().any(char::is_alphabetic))
        .collect();
    let prefix: String = match words.as_slice() {
        [] => String::new(),
        [one] => one.chars().filter(|c| c.is_alphanumeric()).take(3).collect(),
        many => many.iter().take(3).filter_map(|w| w.chars().find(|c| c.is_alphanumeric())).collect(),
    };
    let prefix = prefix.to_uppercase();
    if prefix.is_empty() { "T".into() } else { prefix }
}

/// A task's key: its project's prefix and its number, `REL-12`.
pub fn task_key(project: &str, id: i64) -> String {
    format!("{}-{id}", task_key_prefix(project))
}

/// The number in a typed task key (`rel-12`, `NVW-3`), if the term is one.
fn key_number(term: &str) -> Option<i64> {
    let (prefix, number) = term.rsplit_once('-')?;
    (!prefix.is_empty() && prefix.len() <= 4 && prefix.chars().all(|c| c.is_alphanumeric()) && prefix.chars().next()?.is_alphabetic())
        .then(|| number.parse().ok())
        .flatten()
}

/// What the search box matches against, lowercased: the id, title, body, labels, agents and
/// module name. A query is expected lowercased too.
pub fn haystack(task: &Value) -> String {
    let join = |key: &str| {
        task[key]
            .as_array()
            .map(|list| list.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "))
            .unwrap_or_default()
    };
    format!(
        "#{} {} {} {} {} {}",
        task["id"],
        text(task, "title"),
        text(task, "body"),
        join("labels"),
        join("sessions"),
        text(task, "module_name")
    )
    .to_lowercase()
}

/// A group's sort rank, a space, then its heading: priority and size keep their own order and
/// the fallback groups (`Unassigned`, `No module`…) sort after every named one.
pub fn group_of(task: &Value, grouping: &str) -> String {
    let named = |name: Option<&str>, none: &str| match name {
        Some(name) => format!("0 {name}"),
        None => format!("9 {none}"),
    };
    match grouping {
        "parent" => named(task["_parent_title"].as_str(), "Top level"),
        "module" => named(task["module_name"].as_str(), "No module"),
        "session" => named(task["sessions"].as_array().and_then(|v| v.last()).and_then(Value::as_str), "Unassigned"),
        "" => String::new(),
        "priority" => {
            let p = text(task, "priority");
            format!("{} {p}", URGENT_FIRST.iter().position(|x| *x == p).unwrap_or(9))
        }
        "size" => match text(task, "size") {
            "" => "9 None".into(),
            s => format!("{} {s}", SIZES.iter().position(|x| *x == s).unwrap_or(8)),
        },
        key => named(task[key].as_str(), "None"),
    }
}
/// The heading of a [`group_of`] key, its rank left off.
pub fn group_title(group: &str) -> String {
    group.split_once(' ').map(|(_, g)| g).unwrap_or(group).to_string()
}

/// The cards of `column` among `visible` (the tasks the filters keep), in the order its lane
/// shows them: by group first, then board position, then id.
pub fn sorted<'a>(visible: &[&'a Value], column: &str, grouping: &str) -> Vec<&'a Value> {
    let mut tasks: Vec<&Value> = visible
        .iter()
        .copied()
        .filter(|t| text(t, "column") == column)
        .collect();
    tasks.sort_by(|a, b| {
        group_of(a, grouping)
            .cmp(&group_of(b, grouping))
            .then_with(|| a["position"].as_i64().cmp(&b["position"].as_i64()))
            .then_with(|| a["id"].as_i64().cmp(&b["id"].as_i64()))
    });
    tasks
}

/// What a Shift+J/K nudge does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Nudge {
    /// The card is first or last in its lane as shown (or not in it): nothing moves.
    Edge,
    /// The neighbour is in another group, which a nudge does not cross.
    CrossesGroup,
    /// Move the card to this final index among its column's other tasks.
    To(usize),
}
/// The focused card `id` swapping places with its neighbour `delta` away in `lane`, the lane as
/// shown (filtered tasks left out, ordered by group first, as [`sorted`] gives it). `order` is
/// the whole column's [`column_order`], which the returned index is into.
pub fn nudge(lane: &[&Value], order: &[i64], id: i64, delta: i64, grouping: &str) -> Nudge {
    let Some(at) = lane.iter().position(|t| t["id"].as_i64() == Some(id)) else { return Nudge::Edge };
    let neighbour = usize::try_from(at as i64 + delta).ok().and_then(|to| lane.get(to));
    let Some(neighbour) = neighbour else { return Nudge::Edge };
    if group_of(lane[at], grouping) != group_of(neighbour, grouping) {
        return Nudge::CrossesGroup;
    }
    let Some(onto) = neighbour["id"].as_i64() else { return Nudge::Edge };
    Nudge::To(drop_index(order, id, onto, delta > 0))
}

/// The bus's columns in board order with their headings, written once for the board, the task
/// pages and the timeline's sentences.
pub const COLUMN_TITLES: [(&str, &str); 5] = [
    ("backlog", "Backlog"),
    ("ready", "Ready"),
    ("active", "Active"),
    ("in_review", "In review"),
    ("done", "Done"),
];
/// A column's heading; an unknown name reads as itself.
pub fn column_title(column: &str) -> &str {
    COLUMN_TITLES.iter().find(|(name, _)| *name == column).map(|(_, title)| *title).unwrap_or(column)
}

/// How much work a size is, in points: what a lane's estimate sums. S, M and L step like a
/// short Fibonacci run, so one large task outweighs a few small ones.
pub fn points(size: &str) -> u32 {
    match size {
        "S" => 1,
        "M" => 3,
        "L" => 5,
        _ => 0,
    }
}
/// The summed [`points`] of `tasks`: a lane head's `Estimate`.
pub fn estimate<'a>(tasks: impl IntoIterator<Item = &'a Value>) -> u32 {
    tasks.into_iter().map(|t| points(text(t, "size"))).sum()
}

/// Who an audit row or comment names, as the timeline shows them: an agent by its session name,
/// a person as `you`.
pub fn actor_name(actor: &str) -> String {
    match actor {
        "user" | "" => "you".into(),
        other => other.strip_prefix("agent:").unwrap_or(other).to_string(),
    }
}

fn titled(value: &str) -> String {
    let mut chars = value.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// One `task.activity` history row as the timeline's sentence, its actor left off: `moved this
/// from Backlog to Ready`. `None` for a row the timeline does not show (a refused or failed
/// attempt, or a comment, which the timeline draws from its own list).
pub fn activity_sentence(row: &Value) -> Option<String> {
    if row["kind"].as_str().is_some_and(|kind| kind != "ok") {
        return None;
    }
    let payload = &row["payload"];
    let before = &row["undo_op"]["payload"];
    let other = |key: &str| payload[key].as_i64().map(|id| format!("#{id}")).unwrap_or_else(|| "a task".into());
    let op = row["op"].as_str().unwrap_or("");
    Some(match op {
        "task.comment" => return None,
        "task.create" => match payload["column"].as_str().filter(|c| *c != "backlog") {
            Some(column) => format!("created this in {}", column_title(column)),
            None => "created this".into(),
        },
        "task.move" => match (before["column"].as_str(), payload["column"].as_str()) {
            (Some(from), Some(to)) if from != to => format!("moved this from {} to {}", column_title(from), column_title(to)),
            (_, Some(to)) => format!("reordered this in {}", column_title(to)),
            _ => "moved this".into(),
        },
        "task.approve" => "closed this as completed by moving it to Done".into(),
        "task.unapprove" => match payload["column"].as_str() {
            Some(column) => format!("reopened this into {}", column_title(column)),
            None => "reopened this".into(),
        },
        "task.update" => {
            let mut changes = Vec::new();
            for (key, value) in payload.as_object().into_iter().flatten() {
                let was = before[key.as_str()].as_str().filter(|was| !was.is_empty());
                let now = value.as_str().filter(|now| !now.is_empty());
                changes.push(match key.as_str() {
                    "task_id" | "expected" => continue,
                    "title" => format!("renamed this to “{}”", now.unwrap_or("")),
                    "body" => "edited the description".into(),
                    "changelog" => "edited the changelog".into(),
                    "module_id" if value.is_null() => "removed the module".into(),
                    "module_id" => "changed the module".into(),
                    field @ ("priority" | "size" | "type" | "state") => {
                        let name = if field == "type" { "type" } else { field };
                        let show = |v: &str| if field == "state" { v.replace('_', " ") } else { titled(v) };
                        match (was, now) {
                            (_, None) => format!("cleared the {name}"),
                            (Some(was), Some(now)) if was != now => format!("changed the {name} from {} to {}", show(was), show(now)),
                            (_, Some(now)) => format!("set the {name} to {}", show(now)),
                        }
                    }
                    other => format!("changed {}", other.replace('_', " ")),
                });
            }
            if changes.is_empty() {
                return None;
            }
            changes.join(", ")
        }
        "task.label.add" => format!("added the label {}", payload["label"].as_str().unwrap_or("")),
        "task.label.remove" => format!("removed the label {}", payload["label"].as_str().unwrap_or("")),
        "task.dispatch" => match payload["session"].as_str().filter(|s| !s.is_empty()) {
            Some(session) => format!("dispatched this to {session}"),
            None => "dispatched this to a new agent".into(),
        },
        "task.parent.set" if payload["parent_id"].is_null() => "removed the parent".into(),
        "task.parent.set" => format!("set the parent to {}", other("parent_id")),
        "task.relate" | "task.unrelate" => {
            let relation = match payload["relation"].as_str() {
                Some("duplicate_of") => "a duplicate of",
                _ => "blocked by",
            };
            let verb = if op == "task.relate" { "marked this as" } else { "unmarked this as" };
            format!("{verb} {relation} {}", other("other_id"))
        }
        "task.link_commit" => format!(
            "linked commit {}",
            payload["sha"].as_str().unwrap_or("").chars().take(7).collect::<String>()
        ),
        "task.attach" => match payload["name"].as_str() {
            Some(name) => format!("attached {name}"),
            None => "attached an image".into(),
        },
        "task.detach" => "removed an attachment".into(),
        "task.attachment.restore" => "restored an attachment".into(),
        "task.delete" => "deleted this".into(),
        "task.restore" => "restored this".into(),
        "task.changelog.write" => "wrote the changelog".into(),
        other => format!("ran {}", other.strip_prefix("task.").unwrap_or(other).replace(['.', '_'], " ")),
    })
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
/// `**bold**`, `*italic*` and `` `code` `` within one line, escaped for Pango. Underscores are
/// left alone: they sit inside identifiers far more often than they mean emphasis.
fn inline(line: &str) -> String {
    let mut out = String::new();
    let ticks = line.matches('`').count();
    for (index, part) in line.split('`').enumerate() {
        // Odd parts sat between backticks; an unclosed backtick leaves the last part as text.
        if index % 2 == 1 && index < ticks {
            out.push_str(&format!("<tt>{}</tt>", escape(part)));
            continue;
        }
        if index % 2 == 1 {
            out.push('`');
        }
        let mut text = escape(part);
        for (mark, tag) in [("**", "b"), ("*", "i")] {
            text = emphasis(&text, mark, tag);
        }
        out.push_str(&text);
    }
    out
}
/// `text` with each `mark`…`mark` pair wrapped in `tag`. A mark opens only before a non-space
/// and closes only after one (neither another `*`), so `2 * 3 * 4` stays as typed.
fn emphasis(text: &str, mark: &str, tag: &str) -> String {
    let word = |c: char| !c.is_whitespace() && c != '*';
    let opens = |s: &str, at: usize| s[at + mark.len()..].chars().next().is_some_and(word);
    let closes = |s: &str, at: usize| s[..at].chars().next_back().is_some_and(word);
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.match_indices(mark).map(|(at, _)| at).find(|&at| opens(rest, at)) {
        let body = open + mark.len();
        let close = rest[body..].match_indices(mark).map(|(at, _)| body + at).find(|&at| at > body && closes(rest, at));
        let Some(close) = close else { break };
        out.push_str(&rest[..open]);
        out.push_str(&format!("<{tag}>{}</{tag}>", &rest[body..close]));
        rest = &rest[close + mark.len()..];
    }
    out.push_str(rest);
    out
}
/// A task's Markdown as Pango markup, for a label: headings, lists, task boxes, quotes, fenced
/// code and the inline marks. Anything else reads as its plain text, escaped.
pub fn markdown_markup(source: &str) -> String {
    let mut lines = Vec::new();
    let mut fenced = false;
    for line in source.trim_end().lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            lines.push(format!("<tt>{}</tt>", escape(line)));
            continue;
        }
        let indent = " ".repeat((line.len() - trimmed.len()).min(8));
        let heading = trimmed.chars().take_while(|c| *c == '#').count();
        lines.push(if (1..=6).contains(&heading) && trimmed[heading..].starts_with(' ') {
            let size = match heading { 1 => "x-large", 2 => "large", _ => "medium" };
            format!("<span size=\"{size}\" weight=\"bold\">{}</span>", inline(trimmed[heading..].trim()))
        } else if let Some(rest) = ["- [ ] ", "* [ ] ", "+ [ ] "].iter().find_map(|p| trimmed.strip_prefix(p)) {
            format!("{indent}☐  {}", inline(rest))
        } else if let Some(rest) = ["- [x] ", "- [X] ", "* [x] ", "* [X] ", "+ [x] ", "+ [X] "].iter().find_map(|p| trimmed.strip_prefix(p)) {
            format!("{indent}☑  {}", inline(rest))
        } else if let Some(rest) = ["- ", "* ", "+ "].iter().find_map(|p| trimmed.strip_prefix(p)) {
            format!("{indent}•  {}", inline(rest))
        } else if let Some(rest) = trimmed.strip_prefix("> ").or_else(|| (trimmed == ">").then_some("")) {
            format!("<i>│  {}</i>", inline(rest))
        } else if trimmed.chars().all(|c| c == '-' || c == '*') && trimmed.len() >= 3 {
            "──────────".into()
        } else {
            format!("{indent}{}", inline(trimmed))
        });
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn task_keys_take_a_prefix_from_the_project_name() {
        assert_eq!(task_key("RelayV4", 12), "REL-12");
        assert_eq!(task_key("Native verification with a long name", 3), "NVW-3");
        assert_eq!(task_key("tally-android", 7), "TA-7");
        assert_eq!(task_key("", 1), "T-1");
        assert_eq!(task_key("2026", 1), "T-1");
    }

    #[test]
    fn a_typed_task_key_finds_its_task() {
        let task = json!({"id": 12, "title": "Board revamp", "column": "ready"});
        assert!(query_matches(&task, "REL-12"));
        assert!(query_matches(&task, "nvw-12"));
        assert!(!query_matches(&task, "REL-13"));
        // A hyphenated word is still a word.
        assert!(!query_matches(&task, "re-do"));
    }

    fn filters(pairs: &[(&str, &str)]) -> Filters {
        let mut map = Filters::new();
        for (k, v) in pairs {
            map.entry(k.to_string()).or_default().insert(v.to_string());
        }
        map
    }
    /// What the engine's `task.move` does with a final index: the column order afterwards.
    fn moved(order: &[i64], moving: i64, at: usize) -> Vec<i64> {
        let mut order: Vec<i64> = order.iter().copied().filter(|id| *id != moving).collect();
        order.insert(at.min(order.len()), moving);
        order
    }
    fn ids(tasks: &[&Value]) -> Vec<i64> {
        tasks.iter().filter_map(|t| t["id"].as_i64()).collect()
    }
    /// One column, positions in id order: 1 urgent, 2 low, 3 urgent (module "Docs"), 4 low,
    /// 5 urgent, 6 low; 7 sits in another column.
    fn lane_fixture() -> Vec<Value> {
        let p = ["urgent", "low", "urgent", "low", "urgent", "low"];
        let mut tasks: Vec<Value> = (1..=6)
            .map(|id| json!({"id":id,"column":"ready","position":id - 1,"priority":p[id as usize - 1],"title":format!("Task {id}")}))
            .collect();
        tasks[2]["module_id"] = json!(4);
        tasks[2]["module_name"] = json!("Docs");
        tasks.push(json!({"id":7,"column":"backlog","position":0,"priority":"urgent"}));
        tasks
    }

    #[test]
    fn lens_combines_metadata_and_text_without_matching_unrelated_fields() {
        let task = json!({"id":9,"title":"Fix notes","body":"Keep drafts","type":"bug","priority":"high","size":"M","parent_id":2,"module_id":3,"labels":["native"],"sessions":["egret"]});
        assert!(task_matches(
            &task,
            &filters(&[("type", "bug"), ("label", "native"), ("session", "egret"), ("parent", "2")]),
            "drafts"
        ));
        assert!(!task_matches(&task, &filters(&[("parent", "roots")]), ""));
        assert!(!task_matches(&task, &filters(&[("module", "4")]), ""));
        assert!(task_matches(&task, &filters(&[("module", "3")]), ""));
        assert!(!task_matches(&task, &Filters::new(), "high"));
        // Several values of one key are alternatives; keys still all have to match.
        assert!(task_matches(&task, &filters(&[("type", "bug"), ("type", "chore")]), ""));
        assert!(!task_matches(&task, &filters(&[("type", "bug"), ("priority", "low")]), ""));
        assert!(task_matches(&task, &Filters::new(), "#9"));
        // A key with no values left filters nothing.
        assert!(task_matches(&task, &Filters::from([("type".to_string(), BTreeSet::new())]), ""));
    }
    #[test]
    fn search_reads_the_module_name_task_list_returns() {
        let task = json!({"id":1,"title":"Fix","module_id":3,"module_name":"Native Board","labels":["UI"]});
        assert!(haystack(&task).contains("native board"));
        assert!(task_matches(&task, &Filters::new(), "native board"));
        assert!(task_matches(&task, &Filters::new(), "ui"));
        assert!(!task_matches(&json!({"id":1,"title":"Fix"}), &Filters::new(), "native"));
    }
    #[test]
    fn drops_beside_a_card_compute_its_final_index() {
        let order = [1, 2, 3, 4];
        // Down the column: the moving card is not counted among the others.
        assert_eq!(drop_index(&order, 1, 3, false), 1);
        assert_eq!(drop_index(&order, 1, 3, true), 2);
        // Up the column.
        assert_eq!(drop_index(&order, 4, 2, false), 1);
        // From another column.
        assert_eq!(drop_index(&order, 9, 4, true), 4);
        assert_eq!(drop_index(&order, 9, 7, false), 4);
        assert_eq!(zone(1., 100.), Zone::Before);
        assert_eq!(zone(50., 100.), Zone::Into);
        assert_eq!(zone(90., 100.), Zone::After);
        // The 30% bands are exclusive at their edges, and a zero-height card only inserts.
        assert_eq!(zone(30., 100.), Zone::Into);
        assert_eq!(zone(70., 100.), Zone::Into);
        assert_eq!(zone(5., 0.), Zone::Before);
    }
    #[test]
    fn derived_orders_follow_the_one_vocabulary() {
        assert_eq!(URGENT_FIRST, ["urgent", "high", "medium", "low"]);
    }
    #[test]
    fn groups_sort_in_their_own_order_with_fallbacks_last() {
        let sizes: Vec<String> = ["L", "", "S", "M"]
            .iter()
            .map(|s| group_of(&json!({"size": s}), "size"))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        assert_eq!(sizes.iter().map(|g| group_title(g)).collect::<Vec<_>>(), ["S", "M", "L", "None"]);
        let priorities: Vec<String> = PRIORITIES
            .iter()
            .map(|p| group_of(&json!({"priority": p}), "priority"))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        assert_eq!(priorities.iter().map(|g| group_title(g)).collect::<Vec<_>>(), URGENT_FIRST);
        assert!(group_of(&json!({"sessions":[]}), "session") > group_of(&json!({"sessions":["zebra"]}), "session"));
        assert_eq!(group_title(&group_of(&json!({}), "module")), "No module");
        assert_eq!(group_title(&group_of(&json!({"module_name":"Docs"}), "module")), "Docs");
        assert!(group_of(&json!({}), "module") > group_of(&json!({"module_name":"Zeta"}), "module"));
        assert_eq!(group_title(&group_of(&json!({"_parent_title":"Epic"}), "parent")), "Epic");
        assert_eq!(group_title(&group_of(&json!({"type":"bug"}), "type")), "bug");
        assert_eq!(group_of(&json!({"type":"bug"}), ""), "");
    }
    #[test]
    fn lanes_sort_by_group_then_position_then_id() {
        let tasks = lane_fixture();
        let all: Vec<&Value> = tasks.iter().collect();
        // Ungrouped: board position, and the other column's task left out.
        assert_eq!(ids(&sorted(&all, "ready", "")), [1, 2, 3, 4, 5, 6]);
        // By priority: urgent before low, position order inside each group.
        assert_eq!(ids(&sorted(&all, "ready", "priority")), [1, 3, 5, 2, 4, 6]);
        // By module: the named module first, "No module" after it.
        assert_eq!(ids(&sorted(&all, "ready", "module")), [3, 1, 2, 4, 5, 6]);
        // Equal positions (a stale refresh) fall back to id.
        let tied = [json!({"id":8,"column":"ready","position":0}), json!({"id":2,"column":"ready","position":0})];
        assert_eq!(ids(&sorted(&tied.iter().collect::<Vec<_>>(), "ready", "")), [2, 8]);
        assert_eq!(column_order(&tied, "ready"), [2, 8]);
    }
    #[test]
    fn column_order_counts_every_task_the_filters_hide() {
        let tasks = lane_fixture();
        assert_eq!(column_order(&tasks, "ready"), [1, 2, 3, 4, 5, 6]);
        assert_eq!(column_order(&tasks, "backlog"), [7]);
        assert_eq!(column_order(&tasks, "done"), Vec::<i64>::new());
    }
    #[test]
    fn filtered_nudges_step_past_hidden_cards_into_place() {
        let tasks = lane_fixture();
        let order = column_order(&tasks, "ready");
        // Only the urgent cards are shown: 1, 3, 5.
        let f = filters(&[("priority", "urgent")]);
        let visible: Vec<&Value> = tasks.iter().filter(|t| task_matches(t, &f, "")).collect();
        let lane = sorted(&visible, "ready", "");
        assert_eq!(ids(&lane), [1, 3, 5]);
        // 1 down: it lands just after 3, past the hidden 2, and the lane as shown reads 3, 1, 5.
        let Nudge::To(at) = nudge(&lane, &order, 1, 1, "") else { panic!("expected a move") };
        let after = moved(&order, 1, at);
        assert_eq!(after, [2, 3, 1, 4, 5, 6]);
        // 5 up: just before 3, with the hidden 4 left behind it.
        let Nudge::To(at) = nudge(&lane, &order, 5, -1, "") else { panic!("expected a move") };
        assert_eq!(moved(&order, 5, at), [1, 2, 5, 3, 4, 6]);
        // The shown ends do not move, whatever is hidden beyond them.
        assert_eq!(nudge(&lane, &order, 5, 1, ""), Nudge::Edge);
        assert_eq!(nudge(&lane, &order, 1, -1, ""), Nudge::Edge);
        // A card the filter hides is not in the lane to nudge.
        assert_eq!(nudge(&lane, &order, 2, 1, ""), Nudge::Edge);
        // A search narrows the lane the same way: only 3 carries the module "Docs".
        let visible: Vec<&Value> = tasks.iter().filter(|t| task_matches(t, &Filters::new(), "docs")).collect();
        assert_eq!(ids(&sorted(&visible, "ready", "")), [3]);
    }
    #[test]
    fn grouped_nudges_keep_to_their_group_and_reorder_within_it() {
        let tasks = lane_fixture();
        let order = column_order(&tasks, "ready");
        let all: Vec<&Value> = tasks.iter().collect();
        let lane = sorted(&all, "ready", "priority");
        assert_eq!(ids(&lane), [1, 3, 5, 2, 4, 6]);
        // 3 down swaps with 5, its neighbour in the urgent group, past the low 4 between them in
        // board order; re-sorting shows urgent 1, 5, 3.
        let Nudge::To(at) = nudge(&lane, &order, 3, 1, "priority") else { panic!("expected a move") };
        let after = moved(&order, 3, at);
        assert_eq!(after, [1, 2, 4, 5, 3, 6]);
        let resorted: Vec<Value> = tasks
            .iter()
            .map(|t| {
                let mut t = t.clone();
                if let Some(p) = t["id"].as_i64().and_then(|id| after.iter().position(|o| *o == id)) {
                    t["position"] = json!(p);
                }
                t
            })
            .collect();
        assert_eq!(ids(&sorted(&resorted.iter().collect::<Vec<_>>(), "ready", "priority")), [1, 5, 3, 2, 4, 6]);
        // 5 is the last urgent card: down would cross into the low group.
        assert_eq!(nudge(&lane, &order, 5, 1, "priority"), Nudge::CrossesGroup);
        assert_eq!(nudge(&lane, &order, 2, -1, "priority"), Nudge::CrossesGroup);
        // 4 up swaps with 2 within the low group.
        let Nudge::To(at) = nudge(&lane, &order, 4, -1, "priority") else { panic!("expected a move") };
        assert_eq!(moved(&order, 4, at), [1, 4, 2, 3, 5, 6]);
        // Grouped and filtered at once: low cards only, grouped by priority, 6 up past hidden 5.
        let f = filters(&[("priority", "low")]);
        let visible: Vec<&Value> = tasks.iter().filter(|t| task_matches(t, &f, "")).collect();
        let lane = sorted(&visible, "ready", "priority");
        assert_eq!(ids(&lane), [2, 4, 6]);
        let Nudge::To(at) = nudge(&lane, &order, 6, -1, "priority") else { panic!("expected a move") };
        assert_eq!(moved(&order, 6, at), [1, 2, 3, 6, 4, 5]);
    }
    #[test]
    fn grouped_drops_land_beside_the_card_they_were_dropped_on() {
        let tasks = lane_fixture();
        let order = column_order(&tasks, "ready");
        // Grouped by priority the lane shows 1, 3, 5 | 2, 4, 6. Dropping 6 on the lower part of 3
        // places it right after 3 in board order, ahead of the 4 and 5 it skipped over.
        let at = drop_index(&order, 6, 3, zone(80., 100.) == Zone::After);
        assert_eq!(moved(&order, 6, at), [1, 2, 3, 6, 4, 5]);
        // On the upper part of 1, the first card: to the top.
        let at = drop_index(&order, 4, 1, zone(10., 100.) == Zone::After);
        assert_eq!(moved(&order, 4, at), [4, 1, 2, 3, 5, 6]);
        // From another column (7, in backlog) onto 5's lower part.
        let at = drop_index(&order, 7, 5, true);
        assert_eq!(moved(&order, 7, at), [1, 2, 3, 4, 5, 7, 6]);
    }

    #[test]
    fn estimates_sum_size_points() {
        let tasks = [json!({"size":"S"}), json!({"size":"L"}), json!({"size":null}), json!({"size":"M"})];
        assert_eq!(estimate(&tasks), 9);
        assert_eq!(estimate(&[] as &[Value]), 0);
        assert_eq!(points(""), 0);
    }
    #[test]
    fn activity_reads_as_sentences() {
        let moved = json!({"op":"task.move","kind":"ok","payload":{"task_id":1,"column":"ready"},"undo_op":{"op":"task.move","payload":{"task_id":1,"column":"backlog"}}});
        assert_eq!(activity_sentence(&moved).unwrap(), "moved this from Backlog to Ready");
        let reordered = json!({"op":"task.move","kind":"ok","payload":{"task_id":1,"column":"ready","position":0},"undo_op":{"op":"task.move","payload":{"column":"ready"}}});
        assert_eq!(activity_sentence(&reordered).unwrap(), "reordered this in Ready");
        let update = json!({"op":"task.update","kind":"ok","payload":{"task_id":1,"priority":"high","size":null,"expected":{}},"undo_op":{"payload":{"priority":"medium","size":"S"}}});
        assert_eq!(activity_sentence(&update).unwrap(), "changed the priority from Medium to High, cleared the size");
        let refused = json!({"op":"task.move","kind":"refused","payload":{}});
        assert_eq!(activity_sentence(&refused), None);
        assert_eq!(activity_sentence(&json!({"op":"task.comment","kind":"ok","payload":{}})), None);
        assert_eq!(activity_sentence(&json!({"op":"task.approve","kind":"ok","payload":{"task_id":1}})).unwrap(), "closed this as completed by moving it to Done");
        assert_eq!(actor_name("agent:quick-newt"), "quick-newt");
        assert_eq!(actor_name("user"), "you");
    }
    #[test]
    fn markdown_becomes_escaped_markup() {
        assert_eq!(markdown_markup("# Title"), "<span size=\"x-large\" weight=\"bold\">Title</span>");
        assert_eq!(markdown_markup("a **b** *c* `d<e>` f_g_h"), "a <b>b</b> <i>c</i> <tt>d&lt;e&gt;</tt> f_g_h");
        assert_eq!(markdown_markup("- [ ] todo\n- [x] done\n- item"), "☐  todo\n☑  done\n•  item");
        assert_eq!(markdown_markup("```\nlet a = 1 < 2;\n```"), "<tt>let a = 1 &lt; 2;</tt>");
        // An unpaired mark stays as typed.
        assert_eq!(markdown_markup("2 * 3 and `open"), "2 * 3 and `open");
        // A mark beside a space is not emphasis, so arithmetic pairs nothing.
        assert_eq!(markdown_markup("2 * 3 * 4"), "2 * 3 * 4");
        assert_eq!(markdown_markup("a ** b ** c"), "a ** b ** c");
        assert_eq!(markdown_markup("2 * 3 is *six*"), "2 * 3 is <i>six</i>");
        assert_eq!(markdown_markup("**bold** and **more**"), "<b>bold</b> and <b>more</b>");
        assert_eq!(markdown_markup("***both***"), "**<i>both</i>**");
        // Every bullet takes a task box, ticked in either case.
        assert_eq!(markdown_markup("* [X] done\n+ [ ] todo\n+ [x] also"), "☑  done\n☐  todo\n☑  also");
    }

    #[test]
    fn search_filters_by_field_and_keyword() {
        let task = json!({"id":4,"title":"Gallery thumbnails","column":"in_review","priority":"high","size":"M","type":"bug",
            "labels":["UI","perf"],"sessions":["quick-newt"],"module_name":"Gallery","module_id":2,"state":"none"});
        for query in ["", "gallery", "priority:high", "p:urgent,high", "size:m", "label:ui", "status:inreview", "is:open",
            "module:gal thumb", "agent:quick-newt type:bug", "#4"] {
            assert!(query_matches(&task, query), "{query} should match");
        }
        for query in ["priority:low", "size:s", "label:docs", "is:closed", "no:module", "gallery missing", "module:profile"] {
            assert!(!query_matches(&task, query), "{query} should not match");
        }
        // A stray colon is a word, not a field.
        assert!(!query_matches(&task, "nothing:"));
    }

    #[test]
    fn is_blocked_finds_what_the_blocked_tile_counts() {
        let tasks = [
            json!({"id":1,"title":"Schema","column":"active","state":"running"}),
            json!({"id":2,"title":"Shipped base","column":"done"}),
            // Waits on #1, which is open: blocked, though no agent said so.
            json!({"id":3,"title":"Migrate","column":"ready","state":"none","blocked_by":[1]}),
            // Its only blocker is done.
            json!({"id":4,"title":"Polish","column":"ready","blocked_by":[2]}),
            // An agent reported itself blocked, but nothing blocks the task.
            json!({"id":5,"title":"Stuck","column":"active","state":"blocked"}),
            // Done tasks are not counted, open blocker or not.
            json!({"id":6,"title":"Closed early","column":"done","blocked_by":[1]}),
            // A blocker the list does not hold (deleted) blocks nothing.
            json!({"id":7,"title":"Orphan","column":"backlog","blocked_by":[99]}),
        ];
        let all: BTreeMap<i64, &Value> = tasks.iter().map(|t| (t["id"].as_i64().unwrap(), t)).collect();
        let tile: Vec<i64> = tasks.iter().filter(|t| is_blocked(t, &all)).map(|t| t["id"].as_i64().unwrap()).collect();
        assert_eq!(tile, [3]);
        assert_eq!(ids(&open_blockers(&tasks[2], &all)), [1]);
        // The board marks what the tile counts, and the search reads the mark.
        let marked: Vec<Value> = tasks
            .iter()
            .map(|t| {
                let mut t = t.clone();
                if is_blocked(&t, &all) {
                    t["_blocked"] = true.into();
                }
                t
            })
            .collect();
        let found = |query: &str| marked.iter().filter(|t| query_matches(t, query)).map(|t| t["id"].as_i64().unwrap()).collect::<Vec<_>>();
        assert_eq!(found("is:blocked"), tile);
        assert_eq!(found("status:blocked"), tile);
        // The agent's report is still searchable, by its own name.
        assert_eq!(found("state:blocked"), [5]);
        assert_eq!(found("state:running"), [1]);
    }
}
