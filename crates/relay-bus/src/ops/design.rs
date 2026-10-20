//! `design.*` — BUS.md §10.18. Design canvases: a project's designs, each optionally tied to a
//! terminal, made and changed by the person on the canvas and by agents through these ops.
//!
//! Every agent role may call these (`relay_core::guardrail::OPEN_TO_EVERY_ROLE`): a design is
//! the person's to ask for, and every change carries its undo. Agents act on one only when asked.
//!
//! A design is pages of nodes (`crate::design`). Every change goes through `design.apply`, a
//! list of node edits run in order, all or none; it answers with the nodes as stored and the
//! edits that undo it, so the canvas's undo and `audit.undo` revert exactly what was done.
use crate::design::{Conflict, Expect, Node, NodeOp, Style, MAX_OPS};
use crate::ops::picture::PictureCredit;
use crate::registry::{Actors, OpEntry, OpMeta, Scope, Undo};
use crate::types::{Design, Id};
use crate::{op, Empty};
use std::collections::BTreeMap;

/// Most ids `design.get`'s `nodes` names.
pub const GET_NODES_MAX: usize = 500;
/// Most credits `design.get` answers, and the most of their JSON.
pub const CREDITS_MAX: usize = 128;
pub const CREDITS_BYTES_MAX: usize = 256 * 1024;

payload!(#[schemars(rename = "DesignListIn")] ListIn {
    pub project_id: Id,
    /// Only the designs tied to this terminal (session name).
    pub session: Option<String>,
    /// Also list deleted designs (`deleted_at` set), which `design.restore` brings back.
    pub include_deleted: Option<bool>,
});
result!(#[schemars(rename = "DesignListOut")] ListOut {
    /// Most recently changed first.
    pub designs: Vec<Design>,
});
op!(List, "design.list", ListIn => ListOut,
    OpMeta::query(Scope::Project, 12, "A project's designs, newest change first; optionally one terminal's"));

payload!(#[schemars(rename = "DesignIdIn")] IdIn { pub design_id: Id });
payload!(#[schemars(rename = "DesignGetIn")] GetIn {
    pub design_id: Id,
    /// Only this page and what is on it: its id, or its name when no other page has that name.
    pub page: Option<String>,
    /// Only the pages themselves (with `page`, that one), without what is on them: the design's
    /// page list, to read one page at a time. Not with `nodes` or `since_rev`.
    pub pages_only: Option<bool>,
    /// Only these nodes (1 to 500 ids) and what is inside them.
    #[schemars(length(min = 1, max = GET_NODES_MAX))]
    pub nodes: Option<Vec<String>>,
    /// Only what changed after this rev (0 to the design's rev): `nodes` are those added,
    /// changed or moved since, in this answer's usual order, and `removed` the ids gone since.
    /// It reaches back 1000 revs (older is design.since_too_old: read it whole). With `nodes`,
    /// only what changed inside them.
    #[schemars(range(min = 0))]
    pub since_rev: Option<i64>,
});
result!(#[schemars(rename = "DesignGetOut")] GetOut {
    pub design: Design,
    /// Pages first in their order, then every other node after its parent, siblings bottom to
    /// top.
    #[serde(serialize_with = "crate::design::write_nodes", deserialize_with = "crate::design::read_nodes")]
    pub nodes: Vec<Node>,
    /// Where an image's `src` is, on this machine.
    pub assets_dir: String,
    /// With `since_rev`: the ids removed since.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed: Option<Vec<String>>,
    /// The design's named colours, by name: a node's colour field takes one as `$name` (a
    /// `color` edit names one). Always whole.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub colors: BTreeMap<String, String>,
    /// The design's named styles, by name: a node takes one with `style` (a `style` edit names
    /// one). Always whole.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub styles: BTreeMap<String, Style>,
    /// Where each picture came from, by `src`, for those stored with a credit
    /// (design.picture.add): at most 128 of them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub credits: BTreeMap<String, PictureCredit>,
});
op!(Get, "design.get", GetIn => GetOut,
    OpMeta::query(Scope::Project, 12, "A design and its nodes: all, one page's (by id or name), the pages alone, some nodes with what is inside them, or only what changed since a rev"));

payload!(#[schemars(rename = "DesignCreateIn")] CreateIn {
    pub project_id: Id,
    /// Default "Untitled design".
    pub title: Option<String>,
    /// The terminal it belongs to. An agent's design is its own session's; the person may
    /// name any session of the project, or none.
    pub session: Option<String>,
});
op!(Create, "design.create", CreateIn => GetOut,
    OpMeta::mutation(Scope::Project, 12, "Start a design with one empty page, only when the person asked for one; returns it and its page")
        .undo(Undo::Inverse).emits(&["design.changed"]));

