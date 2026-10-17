//! `design.*` — BUS.md §10.18. Design canvases: a project's designs, each optionally tied to a
//! terminal, made and changed by the person on the canvas and by agents through these ops.
//!
//! Every agent role may call these (`relay_core::guardrail::OPEN_TO_EVERY_ROLE`): a design is
//! the person's to ask for, and every change carries its undo. Agents act on one only when asked.
//!
//! A design is pages of nodes (`crate::design`). Every change goes through `design.apply`, a
//! list of node edits run in order, all or none; it answers with the nodes as stored and the
//! edits that undo it, so the canvas's undo and `audit.undo` revert exactly what was done.
//!
//! `design.version.*` keeps whole copies of a design's nodes to go back to; `design.code` writes
//! a frame out as HTML and CSS; `design.render` asks Relay's window for a picture of one, which
//! an agent cannot otherwise see.
use crate::design::{Node, NodeOp};
use crate::registry::{Actors, OpMeta, Scope, Undo};
use crate::types::{Design, DesignVersion, Id};
use crate::{op, Empty};
use serde_json::{Map, Value};

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
    /// Start from a template (`templates/<item>` from `design.kits`), placed on the first page.
    pub template: Option<String>,
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
    OpMeta::mutation(Scope::Project, 12, "Delete a deleted design for good, with its nodes and versions")
        .actors(Actors::UserOnly).emits(&["design.deleted"]));

payload!(#[schemars(rename = "DesignApplyIn")] ApplyIn {
    pub design_id: Id,
    /// Run in order, all or none.
    pub ops: Vec<NodeOp>,
    /// Refuse (`design.stale`) unless the design is still at this `rev`.
    pub base_rev: Option<i64>,
    /// Skip an edit of a node that no longer exists, instead of refusing the whole list:
    /// for an undo that may race someone else's delete.
    pub lenient: Option<bool>,
});
result!(#[schemars(rename = "DesignApplyOut")] ApplyOut {
    pub design: Design,
    /// The `rev` before these edits: when it is not the one you last read, someone else
    /// changed the design in between; read it again with `design.get`.
    pub prev_rev: i64,
    /// Each node an edit added, changed or moved, as stored now.
    pub nodes: Vec<Node>,
    /// The ids deleted, with everything that was inside them.
    pub removed: Vec<String>,
    /// For each edit, the id of the node it acted on (an `add`'s assigned id); empty for one
    /// skipped by `lenient`.
    pub ids: Vec<String>,
    /// The edits that undo these, in the order to apply them.
    pub inverse: Vec<NodeOp>,
});
op!(Apply, "design.apply", ApplyIn => ApplyOut,
    OpMeta::mutation(Scope::Project, 12, "Add, change, move or delete a design's nodes in one all-or-none list, only when the person asked for it; returns what changed and its undo")
        .undo(Undo::Inverse).emits(&["design.changed", "design.version.changed"]));

payload!(#[schemars(rename = "DesignKitsIn")] KitsIn {
    /// Only this pack.
    pub kit: Option<String>,
    /// Each item's nodes in full, to read how it is built (large); the default gives each
    /// item's name, category, slots and its root alone (its size).
    pub with_nodes: Option<bool>,
});
result!(#[schemars(rename = "DesignKitsOut")] KitsOut { pub kits: Vec<crate::kits::Kit> });
op!(Kits, "design.kits", KitsIn => KitsOut,
    OpMeta::query(Scope::Global, 12, "Relay's asset packs and templates: buttons, fields, cards, nav bars, flowchart shapes, device frames and whole pages, to place with design.apply's insert"));

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

payload!(#[schemars(rename = "DesignVersionSaveIn")] VersionSaveIn {
    pub design_id: Id,
    /// What to call it, 1 to 200 characters; absent, it is listed by when it was saved.
    pub label: Option<String>,
});
op!(VersionSave, "design.version.save", VersionSaveIn => DesignVersion,
    OpMeta::mutation(Scope::Project, 12, "Save a named copy of a design's nodes to come back to with design.version.restore; returns it without its nodes")
        .undo(Undo::Inverse).emits(&["design.version.changed"]));

result!(#[schemars(rename = "DesignVersionListOut")] VersionListOut {
    /// Newest first: the named ones, kept until deleted, among the newest 30 the engine kept.
    pub versions: Vec<DesignVersion>,
});
op!(VersionList, "design.version.list", IdIn => VersionListOut,
    OpMeta::query(Scope::Project, 12, "A design's saved versions, newest first: the named ones and those the engine kept before an agent's edits or a restore"));

