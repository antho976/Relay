//! An agent's conversation as messages (`session.transcript`), read from the log its provider
//! keeps anyway: Claude Code's `projects/<cwd>/<session id>.jsonl` and Codex's
//! `sessions/YYYY/MM/DD/rollout-…-<id>.jsonl`. The phone draws a terminal agent from it as it
//! draws a thread, so the messages take a thread message's shape (`thread.get`): `user {text}`,
//! `assistant {blocks}`, `tool {tool_use_id, is_error, text}`, `error {text}`, and
//! `note {text, kind, detail?}` for a line that is neither side's words: `compact` (the context
//! was compacted), `summary` (what the compaction kept, in `detail`), `relay` (what Relay typed
//! in: an assignment, a restart), `task` (a background task finished), `interrupt` and
//! `skipped`.
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
/// A compaction's summary runs to tens of thousands of characters; its head is what a person skims.
const MAX_COMPACT_SUMMARY: usize = 12_000;
/// The cursor's low bits are the byte offset; the 12 above them name the file it is in, so the
/// whole stays under 2^53 and reads exactly as a JSON number anywhere.
const OFFSET_BITS: u32 = 40;

/// The log of `reference`, the provider's own conversation id, for an agent working in `worktree`.
pub fn locate(provider: Provider, reference: &str, worktree: &Path) -> Option<PathBuf> {
    match provider {
        Provider::Claude => locate_claude(reference, worktree),
        Provider::Codex => locate_codex(reference),
        // A shell keeps no conversation log.
        Provider::Shell => None,
    }
}

/// The log Claude Code keeps for the helper a Task call `tool_use_id` started: beside the
/// session's log, `<session>/subagents/agent-<id>.jsonl`, named in its `.meta.json` by the call.
pub fn locate_task(session_log: &Path, tool_use_id: &str) -> Option<PathBuf> {
    let dir = session_log.with_extension("").join("subagents");
    for entry in fs::read_dir(&dir).ok()?.flatten() {
        let path = entry.path();
        let Some(stem) = path.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(".meta.json")) else { continue };
        let Ok(bytes) = fs::read(&path) else { continue };
        let Ok(meta) = serde_json::from_slice::<Value>(&bytes) else { continue };
        if meta.get("toolUseId").and_then(Value::as_str) == Some(tool_use_id) {
            let log = dir.join(format!("{stem}.jsonl"));
            return log.is_file().then_some(log);
        }
    }
    None
}

/// A helper's own log (`subagents/`): its turns are all sidechain ones, and they are what is read.
fn is_helper_log(path: &Path) -> bool {
    path.parent().and_then(Path::file_name).is_some_and(|n| n == "subagents")
}

