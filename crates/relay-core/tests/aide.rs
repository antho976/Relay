//! Aide across the bus door (docs/AIDE.md): jobs, runs on a stand-in for Claude, privacy tiers,
//! the outbox, what Aide's own agent may and may not do, and data rules waking a job.

mod common;

use common::{call, code, engine, ok, wait_until};
use relay_bus::{Actor, Request, Response};
use relay_core::engine::{Door, Engine};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// A Claude that logs its arguments and the lines it is given, and answers each with a short
/// reply and a result, as the real one does; `pause` seconds after reading each.
fn fake_claude(dir: &Path, pause: u32) -> PathBuf {
    let log = dir.join("args.log");
    let script = dir.join("claude");
    let body = r#"#!/bin/sh
printf 'OPS=%s %s\n' "$RELAY_MCP_OPS" "$*" | tr '\n' ' ' >> "LOG"; echo >> "LOG"
printf 'BASE=%s MODEL=%s\n' "$ANTHROPIC_BASE_URL" "$ANTHROPIC_MODEL" >> "DIR/env.log"
while IFS= read -r line; do
  printf '%s\n' "$line" >> "DIR/lines.log"
  sleep PAUSE
  echo '{"type":"system","subtype":"init","session_id":"fake-aide-1","mcp_servers":[{"name":"relay","status":"connected"}],"tools":[]}'
  echo '{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Looked; nothing new."}]},"parent_tool_use_id":null}'
  echo '{"type":"result","subtype":"success","is_error":false,"result":"Looked; nothing new."}'
done
"#
    .replace("LOG", &log.display().to_string())
    .replace("DIR", &dir.display().to_string())
    .replace("PAUSE", &pause.to_string());
    std::fs::write(&script, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    log
}

fn with_fake(engine: &Engine, dir: &Path) -> PathBuf {
    with_fake_pausing(engine, dir, 0)
}

fn with_fake_pausing(engine: &Engine, dir: &Path, pause: u32) -> PathBuf {
    let log = fake_claude(dir, pause);
    ok(engine, "settings.set", json!({"path": "providers.claude.path", "value": dir.join("claude")}));
    log
}

fn from_agent(engine: &Engine, thread: &Value, op: &str, payload: Value) -> Response {
    engine.dispatch_from_thread(Request::new(Actor::User, op, payload), Door::InProcess, thread.as_i64().unwrap())
}

fn job(engine: &Engine, spec: Value) -> Value {
    ok(engine, "aide.job.save", json!({"spec": spec}))
}

fn brief_spec() -> Value {
    json!({"name": "Morning brief", "instructions": "Tell me what happened overnight.", "sources": ["dev"],
           "schedule": {"every": "weekdays", "hour": 7}})
}

fn run(engine: &Engine, id: &Value) -> Value {
    ok(engine, "aide.run.get", json!({"id": id}))
}

fn ended(engine: &Engine, id: &Value) -> Value {
    let mut last = Value::Null;
    wait_until("the run to end", || {
        last = run(engine, id);
        !matches!(last["state"].as_str(), Some("queued" | "running"))
    });
    last
}

#[test]
fn the_person_makes_jobs_and_aide_only_proposes_them() {
    let engine = engine();
    let j = job(&engine, brief_spec());
    assert_eq!(j["state"], "active");
    assert!(j["next_due_at"].is_string(), "an active scheduled job knows its next wake");
    assert_eq!(j["when"], json!(["weekdays at 07:00"]));
    assert_eq!(code(call(&engine, "aide.job.save", json!({"spec": {"name": "", "instructions": "x"}}))), "aide.invalid");

    // A call from an Aide thread's agent, told apart from the person by its thread.
    let thread = json!(9_999);
    let proposed = from_agent(&engine, &thread, "aide.job.propose",
        json!({"spec": {"name": "Budget watch", "instructions": "Tell me when a budget runs hot.", "rules": [{"kind": "budget", "percent": 90}]},
               "why": "You asked about budgets twice this week"})).into_result().unwrap();
    assert_eq!(proposed["state"], "proposed");
    assert_eq!(proposed["sources"], json!([]), "a rule's source is read from the rule");
    // Aide's own save is a proposal too, and it cannot turn one on or change the person's job.
    let saved = from_agent(&engine, &thread, "aide.job.save", json!({"spec": brief_spec()})).into_result().unwrap();
    assert_eq!(saved["state"], "proposed");
    assert_eq!(from_agent(&engine, &thread, "aide.job.set", json!({"id": proposed["id"], "state": "active"})).into_result().unwrap_err().code, "aide.person");
    assert_eq!(from_agent(&engine, &thread, "aide.job.save", json!({"id": j["id"], "spec": brief_spec()})).into_result().unwrap_err().code, "aide.person");
    assert_eq!(code(call(&engine, "aide.run.now", json!({"job_id": proposed["id"]}))), "aide.proposed");

    let on = ok(&engine, "aide.job.set", json!({"id": proposed["id"], "state": "active"}));
    assert_eq!(on["state"], "active");
    let summary = ok(&engine, "aide.summary", json!({}));
    assert_eq!((summary["jobs"].as_u64(), summary["proposed"].as_u64()), (Some(3), Some(1)));
    assert!(summary["held"].as_u64().unwrap() >= 1, "a proposal is held for the brief");
    ok(&engine, "aide.job.delete", json!({"id": on["id"]}));
    assert_eq!(code(call(&engine, "aide.job.get", json!({"id": on["id"]}))), "aide.not_found");
}

#[test]
fn a_run_is_a_wake_turn_in_its_jobs_channel_with_only_its_sources() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine();
    relay_core::aide::spawn(&engine);
    let log = with_fake(&engine, dir.path());
    let j = job(&engine, json!({"name": "Dev watch", "instructions": "Say which agents wait on me.", "sources": ["dev"], "model": "claude-haiku-5-5"}));
    let r = ok(&engine, "aide.run.now", json!({"job_id": j["id"], "note": "check the reviewers"}));
    assert_eq!((r["reason"].as_str(), r["state"].as_str()), (Some("manual"), Some("queued")));
    let done = ended(&engine, &r["id"]);
    assert_eq!(done["state"], "done", "{done}");
    assert_eq!((done["model"].as_str(), done["host"].as_str()), (Some("claude-haiku-5-5"), Some("claude")));
    assert_eq!(done["sources"][0]["source"], "dev");
    assert_eq!(done["sources"][0]["ok"], true, "{done}");

    let thread = &done["thread_id"];
    let t = ok(&engine, "thread.get", json!({"id": thread}));
    assert_eq!(t["thread"]["kind"], "aide");
    assert_eq!(ok(&engine, "aide.job.get", json!({"id": j["id"]}))["job"]["thread_id"], *thread);
    let wake = &t["messages"][0];
    assert_eq!(wake["role"], "wake", "the engine's words are not the person's");
    let text = wake["body"]["text"].as_str().unwrap();
    assert!(text.contains("Say which agents wait on me.") && text.contains("check the reviewers") && text.contains("## Relay (read now)"), "{text}");

    let args = std::fs::read_to_string(log).unwrap();
    assert!(args.contains("You are Aide"), "Aide's prompt: {args}");
    assert!(args.contains("--model claude-haiku-5-5"), "{args}");
    let ops = args.split_whitespace().find(|w| w.starts_with("OPS=")).unwrap();
    assert!(ops.contains("aide.notify") && ops.contains("session.list"), "{ops}");
    assert!(!ops.contains("money.") && !ops.contains("gym.") && !ops.contains("thread.send"), "only its sources: {ops}");

    // A second run starts a fresh conversation: no resume of the first.
    let again = ok(&engine, "aide.run.now", json!({"job_id": j["id"]}));
    assert_eq!(ended(&engine, &again["id"])["state"], "done");
    let args = std::fs::read_to_string(dir.path().join("args.log")).unwrap();
    assert!(!args.contains("--resume"), "{args}");
}

