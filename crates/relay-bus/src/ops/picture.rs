//! `design.picture.*` (pictures-SPEC-v2 §5): stock photos for a design's image nodes, from
//! Openverse (no key) or Pexels (with the key the person saved), with their credits.
//!
//! Search, thumb and add come with the Pictures plugin (`.plugin("pictures")`): an agent may call
//! them only in a project with it on. The Pexels key is machine-wide and the person's alone:
//! `key_set` and `key_remove` carry no plugin and admit no agent.
use crate::registry::{Actors, Audit, OpMeta, Scope};
use crate::types::Id;
use crate::{op, Empty};

/// The plugin the picture ops come with.
pub const PLUGIN: &str = "pictures";

/// Where a stock photo comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignPictureSource")]
pub enum PictureSource {
    /// Openverse's openly licensed photos; needs no key.
    Openverse,
    /// Pexels; needs the key the person saved.
    Pexels,
}

/// A photo's shape, to search for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignPictureOrientation")]
pub enum Orientation {
    Landscape,
    Portrait,
    Square,
}

result!(#[schemars(rename = "DesignPictureHit")] PictureHit {
    /// `openverse:<uuid>` or `pexels:<digits>`: pass it to design.picture.add or .thumb.
    pub id: String,
    /// May be empty; at most 200 characters.
    pub title: String,
    pub creator: Option<String>,
    /// "CC BY 2.0", "CC BY-SA 4.0", "CC0", "Public domain" or "Pexels License".
    pub licence: String,
    /// The original's size in px, for its shape.
    pub w: u32,
    pub h: u32,
});

result!(#[schemars(rename = "DesignPictureCredit")] PictureCredit {
    pub source: PictureSource,
    /// The id design.picture.search gave it.
    pub id: String,
    pub title: String,
    pub creator: Option<String>,
    /// https only.
    pub creator_url: Option<String>,
    pub licence: String,
    /// https only.
    pub licence_url: Option<String>,
    /// The photo's page, https only.
    pub source_url: Option<String>,
    /// One sentence naming its title, creator, source and licence.
    pub attribution: String,
    /// The line to put under the photo.
    pub caption: String,
});

payload!(#[schemars(rename = "DesignPictureSearchIn")] SearchIn {
    pub design_id: Id,
    /// Two to four concrete nouns ("sunlit kitchen plants"); at most 200 characters.
    #[schemars(length(min = 1, max = 200))]
    pub query: String,
    /// Default openverse (no key). pexels needs the key the person saved.
    pub source: Option<PictureSource>,
    pub orientation: Option<Orientation>,
    /// 1 to 10; default 1.
    #[schemars(range(min = 1, max = 10))]
    pub page: Option<u32>,
    /// 1 to 20; default 12; larger asks are cut to 20.
    #[schemars(range(min = 1))]
    pub count: Option<u32>,
});
result!(#[schemars(rename = "DesignPictureSearchOut")] SearchOut {
    pub source: PictureSource,
    pub page: u32,
    /// Whether `page + 1` would answer (never past page 10).
    pub has_more: bool,
    pub results: Vec<PictureHit>,
});
op!(Search, "design.picture.search", SearchIn => SearchOut,
    OpMeta::query(Scope::Project, 12, "Search free stock photos for a design (Openverse; Pexels with the person's key); answers ids for design.picture.add")
        .plugin(PLUGIN));

payload!(#[schemars(rename = "DesignPictureThumbIn")] ThumbIn {
    pub design_id: Id,
    /// An id design.picture.search gave.
    pub id: String,
});
result!(#[schemars(rename = "DesignPictureThumbOut")] ThumbOut {
    /// A small JPEG, PNG, WebP or GIF on this machine. Through `relay mcp` the picture itself is
    /// attached to the answer; otherwise read the file.
    pub path: String,
    pub w: u32,
    pub h: u32,
});
op!(Thumb, "design.picture.thumb", ThumbIn => ThumbOut,
    OpMeta::query(Scope::Project, 12, "A search result's small preview, to look at before choosing; through MCP the picture comes with the answer")
        .plugin(PLUGIN));

payload!(#[schemars(rename = "DesignPictureAddIn")] AddIn {
    pub design_id: Id,
    /// An id design.picture.search gave.
    pub id: String,
});
result!(#[schemars(rename = "DesignPictureAddOut")] AddOut {
    /// What an image node's `src` names.
    pub src: String,
    pub path: String,
    /// The stored picture's size in px: give an image node this `w` and `h`.
    pub w: u32,
    pub h: u32,
    /// Where it came from and the line to put under it; design.get's `credits` keeps it by `src`.
    pub credit: PictureCredit,
});
op!(Add, "design.picture.add", AddIn => AddOut,
    OpMeta::mutation(Scope::Project, 12, "Store a photo from design.picture.search for a design, with its credit; answers the src for an image node, as design.asset.add does")
        .plugin(PLUGIN));

payload!(#[schemars(rename = "DesignPictureKeySetIn")] KeySetIn {
    /// The Pexels API key (pexels.com/api).
    #[schemars(length(min = 1, max = 200))]
    pub key: String,
});
result!(#[schemars(rename = "DesignPictureKeyOut")] KeyOut {
    /// Whether a key is saved now.
    pub saved: bool,
});
op!(KeySet, "design.picture.key_set", KeySetIn => KeyOut,
    OpMeta::mutation(Scope::Global, 12, "Check a Pexels API key with Pexels and keep it in the system keyring")
        // The payload is the key itself: an audit row would keep it in the store and every backup.
        .audit(Audit::Never).actors(Actors::UserOnly));
op!(KeyRemove, "design.picture.key_remove", Empty => KeyOut,
    OpMeta::mutation(Scope::Global, 12, "Forget the saved Pexels key").actors(Actors::UserOnly));

entries!(Search, Thumb, Add, KeySet, KeyRemove);
