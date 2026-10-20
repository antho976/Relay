//! `design.*` — BUS.md §10.18. Design canvases: a project's designs, each optionally tied to a
//! terminal, made and changed by the person on the canvas and by agents through these ops.
//!
//! Every agent role may call these (`relay_core::guardrail::OPEN_TO_EVERY_ROLE`): a design is
//! the person's to ask for, and every change carries its undo. Agents act on one only when asked.
//!
//! A design is pages of nodes (`crate::design`). Every change goes through `design.apply`, a
//! list of node edits run in order, all or none; it answers with the nodes as stored and the
//! edits that undo it, so the canvas's undo and `audit.undo` revert exactly what was done.
use crate::design::{Conflict, Expect, Node, NodeKind, NodeOp, Style, MAX_OPS};
use crate::ops::picture::PictureCredit;
use crate::registry::{Actors, Doors, OpEntry, OpMeta, Scope, Undo};
use crate::types::{Design, Id};
use crate::{op, Empty};
use std::collections::BTreeMap;

/// Most ids `design.get`'s `nodes` names.
pub const GET_NODES_MAX: usize = 500;
/// Most matches `design.find` returns whole with `with_nodes`, and the most of their JSON.
pub const FIND_NODES_MAX: usize = 200;
pub const FIND_BYTES_MAX: usize = 512 * 1024;
/// The largest piece of a picture one `design.asset.get` answers, decoded.
pub const ASSET_PIECE: usize = 3 * 1024 * 1024;
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
    /// changed or moved since, in this answer's usual order, each with its place in `indexes`,
    /// and `removed` the ids gone since. It reaches back 1000 revs (older is
    /// design.since_too_old: read it whole). With `page` or `nodes`, what changed in what they
    /// name, a changed node with what is inside it (unless it is the page or a node named), and
    /// in `left` what changed outside them.
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
    /// With `since_rev`: the ids removed since, in the order they went (everything a delete took
    /// is listed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed: Option<Vec<String>>,
    /// With `since_rev`: each of `nodes`' place among its siblings now, in the same order: 0 at
    /// the bottom, as an add's or a move's `index` counts it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexes: Option<Vec<u32>>,
    /// With `since_rev`: the ids changed since that this answer leaves out, with what is inside
    /// them: with `page` or `nodes`, those outside what it takes now (each may have moved out),
    /// and any this Relay cannot read. None was removed: drop any you hold.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub left: Vec<String>,
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

payload!(#[schemars(rename = "DesignFindIn")] FindIn {
    pub design_id: Id,
    /// Only on this page: its id, or its name when no other page has that name.
    pub page: Option<String>,
    /// Only inside this node (not the node itself).
    pub inside: Option<String>,
    /// Any of these kinds.
    pub kind: Option<Vec<NodeKind>>,
    /// Words in its name, any case.
    #[schemars(length(min = 1))]
    pub name: Option<String>,
    /// Words in its text, any case.
    #[schemars(length(min = 1))]
    pub text: Option<String>,
    /// Its fill as drawn (a `$name` colour or a style gives one too). `#RGB` or `#RRGGBB` matches
    /// any alpha; `#RRGGBBAA` matches exactly.
    pub fill: Option<String>,
    /// Its stroke as drawn, compared the same way.
    pub stroke: Option<String>,
    /// Its text colour (`color`) as drawn, compared the same way.
    pub color: Option<String>,
    /// This colour anywhere: fill, stroke, text colour, a shadow, a gradient's or stroke
    /// gradient's stop, or a run of its text.
    pub any_color: Option<String>,
    /// Its font family (or a run's), any case.
    #[schemars(length(min = 1))]
    pub font: Option<String>,
    /// Also return the matching nodes as stored: the first 200, up to 512 KiB.
    pub with_nodes: Option<bool>,
});
result!(#[schemars(rename = "DesignFindOut")] FindOut {
    /// Every match, in design.get's order. With no filter, every node but the pages.
    pub ids: Vec<String>,
    /// With `with_nodes`: the first matches as stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(serialize_with = "crate::design::write_nodes_opt", deserialize_with = "crate::design::read_nodes_opt")]
    pub nodes: Option<Vec<Node>>,
    /// With `with_nodes`: true when `nodes` stops before `ids` does.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cut: bool,
});
op!(Find, "design.find", FindIn => FindOut,
    OpMeta::query(Scope::Project, 12, "Find a design's nodes by kind, name, words, colour or font; returns their ids in paint order"));