fn locate_claude(reference: &str, worktree: &Path) -> Option<PathBuf> {
    let file = format!("{reference}.jsonl");
    // Its config folder, and `~/.claude` too: an agent started without the variable wrote there.
    let mut roots: Vec<PathBuf> = [crate::providers::config_dir(Provider::Claude), crate::providers::home_dir().map(|home| home.join(".claude"))]
        .into_iter().flatten().map(|root| root.join("projects")).collect();
    roots.dedup();
    let key = crate::providers::claude_project_key(worktree);
    for root in &roots {
        let path = root.join(&key).join(&file);
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
    let root = crate::providers::config_dir(Provider::Codex)?.join("sessions");
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

/// A person's message sent with `chat.send`, and where the agent's log stood when it was typed.
#[derive(Clone, Debug)]
pub struct Sent {
    pub client_id: String,
    pub text: String,
    /// [`position`] when it was typed; none when the log was not found yet.
    pub log_tag: Option<u64>,
    pub log_offset: Option<u64>,
    pub sent_at: String,
}

/// Where a log stands now (its file's tag and length), for a message about to be typed: its
/// line comes after.
pub fn position(path: &Path) -> Option<(u64, u64)> {
    Some((file_tag(path), fs::metadata(path).ok()?.len()))
}

/// How far before a read the log is searched for the messages sends already matched.
const SENT_LOOKBACK: u64 = 1024 * 1024;

/// The messages in `path` after `after` (a cursor this function returned), the newest `limit`
/// before `before` (one too: a page further back), or its last `limit`. A person's message that
/// `sends` holds comes back with its `client_id`; the sends the log does not hold yet come back
/// as `pending` (not on a page back).
pub fn read(provider: Provider, path: &Path, worktree: &Path, after: Option<u64>, before: Option<u64>, limit: usize, sends: &[Sent]) -> std::io::Result<TranscriptOut> {
    let tag = file_tag(path);
    let helper = is_helper_log(path);
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let in_file = |cursor: u64| (cursor >> OFFSET_BITS == tag).then_some(cursor & ((1 << OFFSET_BITS) - 1)).filter(|offset| *offset <= len);
    let resumed = after.and_then(in_file);
    // A page back ends where an earlier answer began, which is always a line's start.
    let back = if resumed.is_none() { before.and_then(in_file) } else { None };
    let reset = (after.is_some() && resumed.is_none()) || (before.is_some() && after.is_none() && back.is_none());
    let (start, end) = match (resumed, back) {
        (Some(r), _) => (r, len.min(r + MAX_READ)),
        (None, Some(b)) => (b.saturating_sub(MAX_READ), b),
        (None, None) => (len.saturating_sub(MAX_READ), len),
    };
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::with_capacity((end - start) as usize);
    file.by_ref().take(end - start).read_to_end(&mut bytes)?;
    // The reply goes to whoever asked, a phone included.
    crate::redact::Redactor::default().redact(&mut bytes);
    // A tail read begins mid-line; a read of any kind may end mid-line while the agent writes.
    let first = if resumed.is_none() && start > 0 { bytes.iter().position(|b| *b == b'\n').map_or(bytes.len(), |i| i + 1) } else { 0 };
    let last = bytes.iter().rposition(|b| *b == b'\n').map_or(first, |i| i + 1).max(first);
    let mut messages = Vec::new();
    if resumed.is_some() && last == first && end - start == MAX_READ {
        // One line longer than a whole read (a tool result of many images, logged as base64
        // twice): passed over once it ends, or every later read would read it again for nothing.
        if let Some(past) = line_end(&mut file, end)? {
            messages.push(TranscriptMessage { id: format!("o{start}"), role: "note".into(), body: json!({"text": "A long entry was skipped", "kind": "skipped"}), created_at: String::new(), client_id: None });
            return Ok(TranscriptOut { messages, cursor: (tag << OFFSET_BITS) | past, before: (tag << OFFSET_BITS) | start, older: start > 0, reset, found: true, pending: Vec::new() });
        }
    }
    let mut offset = start + first as u64;
    let mut cursor = start + last as u64;
    // Where each line's messages begin, and the line's offset: a page back starts at a line.
    let mut lines: Vec<(usize, u64)> = Vec::new();
    for line in bytes[first..last].split(|b| *b == b'\n') {
        let at = offset;
        offset += line.len() as u64 + 1;
        // Resumed, the oldest `limit` are kept and the cursor stops after the line that holds
        // the last of them, so the rest come next time rather than never.
        if resumed.is_some() && messages.len() >= limit {
            cursor = at;
            break;
        }
        let Ok(record) = serde_json::from_slice::<Value>(line) else { continue };
        let had = messages.len();
        match provider {
            Provider::Claude => claude_record(&record, at, worktree, helper, &mut messages),
            Provider::Codex => codex_record(&record, at, &mut messages),
            Provider::Shell => {}
        }
        if messages.len() > had { lines.push((had, at)); }
    }
    let pending = sent_messages(provider, &mut file, worktree, tag, start + first as u64, &mut messages, &lines, sends)?;
    let pending = if back.is_some() { Vec::new() } else { pending };
    let mut head = start + first as u64;
    if let Some(&(_, at)) = lines.first() { head = at; }
    if messages.len() > limit && resumed.is_none() {
        // The newest `limit`, from the start of the line that holds the oldest of them, so a
        // page back never splits one line's messages.
        let oldest = messages.len() - limit;
        if let Some(&(from, at)) = lines.iter().rev().find(|(from, _)| *from <= oldest) {
            messages.drain(..from);
            head = at;
        }
    }
    // A page back keeps the live cursor where the reader has it.
    let cursor = match (back, before) { (Some(_), Some(b)) => b, _ => (tag << OFFSET_BITS) | cursor };
    Ok(TranscriptOut { messages, cursor, before: (tag << OFFSET_BITS) | head, older: head > 0, reset, found: true, pending })
}

/// Give each send the first person's message with its words logged after it was typed, oldest
/// send first, so two sends of the same words take two messages. A send matched before this read
/// finds its message in the log just before it ([`SENT_LOOKBACK`]); one typed longer ago than
/// that was answered long since. The sends left over are returned: typed, not logged yet.
#[allow(clippy::too_many_arguments)]
fn sent_messages(
    provider: Provider, file: &mut File, worktree: &Path, tag: u64, from: u64, messages: &mut [TranscriptMessage],
    lines: &[(usize, u64)], sends: &[Sent],
) -> std::io::Result<Vec<relay_bus::ops::session::TranscriptPending>> {
    if sends.is_empty() { return Ok(Vec::new()); }
    let since = |s: &Sent| if s.log_tag == Some(tag) { s.log_offset.unwrap_or(0) } else { 0 };
    let mut live: Vec<&Sent> = sends.iter().filter(|s| since(s) >= from.saturating_sub(SENT_LOOKBACK)).collect();
    live.sort_by_key(|s| since(s));
    // The person's words in the log from the oldest send on: (line offset, words, index here).
    let mut said: Vec<(u64, String, Option<usize>)> = Vec::new();
    if let Some(lo) = live.first().map(|s| since(s)).filter(|lo| *lo < from) {
        let mut bytes = Vec::with_capacity((from - lo) as usize + 1);
        file.seek(SeekFrom::Start(lo.saturating_sub(1)))?;
        file.by_ref().take(from - lo.saturating_sub(1)).read_to_end(&mut bytes)?;
        crate::redact::Redactor::default().redact(&mut bytes);
        // `lo` was the log's length when the send was typed, so normally a line's start.
        let skip = if lo == 0 || bytes.first() == Some(&b'\n') { usize::from(lo > 0) } else { bytes.iter().position(|b| *b == b'\n').map_or(bytes.len(), |i| i + 1) };
        let mut at = lo.saturating_sub(1) + skip as u64;
        for line in bytes[skip..].split(|b| *b == b'\n') {
            let line_at = at;
            at += line.len() as u64 + 1;
            let Ok(record) = serde_json::from_slice::<Value>(line) else { continue };
            let mut found = Vec::new();
            match provider {
                Provider::Claude => claude_record(&record, line_at, worktree, false, &mut found),
                Provider::Codex => codex_record(&record, line_at, &mut found),
                Provider::Shell => {}
            }
            said.extend(found.iter().filter(|m| m.role == "user").map(|m| (line_at, words(&m.body), None)));
        }
    }
    for (i, &(first, at)) in lines.iter().enumerate() {
        let end = lines.get(i + 1).map_or(messages.len(), |next| next.0);
        said.extend((first..end).filter(|k| messages[*k].role == "user").map(|k| (at, words(&messages[k].body), Some(k))));
    }
    let mut taken = vec![false; said.len()];
    let mut pending = Vec::new();
    for s in live {
        // The log is redacted before anyone reads it: the words are compared as it holds them.
        let mut sent = s.text.trim().as_bytes().to_vec();
        crate::redact::Redactor::default().redact(&mut sent);
        let sent = String::from_utf8_lossy(&sent);
        let hit = said.iter().enumerate().find(|(j, (at, text, _))| !taken[*j] && *at >= since(s) && text.trim() == sent.trim()).map(|(j, (_, _, k))| (j, *k));
        match hit {
            Some((j, k)) => {
                taken[j] = true;
                if let Some(k) = k { messages[k].client_id = Some(s.client_id.clone()); }
            }
            None => pending.push(relay_bus::ops::session::TranscriptPending { client_id: s.client_id.clone(), text: s.text.clone(), sent_at: s.sent_at.clone() }),
        }
    }
    Ok(pending)
}

/// A person's message's words.
fn words(body: &Value) -> String {
    body.get("text").and_then(Value::as_str).unwrap_or_default().to_string()
}

/// The offset just past the first newline at or after `from`, or `None` when the file ends first
/// (the line is still being written).
fn line_end(file: &mut File, from: u64) -> std::io::Result<Option<u64>> {
    file.seek(SeekFrom::Start(from))?;
    let mut chunk = vec![0u8; 64 * 1024];
    let mut at = from;
    loop {
        let n = file.read(&mut chunk)?;
        if n == 0 {
            return Ok(None);
        }
        if let Some(i) = chunk[..n].iter().position(|b| *b == b'\n') {
            return Ok(Some(at + i as u64 + 1));
        }
        at += n as u64;
    }
}

fn message(id: String, role: &str, body: Value, record: &Value) -> TranscriptMessage {
    let created_at = record.get("timestamp").and_then(Value::as_str).unwrap_or_default().to_string();
    TranscriptMessage { id, role: role.to_string(), body, created_at, client_id: None }
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

fn claude_record(record: &Value, at: u64, worktree: &Path, helper: bool, out: &mut Vec<TranscriptMessage>) {
    // A subagent's own turns (unless this is its own log), and the context Claude Code adds for
    // the model, are not the conversation.
    let sidechain = record.get("isSidechain").and_then(Value::as_bool) == Some(true);
    if (sidechain && !helper) || record.get("isMeta").and_then(Value::as_bool) == Some(true) {
        return;
    }
    let id = record.get("uuid").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("o{at}"));
    let content = record.pointer("/message/content");
    match record.get("type").and_then(Value::as_str) {
        // After a compaction Claude Code hands the model its summary as a user turn: the
        // person never wrote it.
        Some("user") if record.get("isCompactSummary").and_then(Value::as_bool) == Some(true) => {
            let text = match content {
                Some(Value::String(text)) => text.clone(),
                Some(Value::Array(blocks)) => blocks.iter().filter_map(|block| block.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join("\n\n"),
                _ => String::new(),
            };
            let mut body = json!({"text": "What it kept from before", "kind": "summary"});
            let summary = compact_summary(&text);
            if !summary.is_empty() { body["detail"] = json!(cut_lines(&summary, MAX_COMPACT_SUMMARY)); }
            out.push(message(id, "note", body, record));
        }
        Some("user") => claude_user(&id, content, record, out),
        // What the person sent while the agent worked: Claude Code hands it over at the next
        // step, mid-turn, and logs it as an attachment rather than a user turn.
        Some("attachment") => {
            let Some(queued) = record.get("attachment").filter(|a| a.get("type").and_then(Value::as_str) == Some("queued_command")) else { return };
            let human = queued.pointer("/origin/kind").and_then(Value::as_str) == Some("human") || queued.get("humanTurn").and_then(Value::as_bool) == Some(true);
            if human { claude_user(&id, queued.get("prompt"), record, out) }
        }
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
            let meta = record.get("compactMetadata");
            let manual = meta.and_then(|meta| meta.get("trigger")).and_then(Value::as_str) == Some("manual");
            let mut text = String::from(if manual { "Context compacted" } else { "Context full, compacted automatically" });
            let count = |key: &str| meta.and_then(|meta| meta.get(key)).and_then(Value::as_u64);
            if let (Some(before), Some(after)) = (count("preTokens"), count("postTokens")) {
                text.push_str(&format!(" · {} → {} tokens", tokens(before), tokens(after)));
            }
            out.push(message(id, "note", json!({"text": text, "kind": "compact"}), record));
        }
        _ => {}
    }
}

/// A person's turn, as words (text and pictures) and the tools' answers it carries.
fn claude_user(id: &str, content: Option<&Value>, record: &Value, out: &mut Vec<TranscriptMessage>) {
    match content {
        Some(Value::String(text)) => {
            if let Some((role, body)) = claude_user_text(text) { out.push(message(id.to_string(), role, body, record)); }
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
                        else if let Some((role, body)) = claude_user_text(text) {
                            if role == "note" { out.push(message(format!("{id}:{i}"), role, body, record)) }
                            else { words.push(body["text"].as_str().unwrap_or_default().to_string()) }
                        }
                    }
                    Some("image") => words.push("[image]".to_string()),
                    _ => {}
                }
            }
            if !words.is_empty() { out.push(message(id.to_string(), "user", json!({"text": words.join("\n\n")}), record)); }
        }
        _ => {}
    }
}

