//! An agent's conversation as messages (`session.transcript`), read from the log its provider
//! keeps anyway: Claude Code's `projects/<cwd>/<session id>.jsonl` and Codex's
//! `sessions/YYYY/MM/DD/rollout-…-<id>.jsonl`. The phone draws a terminal agent from it as it
//! draws a thread, so the messages take a thread message's shape (`thread.get`): `user {text}`,
//! `assistant {blocks}`, `tool {tool_use_id, is_error, text}`, `error {text}`, and `note {text}`
//! for a quiet line (an interrupt, a compaction).
//!
//! Only what a person reads is kept: a tool call carries its name, a one-line `summary` (the
//! command, the file) and a `detail` cut to its head (the whole command, an edit as `-`/`+`
//! lines, a patch, a question and its choices), never its raw input; a tool's answer is cut to
//! its head. The model's reasoning, where the provider logged it in words, is a `thinking`
//! block.

use relay_bus::ops::session::{TranscriptMessage, TranscriptOut};
use relay_bus::types::Provider;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// How much of a log one read takes: the tail on a first read, the new part after.
const MAX_READ: u64 = 8 * 1024 * 1024;
/// A tool's answer is shown folded; its head is all a phone needs.
const MAX_ANSWER: usize = 2000;
const MAX_SUMMARY: usize = 300;
/// What a call did, opened under its line: enough of a diff or a script to read on a phone.
const MAX_DETAIL: usize = 4000;
const MAX_THINKING: usize = 6000;
/// The cursor's low bits are the byte offset; the 12 above them name the file it is in, so the
/// whole stays under 2^53 and reads exactly as a JSON number anywhere.
const OFFSET_BITS: u32 = 40;

/// The log of `reference`, the provider's own conversation id, for an agent working in `worktree`.
pub fn locate(provider: Provider, reference: &str, worktree: &Path) -> Option<PathBuf> {
    match provider {
        Provider::Claude => locate_claude(reference, worktree),
        Provider::Codex => locate_codex(reference),
    }
}

fn env_dir(variable: &str) -> Option<PathBuf> {
    std::env::var_os(variable).filter(|value| !value.is_empty()).map(PathBuf::from)
}

fn home_dir(dir: &str) -> Option<PathBuf> { env_dir("HOME").map(|home| home.join(dir)) }

/// Claude Code names a project's folder after its directory with every character that is not
/// a letter or digit made `-`.
fn claude_slug(worktree: &Path) -> String {
    worktree.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

fn locate_claude(reference: &str, worktree: &Path) -> Option<PathBuf> {
    let file = format!("{reference}.jsonl");
    let roots: Vec<PathBuf> = [env_dir("CLAUDE_CONFIG_DIR"), home_dir(".claude")].into_iter().flatten().map(|root| root.join("projects")).collect();
    for root in &roots {
        let path = root.join(claude_slug(worktree)).join(&file);
        if path.is_file() { return Some(path); }
    }
    // A conversation begun in another directory and resumed here keeps its first folder.
    roots.iter().find_map(|root| {
        fs::read_dir(root).ok()?.flatten().map(|entry| entry.path().join(&file)).find(|path| path.is_file())
    })
}

/// Codex files a rollout by the day it began, so finding one is a walk; the answer is kept.
fn locate_codex(reference: &str) -> Option<PathBuf> {
    static FOUND: OnceLock<Mutex<HashMap<String, PathBuf>>> = OnceLock::new();
    let found = FOUND.get_or_init(Default::default);
    if let Some(path) = found.lock().ok()?.get(reference).filter(|path| path.is_file()) {
        return Some(path.clone());
    }
    let root = env_dir("CODEX_HOME").or_else(|| home_dir(".codex"))?.join("sessions");
    let suffix = format!("-{reference}.jsonl");
    let path = find_rollout(&root, &suffix, 0)?;
    found.lock().ok()?.insert(reference.to_string(), path.clone());
    Some(path)
}

/// Newest directory first: an agent Relay runs now began recently.
fn find_rollout(dir: &Path, suffix: &str, depth: u8) -> Option<PathBuf> {
    if depth > 4 { return None; }
    let mut entries: Vec<_> = fs::read_dir(dir).ok()?.flatten().collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.file_name()));
    for entry in entries {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            if let Some(found) = find_rollout(&path, suffix, depth + 1) { return Some(found); }
        } else if entry.file_name().to_string_lossy().ends_with(suffix) {
            return Some(path);
        }
    }
    None
}

fn file_tag(path: &Path) -> u64 {
    // FNV-1a over the path: stable across runs, unlike `DefaultHasher`'s seed.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.as_os_str().as_encoded_bytes() {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
    }
    hash & 0xfff
}

