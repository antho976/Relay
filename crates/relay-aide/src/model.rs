//! What Aide keeps: jobs, the runs of each, what it has learned, its errands, what it has to say,
//! its living pages, and its settings. These are the bus's shapes as well as the store's.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Implements `as_str` and `parse` over a unit enum's snake_case names, the stored form.
macro_rules! named {
    ($ty:ident { $($v:ident => $s:literal),* $(,)? }) => {
        impl $ty {
            pub const ALL: &'static [$ty] = &[$($ty::$v),*];
            pub fn as_str(self) -> &'static str {
                match self { $($ty::$v => $s),* }
            }
            pub fn parse(s: &str) -> Option<$ty> {
                match s { $($s => Some($ty::$v),)* _ => None }
            }
        }
    };
}

/// Where Aide reads from. Each has a privacy tier ([`Tier`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Relay's dev side: sessions, tasks, Git, notifications.
    Dev,
    /// Tally's ledger and investments.
    Money,
    /// Arbiter's strategies, orders and decisions.
    Arbiter,
    /// Avex's training history.
    Gym,
}
named!(Source { Dev => "dev", Money => "money", Arbiter => "arbiter", Gym => "gym" });

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Dev => "Relay",
            Source::Money => "Tally",
            Source::Arbiter => "Arbiter",
            Source::Gym => "Avex",
        }
    }
}

/// Which models may read a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Any model: Claude through the person's account, or one of theirs.
    #[default]
    Any,
    /// Only a model the person runs themselves: on this PC or on their own server. A job that
    /// reads it on Claude does not run.
    LocalOnly,
    /// No model reads it.
    Off,
}
named!(Tier { Any => "any", LocalOnly => "local_only", Off => "off" });

/// How loudly a message is said.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// A notification now, on the phone too. Held through quiet hours and past the day's cap.
    Interrupt,
    /// Kept for the next brief.
    #[default]
    Brief,
    /// Only on Aide's page.
    Quiet,
}
named!(Level { Interrupt => "interrupt", Brief => "brief", Quiet => "quiet" });

/// How often a schedule comes round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Every {
    Day,
    /// Monday to Friday.
    Weekdays,
    Week,
    Month,
}
named!(Every { Day => "day", Weekdays => "weekdays", Week => "week", Month => "month" });

/// When a job wakes by the clock, in the PC's own time zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    pub every: Every,
    /// 0 to 23.
    pub hour: u8,
    /// 0 to 59; 0 when absent.
    #[serde(default)]
    pub minute: u8,
    /// For `week`: 1 (Monday) to 7 (Sunday); Monday when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weekday: Option<u8>,
    /// For `month`: 1 to 28; the 1st when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub day: Option<u8>,
}

/// A data rule: a condition on the person's data, checked by Relay without a model when the data
/// changes. A true rule wakes its job, once per thing it found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Rule {
    /// A Tally transaction whose amount is at least `over`, in the ledger's currency.
    Spend { over: f64 },
    /// A budget category past `percent` of its month.
    Budget { percent: u32 },
    /// An agent session waiting on the person for `minutes`.
    Stuck { minutes: u32 },
    /// A Relay notification in `category` (agent_done, agent_blocked, guardrail, integration,
    /// provider, disk, system).
    Notice { category: String },
    /// An Arbiter strategy halted, or its exchange unreachable.
    Arbiter {},
    /// New training history from Avex.
    Workout {},
}

impl Rule {
    /// The data a rule reads, which its job must be allowed.
    pub fn source(&self) -> Source {
        match self {
            Rule::Spend { .. } | Rule::Budget { .. } => Source::Money,
            Rule::Stuck { .. } | Rule::Notice { .. } => Source::Dev,
            Rule::Arbiter {} => Source::Arbiter,
            Rule::Workout {} => Source::Gym,
        }
    }

    /// The engine events after which the rule is checked again.
    pub fn events(&self) -> &'static [&'static str] {
        match self {
            Rule::Spend { .. } | Rule::Budget { .. } => &["money.changed"],
            Rule::Stuck { .. } => &["session.changed"],
            Rule::Notice { .. } => &["notify.new"],
            Rule::Arbiter {} => &["arbiter.changed"],
            Rule::Workout {} => &["gym.changed"],
        }
    }

    /// One line for a person.
    pub fn describe(&self) -> String {
        match self {
            Rule::Spend { over } => format!("a Tally transaction of {over} or more"),
            Rule::Budget { percent } => format!("a budget past {percent}% of its month"),
            Rule::Stuck { minutes } => format!("an agent waiting on you for {minutes} minutes"),
            Rule::Notice { category } => format!("a Relay notification ({category})"),
            Rule::Arbiter {} => "an Arbiter strategy halted or its exchange unreachable".into(),
            Rule::Workout {} => "a new workout from Avex".into(),
        }
    }
}

