//! Sample data for screenshots (`--shot`): what a PC might say, so a screen can be looked at with no
//! PC paired. Never used by the app itself.

use rc_link::{LinkState, Route as Way};
use serde_json::json;

use crate::app::App;
use crate::data::{self, Convo};

pub fn fill(app: &mut App) {
    app.shot = true;
    app.set_link_state(LinkState::Online { host: "anthony-desk".into(), route: Way::new("ws://192.168.1.20:7420") });
    app.projects = data::projects(&json!({"projects": [
        {"id": 1, "name": "RelayV4", "base_branch": "main"},
        {"id": 2, "name": "SuperStar-feel-the-beat", "base_branch": "main"},
        {"id": 3, "name": "Discord-bot-V2", "base_branch": "main"},
    ]}));
    app.repo = Some(1);
    let now = rc_link::link::now_ms();
    let at = |mins: u64| {
        let s = (now / 1000).saturating_sub(mins * 60);
        let days = s / 86_400;
        let (y, m, d) = civil(days as i64);
        let r = s % 86_400;
        format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", r / 3600, r / 60 % 60, r % 60)
    };
    app.agents = data::agents(&json!({"sessions": [
        {"name": "umber-zebra", "project_id": 1, "provider": "claude", "role": "builder", "state": "blocked", "intent": "Push relay/umber-zebra?", "branch": "relay/umber-zebra", "last_output_at": at(2), "model": "claude-opus-5-5"},
        {"name": "bold-puffin", "project_id": 1, "provider": "claude", "role": "builder", "state": "running", "intent": "Fix duplicate chat bubbles", "branch": "relay/bold-puffin", "last_output_at": at(0), "model": "claude-opus-5-5"},
        {"name": "swift-ocelot", "project_id": 1, "provider": "claude", "role": "builder", "state": "running", "intent": "Leaf comics autopilot", "branch": "relay/swift-ocelot", "last_output_at": at(1), "model": "claude-sonnet-5-5"},
        {"name": "vivid-ferret", "project_id": 1, "provider": "claude", "role": "reviewer", "state": "idle", "intent": "Leaf review fixes", "branch": "relay/vivid-ferret", "last_output_at": at(190)},
        {"name": "calm-heron", "project_id": 2, "provider": "claude", "role": "builder", "state": "idle", "intent": "Hit windows on fast songs", "branch": "relay/calm-heron", "last_output_at": at(600)},
    ]}));
    let mut c = Convo::default();
    c.take(&json!({"cursor": 900, "reset": true, "found": true, "messages": [
        {"id": "1", "role": "user", "created_at": at(9), "body": {"text": "Bubbles duplicate when I send while it's working. Fix it?"}},
        {"id": "2", "role": "assistant", "created_at": at(9), "body": {"blocks": [
            {"type": "thinking", "text": "The phone keeps its own copy of a sent message and drops it once the log has it."},
            {"type": "tool_use", "name": "Read", "summary": "data/SessionChat.kt"},
            {"type": "tool_use", "name": "Grep", "summary": "gist("},
            {"type": "tool_use", "name": "Read", "summary": "core/sync/Outbox.kt"}]}},
        {"id": "3", "role": "assistant", "created_at": at(8), "body": {"blocks": [
            {"type": "text", "text": "Found it: the phone matched its own copy to the log **by the words**. A queued message is logged differently, so the match missed and both stayed.\n\n```kotlin\nval said = users.any { it.clientId == mine.id }\n```"}]}},
        {"id": "3b", "role": "assistant", "created_at": at(7), "body": {"blocks": [{"type": "tool_use", "name": "Edit", "summary": "data/SessionChat.kt", "detail": "-        val said = users.any { it.id !in known && gist(it.text) == gist(mine.text) }\n+        val said = users.any { it.clientId == mine.id }"}]}},
        {"id": "4", "role": "assistant", "created_at": at(6), "body": {"blocks": [{"type": "tool_use", "name": "Bash", "summary": "cargo test -p relay-remote"}]}},
        {"id": "5", "role": "tool", "created_at": at(5), "body": {"is_error": false, "text": "test result: ok. 128 passed"}},
        {"id": "6", "role": "assistant", "created_at": at(5), "body": {"blocks": [{"type": "text", "text": "Tests pass, 128 of 128. What changed:\n\n- The phone keeps its copy until the log has it\n- **Matched by count**, never by words ([PR #206](https://github.com/antho976/Relay/pull/206))\n  - a queued message can no longer show twice"}]}},
    ]}));
    app.convos.insert("bold-puffin".into(), c);
    app.screens.insert("bold-puffin".into(), crate::screen::read("· Crunching… (1m 6s · ↓ 6.3k tokens · thought for 2s)\n────\n❯ \n────\n  ⏵⏵ auto mode on (shift+tab to cycle) · esc to interrupt"));
    app.screens.insert("umber-zebra".into(), crate::screen::read("│ Bash command │\n│   git push -u origin relay/umber-zebra │\n│ Do you want to proceed? │\n│ ❯ 1. Yes │\n│   2. Yes, and don't ask again for git push commands │\n│   3. No, and tell Claude what to do differently (esc) │\n  ⏵⏵ accept edits on (shift+tab to cycle)"));
    crate::changes::sample(app);
    crate::board::sample(app);
    crate::inbox::sample(app);
    crate::designs::sample(app);
    let mut s = data::Sending::default();
    s.push("Good. Push it when the tests pass", now);
    app.sending.insert("bold-puffin".into(), s);
}

/// The civil date of a day count since 1970 (Howard Hinnant's algorithm).
fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