payload!(#[schemars(rename = "DesignUpdateIn")] UpdateIn {
    pub design_id: Id,
    pub title: Option<String>,
    /// Tie it to another terminal (session name), or `null` to the project alone.
    #[serde(default, deserialize_with = "crate::nullable")] pub session: Option<Option<String>>,
});
op!(Update, "design.update", UpdateIn => Design,
    OpMeta::mutation(Scope::Project, 12, "Rename a design or tie it to another terminal, when the person asked")
        .undo(Undo::Inverse).emits(&["design.changed"]));

payload!(#[schemars(rename = "DesignDuplicateIn")] DuplicateIn {
    pub design_id: Id,
    /// Default the original's title with " copy".
    pub title: Option<String>,
});
op!(Duplicate, "design.duplicate", DuplicateIn => Design,
    OpMeta::mutation(Scope::Project, 12, "Copy a design, every page and node, under a new title; an agent's copy is tied to its own terminal")
        .undo(Undo::Inverse).emits(&["design.changed"]));

op!(Delete, "design.delete", IdIn => Empty,
    OpMeta::mutation(Scope::Project, 12, "Move a design to the project's deleted designs, only when the person asked; design.restore brings it back")
        .undo(Undo::Inverse).emits(&["design.changed"]));
op!(Restore, "design.restore", IdIn => Design,
    OpMeta::mutation(Scope::Project, 12, "Bring back a deleted design")
        .undo(Undo::Inverse).emits(&["design.changed"]));
op!(Purge, "design.purge", IdIn => Empty,
    OpMeta::mutation(Scope::Project, 12, "Delete a deleted design for good, with its nodes")
        .actors(Actors::UserOnly).emits(&["design.deleted"]));

/// What `design.apply` answers with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignReply")]
pub enum Reply {
    /// The rev, each edit's node id, how many nodes went, the audit row, and what was skipped or
    /// warned of.
    Short,
    /// Also the design row, each node touched as stored, the removed ids, the edits that undo
    /// the list and what that undo expects to find.
    Full,
}

payload!(#[schemars(rename = "DesignApplyIn")] ApplyIn {
    pub design_id: Id,
    /// Run in order, all or none: 1 to 2000 edits. A design holds at most 5000 nodes, 1536 KiB of
    /// them as design.get returns them (ids, parents and its named colours and styles counted)
    /// and 64 levels of nesting. A node an edit adds can be the parent of a later one. An edit
    /// that does not read is refused by its index and the key at fault (`ops[3] update "hero":
    /// set: …`).
    #[serde(deserialize_with = "crate::design::de_ops")]
    #[schemars(length(min = 1, max = MAX_OPS))]
    pub ops: Vec<NodeOp>,
    /// The rev your edits were made against: the one you read, or the last reply's `rev`. The list
    /// is refused (`design.stale`, with `details.conflicts`) only when a node it changes, moves or
    /// deletes, or one inside a node it deletes, changed after this rev; changes elsewhere do not
    /// stop it, and the reply says `rebased: true`.
    pub base_rev: Option<i64>,
    /// Replay an undo (an `inverse` a reply gave): skip an edit whose node or parent is gone,
    /// whose node this Relay cannot read, or that would overwrite a change made since (`expect`),
    /// and put back what was stored even where a new edit's rules would refuse it, instead of
    /// refusing the whole list. `skipped` says which and why. A list that applies nothing changes
    /// nothing: no new rev, no event and no undo. An agent's list holds at most 2000 edits,
    /// lenient or not.
    pub lenient: Option<bool>,
    /// `short` (an agent's default) or `full` (the person's clients' default).
    pub reply: Option<Reply>,
    /// What the nodes must still hold: the `expect` a full reply gave with its `inverse`, sent
    /// back with that undo by the person's clients. An edit that would overwrite a change made
    /// since is refused (`design.stale`), or skipped when lenient.
    #[serde(default, deserialize_with = "crate::design::obj_vec_opt")]
    #[schemars(extend("x-unit" = "omit"))]
    pub expect: Option<Vec<Expect>>,
});
result!(#[schemars(rename = "DesignApplyOut")] ApplyOut {
    pub design_id: Id,
    /// The design's rev now: the next `base_rev`, and the one design.render pictures. Unchanged
    /// when nothing applied.
    pub rev: i64,
    /// The `rev` before these edits: when it is not the one you last read, someone else
    /// changed the design in between; read it again with `design.get`.
    pub prev_rev: i64,
    /// `base_rev` was behind, but nothing this list writes had changed since: it applied on top.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub rebased: bool,
    /// For each edit, the id of the node it acted on (an add's or a copy's assigned id; a
    /// colour's or style's name); "" for one skipped.
    pub ids: Vec<String>,
    /// How many nodes are gone, everything inside deleted nodes included.
    pub removed_count: u32,
    /// The audit row of this change: audit.get shows it with its undo, and the person can undo it
    /// from the board or the audit log. Absent when nothing applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_id: Option<Id>,
    /// Edits (or fields of one) not applied, and why: at most 100, a warning counts the rest.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<Conflict>,
    /// What applied but may not be what you meant (a font the desktop lacks, a written rule a
    /// lenient list let through), each saying what to do: at most 20.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// Each copy edit's new node and the node it copied (`{"card-2": "card"}`). The nodes inside
    /// a copy take ids by copy's rule (`card-title` becomes `card-2-title`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub copied: BTreeMap<String, String>,
    /// Full only: the design row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design: Option<Design>,
    /// Full only: each node an edit added, changed or moved, as stored now. Left out (with
    /// `reread`) when the answer would be too large.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(serialize_with = "crate::design::write_nodes_opt", deserialize_with = "crate::design::read_nodes_opt")]
    pub nodes: Option<Vec<Node>>,
    /// Full only: the ids deleted, with everything that was inside them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed: Option<Vec<String>>,
    /// Full only: the edits that undo these, in the order to apply them, sent `lenient` with
    /// `expect`. Absent when too large to send (a warning says the step cannot be undone).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inverse: Option<Vec<NodeOp>>,
    /// Full only: what the undo expects to find; send it with `inverse`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "omit"))]
    pub expect: Option<Vec<Expect>>,
    /// Full only, when an edit of the list changed them: the design's named colours, whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colors: Option<BTreeMap<String, String>>,
    /// Full only, when an edit of the list changed them: the design's named styles, whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub styles: Option<BTreeMap<String, Style>>,
    /// Full only: `nodes` (and perhaps `inverse`) were left out to keep the answer under the
    /// clients' 2 MiB line; read the design again.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reread: bool,
});
op!(Apply, "design.apply", ApplyIn => ApplyOut,
    OpMeta::mutation(Scope::Project, 12, "Add, change, move, copy or delete a design's nodes and name its colours and styles, in one all-or-none list of up to 2000 edits, only when the person asked for it (a design holds at most 5000 nodes and 1536 KiB; an id an edit adds can be a parent later in the list); returns the new rev and each edit's node id; see the result with design.render")
        .undo(Undo::Inverse).emits(&["design.changed"]));