#[test]
fn private_data_never_goes_to_claude() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine();
    relay_core::aide::spawn(&engine);
    with_fake(&engine, dir.path());
    // A server on the person's network that is down, then one that answers.
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let down = closed.local_addr().unwrap().port();
    drop(closed);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let up = listener.local_addr().unwrap().port();
    let mut settings = ok(&engine, "aide.settings.get", json!({}));
    settings["tiers"]["money"] = json!("local_only");
    settings["endpoints"] = json!([
        {"id": "farm-down", "name": "Old box", "url": format!("http://127.0.0.1:{down}")},
        {"id": "farm", "name": "Farm", "url": format!("http://127.0.0.1:{up}"), "model": "qwen-72b", "context": 131072},
    ]);
    ok(&engine, "aide.settings.set", settings);

    let j = job(&engine, json!({"name": "Spending", "instructions": "Look at my spending.", "sources": ["money"], "model": "claude-sonnet-5-5", "fallback": "farm-down"}));
    let skipped = ended(&engine, &ok(&engine, "aide.run.now", json!({"job_id": j["id"]}))["id"]);
    assert_eq!(skipped["state"], "skipped");
    let why = skipped["note"].as_str().unwrap();
    assert!(why.contains("Tally is for your own models only") && why.contains("farm-down is not available"), "{why}");

    ok(&engine, "aide.job.save", json!({"id": j["id"], "spec": {"name": "Spending", "instructions": "Look at my spending.", "sources": ["money"],
        "model": "claude-sonnet-5-5", "fallback": "farm"}}));
    let done = ended(&engine, &ok(&engine, "aide.run.now", json!({"job_id": j["id"]}))["id"]);
    assert_eq!((done["model"].as_str(), done["host"].as_str()), (Some("farm"), Some("server")), "{done}");
    let env = std::fs::read_to_string(dir.path().join("env.log")).unwrap();
    assert!(env.contains(&format!("BASE=http://127.0.0.1:{up} MODEL=qwen-72b")), "pointed at the server: {env}");

    let models = ok(&engine, "aide.models", json!({}))["models"].as_array().unwrap().clone();
    let farm = models.iter().find(|m| m["id"] == "farm").unwrap();
    assert_eq!((farm["host"].as_str(), farm["available"].as_bool()), (Some("server"), Some(true)));
    assert_eq!(models.iter().find(|m| m["id"] == "farm-down").unwrap()["available"], false);
    drop(listener);
}