/// The messages in `path` after `after` (a cursor this function returned), or its last `limit`.
pub fn read(provider: Provider, path: &Path, worktree: &Path, after: Option<u64>, limit: usize) -> std::io::Result<TranscriptOut> {
    let tag = file_tag(path);
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let resumed = after.filter(|cursor| cursor >> OFFSET_BITS == tag).map(|cursor| cursor & ((1 << OFFSET_BITS) - 1)).filter(|offset| *offset <= len);
    let reset = after.is_some() && resumed.is_none();
    let start = resumed.unwrap_or_else(|| len.saturating_sub(MAX_READ));
    let end = len.min(start + MAX_READ);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::with_capacity((end - start) as usize);
    file.take(end - start).read_to_end(&mut bytes)?;
    // The reply goes to whoever asked, a phone included.
    crate::redact::Redactor::default().redact(&mut bytes);
    // A tail read begins mid-line; a read of any kind may end mid-line while the agent writes.
    let first = if resumed.is_none() && start > 0 { bytes.iter().position(|b| *b == b'\n').map_or(bytes.len(), |i| i + 1) } else { 0 };
    let last = bytes.iter().rposition(|b| *b == b'\n').map_or(first, |i| i + 1).max(first);
    let mut messages = Vec::new();
    let mut offset = start + first as u64;
    for line in bytes[first..last].split(|b| *b == b'\n') {
        let at = offset;
        offset += line.len() as u64 + 1;
        let Ok(record) = serde_json::from_slice::<Value>(line) else { continue };
        match provider {
            Provider::Claude => claude_record(&record, at, worktree, &mut messages),
            Provider::Codex => codex_record(&record, at, &mut messages),
        }
    }
    if messages.len() > limit { messages.drain(..messages.len() - limit); }
    Ok(TranscriptOut { messages, cursor: (tag << OFFSET_BITS) | (start + last as u64), reset, found: true })
}

fn message(id: String, role: &str, body: Value, record: &Value) -> TranscriptMessage {
    let created_at = record.get("timestamp").and_then(Value::as_str).unwrap_or_default().to_string();
    TranscriptMessage { id, role: role.to_string(), body, created_at }
}

fn cut(text: &str, max: usize) -> String {
    let text = text.trim();
    match text.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", text[..i].trim_end()),
        None => text.to_string(),
    }
}

/// The first line of `text`, cut.
fn line_of(text: &str) -> String { cut(text.trim().lines().next().unwrap_or_default(), MAX_SUMMARY) }

// ---- Claude Code ----

fn claude_record(record: &Value, at: u64, worktree: &Path, out: &mut Vec<TranscriptMessage>) {
    // A subagent's own turns, and the context Claude Code adds for the model, are not the conversation.
    if record.get("isSidechain").and_then(Value::as_bool) == Some(true) || record.get("isMeta").and_then(Value::as_bool) == Some(true) {
        return;
    }
    let id = record.get("uuid").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("o{at}"));
    let content = record.pointer("/message/content");
    match record.get("type").and_then(Value::as_str) {
        Some("user") => match content {
            Some(Value::String(text)) => {
                if let Some((role, text)) = claude_user_text(text) { out.push(message(id, role, json!({"text": text}), record)); }
            }
            Some(Value::Array(blocks)) => {
                let mut words = Vec::new();
                for (i, block) in blocks.iter().enumerate() {
                    match block.get("type").and_then(Value::as_str) {
                        Some("tool_result") => {
                            let text = claude_result_text(block.get("content"));
                            out.push(message(format!("{id}:{i}"), "tool", json!({
                                "tool_use_id": block.get("tool_use_id").and_then(Value::as_str).unwrap_or_default(),
                                "is_error": block.get("is_error").and_then(Value::as_bool).unwrap_or(false),
                                "text": cut(&text, MAX_ANSWER),
                            }), record));
                        }
                        Some("text") => {
                            let text = block.get("text").and_then(Value::as_str).unwrap_or_default();
                            if text.starts_with("[Image") { words.push("[image]".to_string()) }
                            else if let Some((role, text)) = claude_user_text(text) {
                                if role == "note" { out.push(message(format!("{id}:{i}"), role, json!({"text": text}), record)) } else { words.push(text) }
                            }
                        }
                        Some("image") => words.push("[image]".to_string()),
                        _ => {}
                    }
                }
                if !words.is_empty() { out.push(message(id, "user", json!({"text": words.join("\n\n")}), record)); }
            }
            _ => {}
        },
        Some("assistant") => {
            let Some(Value::Array(blocks)) = content else { return };
            let text_of = |block: &Value| block.get("text").and_then(Value::as_str).unwrap_or_default().to_string();
            if record.get("isApiErrorMessage").and_then(Value::as_bool) == Some(true) {
                let text = blocks.iter().map(text_of).collect::<Vec<_>>().join("\n");
                out.push(message(id, "error", json!({"text": text}), record));
                return;
            }
            let kept: Vec<Value> = blocks.iter().filter_map(|block| match block.get("type").and_then(Value::as_str) {
                Some("text") => { let text = text_of(block); (!text.trim().is_empty()).then(|| json!({"type": "text", "text": text})) }
                // Redacted reasoning is logged with its signature and no words: nothing to show.
                Some("thinking") => block.get("thinking").and_then(Value::as_str).filter(|text| !text.trim().is_empty())
                    .map(|text| json!({"type": "thinking", "text": cut(text, MAX_THINKING)})),
                Some("tool_use") => {
                    let name = block.get("name").and_then(Value::as_str).unwrap_or_default();
                    let input = block.get("input").unwrap_or(&Value::Null);
                    let mut call = json!({
                        "type": "tool_use",
                        "id": block.get("id").and_then(Value::as_str).unwrap_or_default(),
                        "name": name,
                        "summary": claude_summary(name, input, worktree),
                    });
                    if let Some(detail) = claude_detail(name, input).filter(|detail| !detail.trim().is_empty()) {
                        call["detail"] = json!(cut_lines(&detail, MAX_DETAIL));
                    }
                    Some(call)
                }
                _ => None,
            }).collect();
            if !kept.is_empty() { out.push(message(id, "assistant", json!({"blocks": kept}), record)); }
        }
        Some("system") if record.get("subtype").and_then(Value::as_str) == Some("compact_boundary") => {
            out.push(message(id, "note", json!({"text": "Conversation compacted"}), record));
        }
        _ => {}
    }
}