payload!(#[schemars(rename = "DesignFontsIn")] FontsIn {
    /// Only families whose name holds these words, any case.
    pub name: Option<String>,
});
result!(#[schemars(rename = "DesignFontsOut")] FontsOut {
    /// Families a design's text can be drawn in, sorted. Geist, Geist Mono and Sora are always
    /// there.
    pub families: Vec<String>,
    /// false when no desktop app has reported its fonts: only those three are known.
    pub complete: bool,
});
op!(Fonts, "design.fonts", FontsIn => FontsOut,
    OpMeta::query(Scope::Global, 12, "The font families Relay's desktop app can draw a design's text in"));

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

payload!(#[schemars(rename = "DesignAssetGetIn")] AssetGetIn {
    pub design_id: Id,
    /// An image node's `src`, as `design.asset.add` gave it.
    pub src: String,
    /// Where this piece starts, in bytes (default 0): the `next` of the piece before.
    pub offset: Option<u64>,
});
result!(#[schemars(rename = "DesignAssetGetOut")] AssetGetOut {
    /// `image/png`, `image/jpeg`, `image/gif`, `image/webp` or `image/svg+xml`, from its name.
    pub media_type: String,
    /// The whole picture's length in bytes.
    pub size: u64,
    /// This piece, base64: at most 3 MiB decoded.
    pub bytes_b64: String,
    /// Where the next piece starts; null on the last.
    pub next: Option<u64>,
});
op!(AssetGet, "design.asset.get", AssetGetIn => AssetGetOut,
    OpMeta::query(Scope::Project, 12, "A picture an image node shows, in pieces of at most 3 MiB, to draw it where its file is out of reach")
        .actors(Actors::UserOnly));

/// `design.render`'s limits: a picture's scale, its longer side, the box rows, the wait, the
/// nodes named, the frames of a contact sheet.
pub const RENDER_SCALE_MIN: f64 = 0.25;
pub const RENDER_SCALE_MAX: f64 = 2.0;
pub const RENDER_SIDE_MIN: u32 = 64;
pub const RENDER_SIDE_MAX: u32 = 2560;
pub const RENDER_SIDE_DEFAULT: u32 = 1568;
pub const RENDER_BOXES_MAX: u32 = 500;
pub const RENDER_BOXES_DEFAULT: u32 = 200;
pub const RENDER_TIMEOUT_MIN_MS: u64 = 2_000;
pub const RENDER_TIMEOUT_MAX_MS: u64 = 60_000;
pub const RENDER_TIMEOUT_DEFAULT_MS: u64 = 20_000;
pub const RENDER_NODES_MAX: usize = 200;
pub const RENDER_FRAMES_MIN: u8 = 2;
pub const RENDER_FRAMES_MAX: u8 = 24;
/// Most font families the desktop app reports with `design.render.serve`.
pub const SERVE_FONTS_MAX: usize = 2_000;
/// The columns of every `design.render` box row, in order: what `RenderBoxes.fields` holds. The
/// desktop app builds its rows with them and the engine refuses an answer without them.
pub const RENDER_BOX_FIELDS: [&str; 10] = ["id", "kind", "parent", "x", "y", "w", "h", "rx", "ry", "rot"];
/// Most pixels in a `design.render` picture, width times height (4 megapixels).
pub const RENDER_AREA_PX_MAX: u64 = 4_000_000;