#[test]
fn what_aide_says_is_held_in_quiet_hours_and_past_the_cap() {
    let engine = engine();
    let j = job(&engine, brief_spec());
    // Its channel: a thread the job owns.
    let thread = ok(&engine, "thread.create", json!({"kind": "aide"}));
    relay_core::aide::with_db(&engine, |db| db.set_job_thread(j["id"].as_i64().unwrap(), thread["id"].as_i64().unwrap())).unwrap();
    let say = |level: &str| from_agent(&engine, &thread["id"], "aide.notify", json!({"title": format!("A {level}"), "level": level})).into_result().unwrap();

    // Quiet hours that never end hold every interrupt.
    let mut settings = ok(&engine, "aide.settings.get", json!({}));
    settings["quiet_start"] = json!(0);
    settings["quiet_end"] = json!(0);
    settings["interrupt_cap"] = json!(1);
    ok(&engine, "aide.settings.set", settings.clone());
    let first = say("interrupt");
    assert_eq!((first["notified"].as_bool(), first["held"].as_str()), (Some(true), None), "{first}");
    let second = say("interrupt");
    assert_eq!(second["notified"], false);
    assert!(second["held"].as_str().unwrap().contains("interrupts were used"));
    assert_eq!(say("brief")["held"], "for the brief");
    assert_eq!(say("quiet")["held"], Value::Null);

    let notices = ok(&engine, "notify.list", json!({"category": "aide"}));
    assert_eq!(notices["notifications"].as_array().unwrap().len(), 1, "one interrupt was shown");
    assert_eq!(notices["notifications"][0]["title"], "A interrupt");
    assert_eq!(ok(&engine, "aide.summary", json!({}))["held"], 2);
    let messages = ok(&engine, "aide.message.list", json!({}));
    assert_eq!(messages["messages"][0]["job_id"], j["id"]);

    settings["quiet_start"] = json!(0);
    settings["quiet_end"] = json!(23);
    settings["interrupt_cap"] = json!(5);
    ok(&engine, "aide.settings.set", settings);
    let hour = jiff::Zoned::now().hour();
    if hour < 23 {
        assert_eq!(say("interrupt")["held"], "quiet hours");
    }
}