/// "973k", "1.2M": a context's size, as a person reads it.
fn tokens(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_499 => format!("{}k", (n + 500) / 1000),
        _ => format!("{:.1}M", n as f64 / 1e6),
    }
}

/// A compaction's summary without the lines addressed to the model: the opening that says a
/// summary follows, and the closing that tells it to carry on.
fn compact_summary(text: &str) -> String {
    let mut lines: Vec<&str> = text.trim().lines().collect();
    if lines.first().is_some_and(|line| line.starts_with("This session is being continued")) {
        let body = lines.iter().position(|line| line.trim() == "Summary:").filter(|at| *at < 6).map_or(1, |at| at + 1);
        lines.drain(..body.min(lines.len()));
    }
    while lines.last().is_some_and(|line| {
        let line = line.trim();
        line.is_empty() || line.starts_with("If you need specific details from before compaction") || line.starts_with("Continue the conversation from where it left off")
            || line.starts_with("Please continue the conversation from where we left")
    }) {
        lines.pop();
    }
    lines.join("\n").trim().to_string()
}

/// A note's body.
fn note(text: impl Into<String>, kind: &str) -> Value { json!({"text": text.into(), "kind": kind}) }

/// What a person typed, as they would read it, or a note for what Relay or Claude Code typed in
/// their place; `None` for context Claude Code added for the model.
fn claude_user_text(text: &str) -> Option<(&'static str, Value)> {
    let unwrapped = PASTED.replace_all(text, "");
    let trimmed = unwrapped.trim();
    if trimmed.starts_with("[Request interrupted") { return Some(("note", note("Interrupted", "interrupt"))); }
    if let Some(name) = between(trimmed, "<command-name>", "</command-name>") {
        let args = between(trimmed, "<command-args>", "</command-args>").unwrap_or_default();
        let name = if name.starts_with('/') { name.to_string() } else { format!("/{name}") };
        return Some(("user", json!({"text": if args.trim().is_empty() { name } else { format!("{name} {}", args.trim()) }})));
    }
    if trimmed.starts_with("<task-notification>") {
        // Claude Code's own line for it: `Background command "Run the tests" completed (exit code 0)`.
        let said = between(trimmed, "<summary>", "</summary>").map(str::trim).filter(|summary| !summary.is_empty());
        return Some(("note", note(said.map_or_else(|| "A background task finished".to_string(), line_of), "task")));
    }
    // `<local-command-stdout>`, `<system-reminder>` and the like: context, not words.
    if trimmed.starts_with('<') && trimmed.split_once('>').is_some_and(|(tag, _)| tag[1..].chars().all(|c| c.is_ascii_lowercase() || c == '-')) {
        return None;
    }
    if let Some(found) = relay_typed(trimmed) { return Some(found); }
    (!trimmed.is_empty()).then(|| ("user", json!({"text": trimmed})))
}