/// Whether a job runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    /// Drafted by Aide: waits for the person to turn it on.
    Proposed,
    Active,
    Paused,
}
named!(JobState { Proposed => "proposed", Active => "active", Paused => "paused" });

/// A job as the person (or Aide, as a proposal) writes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JobSpec {
    /// Its name, as the brief and the notifications show it.
    pub name: String,
    /// What Aide does when it wakes, in plain words.
    pub instructions: String,
    /// When it wakes by the clock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<Schedule>,
    /// What wakes it in the data.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<Rule>,
    /// What it may read. A rule's source is added.
    #[serde(default)]
    pub sources: Vec<Source>,
    /// The model it runs on: a Claude model id (`claude-…`, or `opus`, `sonnet`, `haiku`), a
    /// model on this PC, or one on the person's own server (`aide.models`). Aide's default when
    /// absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Tried when `model` cannot run: not loaded, unreachable, or not allowed to read a source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
    /// low, medium, high, xhigh or max, for a Claude model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// How loudly it speaks by default.
    #[serde(default)]
    pub level: Level,
    /// At most this many runs a day; 24 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runs_per_day: Option<u32>,
    /// A run is stopped and called late after this; 10 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minutes_per_run: Option<u32>,
}

pub const DEFAULT_RUNS_PER_DAY: u32 = 24;
pub const DEFAULT_MINUTES_PER_RUN: u32 = 10;
pub const MAX_MINUTES_PER_RUN: u32 = 120;
pub const MAX_RUNS_PER_DAY: u32 = 288;
pub const MAX_NAME: usize = 80;
pub const MAX_INSTRUCTIONS: usize = 8_000;

impl JobSpec {
    pub fn runs_per_day(&self) -> u32 {
        self.runs_per_day.unwrap_or(DEFAULT_RUNS_PER_DAY)
    }
    pub fn minutes_per_run(&self) -> u32 {
        self.minutes_per_run.unwrap_or(DEFAULT_MINUTES_PER_RUN)
    }

    /// Every source the job reads: its own and its rules'.
    pub fn all_sources(&self) -> Vec<Source> {
        let mut all: Vec<Source> = self.sources.iter().copied().chain(self.rules.iter().map(Rule::source)).collect();
        all.sort();
        all.dedup();
        all
    }

    /// The spec, checked: trimmed, in range, with something that wakes it or it is run by hand.
    pub fn checked(mut self) -> Result<JobSpec, String> {
        self.name = self.name.trim().to_string();
        self.instructions = self.instructions.trim().to_string();
        if self.name.is_empty() || self.name.chars().count() > MAX_NAME || self.name.contains(char::is_control) {
            return Err(format!("A job's name is 1 to {MAX_NAME} printable characters"));
        }
        if self.instructions.is_empty() || self.instructions.chars().count() > MAX_INSTRUCTIONS {
            return Err(format!("Say what the job does, in at most {MAX_INSTRUCTIONS} characters"));
        }
        if let Some(s) = &self.schedule {
            crate::schedule::check(s)?;
        }
        for rule in &self.rules {
            match rule {
                Rule::Spend { over } if !over.is_finite() || *over <= 0.0 => return Err("A spend rule's amount is more than 0".into()),
                Rule::Budget { percent } if !(1..=500).contains(percent) => return Err("A budget rule's percent is 1 to 500".into()),
                Rule::Stuck { minutes } if !(1..=10_080).contains(minutes) => return Err("A stuck rule waits 1 minute to a week".into()),
                Rule::Notice { category } if category.trim().is_empty() => return Err("A notice rule names a category".into()),
                // Aide's own notifications would wake it again for what it just said.
                Rule::Notice { category } if category.trim() == "aide" => return Err("A notice rule cannot listen for Aide's own notifications".into()),
                _ => {}
            }
        }
        if !(1..=MAX_RUNS_PER_DAY).contains(&self.runs_per_day()) {
            return Err(format!("A job runs 1 to {MAX_RUNS_PER_DAY} times a day"));
        }
        if !(1..=MAX_MINUTES_PER_RUN).contains(&self.minutes_per_run()) {
            return Err(format!("A run takes 1 to {MAX_MINUTES_PER_RUN} minutes"));
        }
        if let Some(e) = &self.effort {
            if !["low", "medium", "high", "xhigh", "max"].contains(&e.as_str()) {
                return Err(format!("Not an effort: {e} (low, medium, high, xhigh, max)"));
            }
        }
        for m in [&mut self.model, &mut self.fallback] {
            if let Some(id) = m {
                let t = id.trim().to_string();
                if t.is_empty() {
                    *m = None;
                } else if !t.starts_with(|c: char| c.is_ascii_alphanumeric()) || !t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') || t.len() > 100 {
                    return Err(format!("Not a model id: {t:?}"));
                } else {
                    *id = t;
                }
            }
        }
        self.sources.sort();
        self.sources.dedup();
        Ok(self)
    }
}