#[test]
fn aide_learns_and_keeps_errands_but_only_the_person_confirms_and_closes() {
    let engine = engine();
    let thread = json!(4_242);
    let fact = from_agent(&engine, &thread, "aide.fact.add", json!({"text": "Antho trains on Monday mornings", "source": "gym"})).into_result().unwrap();
    assert_eq!((fact["confirmed"].as_bool(), fact["source"].as_str()), (Some(false), Some("gym")));
    let mine = ok(&engine, "aide.fact.add", json!({"text": "Keep briefs under ten lines"}));
    assert_eq!((mine["confirmed"].as_bool(), mine["source"].as_str()), (Some(true), Some("person")));
    assert_eq!(ok(&engine, "aide.summary", json!({}))["unconfirmed"], 1);
    ok(&engine, "aide.fact.update", json!({"id": fact["id"], "confirmed": true, "pinned": true}));
    assert_eq!(ok(&engine, "aide.fact.forget", json!({"source": "gym"}))["forgotten"], 1);
    assert_eq!(ok(&engine, "aide.fact.list", json!({}))["facts"].as_array().unwrap().len(), 1);
    assert_eq!(code(call(&engine, "aide.fact.add", json!({"text": "x", "source": "elsewhere"}))), "aide.invalid");

    let errand = from_agent(&engine, &thread, "aide.errand.add", json!({"title": "Check PR #156 once CI is done", "due_at": "2026-10-28T17:00:00Z"})).into_result().unwrap();
    assert_eq!(errand["state"], "open");
    let asked = from_agent(&engine, &thread, "aide.errand.update", json!({"id": errand["id"], "state": "done"})).into_result().unwrap();
    assert_eq!((asked["state"].as_str(), asked["proposed_done"].as_bool()), (Some("open"), Some(true)), "Aide asks; it does not close");
    let closed = ok(&engine, "aide.errand.update", json!({"id": errand["id"], "state": "done"}));
    assert_eq!(closed["state"], "done");
    assert!(closed["closed_at"].is_string());
    assert!(ok(&engine, "aide.errand.list", json!({}))["errands"].as_array().unwrap().is_empty());

    let page = from_agent(&engine, &thread, "aide.page.write", json!({"title": "Week", "question": "How did the week go?", "body": "Good."})).into_result().unwrap();
    let again = from_agent(&engine, &thread, "aide.page.write", json!({"title": "Week", "question": "How did the week go?", "body": "Better."})).into_result().unwrap();
    assert_eq!(page["id"], again["id"]);
    assert_eq!(ok(&engine, "aide.page.get", json!({"id": page["id"]}))["body"], "Better.");
    // A bot writes its own pages, never another's; and it cannot reopen what the person closed.
    let other = job(&engine, brief_spec());
    let channel = ok(&engine, "thread.create", json!({"kind": "aide"}));
    relay_core::aide::with_db(&engine, |db| db.set_job_thread(other["id"].as_i64().unwrap(), channel["id"].as_i64().unwrap())).unwrap();
    let theirs = from_agent(&engine, &channel["id"], "aide.page.write", json!({"title": "Week", "question": "q", "body": "Mine now."})).into_result().unwrap_err();
    assert_eq!(theirs.code, "aide.person");
    assert_eq!(ok(&engine, "aide.page.get", json!({"id": page["id"]}))["body"], "Better.");
    let reopened = from_agent(&engine, &thread, "aide.errand.update", json!({"id": errand["id"], "state": "open"})).into_result().unwrap_err();
    assert_eq!(reopened.code, "aide.person");

    // The run-only ops are the agent's.
    assert_eq!(code(call(&engine, "aide.run.finish", json!({"outcome": "done"}))), "aide.agent");
    assert_eq!(code(call(&engine, "aide.followup", json!({"minutes": 5, "note": "x"}))), "aide.agent");
}

#[test]
fn every_aide_op_is_the_persons() {
    let engine = engine();
    for op in ["aide.summary", "aide.job.list", "aide.settings.get", "aide.fact.list"] {
        let r = engine.dispatch(Request::new(Actor::agent("brisk-otter"), op, json!({})), Door::InProcess);
        assert!(r.error.is_some(), "{op} is not a session's");
    }
}

