//! What Relay shows of the history and what a thread's agent reads: the shapes the `gym.*` bus
//! ops return, and the pure functions that read them from a [`History`].
//!
//! Weights and volumes are in the person's unit (`unit`, Avex's `useKg`): weights to a tenth,
//! volumes whole. A session's volume, set count and PR count are Avex's own, so the PC says what
//! the phone says; per-lift figures are counted here from the sets. Warm-up sets count toward
//! nothing. Untracked sessions (excluded in Avex) are listed, marked, and left out of every total,
//! week and series, as Avex leaves them out of its stats.
//!
//! An estimated one-rep max (`e1rm`) is Epley's, `weight × (1 + reps / 30)`, for sets of 1 to 12
//! reps; past 12 it says little, so those sets have none.

use crate::model::{Exercise, History, Session, Set};
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const LB_PER_KG: f64 = 2.204_622_621_8;
/// The most reps an estimated one-rep max is read from.
pub const E1RM_REPS: i64 = 12;

/// Where "today" and "this week" are: the PC's zone and date.
#[derive(Debug, Clone)]
pub struct Clock {
    pub tz: TimeZone,
    pub today: Date,
}

impl Clock {
    pub fn system() -> Clock {
        let now = jiff::Zoned::now();
        Clock { tz: now.time_zone().clone(), today: now.date() }
    }
}

// ------------------------------------------------------------------ shapes

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "GymImported")]
pub struct Imported {
    /// As Avex wrote it: the phone's local date and time.
    pub exported_at: String,
    /// RFC 3339, when the PC read it.
    pub imported_at: String,
    pub app_version: String,
    pub file: String,
    pub user_name: String,
    /// Avex's training days a week, 0 when unset.
    pub days_per_week: i64,
}

/// One week (or month) of training.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "GymWeek")]
pub struct Week {
    /// Its first day, `YYYY-MM-DD`.
    pub start: String,
    pub sessions: u32,
    pub sets: i64,
    pub volume: f64,
    pub active_minutes: i64,
    pub cardio_minutes: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "GymTotals")]
pub struct Totals {
    pub sessions: u32,
    pub sets: i64,
    pub volume: f64,
    pub cardio: u32,
    pub cardio_minutes: i64,
    pub first_day: Option<String>,
    pub last_day: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "GymSessionRow")]
pub struct SessionRow {
    /// Avex's id for the session.
    pub id: i64,
    pub day: String,
    /// `HH:MM`, the PC's zone.
    pub time: String,
    /// The program day (`Push A`), or `Freestyle`.
    pub title: String,
    pub session_type: String,
    /// Minutes actually training.
    pub minutes: i64,
    /// The lifts done, in order; skipped ones left out.
    pub lifts: Vec<String>,
    pub skipped: u32,
    pub sets: i64,
    pub volume: f64,
    pub prs: i64,
    pub mood: String,
    /// Excluded from Avex's stats, and from every total here.
    pub untracked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "GymSet")]
pub struct SetView {
    /// Absent for a bodyweight set.
    pub weight: Option<f64>,
    pub reps: i64,
    pub rpe: Option<f64>,
    /// `work`, `warmup`, or Avex's own name for another kind (`drop`…).
    pub kind: String,
    pub e1rm: Option<f64>,
    /// The exercise's best set by estimated one-rep max.
    pub best: bool,
    pub amrap: bool,
    pub to_failure: bool,
    pub assisted: bool,
    pub seconds: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "GymExercise")]
pub struct ExerciseView {
    pub name: String,
    pub skipped: bool,
    /// Avex marked a personal record on it.
    pub was_pr: bool,
    pub difficulty: String,
    pub note: String,
    pub superset: String,
    pub volume: f64,
    pub best_e1rm: Option<f64>,
    pub sets: Vec<SetView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "GymSession")]
pub struct SessionDetail {
    pub row: SessionRow,
    pub unit: String,
    /// RFC 3339.
    pub started_at: String,
    pub finished_at: Option<String>,
    pub journal: String,
    pub tags: String,
    pub intensity: String,
    pub exercises: Vec<ExerciseView>,
}

/// A lift's best set: by estimated one-rep max, the heavier on a tie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "GymRecord")]
pub struct Record {
    pub lift: String,
    pub day: String,
    pub session_id: i64,
    pub weight: f64,
    pub reps: i64,
    pub e1rm: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "GymLiftRow")]
pub struct LiftRow {
    pub name: String,
    pub sessions: u32,
    pub sets: i64,
    pub last_day: String,
    /// The best set ever, by estimated one-rep max.
    pub best: Option<Record>,
    /// The best estimated one-rep max of the last 28 days.
    pub recent_e1rm: Option<f64>,
    /// That beside the best of the 28 days before, in percent.
    pub change_pct: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "GymLiftPoint")]
pub struct LiftPoint {
    pub day: String,
    pub session_id: i64,
    /// The heaviest working set's weight and its reps.
    pub top_weight: Option<f64>,
    pub top_reps: i64,
    pub e1rm: Option<f64>,
    pub sets: i64,
    pub volume: f64,
    /// Its e1rm beat every earlier session's.
    pub record: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "GymLift")]
pub struct LiftDetail {
    pub row: LiftRow,
    pub unit: String,
    /// Newest first.
    pub history: Vec<LiftPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "GymCardioRow")]
pub struct CardioRow {
    pub day: String,
    pub time: String,
    /// Avex's name for it: `run`, `bike`, `rest`…
    pub kind: String,
    pub minutes: i64,
    pub distance_km: Option<f64>,
    pub effort: String,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "GymGoalRow")]
