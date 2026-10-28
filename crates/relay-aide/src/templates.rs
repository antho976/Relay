//! The jobs Aide starts with (docs/AIDE.md, "The first jobs"): each on data Relay already holds,
//! offered for the person to turn on as they are or change first.

use crate::model::{Every, JobSpec, Level, Rule, Schedule, Source};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Template {
    /// Stable: `brief`, `stuck`, `budget`, `arbiter`, `weekly`.
    pub id: String,
    /// What it is for, in a sentence.
    pub why: String,
    pub spec: JobSpec,
}

fn spec(name: &str, instructions: &str, schedule: Option<Schedule>, rules: Vec<Rule>, sources: Vec<Source>, level: Level) -> JobSpec {
    JobSpec { name: name.into(), instructions: instructions.into(), schedule, rules, sources, model: None, fallback: None, effort: None, level, runs_per_day: None, minutes_per_run: None }
}

pub fn all() -> Vec<Template> {
    let at = |every, hour| Some(Schedule { every, hour, minute: 0, weekday: None, day: None });
    vec![
        Template {
            id: "brief".into(),
            why: "A short morning read of the night: agents, money, Arbiter and training, with what waits on you first.".into(),
            spec: spec(
                "Morning brief",
                "Write my morning brief as one aide.notify at brief level, ten lines at most. First what waits on me (agents \
                 blocked or waiting, holds, errands), then what finished overnight, then yesterday's spending against my budgets, \
                 Arbiter's night (orders, halts) and my last workout. Include everything held for this brief. Name each source you \
                 read, and say plainly which could not be read. Skip a section that has nothing in it.",
                at(Every::Weekdays, 7),
                vec![],
                vec![Source::Dev, Source::Money, Source::Arbiter, Source::Gym],
                Level::Interrupt,
            ),
        },
        Template {
            id: "stuck".into(),
            why: "An agent waiting on you for an hour gets one nudge, not a pile of them.".into(),
            spec: spec(
                "Agent waiting on me",
                "An agent session has waited on me for an hour. Say which one, what it waits for (its question, hold or \
                 approval) and what I would need to do, in two lines. One interrupt; if it is still waiting at the next brief, \
                 the brief says so.",
                None,
                vec![Rule::Stuck { minutes: 60 }],
                vec![Source::Dev],
                Level::Interrupt,
            ),
        },
        Template {
            id: "budget".into(),
            why: "A budget past 90% of its month, said once, with what pushed it.".into(),
            spec: spec(
                "Budget watch",
                "A budget category has passed 90% of its month. Say which, how much is left, how many days are left in the \
                 period, and the two or three entries that pushed it, each named by its Tally entry.",
                None,
                vec![Rule::Budget { percent: 90 }],
                vec![Source::Money],
                Level::Interrupt,
            ),
        },
        Template {
            id: "arbiter".into(),
            why: "A halted strategy or an exchange that stopped answering, the moment Arbiter sees it.".into(),
            spec: spec(
                "Arbiter watch",
                "Arbiter halted a strategy, its kill switch is on, or the exchange stopped answering. Say what happened, what \
                 it holds now, and what I can do (restart, flatten or wait). Never place, change or cancel an order.",
                None,
                vec![Rule::Arbiter {}],
                vec![Source::Arbiter],
                Level::Interrupt,
            ),
        },
        Template {
            id: "weekly".into(),
            why: "A living page every Sunday evening: the week in money, training and work.".into(),
            spec: spec(
                "Weekly review",
                "Write the page \"Weekly review\" with aide.page.write (question: how did my week go?): money against the \
                 budgets, training against the week before, and what the agents shipped and what stalled. Every number names \
                 its source. Then one brief-level aide.notify saying the page is ready, with its one most useful line.",
                Some(Schedule { every: Every::Week, hour: 18, minute: 0, weekday: Some(7), day: None }),
                vec![],
                vec![Source::Dev, Source::Money, Source::Gym],
                Level::Brief,
            ),
        },
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_template_is_a_valid_job() {
        let all = super::all();
        assert_eq!(all.len(), 5);
        for t in all {
            t.spec.clone().checked().unwrap_or_else(|e| panic!("{}: {e}", t.id));
        }
    }
}
