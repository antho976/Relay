//! Rows that tell you what happened (backlog D12), as Claude's app lists conversations: each agent
//! row shows its latest words in one quiet line, and a dot when it has news you have not seen: it
//! finished its turn, or it asks, since you last had its chat open. Opening the chat clears it.
//!
//! No request per row. "Seen" is when you last had the chat open (kept in `cache/seen.json`),
//! compared with the `last_output_at` every `session.list` already carries. The words come from the
//! conversation in memory, or from the phone's own copy of it on disk (`cache/chat-*.json`), read
//! once per agent off the UI thread; an agent never opened here has none until it is.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::app::{App, Route};
use crate::data::{self, Agent, Convo};

/// Output within this long after you left does not count as news (the last redraw of its prompt).
const SLACK_MS: u64 = 3_000;

#[derive(Default)]
pub struct State {
    /// When each agent's chat was last open here (ms).
    seen: HashMap<String, u64>,
    loaded: bool,
    /// The chat open on the last frame, to note when you leave it.
    open: Option<String>,
    /// Previews worked out from conversations in memory, with what they were worked out from.
    previews: HashMap<String, ((usize, String), Option<String>)>,
    /// Previews read off the UI thread from the phone's copies, and the agents asked for.
    disk: Arc<Mutex<HashMap<String, Option<String>>>>,
    asked: HashSet<String>,
}

fn seen_file(app: &App) -> std::path::PathBuf {
    app.data.join("cache").join("seen.json")
}

fn load(app: &mut App) {
    if !app.st.news.loaded {
        app.st.news.loaded = true;
        // A few hundred bytes, read once.
        if let Some(s) = crate::app::read_json::<HashMap<String, u64>>(seen_file(app)) {
            app.st.news.seen = s;
        }
    }
}

fn save(app: &App) {
    crate::app::write_json(seen_file(app), &app.st.news.seen);
}

/// The agent's latest words, one line, plain: what Markdown marks and whitespace are not.
pub fn line_of(convo: &Convo) -> Option<String> {
    let items = data::items(&convo.msgs);
    let said = items.iter().rev().find_map(|it| match it {
        data::Item::Said(s) => Some(s.as_str()),
        _ => None,
    })?;
    let line = said.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with("```"))?;
    let plain: String = line.trim_start_matches(['#', '-', '*', '>', ' ']).chars().filter(|c| !matches!(c, '*' | '`' | '_')).collect();
    let plain = plain.trim().to_string();
    (!plain.is_empty()).then_some(plain)
}

/// Each frame: note the chat you are in (and when you leave it), give a first-seen agent its
/// baseline, and read the phone's copies for agents whose words are not in memory yet.
pub fn keep_up(app: &mut App) {
    load(app);
    let now = rc_link::link::now_ms();
    let open = match app.route() {
        Route::Chat(n) => Some(n.clone()),
        _ => None,
    };
    let mut changed = false;
    if let Some(n) = &open {
        app.st.news.seen.insert(n.clone(), now);
    }
    if app.st.news.open != open {
        // Left a chat, or opened one: what you saw is kept.
        changed = true;
        app.st.news.open = open;
    }
    // An agent first seen here starts seen at its last output: nothing old lights up.
    for a in &app.agents {
        if !app.st.news.seen.contains_key(&a.name) {
            let at = a.last_output_at.as_deref().and_then(data::parse_ms).unwrap_or(now);
            app.st.news.seen.insert(a.name.clone(), at);
            changed = true;
        }
    }
    if changed {
        save(app);
    }
    // The phone's copies of conversations not in memory, read once each on a thread.
    let want: Vec<String> = app
        .agents
        .iter()
        .chain(app.earlier.iter())
        .filter(|a| Some(a.project_id) == app.repo && !app.convos.contains_key(&a.name) && !app.st.news.asked.contains(&a.name))
        .map(|a| a.name.clone())
        .collect();
    if !want.is_empty() && !app.shot {
        app.st.news.asked.extend(want.iter().cloned());
        let disk = app.st.news.disk.clone();
        let data = app.data.clone();
        let _ = std::thread::Builder::new().name("rc-previews".into()).spawn(move || {
            for name in want {
                let line = crate::app::read_json::<Convo>(crate::app::chat_file(&data, &name)).as_ref().and_then(line_of);
                disk.lock().unwrap_or_else(|e| e.into_inner()).insert(name, line);
            }
            crate::wake::wake();
        });
    }
}

/// The agent's latest words for its row, if the phone has them.
pub fn preview(app: &mut App, name: &str) -> Option<String> {
    if let Some(c) = app.convos.get(name) {
        let key = (c.msgs.len(), c.msgs.last().map(|m| m.id.clone()).unwrap_or_default());
        if let Some((k, p)) = app.st.news.previews.get(name)
            && *k == key
        {
            return p.clone();
        }
        // Worked out again only when its conversation changed.
        let p = line_of(c);
        app.st.news.previews.insert(name.to_string(), (key, p.clone()));
        return p;
    }
    app.st.news.disk.lock().unwrap_or_else(|e| e.into_inner()).get(name).cloned().flatten()
}

/// It has news you have not seen: a live agent, not at work now, that wrote after you last had its
/// chat open (it finished its turn, or it asks).
pub fn unseen(app: &App, a: &Agent) -> bool {
    if !crate::agents::live(a) || app.open_chat().as_deref() == Some(a.name.as_str()) || app.busy(&a.name) {
        return false;
    }
    let Some(out) = a.last_output_at.as_deref().and_then(data::parse_ms) else { return false };
    app.st.news.seen.get(&a.name).is_some_and(|seen| out > seen + SLACK_MS)
}

/// Screenshots and tests: `name` last seen at `ms`.
pub fn set_seen(app: &mut App, name: &str, ms: u64) {
    app.st.news.loaded = true;
    app.st.news.seen.insert(name.to_string(), ms);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_preview_is_the_agents_latest_words_plain() {
        let mut c = Convo::default();
        c.take(&json!({"cursor": 3, "reset": true, "found": true, "messages": [
            {"id": "1", "role": "user", "created_at": "2026-11-26T10:00:00Z", "body": {"text": "Fix it"}},
            {"id": "2", "role": "assistant", "created_at": "2026-11-26T10:01:00Z", "body": {"blocks": [{"type": "text", "text": "## Done\n\n**Tests pass**, 128 of 128."}]}},
            {"id": "3", "role": "assistant", "created_at": "2026-11-26T10:02:00Z", "body": {"blocks": [{"type": "tool_use", "name": "Bash", "summary": "cargo test"}]}},
        ]}));
        assert_eq!(line_of(&c).as_deref(), Some("Done"));
        let mut only_you = Convo::default();
        only_you.take(&json!({"cursor": 1, "reset": true, "found": true, "messages": [{"id": "1", "role": "user", "created_at": "2026-11-26T10:00:00Z", "body": {"text": "Hi"}}]}));
        assert_eq!(line_of(&only_you), None, "your own words are not its preview");
    }
}
