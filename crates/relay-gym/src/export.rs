//! Reading Avex's training history export (`BackupRepository.exportFullDataJson` in
//! antho976/Avex, format 1), as leniently as Avex's own `ForgeJsonImporter` reads it back: a field
//! that is missing or of the wrong kind takes its default, a set with a non-finite weight loses the
//! weight, and a session with no start is left out.
//!
//! Only the full export is accepted. Avex's weekly file and its one-workout file carry the same
//! session shape, but an import replaces the PC's whole copy, so either would erase every other
//! workout.

use crate::model::{Cardio, Exercise, Goal, History, Meta, Session, Set};
use serde_json::{Map, Value};

/// The newest export format this reads.
pub const FORMAT: i64 = 1;
/// A multi-year export is ~12 MB; this leaves room and still refuses a file that is not one.
pub const MAX_BYTES: u64 = 200 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    /// Not JSON, or JSON that is not an Avex export.
    NotAvex(String),
    /// An Avex file, but not the one that carries the whole history.
    Partial(&'static str),
    /// A format this build does not know.
    TooNew(i64),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::NotAvex(why) => write!(f, "That is not an Avex export: {why}"),
            ReadError::Partial(which) => write!(
                f,
                "That is Avex's {which} export, not the whole history. In Avex: Settings, Export, Training history (JSON)"
            ),
            ReadError::TooNew(v) => write!(f, "That export is format {v}; this Relay reads up to format {FORMAT}. Update Relay"),
        }
    }
}

/// The history in `text`, an `avex_export.json`. `file` and `imported_at` are recorded with it.
pub fn read(text: &str, file: &str, imported_at: &str) -> Result<History, ReadError> {
    let root: Value = serde_json::from_str(text).map_err(|e| ReadError::NotAvex(e.to_string()))?;
    let Some(root) = root.as_object() else {
        return Err(ReadError::NotAvex("it is not a JSON object".into()));
    };
    if root.contains_key("session") {
        return Err(ReadError::Partial("single workout"));
    }
    let Some(sessions) = root.get("sessions").and_then(Value::as_array) else {
        return Err(ReadError::NotAvex("it has no sessions".into()));
    };
    // The weekly file has sessions and cardio but no settings or goals; the full one writes both.
    if root.contains_key("periodStart") || !root.contains_key("settings") {
        return Err(ReadError::Partial("weekly"));
    }
    let version = int(root, "exportVersion").unwrap_or(1);
    if version > FORMAT {
        return Err(ReadError::TooNew(version));
    }
    let settings = root.get("settings").and_then(Value::as_object);
    let setting = |k: &str| settings.and_then(|s| s.get(k));
    let meta = Meta {
        export_version: version,
        exported_at: string(root, "exportedAt"),
        app_version: string(root, "appVersion"),
        imported_at: imported_at.to_string(),
        file: file.to_string(),
        use_kg: setting("useKg").and_then(Value::as_bool).unwrap_or(false),
        user_name: setting("userName").and_then(Value::as_str).unwrap_or_default().to_string(),
        user_goal: setting("userGoal").and_then(Value::as_str).unwrap_or_default().to_string(),
        days_per_week: setting("daysPerWeek").and_then(Value::as_i64).unwrap_or(0),
        first_day_monday: setting("firstDayMonday").and_then(Value::as_bool).unwrap_or(true),
    };
    let mut out = History { meta, ..History::default() };
    out.sessions = sessions.iter().filter_map(Value::as_object).filter_map(session).collect();
    out.sessions.sort_by_key(|s| (s.started_at, s.id));
    if let Some(cardio) = root.get("cardio").and_then(Value::as_array) {
        out.cardio = cardio.iter().filter_map(Value::as_object).filter_map(cardio_entry).collect();
        out.cardio.sort_by_key(|c| c.date);
    }
    if let Some(goals) = root.get("coachGoals").and_then(Value::as_array) {
        out.goals = goals.iter().filter_map(Value::as_object).filter_map(goal).collect();
    }
    Ok(out)
}

fn string(o: &Map<String, Value>, k: &str) -> String {
    o.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn int(o: &Map<String, Value>, k: &str) -> Option<i64> {
    match o.get(k)? {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().filter(|f| f.is_finite()).map(|f| f as i64)),
        _ => None,
    }
}

/// A positive instant or count; Avex writes 0 for "none".
fn positive(o: &Map<String, Value>, k: &str) -> Option<i64> {
    int(o, k).filter(|v| *v > 0)
}

fn float(o: &Map<String, Value>, k: &str) -> Option<f64> {
    o.get(k).and_then(Value::as_f64).filter(|f| f.is_finite())
}

fn flag(o: &Map<String, Value>, k: &str) -> bool {
    o.get(k).and_then(Value::as_bool).unwrap_or(false)
}

/// The most active time one session can claim: a day. An export that says more is corrupt, and
/// unbounded it is summed into weekly totals.
const MAX_ACTIVE_SECONDS: i64 = 24 * 60 * 60;
/// The most sets or PRs one session can claim. A float in the file saturates to `i64::MAX`.
const MAX_COUNT: i64 = 10_000;