/// The tags Claude Code logs a pasted message in (`<pasted_content id="d72e">`…`</pasted_content id="d72e">`):
/// the person sees only what they pasted.
static PASTED: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r"</?pasted_content(?:\s[^>]*)?>").unwrap());

/// What Relay types into an agent's terminal (`handlers::session`: `launch_nudge`,
/// `assignment_text`), read back as what it was: the person's own launch words where it carries
/// them, otherwise a line saying what Relay asked.
fn relay_typed(text: &str) -> Option<(&'static str, Value)> {
    let task = |rest: &str| rest.split_once("Task #").and_then(|(_, n)| n.split(|c: char| !c.is_ascii_digit()).next()).filter(|n| !n.is_empty()).map(str::to_string);
    if let Some(rest) = text.strip_prefix("Call session.bootstrap first") {
        if let Some(words) = rest.strip_prefix(", then begin this assignment:") {
            return Some(("user", json!({"text": words.trim()})));
        }
        if let Some((_, words)) = rest.split_once("check before acting on them:") {
            let mut body = note("Relay restarted this agent with the instructions it was launched with", "relay");
            body["detail"] = json!(cut_lines(words.trim(), MAX_DETAIL));
            return Some(("note", body));
        }
        let n = task(rest)?;
        let what = if rest.contains("review-group work") { format!("Relay started the review of Task #{n}") }
            else if rest.contains("documentation work") { format!("Relay started the docs for Task #{n}") }
            else { format!("Relay started Task #{n}") };
        return Some(("note", note(what, "relay")));
    }
    if let Some(rest) = text.strip_prefix("Your current assignment is Task #") {
        return Some(("note", note(format!("Relay assigned Task #{}", task(&format!("Task #{rest}"))?), "relay")));
    }
    if text.starts_with("All builders finished Task #") {
        return Some(("note", note(format!("The builders finished Task #{}; Relay asked for its review", task(text)?), "relay")));
    }
    None
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
            if let Some((role, body)) = relay_typed(text) { out.push(message(id, role, body, record)); }
            else if !text.is_empty() { out.push(message(id, "user", json!({"text": text}), record)); }
        }
        ("event_msg", "turn_aborted") => out.push(message(id, "note", note("Interrupted", "interrupt"), record)),
        // The history replaced by a summary of it, which Codex keeps to itself.
        ("compacted", _) => out.push(message(id, "note", note("Context compacted", "compact"), record)),
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
        assert_eq!(crate::providers::claude_project_key(Path::new("/home/a/dev3/RelayV4/.relay/worktrees/proud-llama")), "-home-a-dev3-RelayV4--relay-worktrees-proud-llama");
    }

    #[test]
    fn a_task_reads_its_helpers_own_steps() {
        let dir = tempfile::tempdir().unwrap();
        let session = dir.path().join("c5201e10.jsonl");
        let wt = Path::new("/wt");
        fs::write(&session, lines(&[
            json!({"type": "assistant", "uuid": "a1", "message": {"content": [{"type": "tool_use", "id": "toolu_01", "name": "Task", "input": {"description": "Map the roots", "prompt": "Find where guardrail roots are set"}}]}}),
        ])).unwrap();
        let helpers = dir.path().join("c5201e10").join("subagents");
        fs::create_dir_all(&helpers).unwrap();
        fs::write(helpers.join("agent-a2d3.meta.json"), r#"{"agentType":"Explore","description":"Map the roots","toolUseId":"toolu_01","finished":true}"#).unwrap();
        fs::write(helpers.join("agent-zzzz.meta.json"), r#"{"toolUseId":"toolu_other"}"#).unwrap();
        // As this Claude Code writes it: every turn a sidechain one, its brief first.
        fs::write(helpers.join("agent-a2d3.jsonl"), lines(&[
            json!({"type": "user", "uuid": "h0", "isSidechain": true, "agentId": "a2d3", "message": {"role": "user", "content": "Find where guardrail roots are set"}}),
            json!({"type": "user", "uuid": "hm", "isSidechain": true, "isMeta": true, "message": {"content": "context"}}),
            json!({"type": "assistant", "uuid": "h1", "isSidechain": true, "message": {"content": [{"type": "tool_use", "id": "g1", "name": "Grep", "input": {"pattern": "write_roots"}}]}}),
            json!({"type": "user", "uuid": "h2", "isSidechain": true, "message": {"content": [{"type": "tool_result", "tool_use_id": "g1", "content": "crates/relay-core/src/guardrails.rs:40"}]}}),
            json!({"type": "assistant", "uuid": "h3", "isSidechain": true, "message": {"content": [{"type": "text", "text": "They are set in guardrails.rs."}]}}),
        ])).unwrap();
        let log = locate_task(&session, "toolu_01").unwrap();
        assert_eq!(log, helpers.join("agent-a2d3.jsonl"));
        assert_eq!(locate_task(&session, "toolu_nope"), None);
        let out = read(Provider::Claude, &log, wt, None, None, 200, &[]).unwrap();
        let got: Vec<(&str, &str)> = out.messages.iter().map(|m| (m.id.as_str(), m.role.as_str())).collect();
        assert_eq!(got, [("h0", "user"), ("h1", "assistant"), ("h2:0", "tool"), ("h3", "assistant")], "its own turns, its context left out");
        // The session's own log still leaves a helper's turns out.
        let main = read(Provider::Claude, &session, wt, None, None, 200, &[]).unwrap();
        assert_eq!(main.messages.len(), 1);
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
        let out = read(Provider::Claude, &path, wt, None, None, 200, &[]).unwrap();
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
        assert_eq!(out.messages[6].body, json!({"text": "Interrupted", "kind": "interrupt"}));
        assert!(out.found && !out.reset);
    }

    /// A paste reads as what was pasted, and a message sent while the agent worked (logged as a
    /// queued attachment, handed over mid-turn) reads as the person's, as any other.
    #[test]
    fn a_paste_and_a_message_sent_mid_turn_read_as_the_persons() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.jsonl");
        fs::write(&path, lines(&[
            json!({"type": "user", "uuid": "u1", "message": {"content": "\n\n<pasted_content id=\"d72e\">\n1. one\n2. two\n</pasted_content id=\"d72e\">\n"}}),
            json!({"type": "attachment", "uuid": "q1", "timestamp": "2026-11-23T03:32:16Z", "attachment": {"type": "queued_command", "prompt": "make it slower", "origin": {"kind": "human"}, "humanTurn": true}}),
            json!({"type": "attachment", "uuid": "q2", "attachment": {"type": "queued_command", "prompt": [{"type": "text", "text": "this? [Image #11]"}, {"type": "image", "source": {}}], "humanTurn": true}}),
            json!({"type": "attachment", "uuid": "q3", "attachment": {"type": "queued_command", "prompt": "<task-notification>…</task-notification>"}}),
            json!({"type": "attachment", "uuid": "d", "attachment": {"type": "deferred_tools_record", "entries": []}}),
        ])).unwrap();
        let out = read(Provider::Claude, &path, Path::new("/wt"), None, None, 200, &[]).unwrap();
        let got: Vec<(&str, &str, &str)> = out.messages.iter().map(|m| (m.id.as_str(), m.role.as_str(), m.body["text"].as_str().unwrap_or_default())).collect();
        assert_eq!(got, [("u1", "user", "1. one\n2. two"), ("q1", "user", "make it slower"), ("q2", "user", "this? [Image #11]\n\n[image]")]);
        assert_eq!(out.messages[1].created_at, "2026-11-23T03:32:16Z");
    }

    /// A compaction reads as a line saying so, and the summary Claude Code hands the model as a
    /// user turn reads as what the agent kept, never as the person's words.
    #[test]
    fn a_compaction_is_a_note_with_what_it_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.jsonl");
        let summary = "This session is being continued from a previous conversation that ran out of context. The summary below covers the earlier portion of the conversation.\n\nSummary:\n1. Primary Request:\n   - Fix the build\n\nIf you need specific details from before compaction (like exact code snippets), read the full transcript at: /x.jsonl\nContinue the conversation from where it left off without asking the user any further questions.";
        fs::write(&path, lines(&[
            json!({"type": "user", "uuid": "u1", "message": {"content": "fix the build"}}),
            json!({"type": "system", "subtype": "compact_boundary", "uuid": "b1", "content": "Conversation compacted", "compactMetadata": {"trigger": "auto", "preTokens": 972_511, "postTokens": 27_710}}),
            json!({"type": "user", "uuid": "s1", "isCompactSummary": true, "isVisibleInTranscriptOnly": true, "message": {"content": summary}}),
            json!({"type": "system", "subtype": "compact_boundary", "uuid": "b2", "compactMetadata": {"trigger": "manual", "preTokens": 1_260_000, "postTokens": 900}}),
            json!({"type": "user", "uuid": "s2", "isCompactSummary": true, "message": {"content": [{"type": "text", "text": "Just this."}]}}),
        ])).unwrap();
        let out = read(Provider::Claude, &path, Path::new("/wt"), None, None, 200, &[]).unwrap();
        let roles: Vec<(&str, &str)> = out.messages.iter().map(|m| (m.id.as_str(), m.role.as_str())).collect();
        assert_eq!(roles, [("u1", "user"), ("b1", "note"), ("s1", "note"), ("b2", "note"), ("s2", "note")]);
        assert_eq!(out.messages[1].body, json!({"text": "Context full, compacted automatically · 973k → 28k tokens", "kind": "compact"}));
        assert_eq!(out.messages[2].body, json!({"text": "What it kept from before", "kind": "summary", "detail": "1. Primary Request:\n   - Fix the build"}));
        assert_eq!(out.messages[3].body["text"], "Context compacted · 1.3M → 900 tokens");
        assert_eq!(out.messages[4].body["detail"], "Just this.");
    }

    /// What Relay types into the terminal is said to be Relay's, and a background task's note
    /// says which task.
    #[test]
    fn what_relay_typed_reads_as_relay() {
        let said = |text: &str| claude_user_text(text).unwrap();
        assert_eq!(said("Call session.bootstrap first, then begin this assignment: make the icons crisp"), ("user", json!({"text": "make the icons crisp"})));
        assert_eq!(said("Call session.bootstrap first, then begin current Task #42. Work through queued tasks one at a time; session.done completes only the current task."), ("note", json!({"text": "Relay started Task #42", "kind": "relay"})));
        assert_eq!(said("Call session.bootstrap first, then begin the review-group work for current Task #7. Watch Relay mailbox for file-ready messages from the builders.").1["text"], "Relay started the review of Task #7");
        let restarted = said("Call session.bootstrap first. This session was launched earlier with these instructions, which may already be done; check before acting on them: ship it");
        assert_eq!((restarted.0, &restarted.1["kind"], &restarted.1["detail"]), ("note", &json!("relay"), &json!("ship it")));
        assert_eq!(said("Your current assignment is Task #9. Read session.bootstrap for the task and group context before continuing.").1["text"], "Relay assigned Task #9");
        assert_eq!(said("All builders finished Task #9. Review the current task in the shared worktree; report your review outcome before the group advances.").1["text"], "The builders finished Task #9; Relay asked for its review");
        let task = said("<task-notification>\n<task-id>b8</task-id>\n<status>completed</status>\n<summary>Background command \"Run the tests\" completed (exit code 0)</summary>\n</task-notification>");
        assert_eq!(task, ("note", json!({"text": "Background command \"Run the tests\" completed (exit code 0)", "kind": "task"})));
        assert_eq!(said("<task-notification>\n<task-id>b8</task-id>\n</task-notification>").1["text"], "A background task finished");
        // The person's own words, even about Relay, stay theirs.
        assert_eq!(said("Call session.bootstrap first please"), ("user", json!({"text": "Call session.bootstrap first please"})));
    }

    /// A resumed read keeps the oldest `limit` and leaves its cursor after them, so a phone that
    /// was away for 300 messages gets them all over its next reads, with no hole.
    #[test]
    fn a_resumed_read_past_its_limit_leaves_the_rest_for_the_next() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.jsonl");
        let say = |n: usize| json!({"type": "user", "uuid": format!("u{n}"), "message": {"content": format!("message {n}")}});
        fs::write(&path, lines(&[say(0)])).unwrap();
        let wt = Path::new("/wt");
        let first = read(Provider::Claude, &path, wt, None, None, 2, &[]).unwrap();
        let more: Vec<Value> = (1..=5).map(say).collect();
        fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(lines(&more).as_bytes()).unwrap();
        let (mut cursor, mut seen) = (first.cursor, Vec::new());
        for _ in 0..4 {
            let out = read(Provider::Claude, &path, wt, Some(cursor), None, 2, &[]).unwrap();
            assert!(!out.reset && out.messages.len() <= 2);
            seen.extend(out.messages.into_iter().map(|m| m.id));
            cursor = out.cursor;
        }
        assert_eq!(seen, ["u1", "u2", "u3", "u4", "u5"]);
        // A tail read still takes the newest.
        let tail = read(Provider::Claude, &path, wt, None, None, 2, &[]).unwrap();
        assert_eq!(tail.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["u4", "u5"]);
    }

    /// Scrolling back: each page's `before` asks for the one older than it, until none is left,
    /// with every message once, and the live cursor untouched.
    #[test]
    fn pages_back_read_the_whole_conversation_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.jsonl");
        let say = |n: usize| json!({"type": "user", "uuid": format!("u{n}"), "message": {"content": format!("message {n}")}});
        fs::write(&path, lines(&(0..7).map(say).collect::<Vec<_>>())).unwrap();
        let wt = Path::new("/wt");
        let tail = read(Provider::Claude, &path, wt, None, None, 3, &[]).unwrap();
        assert_eq!(tail.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["u4", "u5", "u6"]);
        assert!(tail.older);
        let (mut before, mut pages) = (tail.before, Vec::new());
        loop {
            let page = read(Provider::Claude, &path, wt, None, Some(before), 3, &[]).unwrap();
            assert!(!page.reset);
            assert_eq!(page.cursor, before, "a page back leaves the live cursor alone");
            pages.push(page.messages.iter().map(|m| m.id.clone()).collect::<Vec<_>>());
            if !page.older {
                break;
            }
            before = page.before;
        }
        assert_eq!(pages, [vec!["u1", "u2", "u3"], vec!["u0"]]);
        // The live read goes on from the tail's cursor as before.
        fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(lines(&[say(7)]).as_bytes()).unwrap();
        let on = read(Provider::Claude, &path, wt, Some(tail.cursor), None, 3, &[]).unwrap();
        assert_eq!(on.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["u7"]);
        // A page back from another file's cursor starts over.
        let other = dir.path().join("d.jsonl");
        fs::write(&other, lines(&[say(9)])).unwrap();
        assert!(read(Provider::Claude, &other, wt, None, Some(tail.before), 3, &[]).unwrap().reset);
    }

    /// chat.send's messages come back under their ids: pending until logged, then on their own
    /// message only, once; the same words sent twice take two messages; the same words said
    /// before the send are not taken for it.
    #[test]
    fn a_sent_message_comes_back_under_its_id_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.jsonl");
        let say = |n: usize, text: &str| json!({"type": "user", "uuid": format!("u{n}"), "message": {"content": text}});
        fs::write(&path, lines(&[say(0, "yes")])).unwrap();
        let wt = Path::new("/wt");
        let (tag, len) = position(&path).unwrap();
        let sent = |id: &str, text: &str| Sent { client_id: id.into(), text: text.into(), log_tag: Some(tag), log_offset: Some(len), sent_at: "t".into() };
        let sends = [sent("a", "yes"), sent("b", "yes"), sent("c", "go on\nplease")];
        let ids = |out: &TranscriptOut| out.messages.iter().map(|m| (m.id.clone(), m.client_id.clone())).collect::<Vec<_>>();
        let first = read(Provider::Claude, &path, wt, None, None, 50, &sends).unwrap();
        assert_eq!(ids(&first), [("u0".to_string(), None)], "said before the sends");
        assert_eq!(first.pending.iter().map(|p| p.client_id.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
        // The first is logged (as Claude Code logs a paste).
        let pasted = "<pasted_content id=\"x\">go on\nplease</pasted_content id=\"x\">";
        fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(lines(&[say(1, "yes"), say(2, pasted)]).as_bytes()).unwrap();
        let next = read(Provider::Claude, &path, wt, Some(first.cursor), None, 50, &sends).unwrap();
        assert_eq!(ids(&next), [("u1".to_string(), Some("a".to_string())), ("u2".to_string(), Some("c".to_string()))]);
        assert_eq!(next.pending.iter().map(|p| p.client_id.as_str()).collect::<Vec<_>>(), ["b"]);
        // The second "yes": its own message, and "a" is not given twice.
        fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(lines(&[say(3, "yes")]).as_bytes()).unwrap();
        let last = read(Provider::Claude, &path, wt, Some(next.cursor), None, 50, &sends).unwrap();
        assert_eq!(ids(&last), [("u3".to_string(), Some("b".to_string()))]);
        assert!(last.pending.is_empty());
        // A fresh read from the tail gives each its id again, and a page back no pending.
        let tail = read(Provider::Claude, &path, wt, None, None, 50, &sends).unwrap();
        assert_eq!(tail.messages.iter().filter_map(|m| m.client_id.as_deref()).collect::<Vec<_>>(), ["a", "c", "b"]);
        let back = read(Provider::Claude, &path, wt, None, Some(last.cursor), 50, &sends).unwrap();
        assert!(back.pending.is_empty());
    }

    /// One line longer than a whole read is passed over with a note, once it has ended; before
    /// that, the cursor waits for it.
    #[test]
    fn a_line_longer_than_a_read_is_skipped_once_it_ends() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.jsonl");
        let wt = Path::new("/wt");
        fs::write(&path, lines(&[json!({"type": "user", "uuid": "u0", "message": {"content": "before"}})])).unwrap();
        let cursor = read(Provider::Claude, &path, wt, None, None, 50, &[]).unwrap().cursor;
        let huge = format!("{{\"type\":\"user\",\"uuid\":\"big\",\"message\":{{\"content\":\"{}\"}}}}", "x".repeat(MAX_READ as usize + 10));
        fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(huge.as_bytes()).unwrap();
        let waiting = read(Provider::Claude, &path, wt, Some(cursor), None, 50, &[]).unwrap();
        assert!(waiting.messages.is_empty());
        assert_eq!(waiting.cursor, cursor, "the line is still being written");
        let after = lines(&[json!({"type": "user", "uuid": "u1", "message": {"content": "after"}})]);
        fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(format!("\n{after}").as_bytes()).unwrap();
        let skipped = read(Provider::Claude, &path, wt, Some(cursor), None, 50, &[]).unwrap();
        assert_eq!(skipped.messages.len(), 1);
        assert_eq!((skipped.messages[0].role.as_str(), &skipped.messages[0].body["text"]), ("note", &json!("A long entry was skipped")));
        let next = read(Provider::Claude, &path, wt, Some(skipped.cursor), None, 50, &[]).unwrap();
        assert_eq!(next.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["u1"]);
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
        let out = read(Provider::Claude, &path, wt, None, None, 200, &[]).unwrap();
        assert_eq!(out.messages.len(), 1);
        // Half a line: nothing yet, and the cursor stays put.
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        let second = json!({"type": "user", "uuid": "u2", "message": {"content": "two"}}).to_string();
        file.write_all(&second.as_bytes()[..10]).unwrap();
        let mid = read(Provider::Claude, &path, wt, Some(out.cursor), None, 200, &[]).unwrap();
        assert!(mid.messages.is_empty());
        assert_eq!(mid.cursor, out.cursor);
        file.write_all(format!("{}\n", &second[10..]).as_bytes()).unwrap();
        let next = read(Provider::Claude, &path, wt, Some(mid.cursor), None, 200, &[]).unwrap();
        assert_eq!(next.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["u2"]);
        // A cursor from another file starts again.
        let other = dir.path().join("d.jsonl");
        fs::write(&other, lines(&[json!({"type": "user", "uuid": "x", "message": {"content": "x"}})])).unwrap();
        let moved = read(Provider::Claude, &other, wt, Some(next.cursor), None, 200, &[]).unwrap();
        assert!(moved.reset);
        assert_eq!(moved.messages.len(), 1);
    }

    #[test]
    fn limit_keeps_the_newest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.jsonl");
        let records: Vec<Value> = (0..5).map(|i| json!({"type": "user", "uuid": format!("u{i}"), "message": {"content": format!("m{i}")}})).collect();
        fs::write(&path, lines(&records)).unwrap();
        let out = read(Provider::Claude, &path, Path::new("/wt"), None, None, 2, &[]).unwrap();
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
            json!({"type": "compacted", "payload": {"message": "", "replacement_history": []}}),
        ])).unwrap();
        let out = read(Provider::Codex, &path, Path::new("/wt"), None, None, 200, &[]).unwrap();
        let roles: Vec<&str> = out.messages.iter().map(|m| m.role.as_str()).collect();
        assert_eq!(roles, ["user", "assistant", "tool", "assistant", "assistant", "tool", "assistant", "assistant", "note"]);
        assert_eq!(out.messages[8].body, json!({"text": "Context compacted", "kind": "compact"}));
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
