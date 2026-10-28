//! What a run may do and on which model: the privacy tier of each source decides which models may
//! read it, and the run is given its sources' reads, Aide's own ops and nothing else.
//!
//! Aide's approval policy has four rungs (docs/AIDE.md, "Approvals by kind"):
//! - **free**: reads, its own notes to itself (facts it has to have confirmed, errands, pages,
//!   follow-ups) and messages to the person;
//! - **within limits**: interrupts, at most [`crate::model::Settings::interrupt_cap`] a day and
//!   none in quiet hours, which the engine holds for the brief past either;
//! - **asks**: a new job (`aide.job.propose`), which waits for the person to turn it on;
//! - **refused**: everything not in [`ops`]. [`REFUSED`] names the kinds a person might expect
//!   it to do, so a screen can say so.

use crate::model::{Settings, Source, Tier};

/// Where a model runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    /// Anthropic's, through the person's own Claude account (the CLI's sign-in).
    Claude,
    /// A model on this PC that Relay starts (`local_models`).
    Pc,
    /// A model on the person's own server ([`crate::model::Endpoint`]).
    Server,
}

impl Host {
    pub fn as_str(self) -> &'static str {
        match self {
            Host::Claude => "claude",
            Host::Pc => "pc",
            Host::Server => "server",
        }
    }
    /// Run by the person: on this PC or their own server.
    pub fn is_own(self) -> bool {
        !matches!(self, Host::Claude)
    }
}

/// Where `model` runs: an endpoint id is the person's server, a model id Relay runs here
/// (`pc_models`) is this PC, and anything else is a Claude model. `None` is Claude's default.
pub fn host_of(model: Option<&str>, settings: &Settings, pc_models: &[&str]) -> Host {
    match model {
        Some(m) if settings.endpoint(m).is_some() => Host::Server,
        Some(m) if pc_models.contains(&m) => Host::Pc,
        _ => Host::Claude,
    }
}

/// The model a run gets, and what it may read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub model: Option<String>,
    pub host: Host,
    /// The sources it reads.
    pub sources: Vec<Source>,
    /// Sources turned off for every model, which it does not read.
    pub off: Vec<Source>,
}

/// Why a run could not start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal(pub String);

/// The first of the job's model, its fallback, Aide's model and Aide's fallback that may read every
/// source the job reads. A source with [`Tier::LocalOnly`] rules out Claude; one with [`Tier::Off`]
/// is left out of the run, whichever model it is.
pub fn choose(
    job_model: Option<&str>,
    job_fallback: Option<&str>,
    sources: &[Source],
    settings: &Settings,
    pc_models: &[&str],
    available: impl Fn(Option<&str>, Host) -> bool,
) -> Result<Choice, Refusal> {
    let off: Vec<Source> = sources.iter().copied().filter(|s| settings.tiers.of(*s) == Tier::Off).collect();
    let reads: Vec<Source> = sources.iter().copied().filter(|s| !off.contains(s)).collect();
    let private: Vec<Source> = reads.iter().copied().filter(|s| settings.tiers.of(*s) == Tier::LocalOnly).collect();
    let mut candidates: Vec<Option<&str>> = Vec::new();
    for m in [job_model, job_fallback, settings.model.as_deref(), settings.fallback.as_deref()] {
        if m.is_some() && !candidates.contains(&m) {
            candidates.push(m);
        }
    }
    // Nothing named anywhere is Claude's default model.
    if candidates.is_empty() {
        candidates.push(None);
    }
    let mut why = Vec::new();
    for m in candidates {
        let host = host_of(m, settings, pc_models);
        let name = m.unwrap_or("Claude's default model");
        if !host.is_own() && !private.is_empty() {
            let names: Vec<&str> = private.iter().map(|s| s.label()).collect();
            why.push(format!("{name} is Claude's, and {} is for your own models only", names.join(" and ")));
            continue;
        }
        if !available(m, host) {
            why.push(format!("{name} is not available"));
            continue;
        }
        return Ok(Choice { model: m.map(str::to_string), host, sources: reads, off });
    }
    Err(Refusal(why.join("; ")))
}

/// The reads of each source a run is given.
pub fn reads(source: Source) -> &'static [&'static str] {
    match source {
        Source::Dev => &[
            "project.list", "session.list", "session.get", "task.list", "task.get", "git.pr.list", "notify.list",
            "guardrail.holds.list", "usage.get",
        ],
        Source::Money => &["money.summary", "money.lists", "money.tx.list", "money.series", "money.invest.summary", "money.invest.list"],
        Source::Arbiter => &["arbiter.summary", "arbiter.strategy.get", "arbiter.order.list", "arbiter.decision.list", "arbiter.series"],
        Source::Gym => &["gym.summary", "gym.sessions", "gym.session.get", "gym.lifts", "gym.lift.get", "gym.cardio", "gym.series"],
    }
}