#[test]
fn a_spend_rule_wakes_its_job_for_new_entries_only() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine();
    relay_core::aide::spawn(&engine);
    with_fake(&engine, dir.path());
    let account = ok(&engine, "money.account.add", json!({"name": "Chequing", "type": "CHEQUING", "opening_balance": 500_000}));
    let groceries = ok(&engine, "money.lists", json!({}))["categories"].as_array().unwrap().iter().find(|c| c["name"] == "Groceries").unwrap()["id"].clone();
    let today = jiff::Zoned::now().date().to_string();
    let add = |amount: i64, note: &str| ok(&engine, "money.tx.add", json!({
        "type": "EXPENSE", "amount": amount, "date": today, "account_id": account["id"], "category_id": groceries, "note": note
    }));
    add(30_000, "Before the rule");
    let j = job(&engine, json!({"name": "Big spends", "instructions": "Tell me about it.", "rules": [{"kind": "spend", "over": 200}]}));
    // What was there before the rule was taken as seen when the job was saved; the first entry's
    // own event settles before the next ones.
    std::thread::sleep(relay_core::aide::rules::SETTLE * 2);
    add(1_200, "Small");
    add(25_000, "New TV");
    let mut runs = Vec::new();
    wait_until("the rule's run", || {
        runs = ok(&engine, "aide.run.list", json!({"job_id": j["id"]}))["runs"].as_array().unwrap().clone();
        !runs.is_empty()
    });
    std::thread::sleep(relay_core::aide::rules::SETTLE * 2);
    let runs = ok(&engine, "aide.run.list", json!({"job_id": j["id"]}))["runs"].as_array().unwrap().clone();
    assert_eq!(runs.len(), 1, "{runs:?}");
    assert_eq!(runs[0]["reason"], "rule");
    let trigger = runs[0]["trigger"].as_str().unwrap();
    assert!(trigger.contains("250.00") && trigger.contains("New TV") && !trigger.contains("Before the rule"), "{trigger}");
    ended(&engine, &runs[0]["id"]);
}

#[test]
fn a_schedule_missed_while_off_catches_up_inside_its_grace_only() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine();
    with_fake(&engine, dir.path());
    let recent = job(&engine, brief_spec());
    let stale = job(&engine, json!({"name": "Stale", "instructions": "x", "schedule": {"every": "day", "hour": 7}}));
    let ago = |mins: i64| relay_core::time::stamp(jiff::Timestamp::now().checked_sub(jiff::SignedDuration::from_mins(mins)).unwrap());
    relay_core::aide::with_db(&engine, |db| {
        db.set_job_due(recent["id"].as_i64().unwrap(), Some(&ago(30)))?;
        db.set_job_due(stale["id"].as_i64().unwrap(), Some(&ago(180)))
    }).unwrap();
    let next = relay_core::aide::runs::tick(&engine);
    assert!(next.is_some(), "the scheduler knows when to look next");
    let runs = |j: &Value| ok(&engine, "aide.run.list", json!({"job_id": j["id"]}))["runs"].as_array().unwrap().clone();
    let caught = runs(&recent);
    assert_eq!(caught.len(), 1);
    assert_eq!(caught[0]["reason"], "catch_up");
    assert!(runs(&stale).is_empty(), "three hours late is past the grace window");
    let rolled = ok(&engine, "aide.job.get", json!({"id": stale["id"]}))["job"]["next_due_at"].as_str().unwrap().to_string();
    assert!(rolled > relay_core::time::now(), "it waits for its next time: {rolled}");
}

#[test]
fn a_channel_moved_to_claude_after_a_run_on_your_own_model_starts_afresh() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine();
    relay_core::aide::spawn(&engine);
    with_fake(&engine, dir.path());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut settings = ok(&engine, "aide.settings.get", json!({}));
    settings["endpoints"] = json!([{"id": "farm", "name": "Farm", "url": format!("http://{}", listener.local_addr().unwrap())}]);
    ok(&engine, "aide.settings.set", settings);
    let j = job(&engine, json!({"name": "Mine", "instructions": "Look.", "sources": ["gym"], "model": "farm"}));
    let done = ended(&engine, &ok(&engine, "aide.run.now", json!({"job_id": j["id"]}))["id"]);
    assert_eq!(done["host"], "server");
    let thread = done["thread_id"].clone();
    // The person moves the channel to Claude and asks a follow-up there.
    ok(&engine, "thread.set", json!({"id": thread, "model": "claude-sonnet-5-5"}));
    ok(&engine, "thread.send", json!({"id": thread, "text": "And last week?"}));
    wait_until("the reply", || ok(&engine, "thread.get", json!({"id": thread}))["thread"]["working"] == false
        && ok(&engine, "thread.get", json!({"id": thread}))["messages"].as_array().unwrap().len() >= 4);
    let args = std::fs::read_to_string(dir.path().join("args.log")).unwrap();
    let last = args.lines().last().unwrap();
    assert!(last.contains("--model claude-sonnet-5-5") && !last.contains("--resume"), "{last}");
    drop(listener);
}

