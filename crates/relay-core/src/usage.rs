//! Bounded, read-only provider usage discovery.
//!
//! Both CLIs already receive rate-limit data while doing normal work. Relay reads the
//! small state they leave behind only when `usage.get` is requested. It never polls a
//! provider, calls a usage endpoint, or spends a provider token to populate chrome.

use relay_bus::types::{Provider, Usage};
use serde_json::{json, Map, Value};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_STATE_FILE: u64 = 2 * 1024 * 1024;
const MAX_CODEX_FILES: usize = 32;
const CODEX_TAIL: u64 = 1024 * 1024;

pub fn read(provider: Provider) -> Option<Usage> {
    match provider {
        Provider::Claude => read_claude(),
        Provider::Codex => read_codex(),
        Provider::Shell => None,
    }
}

fn read_claude() -> Option<Usage> {
    read_claude_file(&crate::providers::claude_usage_file()?)
}

fn read_claude_file(path: &Path) -> Option<Usage> {
    let metadata = fs::metadata(path).ok()?;
    if metadata.len() > MAX_STATE_FILE { return None; }
    let raw = fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&raw).ok()?;
    let limits = find_named(&value, &["rate_limits", "rateLimits"], 0)?;
    let windows = normalize_group(limits);
    if windows.is_empty() { return None; }
    // The status-line hook rewrites this file on every report and the payload carries no
    // timestamp of its own, so the file's mtime is when Claude Code last reported.
    let taken_at = value.get("timestamp").and_then(Value::as_str).map(str::to_string)
        .or_else(|| metadata.modified().ok().and_then(|at| jiff::Timestamp::try_from(at).ok()).map(|at| at.to_string()))
        .unwrap_or_else(crate::time::now);
    Some(Usage { provider: Provider::Claude, windows: Value::Object(windows), taken_at })
}

fn read_codex() -> Option<Usage> {
    let root = crate::providers::config_dir(Provider::Codex)?.join("sessions");
    let mut files = Vec::new();
    collect_jsonl(&root, 0, &mut files);
    files.sort_by_key(|(_, modified)| std::cmp::Reverse(*modified));
    for (path, _) in files.into_iter().take(MAX_CODEX_FILES) {
        // One unreadable rollout says nothing about the others.
        let Ok(raw) = read_tail(&path, CODEX_TAIL) else { continue };
        for line in raw.lines().rev() {
            let Ok(value) = serde_json::from_str::<Value>(line) else { continue };
            let limits = value.pointer("/payload/rate_limits")
                .or_else(|| value.pointer("/payload/rateLimits"));
            let Some(limits) = limits.filter(|value| !value.is_null()) else { continue };
            let windows = normalize_codex(limits);
            if windows.is_empty() { continue; }
            let taken_at = value.get("timestamp").and_then(Value::as_str)
                .map(str::to_string).unwrap_or_else(crate::time::now);
            return Some(Usage { provider: Provider::Codex, windows: Value::Object(windows), taken_at });
        }
    }
    None
}

/// How many rollouts the walk stats before it stops. Only a bound against a pathological tree:
/// a stat is cheap, and the newest file has to be among those seen for the sort to find it.
const MAX_CODEX_WALK: usize = 20_000;

/// Every rollout under `dir` with its mtime. Codex files them as `YYYY/MM/DD/*.jsonl`, and
/// `read_dir` order is creation order on btrfs, hash order on ext4 — never date order. So the
/// walk takes every file and the caller sorts by mtime (a resumed session appends to a file in
/// an older day's directory); directories are visited newest name first so that, if the bound
/// above is ever reached, what is dropped is the oldest days, not the newest (RA-016).
fn collect_jsonl(dir: &Path, depth: u8, out: &mut Vec<(PathBuf, SystemTime)>) {
    if depth > 4 || out.len() >= MAX_CODEX_WALK { return; }
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.file_name()));
    for entry in entries {
        if out.len() >= MAX_CODEX_WALK { return; }
        let Ok(kind) = entry.file_type() else { continue };
        let path = entry.path();
        if kind.is_dir() { collect_jsonl(&path, depth + 1, out); }
        else if path.extension().is_some_and(|extension| extension == "jsonl") {
            let modified = entry.metadata().and_then(|metadata| metadata.modified()).unwrap_or(UNIX_EPOCH);
            out.push((path, modified));
        }
    }
}