/// A job as kept.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Job {
    pub id: i64,
    #[serde(flatten)]
    pub spec: JobSpec,
    pub state: JobState,
    /// Its channel: the Aide thread its runs talk in.
    pub thread_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub last_run_at: Option<String>,
    /// When its schedule next wakes it, while it is active.
    pub next_due_at: Option<String>,
}

/// Why a run started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    Schedule,
    Rule,
    FollowUp,
    /// The person pressed Run.
    Manual,
    /// A schedule that came round while the PC was off, inside the grace window.
    CatchUp,
}
named!(Reason { Schedule => "schedule", Rule => "rule", FollowUp => "follow_up", Manual => "manual", CatchUp => "catch_up" });

/// How a run is going, or how it ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    /// Waiting for a seat (a person's conversation has it) or for its model to load.
    Queued,
    Running,
    Done,
    /// It looked and found nothing worth saying.
    Nothing,
    /// It needs the person: an approval, an answer.
    Waiting,
    /// Past its minutes: stopped.
    Late,
    Failed,
    /// It did not start: paused, over its day's runs, or no model allowed to read its sources.
    Skipped,
}
named!(RunState {
    Queued => "queued", Running => "running", Done => "done", Nothing => "nothing", Waiting => "waiting",
    Late => "late", Failed => "failed", Skipped => "skipped",
});

impl RunState {
    pub fn is_open(self) -> bool {
        matches!(self, RunState::Queued | RunState::Running)
    }
}

/// Whether a source was read for a run, and what was wrong when it was not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SourceRead {
    pub source: Source,
    pub ok: bool,
    /// What it held, in a few words, or why it could not be read.
    pub detail: String,
}

/// One wake of one job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Run {
    pub id: i64,
    pub job_id: i64,
    pub job_name: String,
    pub reason: Reason,
    /// What woke it: the rule's finding, the follow-up's note, the schedule.
    pub trigger: String,
    pub state: RunState,
    /// The model it ran on, once chosen.
    pub model: Option<String>,
    /// claude, pc (this PC) or server (the person's own server).
    pub host: Option<String>,
    /// How it ended, in its own words or Relay's.
    pub note: Option<String>,
    pub thread_id: Option<i64>,
    pub sources: Vec<SourceRead>,
    pub started_at: String,
    pub deadline_at: Option<String>,
    pub ended_at: Option<String>,
}

/// Something Aide has learned and keeps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Fact {
    pub id: i64,
    pub text: String,
    /// Where it came from: `person`, a source's name (`money`, `gym`, …), or `thread:<id>`.
    pub source: String,
    pub job_id: Option<i64>,
    /// The person has confirmed it; Aide's own guesses are not handed to a run until then.
    pub confirmed: bool,
    /// Always handed to a run.
    pub pinned: bool,
    pub learned_at: String,
    pub used_at: Option<String>,
}

/// Where an errand stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrandState {
    /// Aide is on it.
    Open,
    /// It waits on the person.
    Waiting,
    Done,
    Dropped,
}
named!(ErrandState { Open => "open", Waiting => "waiting", Done => "done", Dropped => "dropped" });

/// Something Aide is doing, or waits on the person for. Only the person closes one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Errand {
    pub id: i64,
    pub title: String,
    pub detail: String,
    pub job_id: Option<i64>,
    pub state: ErrandState,
    /// When Aide said it would be done; past it, Aide says it is late.
    pub due_at: Option<String>,
    /// Aide thinks it is done and asks the person to close it.
    pub proposed_done: bool,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
}