#[test]
fn bots_talk_in_their_own_chat_and_report_to_the_chief_of_staff() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine();
    relay_core::aide::spawn(&engine);
    with_fake(&engine, dir.path());
    let chief = ok(&engine, "aide.chat", json!({}))["thread_id"].clone();
    assert_eq!(ok(&engine, "aide.chat", json!({}))["thread_id"], chief, "one Chief of Staff, made once");
    assert_eq!(ok(&engine, "thread.get", json!({"id": chief}))["thread"]["kind"], "aide");

    let j = job(&engine, json!({"name": "Dev watch", "instructions": "Say which agents wait on me.", "sources": ["dev"]}));
    // A bot's chat is there before it ever ran, and its runs talk in it.
    let opened = ok(&engine, "aide.chat", json!({"job_id": j["id"]}))["thread_id"].clone();
    assert_eq!(ok(&engine, "thread.get", json!({"id": opened}))["thread"]["title"], "Dev watch");
    let r = ok(&engine, "aide.run.now", json!({"job_id": j["id"]}));
    let channel = ended(&engine, &r["id"])["thread_id"].clone();
    assert_eq!(channel, opened);
    let said = from_agent(&engine, &channel, "aide.notify", json!({"title": "Two agents wait", "body": "amber-walrus and coral-marten", "level": "brief"}))
        .into_result().unwrap();
    let note = |thread: &Value| {
        let mut found = Value::Null;
        wait_until("the note in the chat", || {
            let t = ok(&engine, "thread.get", json!({"id": thread}));
            found = t["messages"].as_array().unwrap().iter().find(|m| m["role"] == "note").cloned().unwrap_or(Value::Null);
            !found.is_null()
        });
        found
    };
    for thread in [&channel, &chief] {
        let n = note(thread);
        assert_eq!((n["body"]["bot"].as_str(), n["body"]["title"].as_str()), (Some("Dev watch"), Some("Two agents wait")), "{n}");
        assert_eq!(n["body"]["message_id"], said["message"]["id"]);
    }

    // The Chief of Staff reads every source; a bot's chat only its own.
    let ops = relay_core::aide::runs::ops_for_thread(&engine, chief.as_i64().unwrap());
    assert!(ops.contains(&"money.summary") && ops.contains(&"session.list") && ops.contains(&"aide.job.propose"), "{ops:?}");
    let ops = relay_core::aide::runs::ops_for_thread(&engine, channel.as_i64().unwrap());
    assert!(!ops.contains(&"money.summary"), "{ops:?}");

    // A bot asked for in the Chief of Staff waits there as a proposal to turn on.
    let proposed = from_agent(&engine, &chief, "aide.job.propose",
        json!({"spec": {"name": "Budget watch", "instructions": "Tell me when a budget runs hot.", "rules": [{"kind": "budget", "percent": 90}]},
               "why": "You asked for it"})).into_result().unwrap();
    wait_until("the proposal in the Chief of Staff", || {
        let t = ok(&engine, "thread.get", json!({"id": chief}));
        t["messages"].as_array().unwrap().iter().any(|m| m["role"] == "note" && m["body"]["proposal"] == proposed["id"])
    });
}

/// A server of the person's own that answers, named `farm` in Aide's settings, with Tally for
/// their own models only.
fn own_server(engine: &Engine) -> std::net::TcpListener {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut settings = ok(engine, "aide.settings.get", json!({}));
    settings["tiers"]["money"] = json!("local_only");
    settings["endpoints"] = json!([{"id": "farm", "name": "Farm", "url": format!("http://{}", listener.local_addr().unwrap())}]);
    ok(engine, "aide.settings.set", settings);
    listener
}

/// Sends `text` in `thread` and waits for the answer; the agent's command line.
fn talk(engine: &Engine, dir: &Path, thread: &Value, text: &str) -> String {
    let before = ok(engine, "thread.get", json!({"id": thread}))["messages"].as_array().unwrap().len();
    ok(engine, "thread.send", json!({"id": thread, "text": text}));
    wait_until("the answer", || {
        let t = ok(engine, "thread.get", json!({"id": thread}));
        t["thread"]["working"] == false && t["messages"].as_array().unwrap().len() >= before + 2
    });
    std::fs::read_to_string(dir.join("args.log")).unwrap().lines().last().unwrap().to_string()
}