fn read_tail(path: &Path, max: u64) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let start = len.saturating_sub(max);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    // The seek can land inside a multi-byte character; that partial first line is dropped
    // below anyway, so decode lossily rather than reject the whole tail.
    let mut raw = String::from_utf8_lossy(&bytes).into_owned();
    if start > 0 {
        raw = raw.split_once('\n').map(|(_, rest)| rest.to_string()).unwrap_or_default();
    }
    Ok(raw)
}

fn find_named<'a>(value: &'a Value, names: &[&str], depth: u8) -> Option<&'a Value> {
    if depth > 8 { return None; }
    match value {
        Value::Object(map) => names.iter().find_map(|name| map.get(*name))
            .or_else(|| map.values().find_map(|child| find_named(child, names, depth + 1))),
        Value::Array(values) => values.iter().find_map(|child| find_named(child, names, depth + 1)),
        _ => None,
    }
}

fn normalize_group(value: &Value) -> Map<String, Value> {
    let mut out = Map::new();
    let Some(group) = value.as_object() else { return out };
    for (name, value) in group {
        if let Some(window) = normalize_window(value) { out.insert(clean_name(name), window); }
        if let Some(items) = value.as_array() {
            for item in items {
                let label = item.pointer("/scope/model/display_name").and_then(Value::as_str)
                    .or_else(|| item.get("name").and_then(Value::as_str));
                if let (Some(label), Some(window)) = (label, normalize_window(item)) {
                    out.entry(clean_name(label)).or_insert(window);
                }
            }
        }
    }
    out
}

fn normalize_codex(value: &Value) -> Map<String, Value> {
    let mut out = Map::new();
    let Some(group) = value.as_object() else { return out };
    for key in ["primary", "secondary", "individual_limit"] {
        let Some(window) = group.get(key).filter(|value| !value.is_null()) else { continue };
        let minutes = window.get("window_minutes").and_then(Value::as_u64);
        let name = match minutes {
            Some(300) => "five_hour".to_string(),
            Some(10_080) => "weekly".to_string(),
            Some(value) => format!("{value}_minute"),
            None => key.to_string(),
        };
        if let Some(window) = normalize_window(window) { out.insert(name, window); }
    }
    out
}

fn normalize_window(value: &Value) -> Option<Value> {
    let object = value.as_object()?;
    let used = ["used_pct", "used_percent", "used_percentage", "utilization", "percent"]
        .iter().find_map(|key| object.get(*key).and_then(number))?;
    let resets = ["resets_at", "resetsAt", "reset_at", "resetAt"]
        .iter().find_map(|key| object.get(*key));
    // `resets_in` is relative to this read; `resets_at` (unix seconds) lets the client show the
    // wall-clock reset and notice a window that has already rolled over since the report.
    Some(json!({"used_pct": used.clamp(0.0, 100.0).round(),
        "resets_in": resets.and_then(reset_label), "resets_at": resets.and_then(reset_epoch)}))
}

fn reset_epoch(value: &Value) -> Option<u64> {
    let epoch = value.as_f64().or_else(|| value.as_str()?.parse::<f64>().ok())?;
    Some(if epoch > 100_000_000_000.0 { epoch / 1000.0 } else { epoch } as u64)
}

fn number(value: &Value) -> Option<f64> {
    value.as_f64().or_else(|| value.as_str()?.trim_end_matches('%').parse().ok())
}

fn reset_label(value: &Value) -> Option<String> {
    let seconds = reset_epoch(value)? as f64;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs_f64();
    Some(crate::time::span((seconds - now).max(0.0) as u64, crate::time::Unit::Minute))
}