fn session(s: &Map<String, Value>) -> Option<Session> {
    let started_at = positive(s, "startedAt")?;
    let mut exercises: Vec<Exercise> = s
        .get("exercises")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_object).enumerate().map(|(i, e)| exercise(e, i as i64)).collect())
        .unwrap_or_default();
    exercises.sort_by_key(|e| e.position);
    Some(Session {
        id: int(s, "id").unwrap_or(0),
        day_key: string(s, "dayKey"),
        started_at,
        finished_at: positive(s, "finishedAt"),
        active_seconds: positive(s, "activeSeconds").unwrap_or(0).min(MAX_ACTIVE_SECONDS),
        volume_lb: float(s, "totalVolumeLb").filter(|v| *v > 0.0).unwrap_or(0.0),
        pr_count: int(s, "prCount").filter(|v| *v > 0).unwrap_or(0).min(MAX_COUNT),
        set_count: int(s, "setCount").filter(|v| *v > 0).unwrap_or(0).min(MAX_COUNT),
        session_type: string(s, "sessionType"),
        intensity: string(s, "intensity"),
        untracked: flag(s, "isUntracked"),
        tags: string(s, "tags"),
        journal: string(s, "journal"),
        mood: string(s, "mood"),
        exercises,
    })
}

fn exercise(e: &Map<String, Value>, index: i64) -> Exercise {
    let id = string(e, "exerciseId");
    let name = Some(string(e, "name")).filter(|n| !n.trim().is_empty()).unwrap_or_else(|| id.clone());
    Exercise {
        name: name.trim().to_string(),
        exercise_id: id,
        position: int(e, "orderIndex").filter(|v| *v >= 0).unwrap_or(index),
        difficulty: string(e, "difficulty"),
        skipped: flag(e, "skipped"),
        note: string(e, "note"),
        was_pr: flag(e, "wasPr"),
        hit_full_target: flag(e, "hitFullTarget"),
        superset: string(e, "supersetGroup"),
        sets: e.get("sets").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_object).map(set).collect()).unwrap_or_default(),
    }
}

fn set(s: &Map<String, Value>) -> Set {
    Set {
        // Avex's own ceiling for a plausible weight (ImportBounds).
        weight_lb: float(s, "weightLb").filter(|w| *w > 0.0 && *w <= 2000.0),
        weight_text: string(s, "weightText"),
        reps: int(s, "reps").unwrap_or(0).clamp(0, 1000),
        rpe: float(s, "rpe").filter(|r| (1.0..=10.0).contains(r)),
        completed_at: positive(s, "completedAt"),
        set_type: string(s, "setType"),
        difficulty_tag: string(s, "difficultyTag"),
        duration_seconds: positive(s, "durationSeconds"),
        assisted: flag(s, "isAssisted"),
        amrap: flag(s, "isAmrap"),
        to_failure: flag(s, "toFailure"),
    }
}

fn cardio_entry(c: &Map<String, Value>) -> Option<Cardio> {
    let date = positive(c, "date")?;
    let kind = string(c, "type");
    if kind.trim().is_empty() {
        return None;
    }
    Some(Cardio {
        date,
        kind,
        duration_min: int(c, "durationMin").unwrap_or(0).clamp(0, 24 * 60),
        distance_km: float(c, "distanceKm").filter(|d| *d > 0.0 && *d < 1000.0),
        effort: string(c, "effort"),
        rest_reason: string(c, "restReason"),
        note: string(c, "note"),
        interval_count: positive(c, "intervalCount"),
        hr_zone: string(c, "hrZone"),
        incline_pct: float(c, "inclinePct").filter(|v| *v > 0.0),
        laps: positive(c, "laps"),
        elevation_m: float(c, "elevationM").filter(|v| *v > 0.0),
        conditions: string(c, "conditions"),
    })
}