payload!(#[schemars(rename = "DesignRenderIn")] RenderIn {
    pub design_id: Id,
    /// The page to picture: its id, or its name when no other page has that name. Default: the
    /// design's first page; with `nodes`, the page they are on.
    pub page: Option<String>,
    /// Picture and measure only these nodes (the outermost of them) and what is inside them.
    /// At most 200, all on one page.
    #[schemars(length(max = RENDER_NODES_MAX))]
    pub nodes: Option<Vec<String>>,
    /// A picture as well as the boxes. Default true; false gives the boxes and issues alone.
    pub png: Option<bool>,
    /// Picture pixels per page px, 0.25 to 2. Default 1. Lowered to fit `max_side` and 4
    /// megapixels.
    #[schemars(range(min = RENDER_SCALE_MIN, max = RENDER_SCALE_MAX))]
    pub scale: Option<f64>,
    /// The picture's longer side at most, 64 to 2560 px. Default 1568.
    #[schemars(range(min = RENDER_SIDE_MIN, max = RENDER_SIDE_MAX))]
    pub max_side: Option<u32>,
    /// Box rows in the answer, 0 to 500. Default 200. `boxes.total` counts every box.
    #[schemars(range(max = RENDER_BOXES_MAX))]
    pub boxes_limit: Option<u32>,
    /// Refuse (`design.stale`) unless the design is still at this `rev`: the `rev` your last
    /// design.apply answered.
    pub base_rev: Option<i64>,
    /// How long to wait for the desktop app, 2000 to 60000 ms. Default 20000.
    #[schemars(range(min = RENDER_TIMEOUT_MIN_MS, max = RENDER_TIMEOUT_MAX_MS))]
    pub timeout_ms: Option<u64>,
    /// Picture it at this time of its motion, in ms from the start (0 to 600000), each node posed
    /// on its own timeline. Default: at rest, as stored. Not with `frames`.
    #[schemars(range(min = 0, max = crate::design::motion::MAX_MS))]
    pub at_ms: Option<f64>,
    /// A contact sheet of this many frames (2 to 24), evenly spaced over the longest timeline in
    /// scope, each labelled with its time (`times_ms`). Not with `at_ms`.
    #[schemars(range(min = RENDER_FRAMES_MIN, max = RENDER_FRAMES_MAX))]
    pub frames: Option<u8>,
});
result!(#[schemars(rename = "DesignRenderOut")] RenderOut {
    pub design_id: Id,
    /// The rev the picture and the boxes show.
    pub rev: i64,
    /// The page's id.
    pub page: String,
    /// The page rectangle pictured, in page px: the page's content with 32 px around it, or the
    /// nodes' painted bounds, widened to whole px. Picture pixel (u, v) is page point
    /// (x + u / scale, y + v / scale); on a contact sheet, in each cell.
    pub area: RenderRect,
    /// Picture pixels per page px, after the size limits.
    pub scale: f64,
    /// Absent with `png: false`, or when nothing in scope is visible (a warning says which).
    pub png: Option<RenderPng>,
    /// Layout problems, worst first, at most 100.
    pub issues: Vec<RenderIssue>,
    /// Every problem found, listed or not.
    pub issue_count: u32,
    /// What to know that is not an error, at most 20; each says what to do next.
    pub warnings: Vec<String>,
    pub boxes: RenderBoxes,
    pub timings_ms: RenderTimings,
    /// A contact sheet's frame times in ms, in reading order; empty for one picture.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub times_ms: Vec<f64>,
});
result!(#[schemars(rename = "DesignRenderRect")] RenderRect { pub x: f64, pub y: f64, pub w: f64, pub h: f64 });
result!(#[schemars(rename = "DesignRenderPng")] RenderPng {
    /// The PNG on this machine. The newest 32 renders are kept, none older than an hour once a
    /// newer one is written.
    pub path: String,
    pub w: u32,
    pub h: u32,
});
result!(#[schemars(rename = "DesignRenderBoxes")] RenderBoxes {
    /// Always ["id","kind","parent","x","y","w","h","rx","ry","rot"].
    pub fields: Vec<String>,
    /// One row per visible node in scope, in design.get's order (parents before children,
    /// siblings bottom to top). x, y, w, h: the laid-out box in page px, hugged text measured.
    /// rx, ry: its corner from the parent's origin. rot: degrees as stored (boxes are unrotated).
    /// At rest, whatever `at_ms` asked.
    pub rows: Vec<Vec<serde_json::Value>>,
    /// Visible nodes in scope, listed or not.
    pub total: u32,
    /// Nodes in scope that are hidden, so have no box.
    pub hidden: u32,
});
result!(#[schemars(rename = "DesignRenderIssue")] RenderIssue {
    /// overflow, overlap, text_overlap, clipped or contrast.
    pub code: String,
    pub ids: Vec<String>,
    pub message: String,
});
result!(#[schemars(rename = "DesignRenderTimings")] RenderTimings { pub read: f64, pub layout: f64, pub paint: f64, pub encode: f64 });

op!(Render, "design.render", RenderIn => RenderOut,
    OpMeta::query(Scope::Project, 12, "See a design: a PNG of a page or some of its nodes (shown to you as an image), at rest, at a time of its motion or as a contact sheet of frames, and every node's laid-out box in page px, hugged text measured, with layout issues; drawn by Relay's desktop app, which must be open")
        .doors(Doors::SocketOnly));

payload!(#[schemars(rename = "DesignRenderServeIn")] ServeIn {
    /// The font families this app can draw, at most 2000, each 1 to 200 characters: what
    /// design.fonts answers and design.apply's font warnings read.
    #[schemars(length(max = SERVE_FONTS_MAX), inner(length(min = 1, max = crate::design::MAX_NAME)))]
    pub fonts: Option<Vec<String>>,
});
op!(RenderServe, "design.render.serve", ServeIn => Empty,
    OpMeta::query(Scope::Global, 12, "The desktop app offers to draw designs for agents on this connection, until it closes")
        .actors(Actors::UserOnly).doors(Doors::SocketOnly));