pub struct GoalRow {
    pub kind: String,
    /// What it is about: a lift's id, a measure.
    pub target_key: String,
    /// A `lift_target`'s weight is in the person's unit like every other weight; any other
    /// kind's figure is as Avex stores it.
    pub target_value: Option<f64>,
    pub created_day: String,
    pub completed_day: Option<String>,
    pub source: String,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "GymSummary")]
pub struct Summary {
    /// Absent until an export was imported.
    pub imported: Option<Imported>,
    /// `kg` or `lb`.
    pub unit: String,
    pub totals: Totals,
    pub this_week: Week,
    pub last_week: Week,
    /// The last 12 weeks, oldest first, this one last.
    pub weeks: Vec<Week>,
    /// Weeks in a row with a workout, counting back from this week (or last, while this one is
    /// still empty).
    pub streak_weeks: u32,
    pub days_since_last: Option<i64>,
    /// The newest five.
    pub recent: Vec<SessionRow>,
    /// The newest eight lifts Avex marked as records, each with its best set that day.
    pub records: Vec<Record>,
    /// The lifts done most in the last 12 weeks (all time when none were).
    pub lifts: Vec<LiftRow>,
    /// Goals neither met nor archived.
    pub goals: Vec<GoalRow>,
}

/// What [`series`] reads. Every field is optional.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "GymSeriesQuery")]
pub struct SeriesQuery {
    /// `volume` (default), `sessions`, `sets`, `minutes`, `e1rm`, `top_weight`, `cardio_minutes`
    /// or `distance`. `e1rm` and `top_weight` need a `lift`.
    pub measure: Option<String>,
    /// `week` (default), `month`, or `session` (one point per session; with a `lift`, each session
    /// it was done in).
    pub by: Option<String>,
    /// How many weeks or months end at the current one; for `session`, how many weeks back. 12 by
    /// default, at most 260.
    pub periods: Option<u32>,
    /// A lift by name, as Avex shows it; a unique part of the name is enough.
    pub lift: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "GymLine")]
pub struct Line {
    pub name: String,
    /// One per label; null where there is nothing to read (a week without that lift).
    pub values: Vec<Option<f64>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "GymSeries")]
pub struct Series {
    pub measure: String,
    pub by: String,
    /// The lift's name as Avex shows it, when one was asked for.
    pub lift: Option<String>,
    /// What the values are in: `kg`, `lb`, `sessions`, `sets`, `minutes` or `km`.
    pub unit: String,
    /// Short captions: `Oct 6`, `Oct`.
    pub labels: Vec<String>,
    /// Each label's first day, `YYYY-MM-DD`.
    pub days: Vec<String>,
    pub series: Vec<Line>,
}

// ------------------------------------------------------------------ helpers

fn date_of(ms: i64, tz: &TimeZone) -> Date {
    Timestamp::from_millisecond(ms).map(|t| t.to_zoned(tz.clone()).date()).unwrap_or_default()
}

fn time_of(ms: i64, tz: &TimeZone) -> String {
    Timestamp::from_millisecond(ms).map(|t| t.to_zoned(tz.clone()).strftime("%H:%M").to_string()).unwrap_or_default()
}

fn rfc3339(ms: i64, tz: &TimeZone) -> String {
    Timestamp::from_millisecond(ms).map(|t| t.to_zoned(tz.clone()).strftime("%Y-%m-%dT%H:%M:%S%:z").to_string()).unwrap_or_default()
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

fn short_day(d: Date) -> String {
    format!("{} {}", MONTHS[(d.month() - 1) as usize], d.day())
}

fn week_start(d: Date, monday: bool) -> Date {
    let from_monday = i64::from(d.weekday().to_monday_zero_offset());
    let back = if monday { from_monday } else { (from_monday + 1) % 7 };
    d.checked_sub(back.days()).unwrap_or(d)
}

fn month_start(d: Date) -> Date {
    d.first_of_month()
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// Pounds in the person's unit, to a tenth.
fn weight(lb: f64, kg: bool) -> f64 {
    round1(if kg { lb / LB_PER_KG } else { lb })
}

/// A volume in pounds in the person's unit, whole.
fn volume(lb: f64, kg: bool) -> f64 {
    (if kg { lb / LB_PER_KG } else { lb }).round()
}

/// Epley's estimate in pounds, for 1 to 12 reps.
pub fn e1rm_lb(set: &Set) -> Option<f64> {
    let w = set.weight_lb?;
    match set.reps {
        1 => Some(w),
        r if (2..=E1RM_REPS).contains(&r) => Some(w * (1.0 + r as f64 / 30.0)),
        _ => None,
    }
}

fn working(e: &Exercise) -> impl Iterator<Item = &Set> {
    e.sets.iter().filter(|s| !s.warmup() && s.reps > 0)
}

/// The index in `e.sets` of the best working set: by e1rm, then weight, then the earliest.
fn best_index(e: &Exercise) -> Option<usize> {
    let key = |s: &Set| (e1rm_lb(s).unwrap_or(0.0), s.weight_lb.unwrap_or(0.0));
    // `max_by` keeps the last of equals; reversed, that is the first set.
    e.sets
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, s)| !s.warmup() && s.reps > 0 && s.weight_lb.is_some())
        .max_by(|(_, a), (_, b)| key(a).partial_cmp(&key(b)).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
}

fn best_set(e: &Exercise) -> Option<&Set> {
    best_index(e).map(|i| &e.sets[i])
}

fn lift_volume_lb(e: &Exercise) -> f64 {
    working(e).map(|s| s.weight_lb.unwrap_or(0.0) * s.reps as f64).sum()
}

fn lift_key(name: &str) -> String {
    name.trim().to_lowercase()
}

/// `push_a` → `Push A`.
fn title(day_key: &str, session_type: &str) -> String {
    let words: Vec<String> = day_key
        .split(['_', '-', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
        })
        .collect();
    if !words.is_empty() {
        words.join(" ")
    } else if !session_type.is_empty() && session_type != "normal" {
        title(session_type, "")
    } else {
        "Freestyle".into()
    }
}

fn unit(h: &History) -> &'static str {
    if h.meta.use_kg {
        "kg"
    } else {
        "lb"
    }
}

