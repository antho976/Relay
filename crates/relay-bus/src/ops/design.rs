//! `design.*` — BUS.md §10.18. Design canvases: a project's designs, each optionally tied to a
//! terminal, made and changed by the person on the canvas and by agents through these ops.
//!
//! Every agent role may call these (`relay_core::guardrail::OPEN_TO_EVERY_ROLE`): a design is
//! the person's to ask for, and every change carries its undo. Agents act on one only when asked.
//!
//! A design is pages of nodes (`crate::design`). Every change goes through `design.apply`, a
//! list of node edits run in order, all or none; it answers with the nodes as stored and the
//! edits that undo it, so the canvas's undo and `audit.undo` revert exactly what was done.
use crate::design::{Node, NodeOp, MAX_OPS};
use crate::registry::{Actors, OpEntry, OpMeta, Scope, Undo};
use crate::types::{Design, Id};
use crate::{op, Empty};

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
    /// Only this page and what is on it.
    pub page: Option<String>,
});
result!(#[schemars(rename = "DesignGetOut")] GetOut {
    pub design: Design,
    /// Pages first in their order, then every other node after its parent, siblings bottom to
    /// top.
    #[serde(serialize_with = "crate::design::write_nodes", deserialize_with = "crate::design::read_nodes")]
    pub nodes: Vec<Node>,
    /// Where an image's `src` is, on this machine.
    pub assets_dir: String,
});
op!(Get, "design.get", GetIn => GetOut,
    OpMeta::query(Scope::Project, 12, "A design and all its nodes (or one page's)"));

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
    OpMeta::mutation(Scope::Project, 12, "Copy a design, every page and node, under a new title")
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

payload!(#[schemars(rename = "DesignApplyIn")] ApplyIn {
    pub design_id: Id,
    /// Run in order, all or none: 1 to 2000 edits. A design holds at most 5000 nodes, 1536 KiB of
    /// node JSON and 64 levels of nesting. A node an edit adds can be the parent of a later one.
    /// An edit that does not read is refused by its index and the key at fault (`ops[3] update
    /// "hero": set: …`).
    #[serde(deserialize_with = "crate::design::de_ops")]
    #[schemars(length(min = 1, max = MAX_OPS))]
    pub ops: Vec<NodeOp>,
    /// Refuse (`design.stale`) unless the design is still at this `rev`.
    pub base_rev: Option<i64>,
    /// Replay an undo: skip an edit of a node that no longer exists (or that this Relay cannot
    /// read), and put back what was stored even where a new edit's rules would refuse it,
    /// instead of refusing the whole list. Only for an `inverse` an answer gave you.
    pub lenient: Option<bool>,
});
result!(#[schemars(rename = "DesignApplyOut")] ApplyOut {
    pub design: Design,
    /// The `rev` before these edits: when it is not the one you last read, someone else
    /// changed the design in between; read it again with `design.get`.
    pub prev_rev: i64,
    /// Each node an edit added, changed or moved, as stored now.
    #[serde(serialize_with = "crate::design::write_nodes", deserialize_with = "crate::design::read_nodes")]
    pub nodes: Vec<Node>,
    /// The ids deleted, with everything that was inside them.
    pub removed: Vec<String>,
    /// For each edit, the id of the node it acted on (an `add`'s assigned id); empty for one
    /// skipped by `lenient`.
    pub ids: Vec<String>,
    /// The edits that undo these, in the order to apply them.
    pub inverse: Vec<NodeOp>,
    /// For an agent, what a lenient list applied as written although a strict one would have
    /// refused it, or skipped because this Relay cannot read the node: each edit's label and
    /// why (at most 20).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
});
op!(Apply, "design.apply", ApplyIn => ApplyOut,
    OpMeta::mutation(Scope::Project, 12, "Add, change, move or delete a design's nodes in one all-or-none list of up to 2000 edits, only when the person asked for it (a design holds at most 5000 nodes and 1536 KiB; an id an edit adds can be a parent later in the list); returns what changed and its undo")
        .undo(Undo::Inverse).emits(&["design.changed"]));

payload!(#[schemars(rename = "DesignAssetAddIn")] AssetAddIn {
    pub design_id: Id,
    /// A PNG, JPEG, GIF, WebP or SVG file on this machine, copied in.
    pub path: Option<String>,
    /// The picture itself, base64, instead of `path` (at most 1 MiB decoded).
    pub bytes_b64: Option<String>,
});
result!(#[schemars(rename = "DesignAssetAddOut")] AssetAddOut {
    /// What an image node's `src` names.
    pub src: String,
    /// Where the copy is.
    pub path: String,
    /// Its natural size in px (an SVG's declared size, or 300 by 150).
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