/// Something Aide said or has to say.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Message {
    pub id: i64,
    pub job_id: Option<i64>,
    pub run_id: Option<i64>,
    pub level: Level,
    pub title: String,
    pub body: String,
    pub created_at: String,
    /// Shown as a notification (an interrupt that was not held).
    pub notified: bool,
    /// Held for the next brief: a brief message, or an interrupt in quiet hours or past the cap.
    pub held: bool,
    /// The brief that carried it.
    pub briefed_at: Option<String>,
}

/// A one-shot wake a run promised: "look again at five".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FollowUp {
    pub id: i64,
    pub job_id: i64,
    pub due_at: String,
    pub note: String,
}

/// A page Aide keeps current: it stores its question, and each run of its job writes it again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Page {
    pub id: i64,
    pub title: String,
    /// What the page answers; the job's run rewrites `body` from current data.
    pub question: String,
    /// Markdown.
    pub body: String,
    pub job_id: Option<i64>,
    pub updated_at: String,
}

/// The tier of each source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct Tiers {
    #[serde(default)]
    pub dev: Tier,
    #[serde(default)]
    pub money: Tier,
    #[serde(default)]
    pub arbiter: Tier,
    #[serde(default)]
    pub gym: Tier,
}

impl Tiers {
    pub fn of(&self, source: Source) -> Tier {
        match source {
            Source::Dev => self.dev,
            Source::Money => self.money,
            Source::Arbiter => self.arbiter,
            Source::Gym => self.gym,
        }
    }
}

/// A model on the person's own server: an address that answers Anthropic's Messages API (a
/// llama.cpp server, vLLM or LiteLLM in front of one), so Aide's agent talks to it as it talks to
/// Claude.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Endpoint {
    /// The id a job names: letters, digits, `-` and `_`.
    pub id: String,
    /// As shown.
    pub name: String,
    /// `http://host:port` (or `https://`), without a path.
    pub url: String,
    /// The model name the server is asked for; `id` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Its context window in tokens; 32768 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<u32>,
    /// It reads pictures.
    #[serde(default)]
    pub vision: bool,
}

/// Aide's settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// Nothing runs and nothing is said while true.
    #[serde(default)]
    pub paused: bool,
    /// The model a job runs on when it names none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Tried when that one cannot run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
    /// Quiet hours, start and end (0 to 23, the PC's time): interrupts are held for the brief. The
    /// same hour twice means none.
    #[serde(default = "quiet_start")]
    pub quiet_start: u8,
    #[serde(default = "quiet_end")]
    pub quiet_end: u8,
    /// At most this many interrupts a day; past it they wait for the brief.
    #[serde(default = "interrupt_cap")]
    pub interrupt_cap: u32,
    #[serde(default)]
    pub tiers: Tiers,
    /// The person's own servers' models.
    #[serde(default)]
    pub endpoints: Vec<Endpoint>,
}

fn quiet_start() -> u8 {
    22
}
fn quiet_end() -> u8 {
    7
}
fn interrupt_cap() -> u32 {
    6
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            paused: false,
            model: None,
            fallback: None,
            quiet_start: quiet_start(),
            quiet_end: quiet_end(),
            interrupt_cap: interrupt_cap(),
            tiers: Tiers::default(),
            endpoints: Vec::new(),
        }
    }
}

impl Settings {
    /// Whether `hour` (0 to 23) is in quiet hours.
    pub fn quiet_at(&self, hour: u8) -> bool {
        let (s, e) = (self.quiet_start % 24, self.quiet_end % 24);
        if s == e {
            false
        } else if s < e {
            (s..e).contains(&hour)
        } else {
            hour >= s || hour < e
        }
    }

    pub fn endpoint(&self, id: &str) -> Option<&Endpoint> {
        self.endpoints.iter().find(|e| e.id == id)
    }