fn tracked(h: &History) -> impl DoubleEndedIterator<Item = &Session> {
    h.sessions.iter().filter(|s| !s.untracked)
}

pub fn row(h: &History, s: &Session, clock: &Clock) -> SessionRow {
    let kg = h.meta.use_kg;
    let done: Vec<&Exercise> = s.exercises.iter().filter(|e| !e.skipped).collect();
    let counted: i64 = done.iter().map(|e| working(e).count() as i64).sum();
    SessionRow {
        id: s.id,
        day: date_of(s.started_at, &clock.tz).to_string(),
        time: time_of(s.started_at, &clock.tz),
        title: title(&s.day_key, &s.session_type),
        session_type: s.session_type.clone(),
        minutes: if s.active_seconds > 0 {
            s.active_seconds / 60
        } else {
            s.finished_at.map_or(0, |f| ((f - s.started_at) / 60_000).max(0))
        },
        lifts: done.iter().map(|e| e.name.clone()).collect(),
        skipped: (s.exercises.len() - done.len()) as u32,
        sets: if s.set_count > 0 { s.set_count } else { counted },
        volume: volume(if s.volume_lb > 0.0 { s.volume_lb } else { done.iter().map(|e| lift_volume_lb(e)).sum() }, kg),
        prs: s.pr_count,
        mood: s.mood.clone(),
        untracked: s.untracked,
    }
}

fn record(h: &History, s: &Session, e: &Exercise, best: &Set, clock: &Clock) -> Record {
    let kg = h.meta.use_kg;
    Record {
        lift: e.name.clone(),
        day: date_of(s.started_at, &clock.tz).to_string(),
        session_id: s.id,
        weight: weight(best.weight_lb.unwrap_or(0.0), kg),
        reps: best.reps,
        e1rm: e1rm_lb(best).map(|v| weight(v, kg)),
    }
}

fn goal_row(g: &crate::model::Goal, kg: bool, clock: &Clock) -> GoalRow {
    GoalRow {
        kind: g.kind.clone(),
        target_key: g.target_key.clone(),
        // Avex keeps a lift's target in pounds, as it keeps every set.
        target_value: g.target_value.map(|v| if g.kind == "lift_target" { weight(v, kg) } else { v }),
        created_day: date_of(g.created_at, &clock.tz).to_string(),
        completed_day: g.completed_at.map(|c| date_of(c, &clock.tz).to_string()),
        source: g.source.clone(),
        note: g.note.clone(),
    }
}

/// Each period starting at one of `starts` with its totals; `of` is the period a day falls in.
fn buckets(h: &History, clock: &Clock, starts: &[Date], of: impl Fn(Date) -> Date) -> Vec<Week> {
    let kg = h.meta.use_kg;
    let mut out: Vec<Week> = starts.iter().map(|d| Week { start: d.to_string(), ..Week::default() }).collect();
    let index: HashMap<Date, usize> = starts.iter().enumerate().map(|(i, d)| (*d, i)).collect();
    for s in tracked(h) {
        if let Some(&i) = index.get(&of(date_of(s.started_at, &clock.tz))) {
            let r = row(h, s, clock);
            let w = &mut out[i];
            w.sessions += 1;
            // Saturating: a stored count is whatever an export said, and a sum past i64 is a panic.
            w.sets = w.sets.saturating_add(r.sets);
            w.active_minutes = w.active_minutes.saturating_add(r.minutes);
            w.volume += if s.volume_lb > 0.0 { s.volume_lb } else { s.exercises.iter().filter(|e| !e.skipped).map(lift_volume_lb).sum() };
        }
    }
    for c in &h.cardio {
        if let Some(&i) = index.get(&of(date_of(c.date, &clock.tz))) {
            out[i].cardio_minutes = out[i].cardio_minutes.saturating_add(c.duration_min);
        }
    }
    for w in &mut out {
        w.volume = volume(w.volume, kg);
    }
    out
}

fn week_starts(clock: &Clock, monday: bool, n: u32) -> Vec<Date> {
    let this = week_start(clock.today, monday);
    (0..i64::from(n)).rev().filter_map(|i| this.checked_sub((i * 7).days()).ok()).collect()
}

fn month_starts(clock: &Clock, n: u32) -> Vec<Date> {
    let this = month_start(clock.today);
    (0..i64::from(n)).rev().filter_map(|i| this.checked_sub(i.months()).ok()).collect()
}

fn weeks(h: &History, clock: &Clock, n: u32) -> Vec<Week> {
    let monday = h.meta.first_day_monday;
    buckets(h, clock, &week_starts(clock, monday, n), |d| week_start(d, monday))
}

// ------------------------------------------------------------------ readings

