//! The canvas's tool and property glyphs, on the shell icons' 16-unit grid and 1.5 strokes
//! (`icons::from_geometry`), so the dock reads as part of Relay.

use gtk4 as gtk;

pub fn geometry(name: &str) -> &'static str {
    match name {
        "pointer" => r##"<path d="M4 2.5l8.5 4.5-3.8 1.2-1.4 3.8z"/>"##,
        "hand" => r##"<path d="M5.6 8.2V4.1a1 1 0 0 1 2 0v3.6M7.6 7.4V3a1 1 0 0 1 2 0v4.6M9.6 7.6V4a1 1 0 0 1 2 0v5.2c0 2.5-1.7 4.3-4.1 4.3-1.6 0-2.6-.7-3.5-2L2.6 9.2a1 1 0 0 1 1.6-1.2l1.4 1.6"/>"##,
        "frame" => r##"<path d="M5.5 2v12M10.5 2v12M2 5.5h12M2 10.5h12"/>"##,
        "rect" => r##"<rect x="2.5" y="3.5" width="11" height="9" rx="1.5"/>"##,
        "ellipse" => r##"<circle cx="8" cy="8" r="5.5"/>"##,
        "line" => r##"<path d="M3 13L13 3"/>"##,
        "arrow" => r##"<path d="M3 13L13 3M7.5 3H13v5.5"/>"##,
        "text" => r##"<path d="M3 4.5V3h10v1.5M8 3v10M6 13h4"/>"##,
        "sticky" => r##"<path d="M3 3h10v6.5L9.5 13H3z"/><path d="M13 9.5H9.5V13"/>"##,
        "pen" => r##"<path d="M10.5 2.5l3 3-8 8H2.5v-3z"/><path d="M9 4l3 3"/>"##,
        "highlighter" => r##"<path d="M9.5 2.5l4 4-5 5-4-4z"/><path d="M4.5 7.5L3 12l1 1 4.5-1.5"/>"##,
        "image" => r##"<rect x="2.5" y="3" width="11" height="10" rx="1.5"/><path d="M2.5 11l3.5-3.5 3 3 2-2 2.5 2.5"/><circle cx="10.5" cy="6" r="1"/>"##,
        "layers" => r##"<path d="M8 2.5l5.5 3L8 8.5l-5.5-3z"/><path d="M2.5 8.5L8 11.5l5.5-3"/><path d="M2.5 11.2L8 14.2l5.5-3"/>"##,
        "spark" => r##"<path d="M7 2.5l1.2 3.3 3.3 1.2-3.3 1.2L7 11.5 5.8 8.2 2.5 7l3.3-1.2z"/><path d="M12.5 10v4M10.5 12h4"/>"##,
        "window" => r##"<rect x="2.5" y="4.5" width="9" height="9" rx="1.5"/><path d="M8.5 2.5h5v5M13.5 2.5L8 8"/>"##,
        "align-left" => r##"<path d="M2.5 2v12"/><rect x="5" y="4" width="8" height="3" rx="1"/><rect x="5" y="9" width="5" height="3" rx="1"/>"##,
        "align-center-x" => r##"<path d="M8 2v12"/><rect x="3.5" y="4" width="9" height="3" rx="1"/><rect x="5" y="9" width="6" height="3" rx="1"/>"##,
        "align-right" => r##"<path d="M13.5 2v12"/><rect x="3" y="4" width="8" height="3" rx="1"/><rect x="6" y="9" width="5" height="3" rx="1"/>"##,
        "align-top" => r##"<path d="M2 2.5h12"/><rect x="4" y="5" width="3" height="8" rx="1"/><rect x="9" y="5" width="3" height="5" rx="1"/>"##,
        "align-center-y" => r##"<path d="M2 8h12"/><rect x="4" y="3.5" width="3" height="9" rx="1"/><rect x="9" y="5" width="3" height="6" rx="1"/>"##,
        "align-bottom" => r##"<path d="M2 13.5h12"/><rect x="4" y="3" width="3" height="8" rx="1"/><rect x="9" y="6" width="3" height="5" rx="1"/>"##,
        "distribute-x" => r##"<path d="M2.5 2v12M13.5 2v12"/><rect x="6.5" y="4.5" width="3" height="7" rx="1"/>"##,
        "distribute-y" => r##"<path d="M2 2.5h12M2 13.5h12"/><rect x="4.5" y="6.5" width="7" height="3" rx="1"/>"##,
        "text-left" => r##"<path d="M2.5 4h11M2.5 8h7M2.5 12h9"/>"##,
        "text-center" => r##"<path d="M2.5 4h11M4.5 8h7M3.5 12h9"/>"##,
        "text-right" => r##"<path d="M2.5 4h11M6.5 8h7M4.5 12h9"/>"##,
        "row" => r##"<rect x="2" y="5" width="3.5" height="6" rx="1"/><rect x="6.25" y="5" width="3.5" height="6" rx="1"/><rect x="10.5" y="5" width="3.5" height="6" rx="1"/>"##,
        "column" => r##"<rect x="5" y="2" width="6" height="3.5" rx="1"/><rect x="5" y="6.25" width="6" height="3.5" rx="1"/><rect x="5" y="10.5" width="6" height="3.5" rx="1"/>"##,
        "free" => r##"<rect x="2.5" y="2.5" width="5" height="4" rx="1"/><rect x="8.5" y="9.5" width="5" height="4" rx="1"/>"##,
        // Two rows, the second shorter: a row that breaks onto a new line.
        "wrap" => r##"<rect x="2" y="3" width="3.5" height="4" rx="1"/><rect x="6.25" y="3" width="3.5" height="4" rx="1"/><rect x="10.5" y="3" width="3.5" height="4" rx="1"/><rect x="2" y="9" width="3.5" height="4" rx="1"/><rect x="6.25" y="9" width="3.5" height="4" rx="1"/>"##,
        "lock" => r##"<rect x="3.5" y="7" width="9" height="6.5" rx="1.5"/><path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2"/>"##,
        "unlock" => r##"<rect x="3.5" y="7" width="9" height="6.5" rx="1.5"/><path d="M5.5 7V5a2.5 2.5 0 0 1 4.9-.7"/>"##,
        "eye" => r##"<path d="M1.5 8s2.4-4.5 6.5-4.5S14.5 8 14.5 8s-2.4 4.5-6.5 4.5S1.5 8 1.5 8z"/><circle cx="8" cy="8" r="2"/>"##,
        "eye-off" => r##"<path d="M2.5 2.5l11 11"/><path d="M6.4 3.8A6.6 6.6 0 0 1 8 3.5c4.1 0 6.5 4.5 6.5 4.5a11 11 0 0 1-1.6 2.2M10.5 11.8A6 6 0 0 1 8 12.5C3.9 12.5 1.5 8 1.5 8a11 11 0 0 1 2.3-2.9"/>"##,
        "group" => r##"<rect x="2" y="2" width="12" height="12" rx="2" stroke-dasharray="2 2"/><rect x="4.5" y="4.5" width="4" height="4" rx="1"/><rect x="8" y="8" width="3.5" height="3.5" rx="1"/>"##,
        "page" => r##"<path d="M4 2.5h5.5L12.5 5.5V13.5H4z"/><path d="M9.5 2.5v3h3"/>"##,
        "fit" => r##"<path d="M2.5 5.5v-3h3M10.5 2.5h3v3M13.5 10.5v3h-3M5.5 13.5h-3v-3"/>"##,
        "keyboard" => r##"<rect x="1.5" y="4" width="13" height="8.5" rx="1.5"/><path d="M4 6.8h.01M6.5 6.8h.01M9 6.8h.01M11.5 6.8h.01M4.5 9.8h7"/>"##,
        "padding" => r##"<rect x="2.5" y="2.5" width="11" height="11" rx="1.5"/><rect x="5.5" y="5.5" width="5" height="5" rx="0.5"/>"##,
        "gap" => r##"<rect x="2" y="3" width="4" height="10" rx="1"/><rect x="10" y="3" width="4" height="10" rx="1"/><path d="M8 5v6"/>"##,
        "radius" => r##"<path d="M3 13V8a5 5 0 0 1 5-5h5"/>"##,
        "stroke-width" => r##"<path d="M2.5 4h11"/><path d="M2.5 8h11" stroke-width="2.5"/><path d="M2.5 12.5h11" stroke-width="3.5"/>"##,
        "opacity" => r##"<circle cx="8" cy="8" r="5.5"/><path d="M8 2.5v11"/><path d="M8 5h3.5M8 8h5.5M8 11h3.5"/>"##,
        "triangle" => r##"<path d="M8 2.5l6 10.5H2z"/>"##,
        "diamond" => r##"<path d="M8 2l6 6-6 6-6-6z"/>"##,
        "hexagon" => r##"<path d="M8 2l5.2 3v6L8 14l-5.2-3V5z"/>"##,
        "star" => r##"<path d="M8 2l1.8 3.8 4.2.6-3 2.9.7 4.1L8 11.4l-3.7 2 .7-4.1-3-2.9 4.2-.6z"/>"##,
        "icon" => r##"<circle cx="8" cy="8" r="6"/><path d="M5.5 9.5s.9 1.5 2.5 1.5 2.5-1.5 2.5-1.5M6 6.3h.01M10 6.3h.01"/>"##,
        "connector" => r##"<rect x="1.5" y="2.5" width="5" height="4" rx="1"/><rect x="9.5" y="9.5" width="5" height="4" rx="1"/><path d="M4 6.5v5h5.5"/>"##,
        "eraser" => r##"<path d="M6 13.5h7.5M3.3 10.3l6.5-6.6a1.4 1.4 0 0 1 2 0l1.5 1.5a1.4 1.4 0 0 1 0 2L7 13.5H5.4z"/><path d="M6.5 7l3.5 3.5"/>"##,
        "rotate" => r##"<path d="M13 8a5 5 0 1 1-1.5-3.5"/><path d="M12 2v3h-3"/>"##,
        "gradient" => r##"<rect x="2.5" y="2.5" width="11" height="11" rx="2"/><path d="M2.5 9.5l7-7M5.5 13.5l8-8M9.5 13.5l4-4"/>"##,
        "dash-solid" => r##"<path d="M2 8h12"/>"##,
        "dots" => r##"<path d="M2.5 8h.01M5.25 8h.01M8 8h.01M10.75 8h.01M13.5 8h.01" stroke-width="2"/>"##,
        "dash" => r##"<path d="M2 8h2.5M6.75 8h2.5M11.5 8H14"/>"##,
        "blur" => r##"<circle cx="8" cy="8" r="5.5" stroke-dasharray="1.5 2"/><circle cx="8" cy="8" r="2.5"/>"##,
        "line-height" => r##"<path d="M2.5 2.5h11M2.5 13.5h11"/><path d="M5.8 11.2L8 4.8l2.2 6.4M6.6 9h2.8"/>"##,
        "letter-spacing" => r##"<path d="M2.5 3v10M13.5 3v10"/><path d="M5.6 11.2L8 4.8l2.4 6.4M6.4 9h3.2"/>"##,
        "flip-h" => r##"<path d="M8 2v12" stroke-dasharray="1.5 1.5"/><path d="M6 4.5L2.5 12H6zM10 4.5l3.5 7.5H10z"/>"##,
        "flip-v" => r##"<path d="M2 8h12" stroke-dasharray="1.5 1.5"/><path d="M4.5 6L12 2.5V6zM4.5 10l7.5 3.5V10z"/>"##,
        "vector" => r##"<path d="M3 12.5L6.5 4l6.5 3.5"/><rect x="1.8" y="11.3" width="2.4" height="2.4" rx=".4"/><rect x="5.3" y="2.8" width="2.4" height="2.4" rx=".4"/><rect x="11.8" y="6.3" width="2.4" height="2.4" rx=".4"/>"##,
        "arc" => r##"<path d="M8 2.5A5.5 5.5 0 1 1 2.5 8H8z"/>"##,
        "blend" => r##"<circle cx="6" cy="8" r="4"/><circle cx="10" cy="8" r="4"/>"##,
        "case" => r##"<path d="M1.8 12L4.8 4l3 8M2.9 9.3h3.8M10 7.6a1.9 1.9 0 0 1 3.8 0V12M13.8 9.6c-.6-.4-1.4-.5-2.1-.4-1 .1-1.7.7-1.7 1.5 0 .8.6 1.3 1.5 1.3 1 0 1.8-.6 2.3-1.4"/>"##,
        "copy-style" => r##"<path d="M3 3.5h8.5v3H3z"/><path d="M11.5 5h1.5v3.5H7.5V11"/><rect x="6.3" y="11" width="2.4" height="3" rx=".5"/>"##,
        _ => r##"<circle cx="8" cy="8" r="5"/>"##,
    }
}

/// Whether `name` is one of the canvas's glyphs.
pub fn known(name: &str) -> bool {
    !geometry(name).starts_with("<circle cx=\"8\" cy=\"8\" r=\"5\"/>")
}

/// A glyph of the canvas's set, or a shell icon by its name.
pub fn image(name: &str, size: i32) -> gtk::Image {
    if geometry(name).starts_with("<circle cx=\"8\" cy=\"8\" r=\"5\"/>") {
        crate::icons::image(name, size)
    } else {
        crate::icons::from_geometry(geometry(name), size, 1.5)
    }
}
