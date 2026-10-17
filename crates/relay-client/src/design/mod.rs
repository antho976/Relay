//! The design canvas's GTK-free half. `scene` holds a design's nodes as a tree and applies the
//! same edits the engine does, so the canvas shows an edit before the engine answers; `layout`
//! places every node of a page (auto layout, hugging text) given a text measurer; `hit` finds
//! what is under the pointer; `snap` lines a moving box up with its neighbours; `edit` builds
//! the `design.apply` edits behind duplicate, paste, group, frame, align and reparent, and `flip`
//! those behind flip and copy style; `shapes` is polygons, arcs, smooth lines, rotation and
//! connectors; `icons` the icon set; `geom` is rectangles and the camera.
pub mod edit;
pub mod flip;
pub mod geom;
pub mod hit;
pub mod icons;
pub mod layout;
pub mod scene;
pub mod shapes;
pub mod snap;

pub use geom::{Camera, Rect};
pub use relay_bus::design::{Node, NodeKind, NodeOp};
pub use scene::Scene;