fn clean_name(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_claude_and_codex_shapes() {
        let claude = normalize_group(&json!({
            "five_hour":{"used_percentage":32,"resets_at":4102444800_u64},
            "seven_day":{"utilization":71}
        }));
        assert_eq!(claude["five_hour"]["used_pct"], 32.0);
        assert_eq!(claude["five_hour"]["resets_at"], 4102444800_u64);
        assert_eq!(claude["seven_day"]["used_pct"], 71.0);
        assert!(claude["seven_day"]["resets_at"].is_null());
        let codex = normalize_codex(&json!({
            "primary":{"used_percent":44,"window_minutes":10080,"resets_at":4102444800_u64},
            "secondary":null
        }));
        assert_eq!(codex["weekly"]["used_pct"], 44.0);
        // Milliseconds are accepted and normalized to seconds.
        let window = normalize_window(&json!({"used_percent":5,"resets_at":"4102444800000"})).unwrap();
        assert_eq!(window["resets_at"], 4102444800_u64);
    }

    #[test]
    fn claude_report_is_dated_by_the_file_when_it_carries_no_timestamp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("relay-usage.json");
        fs::write(&path, r#"{"rate_limits":{"five_hour":{"used_percentage":7,"resets_at":4102444800}}}"#).unwrap();
        let old = SystemTime::now() - std::time::Duration::from_secs(600);
        File::options().write(true).open(&path).unwrap().set_modified(old).unwrap();
        let usage = read_claude_file(&path).unwrap();
        assert_eq!(usage.windows["five_hour"]["used_pct"], 7.0);
        let taken: jiff::Timestamp = usage.taken_at.parse().unwrap();
        let age = jiff::Timestamp::now().as_second() - taken.as_second();
        assert!((595..=605).contains(&age), "taken_at follows the file's mtime, got {age}s old");
        fs::write(&path, r#"{"timestamp":"2026-01-02T03:04:05Z","rate_limits":{"seven_day":{"utilization":9}}}"#).unwrap();
        assert_eq!(read_claude_file(&path).unwrap().taken_at, "2026-01-02T03:04:05Z");
    }

    /// RA-016: the walk used to stop after ~256 files in `read_dir` order, which on btrfs is
    /// oldest-first, so the newest rollouts were never even looked at.
    #[test]
    fn the_newest_codex_rollout_is_found_however_many_older_ones_exist() {
        let dir = tempfile::tempdir().unwrap();
        let base = SystemTime::now() - std::time::Duration::from_secs(400 * 86_400);
        // Created oldest first, as Codex would have written them.
        for day in 0..40u64 {
            let folder = dir.path().join(format!("2026/{:02}/{:02}", 1 + day / 28, 1 + day % 28));
            fs::create_dir_all(&folder).unwrap();
            for n in 0..10u64 {
                let path = folder.join(format!("rollout-{n}.jsonl"));
                fs::write(&path, "{}\n").unwrap();
                let at = base + std::time::Duration::from_secs((day * 10 + n) * 3600);
                File::options().write(true).open(&path).unwrap().set_modified(at).unwrap();
            }
        }
        // A resumed session appends to a file in an old day's folder: newest by mtime.
        let resumed = dir.path().join("2026/01/02/rollout-3.jsonl");
        File::options().write(true).open(&resumed).unwrap().set_modified(SystemTime::now()).unwrap();

        let mut files = Vec::new();
        collect_jsonl(dir.path(), 0, &mut files);
        assert_eq!(files.len(), 400, "every rollout is seen");
        files.sort_by_key(|(_, modified)| std::cmp::Reverse(*modified));
        assert_eq!(files[0].0, resumed);
        assert!(files[1].0.ends_with("2026/02/12/rollout-9.jsonl"), "{:?}", files[1].0);
    }

    #[test]
    fn a_tail_that_starts_inside_a_character_still_reads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rollout.jsonl");
        fs::write(&path, "ééééé\n{\"a\":1}\n").unwrap();
        // 11 bytes from the end lands in the middle of the last `é`.
        assert_eq!(read_tail(&path, 11).unwrap(), "{\"a\":1}\n");
    }
}