payload!(#[schemars(rename = "DesignRenderAnswerIn")] AnswerIn {
    /// The request's `render_id`.
    pub render_id: String,
    /// What was drawn. Exactly one of `drawn` and `error`.
    pub drawn: Option<RenderDrawn>,
    pub error: Option<RenderFailure>,
});
result!(#[schemars(rename = "DesignRenderDrawn")] RenderDrawn {
    pub rev: i64,
    pub area: RenderRect,
    pub scale: f64,
    /// The PNG, base64, at most 4 MiB decoded. Absent without a picture.
    pub png_b64: Option<String>,
    pub boxes: RenderBoxes,
    pub issues: Vec<RenderIssue>,
    pub issue_count: u32,
    pub warnings: Vec<String>,
    pub timings_ms: RenderTimings,
    /// A contact sheet's frame times in ms, in reading order (at most 24); empty for one
    /// picture.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub times_ms: Vec<f64>,
});
result!(#[schemars(rename = "DesignRenderFailure")] RenderFailure { pub code: String, pub message: String });
result!(#[schemars(rename = "DesignRenderAnswerOut")] AnswerOut {
    /// Whether a render was still waiting for this answer (it then got it, drawn or refused).
    pub accepted: bool,
});
op!(RenderAnswer, "design.render.answer", AnswerIn => AnswerOut,
    OpMeta::query(Scope::Global, 12, "The desktop app's answer to one design.render request")
        .actors(Actors::UserOnly));