payload!(#[schemars(rename = "DesignAssetAddIn")] AssetAddIn {
    pub design_id: Id,
    /// An absolute path to a PNG, JPEG, GIF, WebP or SVG file on this machine, at most 20 MiB,
    /// copied in.
    pub path: Option<String>,
    /// The picture itself, base64, instead of `path` (at most 1 MiB decoded).
    pub bytes_b64: Option<String>,
});
result!(#[schemars(rename = "DesignAssetAddOut")] AssetAddOut {
    /// What an image node's `src` names.
    pub src: String,
    /// Where the copy is.
    pub path: String,
    /// Its natural size in px: an SVG's width and height, the missing one taken from its
    /// viewBox's aspect, else the viewBox's size, else 300 by 150.
    pub w: u32,
    pub h: u32,
});
op!(AssetAdd, "design.asset.add", AssetAddIn => AssetAddOut,
    OpMeta::mutation(Scope::Project, 12, "Store a picture for a design's image nodes; returns the src to give them"));

/// The design ops; `design.apply`'s `bus.schema` refusals name the edit and key at fault
/// (`design::locate`).
pub fn entries() -> Vec<OpEntry> {
    vec![
        OpEntry::of::<List>(),
        OpEntry::of::<Get>(),
        OpEntry::of::<Create>(),
        OpEntry::of::<Update>(),
        OpEntry::of::<Duplicate>(),
        OpEntry::of::<Delete>(),
        OpEntry::of::<Restore>(),
        OpEntry::of::<Purge>(),
        OpEntry::of::<Apply>().located(crate::design::locate),
        OpEntry::of::<AssetAdd>(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::{MAX_ASSET_B64, MAX_ASSET_FILE};
    use serde_json::{json, Value};

    /// A payload or result type's rendered schema.
    fn schema<T: schemars::JsonSchema>() -> Value {
        serde_json::to_value(schemars::schema_for!(T)).unwrap()
    }

    /// A field's doc, its lines joined.
    fn doc(schema: &Value, field: &str) -> String {
        schema["properties"][field]["description"].as_str().unwrap_or_else(|| panic!("{field} has no doc")).replace('\n', " ")
    }

    /// B124 and W15: what the docs say of a limit is the limit the engine holds a caller to.
    #[test]
    fn the_docs_state_the_limits_the_engine_keeps() {
        let add = schema::<AssetAddIn>();
        let path = doc(&add, "path");
        assert!(path.starts_with("An absolute path") && path.contains(&format!("at most {} MiB", MAX_ASSET_FILE >> 20)), "{path}");
        assert!(doc(&add, "bytes_b64").contains(&format!("at most {} MiB decoded", MAX_ASSET_B64 >> 20)));
        let size = doc(&schema::<AssetAddOut>(), "w");
        assert!(size.contains("viewBox's aspect") && size.contains("300 by 150"), "{size}");
        let get = schema::<GetIn>();
        assert_eq!(get["properties"]["nodes"]["maxItems"].as_u64(), Some(GET_NODES_MAX as u64));
        assert!(doc(&get, "nodes").contains(&format!("1 to {GET_NODES_MAX} ids")));
        assert!(doc(&get, "since_rev").contains(&format!("{} revs", crate::design::SINCE_WINDOW)));
        assert!(doc(&schema::<GetOut>(), "credits").contains(&format!("at most {CREDITS_MAX}")));
    }

    /// B48: the lenient doc says what a lenient list skips, where it says so, that a list that
    /// applies nothing changes nothing, and that an agent's list is capped either way.
    #[test]
    fn the_lenient_doc_says_what_is_skipped_and_what_is_kept() {
        let apply = schema::<ApplyIn>();
        let lenient = doc(&apply, "lenient");
        for said in ["`skipped` says which and why", "A list that applies nothing changes nothing: no new rev, no event and no undo",
            &format!("An agent's list holds at most {MAX_OPS} edits, lenient or not")] {
            assert!(lenient.contains(said), "{said} in {lenient}");
        }
        let skipped = doc(&schema::<ApplyOut>(), "skipped");
        assert!(skipped.contains("at most 100"), "{skipped}");
    }

    /// The person's clients read a full answer; an agent's short one reads too, with nothing of
    /// the full one in it, and the expectations go back as they came.
    #[test]
    fn short_and_full_answers_read_and_write_back() {
        let short = json!({"design_id": 4, "rev": 9, "prev_rev": 8, "ids": ["card", ""], "removed_count": 0, "audit_id": 31,
            "skipped": [{"at": 1, "id": "gone", "reason": "missing"}]});
        let read: ApplyOut = serde_json::from_value(short.clone()).unwrap();
        assert_eq!((read.design.is_none(), read.nodes.is_none(), read.inverse.is_none(), read.reread), (true, true, true, false));
        assert_eq!(serde_json::to_value(&read).unwrap(), short, "nothing a short answer lacks is written");
        let full = json!({"design_id": 4, "rev": 9, "prev_rev": 8, "ids": ["c"], "removed_count": 0, "copied": {"card-2": "card"},
            "nodes": [{"id": "c", "parent": "page-1", "kind": "rect", "future": 1}], "removed": [],
            "inverse": [{"op": "delete", "id": "c"}], "expect": [{"id": "card", "within": []}], "colors": {"accent": "#4C9DFF"}, "reread": true});
        let read: ApplyOut = serde_json::from_value(full).unwrap();
        assert_eq!(read.nodes.as_ref().unwrap()[0].extra["future"], 1, "a node a newer engine wrote reads");
        assert_eq!(read.copied["card-2"], "card");
        let written = serde_json::to_value(&read).unwrap();
        assert_eq!(written["nodes"][0]["future"], 1);
        assert_eq!(written["expect"], json!([{"id": "card", "within": []}]));
        // The payload: reply by name, expectations only as objects (TM-16), nothing unknown.
        let payload = |extra: Value| {
            let mut p = json!({"design_id": 4, "ops": [{"op": "delete", "id": "c"}]});
            p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            serde_json::from_value::<ApplyIn>(p)
        };
        let read = payload(json!({"reply": "full", "lenient": true, "expect": [{"id": "c", "parent": "page-1"}]})).unwrap();
        assert_eq!((read.reply, read.expect.as_ref().map(Vec::len)), (Some(Reply::Full), Some(1)));
        assert!(payload(json!({"expect": [["c", null, "page-1"]]})).is_err(), "an expectation written as a list");
        assert!(payload(json!({"reply": "brief"})).is_err());
        assert_eq!(payload(json!({"expect": null})).unwrap().expect, None);
        // design.get's new reply fields are left out when empty, and read when absent.
        let get: GetOut = serde_json::from_value(json!({"design": design_row(), "nodes": [], "assets_dir": "/a"})).unwrap();
        assert!(get.colors.is_empty() && get.styles.is_empty() && get.credits.is_empty() && get.removed.is_none());
        assert_eq!(serde_json::to_value(&get).unwrap().as_object().unwrap().len(), 3);
    }

    fn design_row() -> Value {
        json!({"id": 4, "project_id": 1, "session": null, "title": "Home", "rev": 9, "pages": 1, "nodes": 1, "created_by": "user",
            "updated_by": "user", "created_at": "2026-10-20T00:00:00Z", "updated_at": "2026-10-20T00:00:00Z", "deleted_at": null})
    }
}
