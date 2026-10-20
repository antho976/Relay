//! Expectations and conflicts (MODEL-SPEC §2.4-§2.5; feature-agent-api §2.2, W7, W18): what a
//! node must still hold for a `design.apply` to go ahead as sent, and why an edit did not.
//!
//! A full reply gives the `expect` of its own undo (each field the list changed at the value it
//! holds now, each moved node's parent, what each added node holds), and the person's clients
//! send it back with the undo, so an undo that would overwrite a later change skips just that
//! field or edit and says so in `skipped`. The engine's checks are engine-expect's.

use super::{ID_PATTERN, MAX_ID};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// What a node must still hold for a `design.apply` to go ahead as sent. The person's clients send
/// the `expect` a full reply gave them with the undo of that list, and the values their own scene
/// showed with an ordinary edit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "DesignExpect")]
pub struct Expect {
    #[schemars(length(min = 1, max = MAX_ID), pattern(ID_PATTERN))]
    pub id: String,
    /// Field values it must hold, as an update's `set` writes them (`null`: the field unset).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set: Option<Map<String, Value>>,
    /// The parent it must sit in. Not used for pages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = MAX_ID), pattern(ID_PATTERN))]
    pub parent: Option<String>,
    /// The ids it may hold: anything else inside it (added since) is a conflict. An undo that
    /// deletes it sends this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub within: Option<Vec<String>>,
}

/// Why an edit did not go ahead as sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignConflictReason")]
pub enum ConflictReason {
    /// The node is gone.
    Missing,
    /// Its parent is gone.
    NoParent,
    /// Its parent's add was skipped, so it was too.
    ParentSkipped,
    /// `fields` hold other values than expected.
    Fields,
    /// It sits in another parent than expected.
    Parent,
    /// It holds nodes added since (`ids`); deleting it would take them.
    Inside,
    /// An undo would add it back, but its id is taken.
    Exists,
    /// It changed after `base_rev` (at `rev`).
    Changed,
    /// Adding it would nest it more than 64 deep.
    TooDeep,
    /// It is of a kind this Relay cannot read, so it is left as it is.
    Unreadable,
}

/// One node an edit could not act on as sent: in `design.stale`'s and `audit.stale`'s
/// `details.conflicts`, and in a lenient reply's `skipped`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "DesignConflict")]
pub struct Conflict {
    /// The edit's index in `ops`, when one edit is to blame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<u32>,
    pub id: String,
    pub reason: ConflictReason,
    /// For `fields`: the fields that hold other values now (a skipped update dropped these and
    /// applied the rest).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
    /// For `inside`: the nodes added inside it since, at most 20.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ids: Vec<String>,
    /// For `changed`, and a `missing` found among the removals: the rev it changed or went at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<i64>,
}

impl Conflict {
    /// A conflict of edit `at` on node `id`, for `reason`, with nothing more to say.
    pub fn new(at: Option<u32>, id: impl Into<String>, reason: ConflictReason) -> Conflict {
        Conflict { at, id: id.into(), reason, fields: Vec::new(), ids: Vec::new(), rev: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn expectations_read_as_a_reply_gives_them() {
        let given = json!([{"id": "save", "set": {"fill": "#4C9DFF", "radius": null}}, {"id": "card", "parent": "page-1"},
            {"id": "list", "within": ["row-1", "row-2"]}]);
        let read: Vec<Expect> = serde_json::from_value(given.clone()).unwrap();
        assert_eq!(read[0].set.as_ref().unwrap()["radius"], Value::Null, "a null expects the field unset");
        assert_eq!(serde_json::to_value(&read).unwrap(), given, "written back as given, nothing added");
        // A key it does not know is refused (`design.apply` also refuses one written as a list).
        assert!(serde_json::from_value::<Expect>(json!({"id": "a", "sett": {}})).is_err());
    }

    #[test]
    fn a_conflict_says_only_what_it_has() {
        let missing = Conflict::new(Some(3), "card-9", ConflictReason::Missing);
        assert_eq!(serde_json::to_value(&missing).unwrap(), json!({"at": 3, "id": "card-9", "reason": "missing"}));
        let fields = Conflict { fields: vec!["fill".into()], ..Conflict::new(None, "save", ConflictReason::Fields) };
        assert_eq!(serde_json::to_value(&fields).unwrap(), json!({"id": "save", "reason": "fields", "fields": ["fill"]}));
        for (reason, name) in [(ConflictReason::NoParent, "no_parent"), (ConflictReason::ParentSkipped, "parent_skipped"),
            (ConflictReason::TooDeep, "too_deep"), (ConflictReason::Unreadable, "unreadable")] {
            assert_eq!(serde_json::to_value(reason).unwrap(), name);
        }
        // A newer engine's conflict, with a field this build does not know, still reads.
        let newer: Conflict = serde_json::from_value(json!({"id": "a", "reason": "changed", "rev": 9, "later": 1})).unwrap();
        assert_eq!(newer.rev, Some(9));
    }
}