/// What a person typed, as they would read it; `None` for what Claude Code added itself.
fn claude_user_text(text: &str) -> Option<(&'static str, String)> {
    let trimmed = text.trim();
    if trimmed.starts_with("[Request interrupted") { return Some(("note", "Interrupted".into())); }
    if let Some(name) = between(trimmed, "<command-name>", "</command-name>") {
        let args = between(trimmed, "<command-args>", "</command-args>").unwrap_or_default();
        let name = if name.starts_with('/') { name.to_string() } else { format!("/{name}") };
        return Some(("user", if args.trim().is_empty() { name } else { format!("{name} {}", args.trim()) }));
    }
    if trimmed.starts_with("<task-notification>") { return Some(("note", "A background task finished".into())); }
    // `<local-command-stdout>`, `<system-reminder>` and the like: context, not words.
    if trimmed.starts_with('<') && trimmed.split_once('>').is_some_and(|(tag, _)| tag[1..].chars().all(|c| c.is_ascii_lowercase() || c == '-')) {
        return None;
    }
    (!trimmed.is_empty()).then(|| ("user", trimmed.to_string()))
}

fn between<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = text.find(open)? + open.len();
    let end = text[start..].find(close)? + start;
    Some(&text[start..end])
}

fn claude_result_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(parts)) => parts.iter().map(|part| match part.get("type").and_then(Value::as_str) {
            Some("text") => part.get("text").and_then(Value::as_str).unwrap_or_default().to_string(),
            Some("image") => "[image]".to_string(),
            _ => String::new(),
        }).filter(|text| !text.is_empty()).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

/// One line saying what a tool call was about: the command, the file, the pattern.
fn claude_summary(name: &str, input: &Value, worktree: &Path) -> String {
    let field = |key: &str| input.get(key).and_then(Value::as_str);
    let path = |key: &str| field(key).map(|path| relative(path, worktree));
    let text = match name {
        "Bash" => field("command").map(str::to_string),
        "Read" | "Write" | "Edit" | "MultiEdit" => path("file_path"),
        "NotebookEdit" => path("notebook_path"),
        "Grep" => field("pattern").map(|pattern| match path("path") { Some(dir) => format!("{pattern} in {dir}"), None => pattern.to_string() }),
        "Glob" => field("pattern").map(str::to_string),
        "WebFetch" => field("url").map(str::to_string),
        "WebSearch" => field("query").map(str::to_string),
        "Task" | "Agent" => field("description").map(str::to_string),
        "TodoWrite" => input.get("todos").and_then(Value::as_array).map(|todos| {
            let done = todos.iter().filter(|todo| todo.get("status").and_then(Value::as_str) == Some("completed")).count();
            format!("{done} of {} done", todos.len())
        }),
        _ => ["command", "file_path", "path", "pattern", "query", "url", "description", "session", "text"]
            .iter().find_map(|key| field(key)).map(str::to_string),
    };
    text.map(|text| line_of(&text)).unwrap_or_default()
}