fn goal(g: &Map<String, Value>) -> Option<Goal> {
    let kind = string(g, "kind");
    if kind.trim().is_empty() {
        return None;
    }
    Some(Goal {
        kind,
        target_key: string(g, "targetKey"),
        target_value: float(g, "targetValue"),
        priority: int(g, "priority").unwrap_or(0),
        created_at: int(g, "createdAt").unwrap_or(0),
        completed_at: positive(g, "completedAt"),
        archived_at: positive(g, "archivedAt"),
        source: Some(string(g, "source")).filter(|s| !s.is_empty()).unwrap_or_else(|| "user".into()),
        note: string(g, "note"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) const SAMPLE: &str = r#"{
      "exportVersion": 1, "exportedAt": "2026-10-08 19:02", "appVersion": "1.4.2",
      "settings": {"useKg": true, "weightUnit": "kg", "userGoal": "strength", "userName": "Antho", "daysPerWeek": 4, "firstDayMonday": true},
      "sessions": [
        {"id": 7, "dayKey": "push_a", "startedAt": 1759870800000, "finishedAt": 1759874400000, "activeSeconds": 3300,
         "totalVolumeLb": 9000.0, "prCount": 1, "setCount": 4, "sessionType": "normal", "intensity": "", "isUntracked": false,
         "tags": "", "journal": "Felt strong", "mood": "good", "segments": [],
         "exercises": [
           {"exerciseId": "bench", "name": "Bench Press", "swappedName": "", "orderIndex": 0, "difficulty": "MEDIUM", "skipped": false,
            "note": "", "wasPr": true, "hitFullTarget": true,
            "sets": [
              {"weightText": "95", "weightLb": 95.0, "reps": 10, "completedAt": 1759871000000, "difficultyTag": "", "durationSeconds": 0,
               "isAssisted": false, "isAmrap": false, "toFailure": false, "setType": "warmup", "dropAnnotation": ""},
              {"weightText": "225", "weightLb": 225.0, "reps": 5, "rpe": 8.5, "completedAt": 1759871300000, "setType": ""},
              {"weightText": "225", "weightLb": "NaN", "reps": 5, "setType": ""}
            ]},
           {"exerciseId": "dips", "name": "Dips", "orderIndex": 1, "skipped": false, "sets": [{"reps": 12}]}
         ]},
        {"id": 3, "startedAt": 1759266000000, "finishedAt": 0, "exercises": []},
        {"id": 9, "exercises": []}
      ],
      "cardio": [{"date": 1759950000000, "type": "run", "durationMin": 30, "distanceKm": 5.2, "effort": "easy"}, {"date": 0, "type": "run"}],
      "coachGoals": [{"kind": "lift_target", "targetKey": "bench", "targetValue": 245.0, "priority": 1, "createdAt": 1759000000000, "source": "", "note": ""}]
    }"#;

    #[test]
    fn reads_the_full_export() {
        let h = read(SAMPLE, "avex_export.json", "2026-10-09T12:00:00-04:00").unwrap();
        assert!(h.meta.use_kg);
        assert_eq!(h.meta.user_name, "Antho");
        assert_eq!(h.meta.exported_at, "2026-10-08 19:02");
        // The session without a start is left out; the rest are oldest first.
        assert_eq!(h.sessions.iter().map(|s| s.id).collect::<Vec<_>>(), [3, 7]);
        let s = &h.sessions[1];
        assert_eq!(s.finished_at, Some(1759874400000));
        assert_eq!(h.sessions[0].finished_at, None);
        let bench = &s.exercises[0];
        assert!(bench.was_pr && bench.sets[0].warmup());
        assert_eq!(bench.sets[1].rpe, Some(8.5));
        // A non-finite weight is dropped, the set kept.
        assert_eq!(bench.sets[2].weight_lb, None);
        assert_eq!(bench.sets[2].reps, 5);
        assert_eq!(s.exercises[1].sets[0].weight_lb, None);
        assert_eq!(h.cardio.len(), 1);
        assert_eq!(h.cardio[0].distance_km, Some(5.2));
        assert_eq!(h.goals[0].source, "user");
        assert_eq!(h.goals[0].target_value, Some(245.0));
    }

    #[test]
    fn refuses_what_would_erase_history() {
        let weekly = r#"{"exportedAt": "x", "periodStart": "2026-10-05", "periodDays": 7, "sessions": [], "cardio": []}"#;
        assert_eq!(read(weekly, "w.json", "t"), Err(ReadError::Partial("weekly")));
        let one = r#"{"exportVersion": 1, "session": {"startedAt": 1, "exercises": []}}"#;
        assert_eq!(read(one, "s.json", "t"), Err(ReadError::Partial("single workout")));
        assert!(matches!(read("[1]", "x", "t"), Err(ReadError::NotAvex(_))));
        assert!(matches!(read("{\"accounts\": []}", "x", "t"), Err(ReadError::NotAvex(_))));
        let future = r#"{"exportVersion": 2, "settings": {}, "sessions": []}"#;
        assert_eq!(read(future, "x", "t"), Err(ReadError::TooNew(2)));
        assert!(ReadError::Partial("weekly").to_string().contains("Training history"));
    }

    /// A count or duration written as a huge float saturates to `i64::MAX` on the way in; it
    /// is clamped instead, so the weekly totals it is summed into stay finite.
    #[test]
    fn implausible_counts_and_durations_are_clamped() {
        let raw = r#"{"exportVersion": 1, "settings": {}, "sessions": [
          {"id": 1, "startedAt": 1759870800000, "activeSeconds": 1e300, "prCount": 9.9e18, "setCount": 1e19, "exercises": []}
        ], "cardio": []}"#;
        let h = read(raw, "x.json", "t").unwrap();
        let s = &h.sessions[0];
        assert_eq!((s.active_seconds, s.pr_count, s.set_count), (MAX_ACTIVE_SECONDS, MAX_COUNT, MAX_COUNT));
    }
}
