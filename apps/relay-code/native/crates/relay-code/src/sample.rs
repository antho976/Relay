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
    // Stopped and closed ones, for Earlier (D1).
    app.agents.extend(data::agents(&json!({"sessions": [
        {"name": "keen-newt", "project_id": 1, "provider": "claude", "role": "builder", "state": "restorable", "intent": "Video plugin round two", "branch": "relay/keen-newt", "last_output_at": at(300), "updated_at": at(240)},
    ]})));
    app.restorable.insert("keen-newt".into(), ("app_restart".into(), false));
    app.earlier = data::agents(&json!({"sessions": [
        {"name": "vivid-dingo", "project_id": 1, "provider": "claude", "role": "builder", "state": "closed", "intent": "Design canvas round two", "branch": "relay/vivid-dingo", "updated_at": at(1500)},
        {"name": "jade-ibis", "project_id": 1, "provider": "claude", "role": "builder", "state": "closed", "intent": "Polish Clips, Notes and Board", "branch": "relay/jade-ibis", "updated_at": at(3000)},
        {"name": "lucky-gecko", "project_id": 1, "provider": "claude", "role": "reviewer", "state": "closed", "intent": "Read-only audit, ten sessions", "branch": "relay/lucky-gecko", "updated_at": at(4400)},
        {"name": "coral-osprey", "project_id": 1, "provider": "claude", "role": "builder", "state": "closed", "intent": "Assistant round two", "branch": "relay/coral-osprey", "updated_at": at(6000)},
        {"name": "lucky-jackal", "project_id": 1, "provider": "claude", "role": "builder", "state": "exited", "intent": "Threads v2 readings", "branch": "relay/lucky-jackal", "updated_at": at(9000)},
    ]}));
    let mut k = Convo::default();
    k.take(&json!({"cursor": 40, "reset": true, "found": true, "messages": [
        {"id": "k1", "role": "user", "created_at": at(320), "body": {"text": "Add the timeline to the video preview."}},
        {"id": "k2", "role": "assistant", "created_at": at(300), "body": {"blocks": [{"type": "text", "text": "The timeline is in: clips, sound and captions on three tracks, scrubbed with a finger. Tests pass."}]}},
    ]}));
    app.convos.insert("keen-newt".into(), k);
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

/// What an agent did, call by call (A3): a to-do list, a failing test with its output, an edit,
/// the passing run, and, with `plan`, a plan put to you that waits for your answer. On swift-ocelot.
pub fn tools(app: &mut App, plan: bool) {
    let fail = (1..=18).map(|i| format!("test flows::case_{i:02} ... ok")).chain([
        "test data::tests::a_failed_send_is_tapped_to_go_again ... FAILED".to_string(),
        "".into(),
        "failures:".into(),
        "---- data::tests::a_failed_send_is_tapped_to_go_again stdout ----".into(),
        "assertion `left == right` failed: its words never went: typed again".into(),
        "  left: None".into(),
        " right: Some(Type(\"a\"))".into(),
        "test result: FAILED. 84 passed; 1 failed; 3 ignored".into(),
    ]).collect::<Vec<_>>().join("\n");
    let pass = (1..=30).map(|i| format!("test screen::tests::case_{i:02} ... ok")).chain(["test result: ok. 85 passed; 0 failed; 3 ignored".to_string()]).collect::<Vec<_>>().join("\n");
    let mut msgs = vec![
        json!({"id": "t0", "role": "user", "body": {"text": "Make a failed send retry from where it stopped"}, "created_at": ""}),
        json!({"id": "t1", "role": "assistant", "body": {"blocks": [{"type": "tool_use", "id": "c1", "name": "TodoWrite", "summary": "3 to-dos", "detail": "[x] Read how Sending moves a message\n[~] Retry from the stage it failed at\n[ ] Run the tests and commit"}]}, "created_at": ""}),
        json!({"id": "t1r", "role": "tool", "body": {"tool_use_id": "c1", "is_error": false, "text": "Todos have been modified successfully"}, "created_at": ""}),
        json!({"id": "t2", "role": "assistant", "body": {"blocks": [{"type": "tool_use", "id": "c2", "name": "Bash", "summary": "cd apps/relay-code/native && cargo test --locked -p relay-code", "detail": "# Run the app's tests\ncd apps/relay-code/native && cargo test --locked -p relay-code 2>&1 | tail -40"}]}, "created_at": ""}),
        json!({"id": "t2r", "role": "tool", "body": {"tool_use_id": "c2", "is_error": true, "text": fail}, "created_at": ""}),
        json!({"id": "t3", "role": "assistant", "body": {"blocks": [{"type": "text", "text": "The retry went back to `Typed` for a send whose words never went. Fixing `retry`."}]}, "created_at": ""}),
        json!({"id": "t4", "role": "assistant", "body": {"blocks": [{"type": "tool_use", "id": "c4", "name": "Edit", "summary": "src/data.rs", "detail": "-                Stage::Typing => Stage::Typed,\n+                Stage::Typing => Stage::Queued,"}]}, "created_at": ""}),
        json!({"id": "t4r", "role": "tool", "body": {"tool_use_id": "c4", "is_error": false, "text": "The file was updated"}, "created_at": ""}),
        json!({"id": "t5", "role": "assistant", "body": {"blocks": [{"type": "tool_use", "id": "c5", "name": "Bash", "summary": "cargo test --locked -p relay-code"}]}, "created_at": ""}),
        json!({"id": "t5r", "role": "tool", "body": {"tool_use_id": "c5", "is_error": false, "text": pass}, "created_at": ""}),
    ];
    if plan {
        msgs.push(json!({"id": "t6", "role": "assistant", "body": {"blocks": [{"type": "tool_use", "id": "c6", "name": "ExitPlanMode", "summary": "", "detail": "## Retry from where it stopped\n\n1. `retry` puts a send back at the stage it failed at\n2. **Enter alone** when its words were typed\n3. A test for each stage\n\nNothing else in the queue changes."}]}, "created_at": ""}));
    }
    let mut c = Convo::default();
    c.take(&json!({"messages": msgs, "cursor": 1, "reset": true, "found": true}));
    app.convos.insert("swift-ocelot".into(), c);
    let screen = if plan {
        "Would you like to proceed?\n\n❯ 1. Yes, and auto-accept edits\n  2. Yes, and manually approve edits\n  3. No, keep planning\n\nctrl-g to edit in your editor"
    } else {
        "· Pondering… (42s · ↓ 1.9k tokens)\n────\n❯ \n────\n  ⏵⏵ auto mode on (shift+tab to cycle)"
    };
    app.screens.insert("swift-ocelot".into(), crate::screen::read(screen));
}