pub fn summary(h: &History, clock: &Clock, imported: bool) -> Summary {
    let kg = h.meta.use_kg;
    let weeks = weeks(h, clock, 12);
    let this_week = weeks.last().cloned().unwrap_or_default();
    let last_week = weeks.get(weeks.len().wrapping_sub(2)).cloned().unwrap_or_default();
    // The streak may run past the twelve weeks shown, so it counts from every session.
    let monday = h.meta.first_day_monday;
    let trained: std::collections::HashSet<Date> = tracked(h).map(|s| week_start(date_of(s.started_at, &clock.tz), monday)).collect();
    let mut at = week_start(clock.today, monday);
    if !trained.contains(&at) {
        at = at.checked_sub(7.days()).unwrap_or(at);
    }
    let mut streak = 0;
    while trained.contains(&at) {
        streak += 1;
        match at.checked_sub(7.days()) {
            Ok(d) => at = d,
            Err(_) => break,
        }
    }
    let rows: Vec<SessionRow> = tracked(h).map(|s| row(h, s, clock)).collect();
    let totals = Totals {
        sessions: rows.len() as u32,
        sets: rows.iter().fold(0, |sum: i64, r| sum.saturating_add(r.sets)),
        volume: volume(tracked(h).map(|s| s.volume_lb).sum(), kg),
        cardio: h.cardio.len() as u32,
        cardio_minutes: h.cardio.iter().fold(0, |sum: i64, c| sum.saturating_add(c.duration_min)),
        first_day: rows.first().map(|r| r.day.clone()),
        last_day: rows.last().map(|r| r.day.clone()),
    };
    let days_since_last = tracked(h).next_back().map(|s| (clock.today - date_of(s.started_at, &clock.tz)).get_days().into());
    let mut records = Vec::new();
    for s in tracked(h).rev() {
        for e in s.exercises.iter().filter(|e| e.was_pr && !e.skipped) {
            if let Some(best) = best_set(e) {
                records.push(record(h, s, e, best, clock));
            }
        }
        if records.len() >= 8 {
            break;
        }
    }
    records.truncate(8);
    let since = clock.today.checked_sub(84.days()).unwrap_or(clock.today);
    let mut lifts = lifts_since(h, clock, Some(since));
    if lifts.is_empty() {
        lifts = lifts_since(h, clock, None);
    }
    lifts.truncate(8);
    Summary {
        imported: imported.then(|| Imported {
            exported_at: h.meta.exported_at.clone(),
            imported_at: h.meta.imported_at.clone(),
            app_version: h.meta.app_version.clone(),
            file: h.meta.file.clone(),
            user_name: h.meta.user_name.clone(),
            days_per_week: h.meta.days_per_week,
        }),
        unit: unit(h).into(),
        totals,
        this_week,
        last_week,
        weeks,
        streak_weeks: streak,
        days_since_last,
        recent: h.sessions.iter().rev().take(5).map(|s| row(h, s, clock)).collect(),
        records,
        lifts,
        goals: h.goals.iter().filter(|g| g.completed_at.is_none() && g.archived_at.is_none()).map(|g| goal_row(g, h.meta.use_kg, clock)).collect(),
    }
}

/// Sessions newest first, from `offset`, at most `limit`; only those with `lift` when given.
/// Returns the page and how many match in all.
pub fn sessions(h: &History, clock: &Clock, offset: usize, limit: usize, lift: Option<&str>) -> Result<(Vec<SessionRow>, usize), String> {
    let key = lift.map(|l| find_lift(h, l)).transpose()?.map(|n| lift_key(&n));
    let matching: Vec<&Session> = h
        .sessions
        .iter()
        .rev()
        .filter(|s| key.as_ref().is_none_or(|k| s.exercises.iter().any(|e| !e.skipped && lift_key(&e.name) == *k)))
        .collect();
    let total = matching.len();
    Ok((matching.into_iter().skip(offset).take(limit).map(|s| row(h, s, clock)).collect(), total))
}

pub fn session(h: &History, clock: &Clock, id: i64) -> Option<SessionDetail> {
    let kg = h.meta.use_kg;
    let s = h.sessions.iter().find(|s| s.id == id)?;
    let exercises = s
        .exercises
        .iter()
        .map(|e| {
            let best = best_index(e);
            ExerciseView {
                name: e.name.clone(),
                skipped: e.skipped,
                was_pr: e.was_pr,
                difficulty: e.difficulty.clone(),
                note: e.note.clone(),
                superset: e.superset.clone(),
                volume: volume(lift_volume_lb(e), kg),
                best_e1rm: best_set(e).and_then(e1rm_lb).map(|v| weight(v, kg)),
                sets: e
                    .sets
                    .iter()
                    .enumerate()
                    .map(|(i, set)| SetView {
                        weight: set.weight_lb.map(|w| weight(w, kg)),
                        reps: set.reps,
                        rpe: set.rpe,
                        kind: if set.warmup() {
                            "warmup".into()
                        } else if set.set_type.is_empty() || set.set_type.eq_ignore_ascii_case("normal") {
                            "work".into()
                        } else {
                            set.set_type.to_lowercase()
                        },
                        e1rm: if set.warmup() { None } else { e1rm_lb(set).map(|v| weight(v, kg)) },
                        best: best == Some(i),
                        amrap: set.amrap,
                        to_failure: set.to_failure,
                        assisted: set.assisted,
                        seconds: set.duration_seconds,
                    })
                    .collect(),
            }
        })
        .collect();
    Some(SessionDetail {
        row: row(h, s, clock),
        unit: unit(h).into(),
        started_at: rfc3339(s.started_at, &clock.tz),
        finished_at: s.finished_at.map(|f| rfc3339(f, &clock.tz)),
        journal: s.journal.clone(),
        tags: s.tags.clone(),
        intensity: s.intensity.clone(),
        exercises,
    })
}