    /// The settings, checked.
    pub fn checked(mut self) -> Result<Settings, String> {
        if self.quiet_start > 23 || self.quiet_end > 23 {
            return Err("Quiet hours are 0 to 23".into());
        }
        if self.interrupt_cap > 100 {
            return Err("At most 100 interrupts a day".into());
        }
        let mut seen = std::collections::HashSet::new();
        for e in &mut self.endpoints {
            e.id = e.id.trim().to_string();
            e.url = e.url.trim().trim_end_matches('/').to_string();
            if e.id.is_empty() || e.id.len() > 60 || !e.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                return Err(format!("An endpoint's id is letters, digits, - and _: {:?}", e.id));
            }
            if e.id.starts_with("claude") || ["opus", "sonnet", "haiku"].contains(&e.id.as_str()) {
                return Err(format!("{} is Claude's name; give the endpoint another", e.id));
            }
            if !seen.insert(e.id.clone()) {
                return Err(format!("Two endpoints are called {}", e.id));
            }
            let rest = e.url.strip_prefix("http://").or_else(|| e.url.strip_prefix("https://"));
            if !rest.is_some_and(|r| !r.is_empty() && !r.contains('/') && !r.contains(char::is_whitespace)) {
                return Err(format!("An endpoint's address is http://host:port, without a path: {:?}", e.url));
            }
            if e.name.trim().is_empty() {
                e.name = e.id.clone();
            }
            if e.context.is_some_and(|c| !(2048..=2_000_000).contains(&c)) {
                return Err("An endpoint's context is 2048 to 2000000 tokens".into());
            }
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> JobSpec {
        JobSpec {
            name: " Morning brief ".into(),
            instructions: "Tell me what happened overnight".into(),
            schedule: Some(Schedule { every: Every::Weekdays, hour: 7, minute: 0, weekday: None, day: None }),
            rules: vec![Rule::Budget { percent: 90 }],
            sources: vec![Source::Gym, Source::Dev, Source::Gym],
            model: Some(" sonnet ".into()),
            fallback: None,
            effort: None,
            level: Level::Brief,
            runs_per_day: None,
            minutes_per_run: None,
        }
    }

    #[test]
    fn a_spec_is_trimmed_and_its_sources_include_its_rules() {
        let s = spec().checked().unwrap();
        assert_eq!(s.name, "Morning brief");
        assert_eq!(s.model.as_deref(), Some("sonnet"));
        assert_eq!(s.sources, vec![Source::Dev, Source::Gym]);
        assert_eq!(s.all_sources(), vec![Source::Dev, Source::Money, Source::Gym]);
    }

    #[test]
    fn a_bad_spec_says_why() {
        assert!(JobSpec { name: "".into(), ..spec() }.checked().is_err());
        assert!(JobSpec { minutes_per_run: Some(0), ..spec() }.checked().is_err());
        assert!(JobSpec { rules: vec![Rule::Spend { over: -1.0 }], ..spec() }.checked().is_err());
        assert!(JobSpec { model: Some("rm -rf".into()), ..spec() }.checked().is_err());
        assert!(JobSpec { effort: Some("huge".into()), ..spec() }.checked().is_err());
    }

    #[test]
    fn quiet_hours_wrap_midnight() {
        let s = Settings::default();
        assert!(s.quiet_at(23) && s.quiet_at(0) && s.quiet_at(6));
        assert!(!s.quiet_at(7) && !s.quiet_at(21));
        let none = Settings { quiet_start: 5, quiet_end: 5, ..Settings::default() };
        assert!(!none.quiet_at(5));
        let day = Settings { quiet_start: 9, quiet_end: 17, ..Settings::default() };
        assert!(day.quiet_at(9) && !day.quiet_at(17));
    }

    #[test]
    fn endpoints_are_checked() {
        let ok = Endpoint { id: "farm-qwen".into(), name: "".into(), url: "http://10.0.0.5:8080/".into(), model: None, context: None, vision: false };
        let s = Settings { endpoints: vec![ok.clone()], ..Settings::default() }.checked().unwrap();
        assert_eq!(s.endpoints[0].url, "http://10.0.0.5:8080");
        assert_eq!(s.endpoints[0].name, "farm-qwen");
        for bad in [
            Endpoint { url: "10.0.0.5:8080".into(), ..ok.clone() },
            Endpoint { url: "http://h/v1".into(), ..ok.clone() },
            Endpoint { id: "sonnet".into(), ..ok.clone() },
            Endpoint { id: "a b".into(), ..ok.clone() },
        ] {
            assert!(Settings { endpoints: vec![bad], ..Settings::default() }.checked().is_err());
        }
        assert!(Settings { endpoints: vec![ok.clone(), ok], ..Settings::default() }.checked().is_err());
    }

    #[test]
    fn a_rule_reads_as_tagged_json() {
        let r: Rule = serde_json::from_str(r#"{"kind":"spend","over":200}"#).unwrap();
        assert_eq!(r, Rule::Spend { over: 200.0 });
        let r: Rule = serde_json::from_str(r#"{"kind":"workout"}"#).unwrap();
        assert_eq!(r, Rule::Workout {});
    }
}