/// The payload of the `design.render.request` line the engine writes to the leased connection
/// (not an op, not an event on the bus; BUS.md §6.2).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RenderRequest {
    pub render_id: String,
    pub design_id: Id,
    /// Resolved by the engine to the page's id: never absent.
    pub page: String,
    pub nodes: Option<Vec<String>>,
    pub png: bool,
    pub scale: f64,
    pub max_side: u32,
    pub boxes_limit: u32,
    /// Unix ms after which the engine no longer waits: skip the job, or the rest of it.
    pub deadline_unix_ms: i64,
    /// Pose each node at this time of its motion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at_ms: Option<f64>,
    /// A contact sheet of this many frames.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames: Option<u8>,
    /// The agent session that asked, for the note an open board shows; none for the person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
}
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
        OpEntry::of::<Find>(),
        OpEntry::of::<Fonts>(),
        OpEntry::of::<AssetAdd>(),
        OpEntry::of::<AssetGet>(),
        OpEntry::of::<Render>(),
        OpEntry::of::<RenderServe>(),
        OpEntry::of::<RenderAnswer>(),
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
        assert!(doc(&schema::<AssetGetOut>(), "bytes_b64").contains(&format!("at most {} MiB", ASSET_PIECE >> 20)));
        assert!(<AssetGet as crate::registry::Op>::META.summary.contains(&format!("{} MiB", ASSET_PIECE >> 20)));
        let get = schema::<GetIn>();
        assert_eq!(get["properties"]["nodes"]["maxItems"].as_u64(), Some(GET_NODES_MAX as u64));
        assert!(doc(&get, "nodes").contains(&format!("1 to {GET_NODES_MAX} ids")));
        assert!(doc(&get, "since_rev").contains(&format!("{} revs", crate::design::SINCE_WINDOW)));
        assert!(doc(&schema::<FindIn>(), "with_nodes").contains(&format!("first {FIND_NODES_MAX}, up to {} KiB", FIND_BYTES_MAX >> 10)));
        assert!(doc(&schema::<GetOut>(), "credits").contains(&format!("at most {CREDITS_MAX}")));
        let render = schema::<RenderIn>();
        for (field, lo, hi) in [("scale", RENDER_SCALE_MIN, RENDER_SCALE_MAX), ("max_side", RENDER_SIDE_MIN.into(), RENDER_SIDE_MAX.into()),
            ("timeout_ms", RENDER_TIMEOUT_MIN_MS as f64, RENDER_TIMEOUT_MAX_MS as f64), ("frames", RENDER_FRAMES_MIN.into(), RENDER_FRAMES_MAX.into()),
            ("at_ms", 0.0, crate::design::motion::MAX_MS)] {
            let property = &render["properties"][field];
            assert_eq!((property["minimum"].as_f64(), property["maximum"].as_f64()), (Some(lo), Some(hi)), "{field}");
            assert!(doc(&render, field).contains(&format!("{lo} to {hi}")), "{field}: {}", doc(&render, field));
        }
        assert_eq!(render["properties"]["boxes_limit"]["maximum"].as_u64(), Some(RENDER_BOXES_MAX.into()));
        assert_eq!(render["properties"]["nodes"]["maxItems"].as_u64(), Some(RENDER_NODES_MAX as u64));
        assert!(doc(&render, "max_side").contains(&format!("Default {RENDER_SIDE_DEFAULT}")));
        assert!(doc(&render, "boxes_limit").contains(&format!("Default {RENDER_BOXES_DEFAULT}")));
        assert!(doc(&render, "timeout_ms").contains(&format!("Default {RENDER_TIMEOUT_DEFAULT_MS}")));
        let fields = serde_json::to_string(&RENDER_BOX_FIELDS).unwrap();
        assert!(doc(&schema::<RenderBoxes>(), "fields").contains(&format!("Always {fields}")), "the rows' columns are the ones the engine holds an answer to");
        let serve = schema::<ServeIn>();
        assert_eq!(serve["properties"]["fonts"]["maxItems"].as_u64(), Some(SERVE_FONTS_MAX as u64));
        assert_eq!(serve["properties"]["fonts"]["items"]["maxLength"].as_u64(), Some(crate::design::MAX_NAME as u64));
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