/// Every lift, done most first; with `query`, only names containing it.
pub fn lifts(h: &History, clock: &Clock, query: Option<&str>) -> Vec<LiftRow> {
    let q = query.map(lift_key).filter(|q| !q.is_empty());
    lifts_since(h, clock, None).into_iter().filter(|l| q.as_ref().is_none_or(|q| lift_key(&l.name).contains(q))).collect()
}

/// Lifts done since `since` (all time when `None`), done most first; each row reads its whole
/// history for the best set.
fn lifts_since(h: &History, clock: &Clock, since: Option<Date>) -> Vec<LiftRow> {
    let mut order: Vec<(String, u32)> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    for s in tracked(h) {
        if since.is_some_and(|d| date_of(s.started_at, &clock.tz) < d) {
            continue;
        }
        for e in s.exercises.iter().filter(|e| !e.skipped && working(e).next().is_some()) {
            let key = lift_key(&e.name);
            match seen.get(&key) {
                Some(&i) => order[i].1 += 1,
                None => {
                    seen.insert(key, order.len());
                    order.push((e.name.clone(), 1));
                }
            }
        }
    }
    order.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    order.into_iter().filter_map(|(name, _)| lift_row(h, clock, &name)).collect()
}

fn lift_row(h: &History, clock: &Clock, name: &str) -> Option<LiftRow> {
    let kg = h.meta.use_kg;
    let key = lift_key(name);
    let recent_from = clock.today.checked_sub(28.days()).unwrap_or(clock.today);
    let before_from = clock.today.checked_sub(56.days()).unwrap_or(clock.today);
    let (mut sessions, mut sets, mut last_day) = (0u32, 0i64, None);
    let mut best: Option<Record> = None;
    let mut best_key = (0.0, 0.0);
    let (mut recent, mut before): (Option<f64>, Option<f64>) = (None, None);
    let mut shown = name.to_string();
    for s in tracked(h) {
        let day = date_of(s.started_at, &clock.tz);
        for e in s.exercises.iter().filter(|e| !e.skipped && lift_key(&e.name) == key) {
            let n = working(e).count() as i64;
            if n == 0 {
                continue;
            }
            shown = e.name.clone();
            sessions += 1;
            sets += n;
            last_day = Some(day);
            if let Some(b) = best_set(e) {
                let k = (e1rm_lb(b).unwrap_or(0.0), b.weight_lb.unwrap_or(0.0));
                if best.is_none() || k > best_key {
                    best_key = k;
                    best = Some(record(h, s, e, b, clock));
                }
                if let Some(v) = e1rm_lb(b) {
                    let slot = if day >= recent_from {
                        Some(&mut recent)
                    } else if day >= before_from {
                        Some(&mut before)
                    } else {
                        None
                    };
                    if let Some(slot) = slot {
                        *slot = Some(slot.map_or(v, |o: f64| o.max(v)));
                    }
                }
            }
        }
    }
    Some(LiftRow {
        name: shown,
        sessions,
        sets,
        last_day: last_day?.to_string(),
        best,
        recent_e1rm: recent.map(|v| weight(v, kg)),
        change_pct: match (recent, before) {
            (Some(r), Some(b)) if b > 0.0 => Some(round1((r / b - 1.0) * 100.0)),
            _ => None,
        },
    })
}

/// The lift `asked` names, as Avex shows it: an exact name (any case), else the one name that
/// contains it.
pub fn find_lift(h: &History, asked: &str) -> Result<String, String> {
    let key = lift_key(asked);
    if key.is_empty() {
        return Err("Name a lift".into());
    }
    let mut names: Vec<&str> = Vec::new();
    for e in h.sessions.iter().flat_map(|s| &s.exercises) {
        if lift_key(&e.name) == key {
            return Ok(e.name.clone());
        }
        if lift_key(&e.name).contains(&key) && !names.iter().any(|n| lift_key(n) == lift_key(&e.name)) {
            names.push(&e.name);
        }
    }
    match names.as_slice() {
        [one] => Ok(one.to_string()),
        [] => Err(format!("No lift called \"{asked}\" in Avex's history")),
        many => Err(format!("\"{asked}\" could be {}; name one", many.iter().take(6).copied().collect::<Vec<_>>().join(", "))),
    }
}

pub fn lift(h: &History, clock: &Clock, asked: &str, limit: usize) -> Result<LiftDetail, String> {
    let kg = h.meta.use_kg;
    let name = find_lift(h, asked)?;
    let row = lift_row(h, clock, &name).ok_or_else(|| format!("{name} has no working sets in Avex's history"))?;
    let key = lift_key(&name);
    let mut history = Vec::new();
    let mut top = 0.0;
    for s in tracked(h) {
        for e in s.exercises.iter().filter(|e| !e.skipped && lift_key(&e.name) == key) {
            let sets = working(e).count() as i64;
            if sets == 0 {
                continue;
            }
            let heaviest = working(e).filter(|s| s.weight_lb.is_some()).max_by(|a, b| {
                (a.weight_lb, a.reps).partial_cmp(&(b.weight_lb, b.reps)).unwrap_or(std::cmp::Ordering::Equal)
            });
            let e1rm = best_set(e).and_then(e1rm_lb);
            let record = e1rm.is_some_and(|v| v > top);
            if let Some(v) = e1rm.filter(|v| *v > top) {
                top = v;
            }
            history.push(LiftPoint {
                day: date_of(s.started_at, &clock.tz).to_string(),
                session_id: s.id,
                top_weight: heaviest.and_then(|s| s.weight_lb).map(|w| weight(w, kg)),
                top_reps: heaviest.map_or_else(|| working(e).map(|s| s.reps).max().unwrap_or(0), |s| s.reps),
                e1rm: e1rm.map(|v| weight(v, kg)),
                sets,
                volume: volume(lift_volume_lb(e), kg),
                record,
            });
        }
    }
    history.reverse();
    history.truncate(limit);
    Ok(LiftDetail { row, unit: unit(h).into(), history })
}

