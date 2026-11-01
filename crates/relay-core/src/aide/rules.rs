//! Data rules: conditions on the person's data that Relay checks itself, without a model, when
//! the data changes. A rule that finds something wakes its job once per thing found (the `seen`
//! table); what was already there when the job was made is taken as seen, so a new rule speaks
//! of what happens next, not of the past. A condition that lasts (a halt, an outage) is seen
//! while it lasts and forgotten once it has passed ([`LASTING`]), so it speaks again when it
//! comes back.
//!
//! The watcher reads the engine's events on a thread of its own. A burst (a phone sync is many
//! `money.changed`) is gathered for [`SETTLE`] before the rules are checked once.

use super::{runs, with_db};
use crate::engine::Engine;
use jiff::Timestamp;
use relay_aide::model::{Job, JobState, Reason, Rule, Source, Tier};
use relay_bus::Event;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::{Arc, Weak};
use std::time::Duration;
use tokio::sync::broadcast::{error::RecvError, error::TryRecvError, Receiver};

/// How long a burst of events is gathered before the rules are checked.
pub const SETTLE: Duration = Duration::from_millis(1500);

/// What a rule found: a key it is known by, and one line for the run.
pub struct Finding {
    pub key: String,
    pub summary: String,
}

pub fn watch(weak: Weak<Engine>, mut events: Receiver<Event>) {
    crate::background_priority();
    loop {
        let first = match events.blocking_recv() {
            Ok(e) => e,
            Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => return,
        };
        let mut names = BTreeSet::new();
        let mut ended = BTreeSet::new();
        let take = |e: &Event, names: &mut BTreeSet<String>, ended: &mut BTreeSet<i64>| {
            if e.ev == "thread.changed" {
                let t = &e.payload["thread"];
                if t["kind"] == crate::threads::AIDE && t["working"] == false {
                    if let Some(id) = t["id"].as_i64() {
                        ended.insert(id);
                    }
                }
            } else {
                names.insert(e.ev.clone());
            }
        };
        take(&first, &mut names, &mut ended);
        let Some(engine) = weak.upgrade() else { return };
        if engine.is_quitting() {
            return;
        }
        // A turn ending is acted on at once; data changes wait for their burst to settle.
        for id in std::mem::take(&mut ended) {
            runs::turn_ended(&engine, id);
        }
        if !names.iter().any(|n| watched(n)) {
            continue;
        }
        drop(engine);
        std::thread::sleep(SETTLE);
        loop {
            match events.try_recv() {
                Ok(e) => take(&e, &mut names, &mut ended),
                Err(TryRecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
        let Some(engine) = weak.upgrade() else { return };
        for id in ended {
            runs::turn_ended(&engine, id);
        }
        check(&engine, &names);
    }
}

fn watched(event: &str) -> bool {
    matches!(event, "money.changed" | "gym.changed" | "arbiter.changed" | "notify.new" | "session.changed")
}

/// Checks the rules of every active job that listens for one of `events`.
pub fn check(engine: &Arc<Engine>, events: &BTreeSet<String>) {
    if events.contains("session.changed") {
        note_blocked(engine);
    }
    let jobs = with_db(engine, |db| db.jobs()).unwrap_or_default();
    let Ok(settings) = super::settings(engine) else { return };
    for job in jobs.iter().filter(|j| j.state == JobState::Active) {
        for (i, rule) in job.spec.rules.iter().enumerate() {
            if matches!(rule, Rule::Stuck { .. }) || !rule.events().iter().any(|e| events.contains(*e)) {
                continue;
            }
            match findings(engine, rule) {
                Ok(found) => {
                    forget_passed(engine, job, rule, &found);
                    fire(engine, job, i, found, settings.tiers.of(rule.source()));
                }
                Err(why) => tracing::debug!(job = job.id, why, "aide rule not checked"),
            }
        }
    }
}

/// The key prefixes of the Arbiter rule's findings, which are conditions that last, not events.
const LASTING: &[&str] = &["arbiter-halt:", "arbiter-unreachable", "strategy-halt:"];

/// Forgets the conditions `rule` no longer finds, so each is new again when it comes back.
fn forget_passed(engine: &Engine, job: &Job, rule: &Rule, found: &[Finding]) {
    if !matches!(rule, Rule::Arbiter {}) {
        return;
    }
    let keys: Vec<&str> = found.iter().map(|f| f.key.as_str()).collect();
    for prefix in LASTING {
        let _ = with_db(engine, |db| db.forget_seen_but(job.id, prefix, &keys));
    }
}

/// Wakes the job for each finding not seen before; the rule's first look only takes note. What a
/// rule finds in a source turned off for every model (`tier`) wakes nothing: its finding would be
/// the run's trigger, which any model reads. It is still taken as seen, so turned back on, the
/// rule speaks of what happens next. One in a source for the person's own models only is marked
/// so, and a run on Claude's is not told it (`runs::wake_text`).
fn fire(engine: &Arc<Engine>, job: &Job, index: usize, found: Vec<Finding>, tier: Tier) {
    let at = crate::time::now();
    let first_look = with_db(engine, |db| db.see(job.id, &format!("baseline:{index}:{}", job.updated_at), &at)).unwrap_or(false);
    let fresh: Vec<Finding> = found.into_iter().filter(|f| with_db(engine, |db| db.see(job.id, &f.key, &at)).unwrap_or(false)).collect();
    if first_look || fresh.is_empty() || tier == Tier::Off {
        return;
    }
    let summary = fresh.iter().map(|f| f.summary.as_str()).collect::<Vec<_>>().join("; ");
    let _ = runs::queue(engine, job.id, Reason::Rule, &summary, tier == Tier::LocalOnly);
}

/// Takes what each of the job's rules finds now as seen: called when a job is saved or turned on,
/// so its rules wake it for what happens after.
pub fn baseline(engine: &Engine, job: &Job) {
    let at = crate::time::now();
    for (i, rule) in job.spec.rules.iter().enumerate() {
        let found = findings(engine, rule).unwrap_or_default();
        let _ = with_db(engine, |db| {
            db.see(job.id, &format!("baseline:{i}:{}", job.updated_at), &at)?;
            for f in &found {
                db.see(job.id, &f.key, &at)?;
            }
            Ok(())
        });
    }
}

fn read(engine: &Engine, op: &str, payload: Value) -> Result<Value, String> {
    engine
        .dispatch(relay_bus::Request::new(relay_bus::Actor::User, op, payload), crate::engine::Door::InProcess)
        .into_result()
        .map_err(|e| e.message)
}

fn money(minor: i64, digits: u32) -> String {
    let scale = 10f64.powi(i32::try_from(digits).unwrap_or(2));
    format!("{:.*}", digits as usize, minor as f64 / scale)
}

/// What `rule` finds in the data now.
pub fn findings(engine: &Engine, rule: &Rule) -> Result<Vec<Finding>, String> {
    match rule {
        Rule::Spend { over } => {
            let summary = read(engine, "money.summary", json!({}))?;
            let digits = summary["fraction_digits"].as_u64().unwrap_or(2) as u32;
            let currency = summary["currency"].as_str().unwrap_or("");
            let limit = (over * 10f64.powi(digits as i32)).round() as i64;
            let page = read(engine, "money.tx.list", json!({"limit": 50}))?;
            Ok(page["transactions"].as_array().into_iter().flatten().filter(|t| t["type"].as_str().is_some_and(|k| k.eq_ignore_ascii_case("expense")) && t["amount"].as_i64().unwrap_or(0).abs() >= limit).map(|t| {
                let what = t["note"].as_str().filter(|n| !n.is_empty()).or(t["category"].as_str()).unwrap_or("an expense");
                Finding {
                    key: format!("tx:{}", t["uid"].as_str().map_or_else(|| t["id"].to_string(), str::to_string)),
                    summary: format!("{} {currency} on {} ({}, Tally entry {})", money(t["amount"].as_i64().unwrap_or(0).abs(), digits), t["date"].as_str().unwrap_or("?"), what, t["id"]),
                }
            }).collect())
        }
        Rule::Budget { percent } => {
            let summary = read(engine, "money.summary", json!({}))?;
            let period = summary["period"]["start"].as_str().or(summary["period"]["label"].as_str()).unwrap_or("").to_string();
            Ok(summary["budgets"].as_array().into_iter().flatten().filter_map(|b| {
                let budget = b["budget"].as_i64().filter(|v| *v > 0)?;
                let spent = b["spent"].as_i64().unwrap_or(0);
                let share = spent * 100 / budget;
                (share >= i64::from(*percent)).then(|| Finding {
                    key: format!("budget:{}:{period}:{percent}", b["category_id"]),
                    summary: format!("{} is at {share}% of its budget", b["name"].as_str().unwrap_or("A category")),
                })
            }).collect())
        }
        Rule::Notice { category } => {
            let list = read(engine, "notify.list", json!({"unread_only": true, "category": category, "limit": 20}))?;
            Ok(list["notifications"].as_array().into_iter().flatten().map(|n| Finding {
                key: format!("notice:{}", n["id"]),
                summary: format!("{}: {}", n["title"].as_str().unwrap_or(""), n["body"].as_str().unwrap_or("")),
            }).collect())
        }
        Rule::Arbiter {} => {
            let s = read(engine, "arbiter.summary", json!({}))?;
            let mut found = Vec::new();
            if s["halted"].as_bool() == Some(true) {
                let why = s["halt_reason"].as_str().unwrap_or("");
                found.push(Finding { key: format!("arbiter-halt:{why}"), summary: format!("Arbiter's kill switch is on: {why}") });
            }
            if s["connection"]["state"] == "error" {
                // One outage is one finding, however many passes see it.
                found.push(Finding { key: "arbiter-unreachable".into(), summary: "The exchange did not answer Arbiter".into() });
            }
            for st in s["strategies"].as_array().into_iter().flatten().filter(|st| st["state"] == "halted") {
                found.push(Finding {
                    key: format!("strategy-halt:{}:{}", st["id"], st["halt_reason"].as_str().unwrap_or("")),
                    summary: format!("Strategy \"{}\" halted: {}", st["name"].as_str().unwrap_or("?"), st["halt_reason"].as_str().unwrap_or("")),
                });
            }
            Ok(found)
        }
        Rule::Workout {} => {
            let s = read(engine, "gym.summary", json!({}))?;
            Ok(s["imported"]["exported_at"].as_str().map(|at| Finding { key: format!("gym:{at}"), summary: format!("Avex sent its history (exported {at})") }).into_iter().collect())
        }
        Rule::Stuck { .. } => Ok(Vec::new()),
    }
}

/// Notes which sessions are blocked now, and since when.
fn note_blocked(engine: &Engine) {
    let Ok(list) = read(engine, "session.list", json!({"state": ["blocked"]})) else { return };
    let now = crate::time::now();
    let blocked: BTreeSet<String> = list["sessions"].as_array().into_iter().flatten().filter_map(|s| s["name"].as_str().map(str::to_string)).collect();
    let mut seen = super::lock(&engine.aide.blocked);
    seen.retain(|name, _| blocked.contains(name));
    for name in blocked {
        seen.entry(name).or_insert_with(|| now.clone());
    }
    drop(seen);
    super::poke(engine);
}

/// Wakes each stuck rule whose session has waited long enough; when the next one will have.
pub fn check_stuck(engine: &Arc<Engine>, jobs: &[Job], now: Timestamp) -> Option<Timestamp> {
    let blocked: Vec<(String, String)> = super::lock(&engine.aide.blocked).iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    if blocked.is_empty() {
        return None;
    }
    let mut next: Option<Timestamp> = None;
    let tier = super::settings(engine).map_or(Tier::Off, |s| s.tiers.of(Source::Dev));
    for job in jobs.iter().filter(|j| j.state == JobState::Active) {
        for rule in &job.spec.rules {
            let Rule::Stuck { minutes } = rule else { continue };
            for (name, since) in &blocked {
                let Some(at) = since.parse::<Timestamp>().ok().and_then(|t| t.checked_add(jiff::SignedDuration::from_mins(i64::from(*minutes))).ok()) else { continue };
                if at > now {
                    next = Some(next.map_or(at, |n| n.min(at)));
                    continue;
                }
                let key = format!("stuck:{name}:{since}");
                // As [`fire`]: Relay's dev data turned off for every model wakes nothing.
                if with_db(engine, |db| db.see(job.id, &key, &crate::time::now())).unwrap_or(false) && tier != Tier::Off {
                    let _ = runs::queue(engine, job.id, Reason::Rule, &format!("session {name} has waited on you since {since}"), tier == Tier::LocalOnly);
                }
            }
        }
    }
    next
}