payload!(#[schemars(rename = "DesignVersionIn")] VersionIn {
    pub design_id: Id,
    pub version_id: Id,
});
result!(#[schemars(rename = "DesignVersionGetOut")] VersionGetOut {
    pub version: DesignVersion,
    /// Every node it holds, in `design.get`'s order.
    pub nodes: Vec<Node>,
});
op!(VersionGet, "design.version.get", VersionIn => VersionGetOut,
    OpMeta::query(Scope::Project, 12, "A saved version of a design with its nodes, to preview or compare"));

result!(#[schemars(rename = "DesignVersionRestoreOut")] VersionRestoreOut {
    pub design: Design,
    /// The version kept of the design as it was just before, which restores it back.
    pub saved_version_id: Id,
});
op!(VersionRestore, "design.version.restore", VersionIn => VersionRestoreOut,
    OpMeta::mutation(Scope::Project, 12, "Put a design's nodes back as a saved version holds them, only when the person asked; what was there is kept first as a version of its own")
        .undo(Undo::Inverse).emits(&["design.changed", "design.version.changed"]));

op!(VersionDelete, "design.version.delete", VersionIn => Empty,
    OpMeta::mutation(Scope::Project, 12, "Delete a saved version of a design for good; an agent deletes only those it saved")
        .emits(&["design.version.deleted"]));

payload!(#[schemars(rename = "DesignCodeIn")] CodeIn {
    pub design_id: Id,
    /// The frame to write out with everything inside it (any node but a page).
    pub node_id: String,
    /// Pictures as `data:` URLs instead of `design-assets/<src>` paths, while they come to at
    /// most 1 MiB together; past that the rest keep their paths.
    pub inline_assets: Option<bool>,
});
result!(#[schemars(rename = "DesignCodeOut")] CodeOut {
    /// The node as nested elements, each with a class named after its layer.
    pub html: String,
    /// The rules for those classes.
    pub css: String,
    /// The pictures the HTML and CSS name by path, `design-assets/<src>` (copy them from
    /// `design.get`'s `assets_dir`), each once.
    pub assets: Vec<String>,
});
op!(Code, "design.code", CodeIn => CodeOut,
    OpMeta::query(Scope::Project, 12, "A frame as static HTML and CSS: flexbox for auto layout, absolute positions otherwise, styled text, pictures, icons and vectors as inline SVG"));

payload!(#[schemars(rename = "DesignRenderIn")] RenderIn {
    pub design_id: Id,
    /// The node to picture with what is inside it; default the design's first page.
    pub node_id: Option<String>,
    /// Pixels per design pixel, 1 to 4 (default 2).
    pub scale: Option<f64>,
});
result!(#[schemars(rename = "DesignRenderOut")] RenderOut {
    /// What the `design.rendered` answering this carries: `<design id>-<uuid>`.
    pub request_id: String,
    pub node_id: String,
    /// The `bus.wait` payload that takes the answer, to pass as it is.
    #[schemars(with = "Map<String, Value>")]
    pub wait: Value,
});
op!(Render, "design.render", RenderIn => RenderOut,
    OpMeta::query(Scope::Project, 12, "Ask Relay's window for a PNG of a design node, to see what you drew: then bus.wait {events: [\"design.rendered\"], matching: {request_id}, timeout_ms: 20000} (the answer's `wait`) and read the PNG at the event's path. Needs Relay's window open: no answer within 20 s means it is closed")
        .emits(&["design.render_requested"]));

payload!(#[schemars(rename = "DesignRenderDoneIn")] RenderDoneIn {
    /// From `design.render_requested`: `<design id>-<uuid>`, so the answer names its design
    /// without the engine keeping anything between the two.
    pub request_id: String,
    /// The PNG written: an absolute path to a file.
    pub path: Option<String>,
    /// Why there is no picture, instead of `path`.
    pub error: Option<String>,
});
op!(RenderDone, "design.render.done", RenderDoneIn => Empty,
    OpMeta::query(Scope::Project, 12, "The window's answer to design.render_requested: the PNG it wrote, or why not; announced as design.rendered")
        .actors(Actors::UserOnly).emits(&["design.rendered"]));

entries!(List, Get, Create, Update, Duplicate, Delete, Restore, Purge, Apply, AssetAdd, Kits,
    VersionSave, VersionList, VersionGet, VersionRestore, VersionDelete, Code, Render, RenderDone);