pub fn cardio(h: &History, clock: &Clock, limit: usize) -> Vec<CardioRow> {
    h.cardio
        .iter()
        .rev()
        .take(limit)
        .map(|c| CardioRow {
            day: date_of(c.date, &clock.tz).to_string(),
            time: time_of(c.date, &clock.tz),
            kind: c.kind.clone(),
            minutes: c.duration_min,
            distance_km: c.distance_km.map(|d| (d * 100.0).round() / 100.0),
            effort: c.effort.clone(),
            note: c.note.clone(),
        })
        .collect()
}

/// A chart's numbers (docs/GYM.md).
pub fn series(h: &History, clock: &Clock, q: &SeriesQuery) -> Result<Series, String> {
    let kg = h.meta.use_kg;
    let measure = q.measure.as_deref().unwrap_or("volume").to_string();
    let by = q.by.as_deref().unwrap_or("week").to_string();
    let periods = q.periods.unwrap_or(12).clamp(1, 260);
    let lift = q.lift.as_deref().map(|l| find_lift(h, l)).transpose()?;
    let key = lift.as_deref().map(lift_key);
    let units = match measure.as_str() {
        "volume" | "e1rm" | "top_weight" => unit(h),
        "sessions" => "sessions",
        "sets" => "sets",
        "minutes" | "cardio_minutes" => "minutes",
        "distance" => "km",
        other => return Err(format!("Unknown measure {other}: volume, sessions, sets, minutes, e1rm, top_weight, cardio_minutes or distance")),
    };
    if matches!(measure.as_str(), "e1rm" | "top_weight") && key.is_none() {
        return Err(format!("{measure} is a lift's: give a lift"));
    }
    let cardio = matches!(measure.as_str(), "cardio_minutes" | "distance");
    // A lift's own reading of one session, in pounds (or a count).
    let read = |s: &Session| -> Option<f64> {
        let exercises: Vec<&Exercise> =
            s.exercises.iter().filter(|e| !e.skipped && key.as_ref().is_none_or(|k| lift_key(&e.name) == *k)).collect();
        if key.is_some() && exercises.iter().all(|e| working(e).next().is_none()) {
            return None;
        }
        match measure.as_str() {
            "volume" if key.is_none() && s.volume_lb > 0.0 => Some(s.volume_lb),
            "volume" => Some(exercises.iter().map(|e| lift_volume_lb(e)).sum()),
            "sessions" => Some(1.0),
            "sets" if key.is_none() && s.set_count > 0 => Some(s.set_count as f64),
            "sets" => Some(exercises.iter().map(|e| working(e).count() as f64).sum()),
            "minutes" => Some(row(h, s, clock).minutes as f64),
            "e1rm" => exercises.iter().filter_map(|e| best_set(e).and_then(e1rm_lb)).reduce(f64::max),
            "top_weight" => exercises.iter().flat_map(|e| working(e)).filter_map(|s| s.weight_lb).reduce(f64::max),
            _ => None,
        }
    };
    // Sums add up within a period; a lift's best is the period's best.
    let best_of = matches!(measure.as_str(), "e1rm" | "top_weight");
    let finish = |v: f64| match measure.as_str() {
        "volume" => volume(v, kg),
        "e1rm" | "top_weight" => weight(v, kg),
        "distance" => (v * 100.0).round() / 100.0,
        _ => v,
    };
    let name = lift.clone().unwrap_or_else(|| match measure.as_str() {
        "volume" => "Volume".into(),
        "sessions" => "Sessions".into(),
        "sets" => "Sets".into(),
        "minutes" => "Minutes".into(),
        "cardio_minutes" => "Cardio minutes".into(),
        _ => "Distance".into(),
    });
    let (labels, days, values) = match by.as_str() {
        "week" | "month" => {
            let monday = h.meta.first_day_monday;
            let starts = if by == "week" { week_starts(clock, monday, periods) } else { month_starts(clock, periods) };
            let of = |d: Date| if by == "week" { week_start(d, monday) } else { month_start(d) };
            let index: HashMap<Date, usize> = starts.iter().enumerate().map(|(i, d)| (*d, i)).collect();
            let mut values: Vec<Option<f64>> = vec![None; starts.len()];
            let mut add = |at: Date, v: f64| {
                if let Some(&i) = index.get(&of(at)) {
                    values[i] = Some(match values[i] {
                        Some(o) if best_of => o.max(v),
                        Some(o) => o + v,
                        None => v,
                    });
                }
            };
            if cardio {
                for c in &h.cardio {
                    let v = if measure == "distance" { c.distance_km.unwrap_or(0.0) } else { c.duration_min as f64 };
                    add(date_of(c.date, &clock.tz), v);
                }
            } else {
                for s in tracked(h) {
                    if let Some(v) = read(s) {
                        add(date_of(s.started_at, &clock.tz), v);
                    }
                }
            }
            // A sum reads zero for an empty period; a lift's best has nothing to read there.
            let values = values.into_iter().map(|v| v.map(finish).or(if best_of { None } else { Some(0.0) })).collect();
            let labels = starts.iter().map(|d| if by == "week" { short_day(*d) } else { MONTHS[(d.month() - 1) as usize].to_string() }).collect();
            (labels, starts.iter().map(Date::to_string).collect(), values)
        }
        "session" => {
            let from = week_start(clock.today, h.meta.first_day_monday).checked_sub((i64::from(periods - 1) * 7).days()).unwrap_or(clock.today);
            let mut labels = Vec::new();
            let mut days = Vec::new();
            let mut values = Vec::new();
            if cardio {
                for c in h.cardio.iter().filter(|c| date_of(c.date, &clock.tz) >= from) {
                    let d = date_of(c.date, &clock.tz);
                    labels.push(short_day(d));
                    days.push(d.to_string());
                    values.push(Some(finish(if measure == "distance" { c.distance_km.unwrap_or(0.0) } else { c.duration_min as f64 })));
                }
            } else {
                for s in tracked(h).filter(|s| date_of(s.started_at, &clock.tz) >= from) {
                    if let Some(v) = read(s) {
                        let d = date_of(s.started_at, &clock.tz);
                        labels.push(short_day(d));
                        days.push(d.to_string());
                        values.push(Some(finish(v)));
                    }
                }
            }
            (labels, days, values)
        }
        other => return Err(format!("Unknown grouping {other}: week, month or session")),
    };
    Ok(Series { measure, by, lift, unit: units.into(), labels, days, series: vec![Line { name, values }] })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Cardio, Meta};

    fn ms(day: &str, hour: i8) -> i64 {
        let d: Date = day.parse().unwrap();
        d.at(hour, 0, 0, 0).to_zoned(TimeZone::UTC).unwrap().timestamp().as_millisecond()
    }

    fn set(lb: f64, reps: i64) -> Set {
        Set { weight_lb: Some(lb), reps, ..Set::default() }
    }

    fn bench(sets: Vec<Set>, pr: bool) -> Exercise {
        Exercise { name: "Bench Press".into(), was_pr: pr, sets, ..Exercise::default() }
    }

    fn history(kg: bool) -> History {
        let warm = Set { set_type: "warmup".into(), ..set(135.0, 10) };
        History {
            meta: Meta { use_kg: kg, first_day_monday: true, exported_at: "2026-10-08 19:02".into(), ..Meta::default() },
            sessions: vec![
                // Week of Sep 14: a bench session.
                Session { id: 1, day_key: "push_a".into(), started_at: ms("2026-09-15", 18), active_seconds: 3000, volume_lb: 2250.0, set_count: 2, exercises: vec![bench(vec![warm.clone(), set(225.0, 5), set(225.0, 5)], false)], ..Session::default() },
                // Week of Sep 28: heavier, a record.
                Session { id: 2, started_at: ms("2026-09-29", 18), active_seconds: 2400, volume_lb: 1395.0, set_count: 2, pr_count: 1,
                    exercises: vec![bench(vec![warm, set(245.0, 3), set(225.0, 3)], true), Exercise { name: "Squat".into(), skipped: true, ..Exercise::default() }], ..Session::default() },
                // This week, untracked: listed, never counted.
                Session { id: 3, started_at: ms("2026-10-06", 7), untracked: true, volume_lb: 9999.0, exercises: vec![bench(vec![set(315.0, 1)], true)], ..Session::default() },
                // This week, tracked.
                Session { id: 4, day_key: "pull_b".into(), started_at: ms("2026-10-07", 18), finished_at: Some(ms("2026-10-07", 19)), volume_lb: 1000.0,
                    exercises: vec![Exercise { name: "Barbell Row".into(), sets: vec![set(100.0, 10)], ..Exercise::default() }], ..Session::default() },
            ],
            cardio: vec![Cardio { date: ms("2026-10-08", 7), kind: "run".into(), duration_min: 30, distance_km: Some(5.0), ..Cardio::default() }],
            goals: vec![],
        }
    }

    fn clock() -> Clock {
        Clock { tz: TimeZone::UTC, today: "2026-10-09".parse().unwrap() }
    }

    #[test]
    fn epley_for_one_to_twelve_reps() {
        assert_eq!(e1rm_lb(&set(200.0, 1)), Some(200.0));
        assert_eq!(e1rm_lb(&set(225.0, 5)).map(round1), Some(262.5));
        assert_eq!(e1rm_lb(&set(100.0, 13)), None);
        assert_eq!(e1rm_lb(&Set { reps: 5, ..Set::default() }), None);
    }

    #[test]
    fn a_lift_target_is_in_the_persons_unit() {
        let goal = |kind: &str, value: f64| crate::model::Goal {
            kind: kind.into(), target_key: "bench".into(), target_value: Some(value), priority: 1,
            created_at: 1_759_000_000_000, completed_at: None, archived_at: None, source: String::new(), note: String::new(),
        };
        let mut h = history(true);
        h.goals = vec![goal("lift_target", 245.0), goal("sessions_per_week", 4.0)];
        let s = summary(&h, &clock(), true);
        assert_eq!(s.goals[0].target_value, Some(111.1));
        assert_eq!(s.goals[1].target_value, Some(4.0));
        h.meta.use_kg = false;
        assert_eq!(summary(&h, &clock(), true).goals[0].target_value, Some(245.0));
    }

    #[test]
    fn summary_counts_what_avex_counts() {
        let h = history(false);
        let s = summary(&h, &clock(), true);
        assert_eq!(s.unit, "lb");
        // The untracked session is left out of totals and weeks, and still listed.
        assert_eq!(s.totals.sessions, 3);
        assert_eq!(s.totals.volume, 2250.0 + 1395.0 + 1000.0);
        assert_eq!(s.recent[0].id, 4);
        assert!(s.recent[1].untracked);
        assert_eq!(s.this_week.start, "2026-10-05");
        assert_eq!(s.this_week.sessions, 1);
        assert_eq!(s.this_week.cardio_minutes, 30);
        assert_eq!(s.weeks.len(), 12);
        // Sep 28 and Oct 5 trained, Sep 21 not.
        assert_eq!(s.streak_weeks, 2);
        assert_eq!(s.days_since_last, Some(2));
        // The record Avex marked, with its best set; not the untracked one.
        assert_eq!(s.records.len(), 1);
        assert_eq!((s.records[0].weight, s.records[0].reps, s.records[0].session_id), (245.0, 3, 2));
        assert_eq!(s.lifts[0].name, "Bench Press");
        assert_eq!(s.lifts[0].sessions, 2);
        assert_eq!(s.recent[0].title, "Pull B");
        assert_eq!(s.recent[2].title, "Freestyle");
        assert_eq!(s.recent[2].skipped, 1);
        assert_eq!(s.recent[0].minutes, 60);
    }

    #[test]
    fn kilograms_when_avex_uses_them() {
        let h = history(true);
        let d = session(&h, &clock(), 2).unwrap();
        assert_eq!(d.unit, "kg");
        let sets = &d.exercises[0].sets;
        assert_eq!(sets[0].kind, "warmup");
        assert_eq!(sets[0].e1rm, None);
        assert_eq!(sets[1].weight, Some(111.1));
        assert!(sets[1].best && !sets[2].best);
        // Two equal sets: the first is the best.
        let tie = session(&history(false), &clock(), 1).unwrap();
        assert!(tie.exercises[0].sets[1].best && !tie.exercises[0].sets[2].best);
        assert_eq!(d.row.volume, (1395.0 / LB_PER_KG).round());
    }

    #[test]
    fn lifts_and_their_history() {
        let h = history(false);
        let rows = lifts(&h, &clock(), Some("bench"));
        assert_eq!(rows.len(), 1);
        let best = rows[0].best.as_ref().unwrap();
        assert_eq!((best.weight, best.reps), (245.0, 3));
        let d = lift(&h, &clock(), "bench", 50).unwrap();
        assert_eq!(d.history.len(), 2);
        assert_eq!(d.history[0].session_id, 2);
        assert!(d.history[0].record && d.history[1].record);
        assert_eq!(d.history[1].top_weight, Some(225.0));
        assert!(find_lift(&h, "row").is_ok());
        assert!(find_lift(&h, "deadlift").is_err());
        let (page, total) = sessions(&h, &clock(), 0, 10, Some("Bench press")).unwrap();
        // Session 3 is untracked but still a session with bench in it.
        assert_eq!((page.len(), total), (3, 3));
    }

    #[test]
    fn series_by_week_month_and_session() {
        let h = history(false);
        let c = clock();
        let v = series(&h, &c, &SeriesQuery { periods: Some(4), ..SeriesQuery::default() }).unwrap();
        assert_eq!(v.days, ["2026-09-14", "2026-09-21", "2026-09-28", "2026-10-05"]);
        assert_eq!(v.labels[0], "Sep 14");
        assert_eq!(v.series[0].values, [Some(2250.0), Some(0.0), Some(1395.0), Some(1000.0)]);
        let e = series(&h, &c, &SeriesQuery { measure: Some("e1rm".into()), lift: Some("bench".into()), periods: Some(4), ..SeriesQuery::default() }).unwrap();
        assert_eq!(e.lift.as_deref(), Some("Bench Press"));
        assert_eq!(e.series[0].values[1], None);
        assert_eq!(e.series[0].values[2], Some(269.5));
        let s = series(&h, &c, &SeriesQuery { measure: Some("top_weight".into()), by: Some("session".into()), lift: Some("bench".into()), ..SeriesQuery::default() }).unwrap();
        assert_eq!(s.series[0].values, [Some(225.0), Some(245.0)]);
        let m = series(&h, &c, &SeriesQuery { measure: Some("distance".into()), by: Some("month".into()), periods: Some(2), ..SeriesQuery::default() }).unwrap();
        assert_eq!(m.labels, ["Sep", "Oct"]);
        assert_eq!(m.series[0].values, [Some(0.0), Some(5.0)]);
        assert!(series(&h, &c, &SeriesQuery { measure: Some("e1rm".into()), ..SeriesQuery::default() }).is_err());
        assert!(series(&h, &c, &SeriesQuery { by: Some("year".into()), ..SeriesQuery::default() }).is_err());
    }

    #[test]
    fn weeks_can_start_on_sunday() {
        let mut h = history(false);
        h.meta.first_day_monday = false;
        let s = summary(&h, &clock(), true);
        assert_eq!(s.this_week.start, "2026-10-04");
        // Monday Oct 6 (untracked) and Tuesday Oct 7 fall in the week from Sunday Oct 4.
        assert_eq!(s.this_week.sessions, 1);
    }

    /// Counts already stored before the import clamped them still sum without overflowing.
    #[test]
    fn totals_saturate_instead_of_overflowing() {
        let mut h = history(false);
        for s in &mut h.sessions {
            s.set_count = i64::MAX;
            s.active_seconds = i64::MAX;
        }
        h.cardio.push(Cardio { date: ms("2026-10-08", 9), duration_min: i64::MAX, ..Cardio::default() });
        let s = summary(&h, &clock(), true);
        assert_eq!(s.totals.sets, i64::MAX);
        assert_eq!(s.totals.cardio_minutes, i64::MAX);
        assert_eq!(s.this_week.cardio_minutes, i64::MAX);
        assert_eq!(s.this_week.sets, i64::MAX);
    }
}