/// What a call did, in full enough to check it: an edit as the lines it took out (`-`) and put
/// in (`+`), a file written as its lines, the whole command, a question with its choices.
/// `None` where the one-line summary already says it all.
fn claude_detail(name: &str, input: &Value) -> Option<String> {
    let field = |key: &str| input.get(key).and_then(Value::as_str);
    match name {
        "Edit" => Some(edit_lines(field("old_string")?, field("new_string").unwrap_or_default())),
        "MultiEdit" => Some(input.get("edits")?.as_array()?.iter()
            .map(|edit| edit_lines(edit.get("old_string").and_then(Value::as_str).unwrap_or_default(), edit.get("new_string").and_then(Value::as_str).unwrap_or_default()))
            .collect::<Vec<_>>().join("\n@@\n")),
        "Write" => Some(edit_lines("", field("content")?)),
        "NotebookEdit" => Some(edit_lines("", field("new_source")?)),
        // The summary is the first line: a heredoc or a chain of commands needs the rest.
        "Bash" => {
            let command = field("command")?;
            let description = field("description").filter(|text| !text.trim().is_empty());
            (command.trim().contains('\n') || command.chars().count() > 60 || description.is_some())
                .then(|| match description { Some(text) => format!("# {}\n{command}", text.trim()), None => command.to_string() })
        }
        "Task" | "Agent" => field("prompt").map(str::to_string),
        "ExitPlanMode" => field("plan").map(str::to_string),
        "AskUserQuestion" => Some(input.get("questions")?.as_array()?.iter().map(|question| {
            let mut text = question.get("question").and_then(Value::as_str).unwrap_or_default().to_string();
            for (i, option) in question.get("options").and_then(Value::as_array).into_iter().flatten().enumerate() {
                let label = option.get("label").and_then(Value::as_str).unwrap_or_default();
                match option.get("description").and_then(Value::as_str).filter(|text| !text.is_empty()) {
                    Some(about) => text.push_str(&format!("\n{}. {label} — {about}", i + 1)),
                    None => text.push_str(&format!("\n{}. {label}", i + 1)),
                }
            }
            text
        }).collect::<Vec<_>>().join("\n\n")),
        "TodoWrite" => Some(input.get("todos")?.as_array()?.iter().map(|todo| {
            format!("{} {}", plan_mark(todo.get("status").and_then(Value::as_str)), todo.get("content").and_then(Value::as_str).unwrap_or_default())
        }).collect::<Vec<_>>().join("\n")),
        _ => None,
    }
}

/// An edit as a diff with no context: every line it took out, then every line it put in.
fn edit_lines(old: &str, new: &str) -> String {
    let taken = old.lines().map(|line| format!("-{line}"));
    taken.chain(new.lines().map(|line| format!("+{line}"))).collect::<Vec<_>>().join("\n")
}

/// `text` cut to about `max` characters at a line's end, saying how many lines were left out.
fn cut_lines(text: &str, max: usize) -> String {
    if text.chars().count() <= max { return text.to_string(); }
    let mut kept = 0;
    let mut out = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    for line in &lines {
        let n = line.chars().count() + 1;
        if kept + n > max && !out.is_empty() { break; }
        out.push(if n > max { cut(line, max) } else { (*line).to_string() });
        kept += n;
    }
    let left = lines.len() - out.len();
    if left > 0 { out.push(format!("… {left} more line{}", if left == 1 { "" } else { "s" })); }
    out.join("\n")
}

fn relative(path: &str, worktree: &Path) -> String {
    Path::new(path).strip_prefix(worktree).map(|rest| rest.to_string_lossy().into_owned()).unwrap_or_else(|_| path.to_string())
}

// ---- Codex ----

