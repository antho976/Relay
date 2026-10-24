//! Editor glyphs the shared icon set does not carry, drawn on the same 16px grid and
//! painted by the same painter and cache (`icons::from_geometry`). Shared names are delegated
//! to `icons::image`.
use gtk4 as gtk;

fn geometry(name: &str) -> Option<&'static str> {
    Some(match name {
        "chevron-up" => r##"<path d="M4 10l4-4 4 4" />"##,
        "list-numbered" => {
            r##"<path d="M6.5 4h7.5M6.5 8h7.5M6.5 12h7.5" /><path d="M2 3l1.25-.75V5.5" stroke-width="1.1" /><path d="M1.75 10.25a1 1 0 0 1 2 .2c0 .8-2 1.6-2 2.3h2.1" stroke-width="1.1" />"##
        }
        "link" => {
            r##"<path d="M6.5 9.5l3-3" /><path d="M7.25 4.75l1.5-1.5a2.47 2.47 0 0 1 3.5 3.5l-1.5 1.5" /><path d="M8.75 11.25l-1.5 1.5a2.47 2.47 0 0 1-3.5-3.5l1.5-1.5" />"##
        }
        "code-inline" => r##"<path d="M5.5 4.5L2 8l3.5 3.5M10.5 4.5L14 8l-3.5 3.5" />"##,
        _ => return None,
    })
}

/// The toolbar's set, in the KWrite (Breeze) manner: one-pixel lines on the half-pixel grid,
/// square corners and mitred joins, so each key reads crisp at 16px. A name without its own
/// drawing here falls back to `geometry`, drawn the same way.
fn crisp(name: &str) -> Option<&'static str> {
    Some(match name {
        "note-new" => r##"<path d="M8.5 14.5h-5v-13h6l3 3v4.5" /><path d="M12.5 10v5M10 12.5h5" />"##,
        // A floppy: clipped corner, shutter with its notch, label.
        "save" => {
            r##"<path d="M2.5 2.5h9l2 2v9h-11z" /><path d="M5 2.5v3.5h5.5V2.5" /><path d="M8.5 3.5V5" /><path d="M4.5 13.5V9.5h7v4" />"##
        }
        "undo" => r##"<path d="M3.5 6.5H10a3 3 0 0 1 0 6H6.5" /><path d="M6 4L3.5 6.5 6 9" />"##,
        "redo" => r##"<path d="M12.5 6.5H6a3 3 0 0 0 0 6h3.5" /><path d="M10 4l2.5 2.5L10 9" />"##,
        "cut" => {
            r##"<circle cx="4.5" cy="12" r="2" /><circle cx="11.5" cy="12" r="2" /><path d="M5.6 10.3L11 1.5M10.4 10.3L5 1.5" />"##
        }
        "copy" => r##"<path d="M5.5 3.5v-2h5l3 3v7h-2" /><path d="M2.5 4.5h6l3 3v7h-9z" />"##,
        "paste" => {
            r##"<path d="M5 3.5H3.5v11h9v-11H11" /><path d="M5.5 1.5h5v3h-5z" /><path d="M5.5 7.5h5M5.5 9.5h5M5.5 11.5h3" />"##
        }
        "search" => r##"<circle cx="6.5" cy="6.5" r="4.5" /><path d="M10 10l4.5 4.5" />"##,
        "replace" => {
            r##"<circle cx="6" cy="6" r="3.75" /><path d="M8.75 8.75L11 11" /><path d="M8.5 14.5h6M12.5 12.5l2 2-2 2" />"##
        }
        "sidebar" => r##"<path d="M1.5 2.5h13v11h-13z" /><path d="M5.5 2.5v11" />"##,
        "image" => {
            r##"<path d="M1.5 2.5h13v11h-13z" /><circle cx="5" cy="6" r="1.25" /><path d="M1.5 11.5l4-4 3 3 2-2 4 4" />"##
        }
        "task" => r##"<path d="M2.5 2.5h11v11h-11z" /><path d="M5 8l2 2 4-4.5" />"##,
        "list" => {
            r##"<path d="M6 4.5h8M6 8.5h8M6 12.5h8" /><path d="M2 4h1.5v1.5H2zM2 8h1.5v1.5H2zM2 12h1.5v1.5H2z" fill="currentColor" stroke="none" />"##
        }
        "checklist" => r##"<path d="M1.5 2.5h4v4h-4z" /><path d="M1.5 11l1.5 1.5 3-3.5" /><path d="M8 4.5h6M8 11.5h6" />"##,
        "quote" => r##"<path d="M2.5 3v10" /><path d="M5.5 4.5h8M5.5 7.5h8M5.5 10.5h5" />"##,
        "code-block" => r##"<path d="M1.5 2.5h13v11h-13z" /><path d="M6.5 6L4.5 8l2 2M9.5 6l2 2-2 2" />"##,
        "plus" => r##"<path d="M8 2.5v11M2.5 8h11" />"##,
        "sort" => r##"<path d="M4.5 2.5v11M2 11l2.5 2.5L7 11" /><path d="M9 4.5h5M9 8.5h4M9 12.5h2.5" />"##,
        // Format: a letter A over a baseline, the key for bold, italic, headings and code.
        "format" => r##"<path d="M3.5 11.5L7.25 2.5h1.5l3.75 9" /><path d="M5.25 8h5.5" /><path d="M2.5 14.5h11" />"##,
        _ => return None,
    })
}

/// A toolbar key's icon (see `crisp`).
pub fn tool_glyph(name: &str, size: i32) -> gtk::Image {
    let Some(geometry) = crisp(name).or_else(|| geometry(name)) else {
        return crate::icons::image_with_stroke(name, size, 1.0);
    };
    crate::icons::from_geometry(&format!(r#"<g stroke-linecap="butt" stroke-linejoin="miter">{geometry}</g>"#), size, 1.0)
}

pub fn glyph(name: &str, size: i32) -> gtk::Image {
    glyph_stroke(name, size, 1.5)
}

/// `glyph` at another stroke.
fn glyph_stroke(name: &str, size: i32, stroke: f64) -> gtk::Image {
    let Some(geometry) = geometry(name) else {
        return crate::icons::image_with_stroke(name, size, stroke);
    };
    crate::icons::from_geometry(geometry, size, stroke)
}
