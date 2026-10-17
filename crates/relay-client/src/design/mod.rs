//! The design canvas's GTK-free half. `scene` holds a design's nodes as a tree and applies the
//! same edits the engine does, so the canvas shows an edit before the engine answers; `layout`
//! places every node of a page (auto layout, hugging text) given a text measurer; `hit` finds
//! what is under the pointer; `snap` lines a moving box up with its neighbours; `edit` builds
//! the `design.apply` edits behind duplicate, paste, group, frame, align and reparent; `geom`
//! is rectangles and the camera.
pub mod edit;
pub mod geom;
pub mod hit;
pub mod layout;
pub mod scene;
pub mod snap;

pub use geom::{Camera, Rect};
pub use relay_bus::design::{Node, NodeKind, NodeOp};
pub use scene::Scene;