#[test]
fn a_chat_on_your_own_model_is_never_resumed_on_claude() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine();
    relay_core::aide::spawn(&engine);
    with_fake(&engine, dir.path());
    let listener = own_server(&engine);
    let mut settings = ok(&engine, "aide.settings.get", json!({}));
    settings["model"] = json!("farm");
    ok(&engine, "aide.settings.set", settings);

    // The Chief of Staff has no runs: what it said on the person's server is theirs.
    let chief = ok(&engine, "aide.chat", json!({}))["thread_id"].clone();
    assert_eq!(ok(&engine, "thread.get", json!({"id": chief}))["thread"]["model"], "farm");
    let first = talk(&engine, dir.path(), &chief, "How is my money this month?");
    assert!(!first.contains("--resume"), "{first}");
    ok(&engine, "thread.set", json!({"id": chief, "model": "claude-sonnet-5-5"}));
    let on_claude = talk(&engine, dir.path(), &chief, "And in short?");
    assert!(on_claude.contains("--model claude-sonnet-5-5") && !on_claude.contains("--resume"), "starts afresh: {on_claude}");

    // A conversation had on Claude's goes on there, and back on the person's own model.
    ok(&engine, "thread.set", json!({"id": chief, "model": "claude-haiku-5-5"}));
    let resumed = talk(&engine, dir.path(), &chief, "Thanks");
    assert!(resumed.contains("--model claude-haiku-5-5") && resumed.contains("--resume fake-aide-1"), "{resumed}");
    ok(&engine, "thread.set", json!({"id": chief, "model": "farm"}));
    assert!(talk(&engine, dir.path(), &chief, "One more").contains("--resume fake-aide-1"));
    // Having been on the person's own model again, it is not Claude's to resume.
    ok(&engine, "thread.set", json!({"id": chief, "model": "claude-sonnet-5-5"}));
    assert!(!talk(&engine, dir.path(), &chief, "Last one").contains("--resume"));
    drop(listener);
}

#[test]
fn what_your_own_model_wrote_from_private_data_never_reaches_claude() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine();
    relay_core::aide::spawn(&engine);
    with_fake(&engine, dir.path());
    let listener = own_server(&engine);

    // A money job on the person's server keeps what it read.
    let local = job(&engine, json!({"name": "Spending", "instructions": "Look at my spending.", "sources": ["money"], "model": "farm"}));
    let ran = ended(&engine, &ok(&engine, "aide.run.now", json!({"job_id": local["id"]}))["id"]);
    assert_eq!(ran["host"], "server", "{ran}");
    let mine = ran["thread_id"].clone();
    let said = from_agent(&engine, &mine, "aide.notify", json!({"title": "SECRET412 at Costco", "level": "brief"})).into_result().unwrap();
    assert_eq!(said["held"], "for the brief");
    from_agent(&engine, &mine, "aide.page.write", json!({"title": "Money review", "question": "How is the month?", "body": "SECRET412"})).into_result().unwrap();
    let errand = from_agent(&engine, &mine, "aide.errand.add", json!({"title": "Ask about SECRET412"})).into_result().unwrap();
    let fact = from_agent(&engine, &mine, "aide.fact.add", json!({"text": "Shops at SECRET412"})).into_result().unwrap();
    ok(&engine, "aide.fact.update", json!({"id": fact["id"], "confirmed": true}));
    from_agent(&engine, &mine, "aide.job.propose", json!({"spec": {"name": "SECRET412 watch", "instructions": "x"}, "why": "SECRET412"})).into_result().unwrap();
    // The person's own words are no secret.
    ok(&engine, "aide.notify", json!({"title": "PR 162 merged", "level": "brief"}));

    // A brief on Claude's: due now, so the scheduler starts it.
    let brief = job(&engine, json!({"name": "Brief", "instructions": "Brief me.", "sources": ["dev"], "model": "claude-haiku-5-5",
        "schedule": {"every": "day", "hour": 7}}));
    let half_a_minute_ago = relay_core::time::stamp(jiff::Timestamp::now().checked_sub(jiff::SignedDuration::from_secs(30)).unwrap());
    relay_core::aide::with_db(&engine, |db| db.set_job_due(brief["id"].as_i64().unwrap(), Some(&half_a_minute_ago))).unwrap();
    relay_core::aide::runs::tick(&engine);
    let run = ok(&engine, "aide.run.list", json!({"job_id": brief["id"]}))["runs"][0].clone();
    let done = ended(&engine, &run["id"]);
    assert_eq!((done["state"].as_str(), done["host"].as_str()), (Some("done"), Some("claude")), "{done}");
    let theirs = done["thread_id"].clone();
    let wake = ok(&engine, "thread.get", json!({"id": theirs}))["messages"][0]["body"]["text"].as_str().unwrap().to_string();
    assert!(wake.contains("PR 162 merged"), "{wake}");
    assert!(!wake.contains("SECRET412"), "{wake}");
    assert!(wake.contains("2 more held for this brief"), "the note and the proposal: {wake}");
    assert_eq!(ok(&engine, "aide.summary", json!({}))["held"], 0, "both were briefed");

    // Nor can its agent list them; the money job's agent and the person can.
    for (op, list) in [("aide.page.list", "pages"), ("aide.errand.list", "errands"), ("aide.fact.list", "facts"), ("aide.job.list", "jobs")] {
        let claude = from_agent(&engine, &theirs, op, json!({})).into_result().unwrap();
        assert!(!claude[list].to_string().contains("SECRET412"), "{op}: {claude}");
        assert!(from_agent(&engine, &mine, op, json!({})).into_result().unwrap()[list].to_string().contains("SECRET412"), "{op}");
        assert!(ok(&engine, op, json!({}))[list].to_string().contains("SECRET412"), "{op}");
    }
    let reached = from_agent(&engine, &theirs, "aide.errand.update", json!({"id": errand["id"], "detail": "x"})).into_result().unwrap_err();
    assert_eq!(reached.code, "aide.not_found", "not by its id either");
    drop(listener);
}