fn codex_record(record: &Value, at: u64, out: &mut Vec<TranscriptMessage>) {
    let id = format!("o{at}");
    let payload = record.get("payload").unwrap_or(&Value::Null);
    let kind = payload.get("type").and_then(Value::as_str).unwrap_or_default();
    let str_of = |key: &str| payload.get(key).and_then(Value::as_str).unwrap_or_default();
    match (record.get("type").and_then(Value::as_str).unwrap_or_default(), kind) {
        // The user's words as typed; the `response_item` copy carries the context Codex adds.
        ("event_msg", "user_message") => {
            let text = str_of("message").trim();
            if !text.is_empty() { out.push(message(id, "user", json!({"text": text}), record)); }
        }
        ("event_msg", "turn_aborted") => out.push(message(id, "note", json!({"text": "Interrupted"}), record)),
        ("event_msg", "error") => out.push(message(id, "error", json!({"text": str_of("message")}), record)),
        ("response_item", "message") if str_of("role") == "assistant" => {
            let text = payload.get("content").and_then(Value::as_array).into_iter().flatten()
                .filter_map(|part| part.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join("");
            if !text.trim().is_empty() { out.push(message(id, "assistant", json!({"blocks": [{"type": "text", "text": text}]}), record)); }
        }
        // Codex logs its reasoning as a summary in words, the rest encrypted.
        ("response_item", "reasoning") => {
            let text = payload.get("summary").and_then(Value::as_array).into_iter().flatten()
                .filter_map(|part| part.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join("\n\n");
            if !text.trim().is_empty() { out.push(message(id, "assistant", json!({"blocks": [{"type": "thinking", "text": cut(&text, MAX_THINKING)}]}), record)); }
        }
        ("response_item", "function_call" | "custom_tool_call" | "local_shell_call") => {
            let (summary, patch) = codex_summary(payload);
            let name = if patch { "apply_patch" } else { payload.get("name").and_then(Value::as_str).unwrap_or(if kind == "local_shell_call" { "shell" } else { "tool" }) };
            let mut call = json!({
                "type": "tool_use",
                "id": str_of("call_id"),
                "name": name,
                "summary": summary,
            });
            if let Some(detail) = codex_detail(payload).filter(|detail| !detail.trim().is_empty() && detail.trim() != summary) {
                call["detail"] = json!(cut_lines(&detail, MAX_DETAIL));
            }
            out.push(message(id, "assistant", json!({"blocks": [call]}), record));
        }
        ("response_item", "function_call_output" | "custom_tool_call_output") => {
            let (text, failed) = codex_output(payload.get("output").unwrap_or(&Value::Null));
            out.push(message(id, "tool", json!({"tool_use_id": str_of("call_id"), "is_error": failed, "text": cut(&text, MAX_ANSWER)}), record));
        }
        _ => {}
    }
}

/// A plan step's mark, as the phone reads a plan: `[x]` done, `[~]` under way, `[ ]` to do.
fn plan_mark(status: Option<&str>) -> &'static str {
    match status {
        Some("completed") => "[x]",
        Some("in_progress") => "[~]",
        _ => "[ ]",
    }
}

/// Codex's plan (`update_plan {plan: [{step, status}]}`), each step with its mark: read as Claude
/// Code's to-do list is, so both draw the same plan.
fn codex_plan(payload: &Value) -> Option<Vec<String>> {
    if payload.get("name").and_then(Value::as_str) != Some("update_plan") {
        return None;
    }
    let arguments: Value = serde_json::from_str(payload.get("arguments")?.as_str()?).ok()?;
    Some(arguments.get("plan")?.as_array()?.iter().map(|step| {
        format!("{} {}", plan_mark(step.get("status").and_then(Value::as_str)), step.get("step").and_then(Value::as_str).unwrap_or_default())
    }).collect())
}

/// One line saying what a call was about, and whether it was a patch (named `apply_patch` then,
/// whatever tool carried it).
fn codex_summary(payload: &Value) -> (String, bool) {
    if let Some(plan) = codex_plan(payload) {
        let done = plan.iter().filter(|step| step.starts_with("[x]")).count();
        return (format!("{done} of {} done", plan.len()), false);
    }
    let command = |value: &Value| match value {
        Value::Array(parts) => Some(parts.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")),
        Value::String(text) => Some(text.clone()),
        _ => None,
    };
    if let Some(found) = payload.pointer("/action/command").and_then(command) { return (line_of(&found), false); }
    let arguments = payload.get("arguments").and_then(Value::as_str).and_then(|raw| serde_json::from_str::<Value>(raw).ok());
    if let Some(arguments) = arguments {
        let found = ["command", "cmd"].iter().find_map(|key| arguments.get(*key).and_then(command))
            .or_else(|| ["path", "file_path", "query", "url"].iter().find_map(|key| arguments.get(*key).and_then(Value::as_str).map(str::to_string)));
        return (found.map(|text| line_of(&text)).unwrap_or_default(), false);
    }
    // A code-mode call is a script; the command it runs or the file it patches is what it is about.
    let input = payload.get("input").and_then(Value::as_str).unwrap_or_default();
    let patched: Vec<&str> = input.split("\\n").flat_map(str::lines)
        .filter_map(|line| ["*** Update File: ", "*** Add File: ", "*** Delete File: "].iter().find_map(|mark| line.trim().strip_prefix(mark)))
        .collect();
    if let Some(first) = patched.first() {
        let more = if patched.len() > 1 { format!(" and {} more", patched.len() - 1) } else { String::new() };
        return (line_of(&format!("{}{more}", first.trim_end_matches(['"', '\\']))), true);
    }
    if let Some(at) = input.find("\"cmd\":") {
        let rest = input[at + 6..].trim_start();
        if let Some(Ok(cmd)) = serde_json::Deserializer::from_str(rest).into_iter::<String>().next() { return (line_of(&cmd), false); }
    }
    (line_of(input), false)
}

/// What a Codex call did in full: the patch it applied, the whole command it ran, or its plan.
fn codex_detail(payload: &Value) -> Option<String> {
    if let Some(plan) = codex_plan(payload) {
        return Some(plan.join("\n"));
    }
    let command = |value: &Value| match value {
        Value::Array(parts) => Some(parts.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")),
        Value::String(text) => Some(text.clone()),
        _ => None,
    };
    if let Some(found) = payload.pointer("/action/command").and_then(command) { return Some(found); }
    let raw = payload.get("arguments").and_then(Value::as_str).or_else(|| payload.get("input").and_then(Value::as_str))?;
    if let Some(patch) = patch_of(raw) { return Some(patch); }
    let Ok(arguments) = serde_json::from_str::<Value>(raw) else {
        // A code-mode script: the command it runs, whole.
        let at = raw.find("\"cmd\":")?;
        return serde_json::Deserializer::from_str(raw[at + 6..].trim_start()).into_iter::<String>().next()?.ok();
    };
    ["command", "cmd"].iter().find_map(|key| arguments.get(*key).and_then(command))
        .or_else(|| arguments.get("input").and_then(Value::as_str).and_then(patch_of))
}

/// The `*** Begin Patch` … `*** End Patch` in `text`, whether written out or escaped in a script's string.
fn patch_of(text: &str) -> Option<String> {
    let start = text.find("*** Begin Patch")?;
    let rest = &text[start..];
    // Unended, it runs to the end of the string that holds it.
    let unended = || rest.char_indices().find(|(i, c)| *c == '"' && !rest[..*i].ends_with('\\')).map_or(rest.len(), |(i, _)| i);
    let end = rest.find("*** End Patch").map_or_else(unended, |at| at + "*** End Patch".len());
    let patch = &rest[..end];
    let patch = if patch.contains('\n') { patch.to_string() } else { patch.replace("\\n", "\n").replace("\\\"", "\"").replace("\\t", "\t") };
    // The markers frame it; the files and lines are what a person reads.
    Some(patch.lines().filter(|line| !matches!(line.trim(), "*** Begin Patch" | "*** End Patch")).collect::<Vec<_>>().join("\n"))
}

/// A tool's answer and whether it failed: Codex writes it as text, as `{output, metadata}`
/// encoded in a string, or as parts.
fn codex_output(output: &Value) -> (String, bool) {
    match output {
        Value::String(raw) => match serde_json::from_str::<Value>(raw) {
            Ok(Value::Object(object)) if object.contains_key("output") => {
                let code = object.get("metadata").and_then(|meta| meta.get("exit_code")).and_then(Value::as_i64);
                (object.get("output").and_then(Value::as_str).unwrap_or_default().to_string(), code.is_some_and(|code| code != 0))
            }
            _ => (raw.clone(), false),
        },
        Value::Array(parts) => {
            let text = parts.iter().filter_map(|part| part.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join("\n");
            // A code-mode script's answer opens with how it ran; what it printed follows `Output:`.
            let text = match text.strip_prefix("Script completed") { Some(rest) => rest.split_once("Output:").map_or(rest, |(_, out)| out).trim().to_string(), None => text };
            (text, false)
        }
        _ => (String::new(), false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn lines(records: &[Value]) -> String { records.iter().map(|r| format!("{r}\n")).collect() }

    #[test]
    fn claude_slug_makes_every_other_character_a_dash() {
        assert_eq!(claude_slug(Path::new("/home/a/dev3/RelayV4/.relay/worktrees/proud-llama")), "-home-a-dev3-RelayV4--relay-worktrees-proud-llama");
    }

    #[test]
    fn claude_conversation_reads_as_a_thread() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.jsonl");
        let wt = Path::new("/wt");
        fs::write(&path, lines(&[
            json!({"type": "user", "uuid": "u1", "timestamp": "2026-10-13T10:00:00Z", "message": {"role": "user", "content": "fix the build"}}),
            json!({"type": "user", "uuid": "m", "isMeta": true, "message": {"content": "context"}}),
            json!({"type": "user", "uuid": "r", "message": {"content": "<system-reminder>x</system-reminder>"}}),
            json!({"type": "assistant", "uuid": "a1", "message": {"content": [{"type": "thinking", "thinking": "hm"}]}}),
            json!({"type": "assistant", "uuid": "a2", "message": {"content": [{"type": "thinking", "thinking": "", "signature": "x"}, {"type": "tool_use", "id": "t1", "name": "Edit", "input": {"file_path": "/wt/src/main.rs", "old_string": "a\nb", "new_string": "c"}}]}}),
            json!({"type": "user", "uuid": "u2", "message": {"content": [{"type": "tool_result", "tool_use_id": "t1", "content": [{"type": "text", "text": "ok"}]}]}}),
            json!({"type": "assistant", "uuid": "s", "isSidechain": true, "message": {"content": [{"type": "text", "text": "subagent"}]}}),
            json!({"type": "assistant", "uuid": "a3", "message": {"content": [{"type": "text", "text": "Done."}]}}),
            json!({"type": "user", "uuid": "u3", "message": {"content": "<command-name>/compact</command-name>\n<command-args></command-args>"}}),
            json!({"type": "user", "uuid": "u4", "message": {"content": [{"type": "text", "text": "[Request interrupted by user]"}]}}),
        ])).unwrap();
        let out = read(Provider::Claude, &path, wt, None, 200).unwrap();
        let got: Vec<(&str, &str)> = out.messages.iter().map(|m| (m.id.as_str(), m.role.as_str())).collect();
        assert_eq!(got, [("u1", "user"), ("a1", "assistant"), ("a2", "assistant"), ("u2:0", "tool"), ("a3", "assistant"), ("u3", "user"), ("u4:0", "note")]);
        assert_eq!(out.messages[0].body["text"], "fix the build");
        assert_eq!(out.messages[0].created_at, "2026-10-13T10:00:00Z");
        assert_eq!(out.messages[1].body["blocks"], json!([{"type": "thinking", "text": "hm"}]));
        // Redacted reasoning has no words to show; the edit carries what it changed.
        assert_eq!(out.messages[2].body["blocks"].as_array().unwrap().len(), 1);
        assert_eq!(out.messages[2].body["blocks"][0]["summary"], "src/main.rs");
        assert_eq!(out.messages[2].body["blocks"][0]["detail"], "-a\n-b\n+c");
        assert_eq!(out.messages[3].body["text"], "ok");
        assert_eq!(out.messages[5].body["text"], "/compact");
        assert!(out.found && !out.reset);
    }

    #[test]
    fn a_call_carries_what_the_summary_leaves_out() {
        let detail = |name: &str, input: Value| claude_detail(name, &input);
        assert_eq!(detail("Bash", json!({"command": "ls"})), None);
        assert_eq!(detail("Bash", json!({"command": "cat <<EOF\nx\nEOF", "description": "Show x"})).unwrap(), "# Show x\ncat <<EOF\nx\nEOF");
        assert_eq!(detail("Write", json!({"file_path": "/a", "content": "one\ntwo\n"})).unwrap(), "+one\n+two");
        assert_eq!(detail("MultiEdit", json!({"edits": [{"old_string": "a", "new_string": "b"}, {"old_string": "c", "new_string": ""}]})).unwrap(), "-a\n+b\n@@\n-c");
        assert_eq!(detail("AskUserQuestion", json!({"questions": [{"question": "Which?", "options": [{"label": "A", "description": "first"}, {"label": "B"}]}]})).unwrap(), "Which?\n1. A — first\n2. B");
        assert_eq!(detail("TodoWrite", json!({"todos": [{"content": "x", "status": "completed"}, {"content": "y", "status": "pending"}]})).unwrap(), "[x] x\n[ ] y");
        // Codex's plan reads as the same list.
        let plan = json!({"type": "function_call", "name": "update_plan", "arguments": "{\"plan\":[{\"step\":\"read\",\"status\":\"completed\"},{\"step\":\"fix\",\"status\":\"in_progress\"},{\"step\":\"test\",\"status\":\"pending\"}]}"});
        assert_eq!(codex_detail(&plan).unwrap(), "[x] read\n[~] fix\n[ ] test");
        assert_eq!(codex_summary(&plan).0, "1 of 3 done");
        let long = (0..2000).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let cut = cut_lines(&long, 100);
        assert!(cut.starts_with("line 0\nline 1\n") && cut.ends_with("more lines"), "{cut}");
    }

    #[test]
    fn a_cursor_reads_only_what_came_since_and_waits_for_a_whole_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.jsonl");
        let first = json!({"type": "user", "uuid": "u1", "message": {"content": "one"}});
        fs::write(&path, lines(&[first])).unwrap();
        let wt = Path::new("/wt");
        let out = read(Provider::Claude, &path, wt, None, 200).unwrap();
        assert_eq!(out.messages.len(), 1);
        // Half a line: nothing yet, and the cursor stays put.
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        let second = json!({"type": "user", "uuid": "u2", "message": {"content": "two"}}).to_string();
        file.write_all(&second.as_bytes()[..10]).unwrap();
        let mid = read(Provider::Claude, &path, wt, Some(out.cursor), 200).unwrap();
        assert!(mid.messages.is_empty());
        assert_eq!(mid.cursor, out.cursor);
        file.write_all(format!("{}\n", &second[10..]).as_bytes()).unwrap();
        let next = read(Provider::Claude, &path, wt, Some(mid.cursor), 200).unwrap();
        assert_eq!(next.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["u2"]);
        // A cursor from another file starts again.
        let other = dir.path().join("d.jsonl");
        fs::write(&other, lines(&[json!({"type": "user", "uuid": "x", "message": {"content": "x"}})])).unwrap();
        let moved = read(Provider::Claude, &other, wt, Some(next.cursor), 200).unwrap();
        assert!(moved.reset);
        assert_eq!(moved.messages.len(), 1);
    }

    #[test]
    fn limit_keeps_the_newest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.jsonl");
        let records: Vec<Value> = (0..5).map(|i| json!({"type": "user", "uuid": format!("u{i}"), "message": {"content": format!("m{i}")}})).collect();
        fs::write(&path, lines(&records)).unwrap();
        let out = read(Provider::Claude, &path, Path::new("/wt"), None, 2).unwrap();
        assert_eq!(out.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["u3", "u4"]);
    }

    #[test]
    fn codex_rollout_reads_as_a_thread() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rollout-2026-10-13T10-00-00-abc.jsonl");
        fs::write(&path, lines(&[
            json!({"type": "session_meta", "payload": {"id": "abc"}}),
            json!({"type": "response_item", "payload": {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "<environment_context>…"}]}}),
            json!({"type": "event_msg", "timestamp": "2026-10-13T10:00:01Z", "payload": {"type": "user_message", "message": "add a test"}}),
            json!({"type": "response_item", "payload": {"type": "custom_tool_call", "call_id": "c1", "name": "exec", "input": "const r = await tools.exec_command({\"cmd\":\"cargo test -p x\",\"yield_time_ms\":1000});"}}),
            json!({"type": "response_item", "payload": {"type": "custom_tool_call_output", "call_id": "c1", "output": [{"type": "input_text", "text": "Script completed\nWall time 0.0 seconds\nOutput:\n"}, {"type": "input_text", "text": "ok"}]}}),
            json!({"type": "response_item", "payload": {"type": "custom_tool_call", "call_id": "c3", "name": "exec", "input": "const patch = \"*** Begin Patch\\n*** Update File: /wt/src/a.rs\\n@@\\n*** Add File: /wt/b.rs\\n\";"}}),
            json!({"type": "response_item", "payload": {"type": "function_call", "call_id": "c2", "name": "shell", "arguments": "{\"command\":[\"bash\",\"-lc\",\"ls\"]}"}}),
            json!({"type": "response_item", "payload": {"type": "function_call_output", "call_id": "c2", "output": "{\"output\":\"nope\",\"metadata\":{\"exit_code\":2}}"}}),
            json!({"type": "response_item", "payload": {"type": "reasoning", "summary": []}}),
            json!({"type": "response_item", "payload": {"type": "reasoning", "summary": [{"type": "summary_text", "text": "**Checking** the tests"}]}}),
            json!({"type": "event_msg", "payload": {"type": "agent_message", "message": "Added."}}),
            json!({"type": "response_item", "payload": {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "Added."}]}}),
        ])).unwrap();
        let out = read(Provider::Codex, &path, Path::new("/wt"), None, 200).unwrap();
        let roles: Vec<&str> = out.messages.iter().map(|m| m.role.as_str()).collect();
        assert_eq!(roles, ["user", "assistant", "tool", "assistant", "assistant", "tool", "assistant", "assistant"]);
        assert_eq!(out.messages[0].body["text"], "add a test");
        assert_eq!(out.messages[1].body["blocks"][0]["summary"], "cargo test -p x");
        assert_eq!(out.messages[2].body["text"], "ok");
        assert_eq!(out.messages[3].body["blocks"][0]["summary"], "/wt/src/a.rs and 1 more");
        assert_eq!(out.messages[3].body["blocks"][0]["name"], "apply_patch");
        assert_eq!(out.messages[3].body["blocks"][0]["detail"], "*** Update File: /wt/src/a.rs\n@@\n*** Add File: /wt/b.rs");
        assert_eq!(out.messages[1].body["blocks"][0].get("detail"), None);
        assert_eq!(out.messages[4].body["blocks"][0]["summary"], "bash -lc ls");
        assert_eq!(out.messages[4].body["blocks"][0].get("detail"), None);
        assert_eq!(out.messages[5].body["is_error"], true);
        assert_eq!(out.messages[5].body["text"], "nope");
        assert_eq!(out.messages[6].body["blocks"], json!([{"type": "thinking", "text": "**Checking** the tests"}]));
        assert_eq!(out.messages[7].body["blocks"][0]["text"], "Added.");
        assert!(out.cursor < 1 << 53);
    }

    #[test]
    fn a_rollout_is_found_by_its_id_under_any_day() {
        let dir = tempfile::tempdir().unwrap();
        let day = dir.path().join("2026/10/13");
        fs::create_dir_all(&day).unwrap();
        fs::write(day.join("rollout-2026-10-13T10-00-00-abc-123.jsonl"), "").unwrap();
        fs::write(day.join("rollout-2026-10-13T11-00-00-def.jsonl"), "").unwrap();
        let found = find_rollout(dir.path(), "-abc-123.jsonl", 0).unwrap();
        assert!(found.ends_with("rollout-2026-10-13T10-00-00-abc-123.jsonl"));
        assert!(find_rollout(dir.path(), "-zzz.jsonl", 0).is_none());
    }
}
