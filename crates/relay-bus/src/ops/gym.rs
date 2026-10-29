//! `gym.*` — Avex, the person's gym app, in the Threads space (docs/GYM.md).
//!
//! The PC's copy is Avex's training history export, kept in its own SQLite file (`gym.db`) and
//! read only: an import replaces it whole. Its read shapes are `relay_gym`'s. Weights and volumes
//! are in the person's unit, the `unit` each reading names. Importing and forgetting emit
//! `gym.changed`, and are the person's alone; a thread's agent reads.
use crate::registry::{Actors, OpMeta, Scope};
use crate::{op, Empty};
use relay_gym::views::{CardioRow, LiftDetail, LiftRow, Series, SeriesQuery, SessionDetail, SessionRow, Summary};

payload!(#[schemars(rename = "GymSummaryIn")] SummaryIn {
    /// Read as if this day (`YYYY-MM-DD`) were today: the weeks, the streak and each lift's four
    /// weeks count back from it. Give the export's day to see the copy's last week. Today on the
    /// PC when absent.
    pub today: Option<String>,
});
op!(SummaryOp, "gym.summary", SummaryIn => Summary,
    OpMeta::query(Scope::Global, 12, "The Avex overview: when the history was exported, totals, this week and the twelve before, the streak, recent workouts, records, the lifts done most and open goals with each lift goal's best"));

payload!(#[schemars(rename = "GymSessionsIn")] SessionsIn {
    /// Skip this many, newest first.
    pub offset: Option<u32>,
    /// At most this many; 50 by default, 500 at most.
    pub limit: Option<u32>,
    /// Only workouts with this lift (a unique part of its name is enough).
    pub lift: Option<String>,
    /// Only workouts from this day (`YYYY-MM-DD`); with no `to`, up to today.
    pub from: Option<String>,
    /// Only workouts up to this day (`YYYY-MM-DD`), taken in.
    pub to: Option<String>,
});
result!(#[schemars(rename = "GymSessionsOut")] SessionsOut {
    pub sessions: Vec<SessionRow>,
    /// How many match in all.
    pub total: u32,
    /// How many of `total` the person excluded in Avex (untracked).
    pub untracked: u32,
});
op!(Sessions, "gym.sessions", SessionsIn => SessionsOut, OpMeta::query(Scope::Global, 12, "Workouts, newest first, each with its lifts, sets, volume and records"));

payload!(#[schemars(rename = "GymIdIn")] IdIn {
    /// The session's id, as the lists give it.
    pub id: i64,
});
op!(SessionGet, "gym.session.get", IdIn => SessionDetail, OpMeta::query(Scope::Global, 12, "One workout: every exercise and set, with each set's estimated one-rep max and each lift's last time before, the journal, and the workouts either side"));

payload!(#[schemars(rename = "GymLiftsIn")] LiftsIn {
    /// Only lifts whose name contains this.
    pub query: Option<String>,
    /// Count each lift's four weeks back from this day (`YYYY-MM-DD`), as gym.summary's `today`.
    pub today: Option<String>,
});
result!(#[schemars(rename = "GymLiftsOut")] LiftsOut { pub unit: String, pub lifts: Vec<LiftRow> });
op!(Lifts, "gym.lifts", LiftsIn => LiftsOut, OpMeta::query(Scope::Global, 12, "Every lift, done most first: how often, its best set, and its last four weeks beside the four before"));

payload!(#[schemars(rename = "GymLiftIn")] LiftIn {
    /// The lift's name as Avex shows it; a unique part of it is enough.
    pub name: String,
    /// At most this many sessions, newest first; 100 by default.
    pub limit: Option<u32>,
    /// Count the lift's four weeks back from this day (`YYYY-MM-DD`), as gym.summary's `today`.
    pub today: Option<String>,
});
op!(LiftGet, "gym.lift.get", LiftIn => LiftDetail, OpMeta::query(Scope::Global, 12, "One lift's history: each session's heaviest set, estimated one-rep max, sets and volume, records marked"));

payload!(#[schemars(rename = "GymCardioIn")] CardioIn {
    /// At most this many, newest first; 50 by default.
    pub limit: Option<u32>,
});
result!(#[schemars(rename = "GymCardioOut")] CardioOut { pub entries: Vec<CardioRow> });
op!(Cardio, "gym.cardio", CardioIn => CardioOut, OpMeta::query(Scope::Global, 12, "Cardio and rest-day entries, newest first"));

op!(SeriesOp, "gym.series", SeriesQuery => Series,
    OpMeta::query(Scope::Global, 12, "A chart's numbers: volume, sessions, sets, minutes, a lift's estimated one-rep max or top weight, cardio minutes or distance, by week, month or session"));

payload!(#[schemars(rename = "GymPathIn")] PathIn {
    /// An absolute path on the PC.
    pub path: String,
});
result!(#[schemars(rename = "GymImportOut")] ImportOut {
    pub sessions: u32,
    pub sets: u32,
    pub cardio: u32,
    pub goals: u32,
    /// As Avex wrote it: the phone's local date and time.
    pub exported_at: String,
});
op!(Import, "gym.import", PathIn => ImportOut,
    OpMeta::mutation(Scope::Global, 12, "Read Avex's training history export (avex_export.json: in Avex, Settings, Export, Training history). It replaces the PC's whole copy").actors(Actors::UserOnly).emits(&["gym.changed"]));
payload!(#[schemars(rename = "GymUploadIn")] UploadIn {
    /// The whole `avex_export.json`, as Avex wrote it.
    pub json: String,
    /// The file's name, recorded with the copy; `avex_export.json` when absent.
    pub name: Option<String>,
});
op!(Upload, "gym.upload", UploadIn => ImportOut,
    OpMeta::mutation(Scope::Global, 12, "Take Avex's training history export as text, sent by Avex itself or by the PC's cloud link (docs/CLOUD.md). It replaces the whole copy, as gym.import does").actors(Actors::UserOnly).emits(&["gym.changed"]));
payload!(#[schemars(rename = "GymRawIn")] RawIn {
    /// The digest the caller already holds: when it is still the copy's, `json` is left out.
    pub have: Option<String>,
});
result!(#[schemars(rename = "GymRawOut")] RawOut {
    /// sha256 of the export text, lowercase hex; absent when nothing was uploaded.
    pub digest: Option<String>,
    /// The export text, unless the caller already holds it or nothing was uploaded.
    pub json: Option<String>,
});
op!(Raw, "gym.raw", RawIn => RawOut,
    OpMeta::query(Scope::Global, 12, "The last export uploaded with gym.upload, as text, for the PC's cloud link to import (docs/CLOUD.md)").actors(Actors::UserOnly));
op!(Reset, "gym.reset", Empty => Empty,
    OpMeta::mutation(Scope::Global, 12, "Forget the imported Avex history on this PC; Avex itself is untouched").actors(Actors::UserOnly).emits(&["gym.changed"]));

entries!(SummaryOp, Sessions, SessionGet, Lifts, LiftGet, Cardio, SeriesOp, Import, Upload, Raw, Reset);