#[test]
fn a_run_ends_with_its_own_turn_not_an_older_one() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine();
    relay_core::aide::spawn(&engine);
    with_fake_pausing(&engine, dir.path(), 2);
    let j = job(&engine, json!({"name": "Dev watch", "instructions": "Look.", "sources": ["dev"], "model": "claude-haiku-5-5"}));
    let r = ok(&engine, "aide.run.now", json!({"job_id": j["id"]}));
    let mut thread = Value::Null;
    wait_until("the run's turn", || {
        let got = run(&engine, &r["id"]);
        thread = got["thread_id"].clone();
        got["state"] == "running" && ok(&engine, "thread.get", json!({"id": thread}))["thread"]["working"] == true
    });
    // An earlier "turn ended" for the channel, read only now (as the watcher may, having
    // started this very run), leaves the run going.
    relay_core::aide::runs::turn_ended(&engine, thread.as_i64().unwrap());
    assert_eq!(run(&engine, &r["id"])["state"], "running");
    let done = ended(&engine, &r["id"]);
    assert_eq!(done["state"], "done", "{done}");

    // Its turn over, the agent cannot rewrite how it ended.
    let late = from_agent(&engine, &thread, "aide.run.finish", json!({"outcome": "nothing", "note": "rewritten"})).into_result().unwrap_err();
    assert_eq!(late.code, "aide.no_run");
    assert_eq!(run(&engine, &r["id"])["note"], done["note"]);
}

#[test]
fn a_brief_waits_while_you_talk_in_its_chat_and_keeps_what_was_held() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine();
    relay_core::aide::spawn(&engine);
    with_fake_pausing(&engine, dir.path(), 2);
    let j = job(&engine, json!({"name": "Brief", "instructions": "Brief me.", "sources": ["dev"], "model": "claude-haiku-5-5",
        "schedule": {"every": "day", "hour": 7}}));
    ok(&engine, "aide.notify", json!({"title": "PR 162 merged", "level": "brief"}));
    // The person talks in the bot's chat, on another model than its runs'.
    let chat = ok(&engine, "aide.chat", json!({"job_id": j["id"]}))["thread_id"].clone();
    ok(&engine, "thread.set", json!({"id": chat, "model": "claude-sonnet-5-5"}));
    ok(&engine, "thread.send", json!({"id": chat, "text": "What did you see today?"}));

    let half_a_minute_ago = relay_core::time::stamp(jiff::Timestamp::now().checked_sub(jiff::SignedDuration::from_secs(30)).unwrap());
    relay_core::aide::with_db(&engine, |db| db.set_job_due(j["id"].as_i64().unwrap(), Some(&half_a_minute_ago))).unwrap();
    relay_core::aide::runs::tick(&engine);
    let r = ok(&engine, "aide.run.list", json!({"job_id": j["id"]}))["runs"][0].clone();
    assert_eq!(run(&engine, &r["id"])["state"], "queued", "it waits for the person's turn");
    assert_eq!(ok(&engine, "aide.summary", json!({}))["held"], 1, "and what it would say is still held");

    let done = ended(&engine, &r["id"]);
    assert_eq!(done["state"], "done", "{done}");
    assert_eq!(done["thread_id"], chat);
    assert_eq!(ok(&engine, "aide.summary", json!({}))["held"], 0);
}