/// Aide's own ops, every run's: what it says, keeps and promises.
pub const OWN: &[&str] = &[
    "aide.notify", "aide.run.finish", "aide.followup", "aide.fact.list", "aide.fact.add", "aide.errand.list", "aide.errand.add",
    "aide.errand.update", "aide.page.list", "aide.page.write", "aide.job.list", "aide.job.propose",
];

/// Every op any Aide run may be given: the socket door admits an Aide agent to these alone, and
/// each run's MCP server lists only [`ops`] of them.
pub fn all_ops() -> Vec<&'static str> {
    let mut all: Vec<&'static str> = OWN.to_vec();
    for s in Source::ALL {
        all.extend(reads(*s));
    }
    all.sort();
    all.dedup();
    all
}

/// The ops a run reading `sources` is given.
pub fn ops(sources: &[Source]) -> Vec<&'static str> {
    let mut ops: Vec<&'static str> = OWN.to_vec();
    for s in sources {
        ops.extend(reads(*s));
    }
    ops
}

/// What Aide never does, whatever is approved: shown on its settings page.
pub const REFUSED: &[&str] = &[
    "Pay, buy or move money",
    "Place, change or cancel trades",
    "Delete anything of yours",
    "Send mail or messages to anyone but you",
    "Change guardrails, keys, pairing or its own settings",
    "Run commands or write files",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Endpoint, Tiers};

    fn settings(money: Tier) -> Settings {
        Settings {
            tiers: Tiers { money, ..Tiers::default() },
            endpoints: vec![Endpoint { id: "farm".into(), name: "Farm".into(), url: "http://farm:8080".into(), model: None, context: None, vision: false }],
            ..Settings::default()
        }
    }

    #[test]
    fn hosts() {
        let s = settings(Tier::Any);
        assert_eq!(host_of(Some("farm"), &s, &["qwen35-4b"]), Host::Server);
        assert_eq!(host_of(Some("qwen35-4b"), &s, &["qwen35-4b"]), Host::Pc);
        assert_eq!(host_of(Some("sonnet"), &s, &["qwen35-4b"]), Host::Claude);
        assert_eq!(host_of(None, &s, &[]), Host::Claude);
    }

    #[test]
    fn local_only_data_never_reaches_claude() {
        let s = settings(Tier::LocalOnly);
        let all = |_: Option<&str>, _: Host| true;
        let c = choose(Some("sonnet"), Some("farm"), &[Source::Money, Source::Dev], &s, &[], all).unwrap();
        assert_eq!((c.model.as_deref(), c.host), (Some("farm"), Host::Server));
        let refused = choose(Some("sonnet"), None, &[Source::Money], &s, &[], all).unwrap_err();
        assert!(refused.0.contains("Tally is for your own models only"), "{}", refused.0);
        // Without the private source, Claude is fine.
        assert_eq!(choose(Some("sonnet"), None, &[Source::Dev], &s, &[], all).unwrap().host, Host::Claude);
    }

    #[test]
    fn an_unavailable_model_falls_back() {
        let s = settings(Tier::Any);
        let farm_down = |m: Option<&str>, _: Host| m != Some("farm");
        let c = choose(Some("farm"), Some("haiku"), &[Source::Gym], &s, &[], farm_down).unwrap();
        assert_eq!(c.model.as_deref(), Some("haiku"));
        let none = choose(Some("farm"), None, &[Source::Gym], &s, &[], |_, _| false).unwrap_err();
        assert!(none.0.contains("farm is not available"));
    }

    #[test]
    fn a_source_turned_off_is_left_out() {
        let s = settings(Tier::Off);
        let c = choose(None, None, &[Source::Money, Source::Gym], &s, &[], |_, _| true).unwrap();
        assert_eq!(c.sources, vec![Source::Gym]);
        assert_eq!(c.off, vec![Source::Money]);
        assert!(!ops(&c.sources).contains(&"money.summary"));
        assert!(ops(&c.sources).contains(&"gym.summary"));
    }

    #[test]
    fn no_op_outside_reads_and_its_own() {
        for op in all_ops() {
            assert!(op.starts_with("aide.") || op.ends_with(".list") || op.ends_with(".get") || op.ends_with(".summary") || op.ends_with(".series")
                || ["money.lists", "gym.sessions", "gym.lifts", "gym.cardio"].contains(&op), "{op}");
        }
    }
}
