package com.tally.app.ui.kit

// Relay's glyphs, copied from antho976/RelayV4 apps/relay-android, which generates them from
// apps/relay-native/src/icons.rs. Copy them again rather than edit them here.
// Each glyph is drawn on the 16-unit grid: stroked unless marked filled, round caps and joins.

internal class Glyph(val scale: Float, val shapes: List<GlyphShape>)

internal class GlyphShape(val d: String, val fill: Boolean, val stroke: Boolean, val rotate: Float)

internal val GLYPHS: Map<String, Glyph> = buildMap {
    put("dashboard", Glyph(1.0f, listOf(
            GlyphShape("M3.25 2h2a1.25 1.25 0 0 1 1.25 1.25v2a1.25 1.25 0 0 1 -1.25 1.25h-2a1.25 1.25 0 0 1 -1.25 -1.25v-2a1.25 1.25 0 0 1 1.25 -1.25z", false, true, 0.0f),
            GlyphShape("M10.75 2h2a1.25 1.25 0 0 1 1.25 1.25v2a1.25 1.25 0 0 1 -1.25 1.25h-2a1.25 1.25 0 0 1 -1.25 -1.25v-2a1.25 1.25 0 0 1 1.25 -1.25z", false, true, 0.0f),
            GlyphShape("M3.25 9.5h2a1.25 1.25 0 0 1 1.25 1.25v2a1.25 1.25 0 0 1 -1.25 1.25h-2a1.25 1.25 0 0 1 -1.25 -1.25v-2a1.25 1.25 0 0 1 1.25 -1.25z", false, true, 0.0f),
            GlyphShape("M10.75 9.5h2a1.25 1.25 0 0 1 1.25 1.25v2a1.25 1.25 0 0 1 -1.25 1.25h-2a1.25 1.25 0 0 1 -1.25 -1.25v-2a1.25 1.25 0 0 1 1.25 -1.25z", false, true, 0.0f),
        )))
    put("skills", Glyph(1.0f, listOf(
            GlyphShape("M2.67 9.33a.67.67 0 0 1-.52-1.09l6.6-6.8a.33.33 0 0 1 .57.31l-1.28 4.01A.67.67 0 0 0 8.67 6.67h4.67a.67.67 0 0 1 .52 1.09l-6.6 6.8a.33.33 0 0 1-.57-.31l1.28-4.01A.67.67 0 0 0 7.33 9.33z", false, true, 0.0f),
        )))
    put("plugins", Glyph(1.0f, listOf(
            GlyphShape("M6.67 14.67V4.67a.67.67 0 0 0-.67-.67H2.67a1.33 1.33 0 0 0-1.34 1.33v8a1.33 1.33 0 0 0 1.34 1.34h8a1.33 1.33 0 0 0 1.33-1.34V10a.67.67 0 0 0-.67-.67H1.33", false, true, 0.0f),
            GlyphShape("M10 1.33h3.99a0.67 0.67 0 0 1 0.67 0.67v3.99a0.67 0.67 0 0 1 -0.67 0.67h-3.99a0.67 0.67 0 0 1 -0.67 -0.67v-3.99a0.67 0.67 0 0 1 0.67 -0.67z", false, true, 0.0f),
        )))
    put("bell", Glyph(1.0f, listOf(
            GlyphShape("M6.85 14a1.33 1.33 0 0 0 2.3 0", false, true, 0.0f),
            GlyphShape("M2.17 10.22A.67.67 0 0 0 2.67 11.33h10.67a.67.67 0 0 0 .49-1.12C12.94 9.3 12 8.33 12 5.33a4 4 0 0 0-8 0c0 3-.94 3.97-1.83 4.89", false, true, 0.0f),
        )))
    put("search", Glyph(1.0f, listOf(
            GlyphShape("M2.5 7a4.5 4.5 0 1 0 9 0a4.5 4.5 0 1 0 -9 0z", false, true, 0.0f),
            GlyphShape("M10.5 10.5L14 14", false, true, 0.0f),
        )))
    put("settings", Glyph(1.0f, listOf(
            GlyphShape("M2 3.33h4.67M9.33 3.33H14M9.33 2v2.67M2 8h3.33M8 8h6M5.33 6.67v2.67M2 12.67h6M10.67 12.67H14M10.67 11.33V14", false, true, 0.0f),
        )))
    put("sliders", Glyph(1.0f, listOf(
            GlyphShape("M2 3.33h4.67M9.33 3.33H14M9.33 2v2.67M2 8h3.33M8 8h6M5.33 6.67v2.67M2 12.67h6M10.67 12.67H14M10.67 11.33V14", false, true, 0.0f),
        )))
    put("gear", Glyph(1.0f, listOf(
            GlyphShape("M12.68 6.55L14.31 6.92 14.31 9.08 12.68 9.45 12.33 10.29 13.23 11.69 11.69 13.23 10.29 12.33 9.45 12.68 9.08 14.31 6.92 14.31 6.55 12.68 5.71 12.33 4.31 13.23 2.77 11.69 3.67 10.29 3.32 9.45 1.69 9.08 1.69 6.92 3.32 6.55 3.67 5.71 2.77 4.31 4.31 2.77 5.71 3.67 6.55 3.32 6.92 1.69 9.08 1.69 9.45 3.32 10.29 3.67 11.69 2.77 13.23 4.31 12.33 5.71z", false, true, 0.0f),
            GlyphShape("M6 8a2 2 0 1 0 4 0a2 2 0 1 0 -4 0z", false, true, 0.0f),
        )))
    put("close", Glyph(1.0f, listOf(
            GlyphShape("M4 4l8 8M12 4l-8 8", false, true, 0.0f),
        )))
    put("minimize", Glyph(1.0f, listOf(
            GlyphShape("M3.5 8h9", false, true, 0.0f),
        )))
    put("maximize", Glyph(1.0f, listOf(
            GlyphShape("M3.5 3.5h9v9h-9z", false, true, 0.0f),
        )))
    put("plus", Glyph(1.0f, listOf(
            GlyphShape("M8 3v10M3 8h10", false, true, 0.0f),
        )))
    put("file", Glyph(1.0f, listOf(
            GlyphShape("M4 2h5l3 3v9H4z", false, true, 0.0f),
            GlyphShape("M9 2v3h3", false, true, 0.0f),
        )))
    put("folder", Glyph(1.0f, listOf(
            GlyphShape("M2 4h4l1.5 1.5H14V13H2z", false, true, 0.0f),
        )))
    put("folder-open", Glyph(1.0f, listOf(
            GlyphShape("M2 4h4l1.5 1.5H13V7", false, true, 0.0f),
            GlyphShape("M2 13l1.5-5.5H15L13.5 13z", false, true, 0.0f),
        )))
    put("file-rust", Glyph(1.0f, listOf(
            GlyphShape("M4.25 8a3.75 3.75 0 1 0 7.5 0a3.75 3.75 0 1 0 -7.5 0z", false, true, 0.0f),
            GlyphShape("M6.75 8a1.25 1.25 0 1 0 2.5 0a1.25 1.25 0 1 0 -2.5 0z", false, true, 0.0f),
            GlyphShape("M8 1.75v2.5M8 11.75v2.5M1.75 8h2.5M11.75 8h2.5M3.6 3.6l1.75 1.75M10.65 10.65l1.75 1.75M3.6 12.4l1.75-1.75M10.65 5.35l1.75-1.75", false, true, 0.0f),
        )))
    put("file-ts", Glyph(1.0f, listOf(
            GlyphShape("M3.75 2.25h8.5a1.5 1.5 0 0 1 1.5 1.5v8.5a1.5 1.5 0 0 1 -1.5 1.5h-8.5a1.5 1.5 0 0 1 -1.5 -1.5v-8.5a1.5 1.5 0 0 1 1.5 -1.5z", false, true, 0.0f),
            GlyphShape("M4.75 7h3.5M6.5 7v4.5M12 7.5a1.4 1.4 0 0 0-1.3-.5c-.8 0-1.3.4-1.3 1s.5.9 1.3 1.1 1.3.5 1.3 1.1-.5 1.05-1.3 1.05a1.6 1.6 0 0 1-1.45-.65", false, true, 0.0f),
        )))
    put("file-js", Glyph(1.0f, listOf(
            GlyphShape("M3.75 2.25h8.5a1.5 1.5 0 0 1 1.5 1.5v8.5a1.5 1.5 0 0 1 -1.5 1.5h-8.5a1.5 1.5 0 0 1 -1.5 -1.5v-8.5a1.5 1.5 0 0 1 1.5 -1.5z", false, true, 0.0f),
            GlyphShape("M7.25 6.75v3.5a1.25 1.25 0 0 1-2.4.5M12 7.5a1.4 1.4 0 0 0-1.3-.5c-.8 0-1.3.4-1.3 1s.5.9 1.3 1.1 1.3.5 1.3 1.1-.5 1.05-1.3 1.05a1.6 1.6 0 0 1-1.45-.65", false, true, 0.0f),
        )))
    put("file-react", Glyph(1.0f, listOf(
            GlyphShape("M1.75 8a6.25 2.4 0 1 0 12.5 0a6.25 2.4 0 1 0 -12.5 0z", false, true, 0.0f),
            GlyphShape("M1.75 8a6.25 2.4 0 1 0 12.5 0a6.25 2.4 0 1 0 -12.5 0z", false, true, 60.0f),
            GlyphShape("M1.75 8a6.25 2.4 0 1 0 12.5 0a6.25 2.4 0 1 0 -12.5 0z", false, true, 120.0f),
            GlyphShape("M7.1 8a0.9 0.9 0 1 0 1.8 0a0.9 0.9 0 1 0 -1.8 0z", true, false, 0.0f),
        )))
    put("file-json", Glyph(1.0f, listOf(
            GlyphShape("M5.5 2.5c-1.5 0-1.75.9-1.75 2.25S3.5 7.6 2.25 8c1.25.4 1.5 1.9 1.5 3.25S4 13.5 5.5 13.5M10.5 2.5c1.5 0 1.75.9 1.75 2.25S12.5 7.6 13.75 8c-1.25.4-1.5 1.9-1.5 3.25S12 13.5 10.5 13.5", false, true, 0.0f),
        )))
    put("file-md", Glyph(1.0f, listOf(
            GlyphShape("M3 3.5h10a1.5 1.5 0 0 1 1.5 1.5v6a1.5 1.5 0 0 1 -1.5 1.5h-10a1.5 1.5 0 0 1 -1.5 -1.5v-6a1.5 1.5 0 0 1 1.5 -1.5z", false, true, 0.0f),
            GlyphShape("M4 10.25v-4.5l2 2.25 2-2.25v4.5M11 5.75v4.5M9.5 8.75L11 10.25l1.5-1.5", false, true, 0.0f),
        )))
    put("file-config", Glyph(1.0f, listOf(
            GlyphShape("M2.5 4.5h3M9.5 4.5h4M2.5 8h7M13 8h.5M2.5 11.5h1.5M7.5 11.5h6", false, true, 0.0f),
            GlyphShape("M6 4.5a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0 -3 0z", false, true, 0.0f),
            GlyphShape("M9.75 8a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0 -3 0z", false, true, 0.0f),
            GlyphShape("M4.25 11.5a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0 -3 0z", false, true, 0.0f),
        )))
    put("file-css", Glyph(1.0f, listOf(
            GlyphShape("M6.5 2.5L5 13.5M11.5 2.5L10 13.5M3 6h10.5M2.5 10H13", false, true, 0.0f),
        )))
    put("file-html", Glyph(1.0f, listOf(
            GlyphShape("M5.5 4L2 8l3.5 4M10.5 4L14 8l-3.5 4M9 3l-2 10", false, true, 0.0f),
        )))
    put("file-py", Glyph(1.0f, listOf(
            GlyphShape("M8 2.25H6.75A2 2 0 0 0 4.75 4.25V6h4.5M4.75 6h-.5A2 2 0 0 0 2.25 8v1.25a2 2 0 0 0 2 2H5.5V9.5a1.5 1.5 0 0 1 1.5-1.5h2.25a1.5 1.5 0 0 0 1.5-1.5V4.25a2 2 0 0 0-2-2H8", false, true, 0.0f),
            GlyphShape("M8 13.75h1.25a2 2 0 0 0 2-2V10h-4.5M11.25 10h.5a2 2 0 0 0 2-2V6.75a2 2 0 0 0-2-2H10.5", false, true, 0.0f),
            GlyphShape("M6.15 4a0.6 0.6 0 1 0 1.2 0a0.6 0.6 0 1 0 -1.2 0z", true, false, 0.0f),
            GlyphShape("M8.65 12a0.6 0.6 0 1 0 1.2 0a0.6 0.6 0 1 0 -1.2 0z", true, false, 0.0f),
        )))
    put("file-image", Glyph(1.0f, listOf(
            GlyphShape("M3.5 2.75h9a1.5 1.5 0 0 1 1.5 1.5v7.5a1.5 1.5 0 0 1 -1.5 1.5h-9a1.5 1.5 0 0 1 -1.5 -1.5v-7.5a1.5 1.5 0 0 1 1.5 -1.5z", false, true, 0.0f),
            GlyphShape("M4.5 6.25a1.25 1.25 0 1 0 2.5 0a1.25 1.25 0 1 0 -2.5 0z", false, true, 0.0f),
            GlyphShape("M2.5 12.25L6.25 8.5l2.5 2.5 2-2 3 3", false, true, 0.0f),
        )))
    put("file-lock", Glyph(1.0f, listOf(
            GlyphShape("M4.5 7h7a1.25 1.25 0 0 1 1.25 1.25v4.5a1.25 1.25 0 0 1 -1.25 1.25h-7a1.25 1.25 0 0 1 -1.25 -1.25v-4.5a1.25 1.25 0 0 1 1.25 -1.25z", false, true, 0.0f),
            GlyphShape("M5.5 7V5a2.5 2.5 0 0 1 5 0v2M8 9.75v1.5", false, true, 0.0f),
        )))
    put("file-shell", Glyph(1.0f, listOf(
            GlyphShape("M3.25 2.75h9.5a1.5 1.5 0 0 1 1.5 1.5v7.5a1.5 1.5 0 0 1 -1.5 1.5h-9.5a1.5 1.5 0 0 1 -1.5 -1.5v-7.5a1.5 1.5 0 0 1 1.5 -1.5z", false, true, 0.0f),
            GlyphShape("M4.5 6.25L6.75 8 4.5 9.75M8.5 10.25h3", false, true, 0.0f),
        )))
    put("file-git", Glyph(1.0f, listOf(
            GlyphShape("M8 1.75L14.25 8 8 14.25 1.75 8z", false, true, 0.0f),
            GlyphShape("M5.5 6.5a1 1 0 1 0 2 0a1 1 0 1 0 -2 0z", false, true, 0.0f),
            GlyphShape("M8.5 9.5a1 1 0 1 0 2 0a1 1 0 1 0 -2 0z", false, true, 0.0f),
            GlyphShape("M7.2 7.2l1.6 1.6M6.5 7.5v3", false, true, 0.0f),
        )))
    put("file-text", Glyph(1.0f, listOf(
            GlyphShape("M4 2h5l3 3v9H4z", false, true, 0.0f),
            GlyphShape("M9 2v3h3M6 8.25h4M6 10.75h4", false, true, 0.0f),
        )))
    put("file-plus", Glyph(1.0f, listOf(
            GlyphShape("M9 2H4v12h5M9 2l3 3v2.5M9 2v3h3", false, true, 0.0f),
            GlyphShape("M12 10v4M10 12h4", false, true, 0.0f),
        )))
    put("folder-plus", Glyph(1.0f, listOf(
            GlyphShape("M8.5 13H2V4h4l1.5 1.5H14V8.5", false, true, 0.0f),
            GlyphShape("M12 10v4M10 12h4", false, true, 0.0f),
        )))
    put("collapse", Glyph(1.0f, listOf(
            GlyphShape("M5 2.5l3 3 3-3M5 13.5l3-3 3 3M2.5 8h11", false, true, 0.0f),
        )))
    put("arrow-up", Glyph(1.0f, listOf(
            GlyphShape("M8 13V3M4 7l4-4 4 4", false, true, 0.0f),
        )))
    put("arrow-down", Glyph(1.0f, listOf(
            GlyphShape("M8 3v10M4 9l4 4 4-4", false, true, 0.0f),
        )))
    put("cloud", Glyph(1.0f, listOf(
            GlyphShape("M4.5 12.5a3 3 0 0 1-.4-6A4 4 0 0 1 11.8 5.6 3.5 3.5 0 0 1 11.5 12.5z", false, true, 0.0f),
        )))
    put("minus", Glyph(1.0f, listOf(
            GlyphShape("M3 8h10", false, true, 0.0f),
        )))
    put("chevron-down", Glyph(1.0f, listOf(
            GlyphShape("M4 6l4 4 4-4", false, true, 0.0f),
        )))
    put("chevron-up", Glyph(1.0f, listOf(
            GlyphShape("M4 10l4-4 4 4", false, true, 0.0f),
        )))
    put("chevron-right", Glyph(1.0f, listOf(
            GlyphShape("M6 4l4 4-4 4", false, true, 0.0f),
        )))
    put("chevron-left", Glyph(1.0f, listOf(
            GlyphShape("M10 4L6 8l4 4", false, true, 0.0f),
        )))
    put("arrow-left", Glyph(1.0f, listOf(
            GlyphShape("M13 8H3M7 4L3 8l4 4", false, true, 0.0f),
        )))
    put("external", Glyph(1.0f, listOf(
            GlyphShape("M6 3H3v10h10v-3M9 3h4v4M13 3L7 9", false, true, 0.0f),
        )))
    put("branch", Glyph(1.0f, listOf(
            GlyphShape("M3 3.5a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0 -3 0z", false, true, 0.0f),
            GlyphShape("M3 12.5a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0 -3 0z", false, true, 0.0f),
            GlyphShape("M10 5.5a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0 -3 0z", false, true, 0.0f),
            GlyphShape("M4.5 5v6M11.5 7a4 4 0 0 1-4 4h-1", false, true, 0.0f),
        )))
    put("commit", Glyph(1.0f, listOf(
            GlyphShape("M5.5 8a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0 -5 0z", false, true, 0.0f),
            GlyphShape("M2 8h3.5M10.5 8H14", false, true, 0.0f),
        )))
    put("play", Glyph(1.0f, listOf(
            GlyphShape("M3.33 3.33a1.33 1.33 0 0 1 2-1.15l8 4.67a1.33 1.33 0 0 1 0 2.3l-8 4.67A1.33 1.33 0 0 1 3.33 12.67z", false, true, 0.0f),
        )))
    put("pause", Glyph(1.0f, listOf(
            GlyphShape("M4.25 2.75h1.5a1 1 0 0 1 1 1v8.5a1 1 0 0 1 -1 1h-1.5a1 1 0 0 1 -1 -1v-8.5a1 1 0 0 1 1 -1z", false, true, 0.0f),
            GlyphShape("M10.25 2.75h1.5a1 1 0 0 1 1 1v8.5a1 1 0 0 1 -1 1h-1.5a1 1 0 0 1 -1 -1v-8.5a1 1 0 0 1 1 -1z", false, true, 0.0f),
        )))
    put("stop", Glyph(1.0f, listOf(
            GlyphShape("M6 4.5h4a1.5 1.5 0 0 1 1.5 1.5v4a1.5 1.5 0 0 1 -1.5 1.5h-4a1.5 1.5 0 0 1 -1.5 -1.5v-4a1.5 1.5 0 0 1 1.5 -1.5z", true, true, 0.0f),
        )))
    put("resume", Glyph(1.0f, listOf(
            GlyphShape("M3 8a5 5 0 1 0 1.5-3.5", false, true, 0.0f),
            GlyphShape("M3 2.5V5h2.5", false, true, 0.0f),
        )))
    put("copy", Glyph(1.0f, listOf(
            GlyphShape("M5.5 5.5h8v8h-8z", false, true, 0.0f),
            GlyphShape("M10.5 5.5v-3h-8v8h3", false, true, 0.0f),
        )))
    put("clear", Glyph(1.0f, listOf(
            GlyphShape("M9.5 2.5l4 4-6 6H4l-1.5-1.5z", false, true, 0.0f),
            GlyphShape("M6.5 5.5l4 4", false, true, 0.0f),
            GlyphShape("M7.5 12.5H14", false, true, 0.0f),
        )))
    put("trash", Glyph(1.0f, listOf(
            GlyphShape("M3 4h10M6 4V2.5h4V4M4.5 4l.7 9h5.6l.7-9", false, true, 0.0f),
        )))
    put("check", Glyph(1.0f, listOf(
            GlyphShape("M3 8.5l3 3 7-7", false, true, 0.0f),
        )))
    put("undo", Glyph(1.0f, listOf(
            GlyphShape("M3 6h7a3.5 3.5 0 0 1 0 7H7", false, true, 0.0f),
            GlyphShape("M5.5 3.5L3 6l2.5 2.5", false, true, 0.0f),
        )))
    put("phone-install", Glyph(1.0f, listOf(
            GlyphShape("M5.25 1.5h5.5a1 1 0 0 1 1 1v11a1 1 0 0 1 -1 1h-5.5a1 1 0 0 1 -1 -1v-11a1 1 0 0 1 1 -1z", false, true, 0.0f),
            GlyphShape("M8 4v5.5M5.75 7.5L8 9.75l2.25-2.25M7.25 12.5h1.5", false, true, 0.0f),
        )))
    put("brief", Glyph(1.0f, listOf(
            GlyphShape("M9 1.75H4.5A1.5 1.5 0 0 0 3 3.25v9.5a1.5 1.5 0 0 0 1.5 1.5h7a1.5 1.5 0 0 0 1.5-1.5V5.75z", false, true, 0.0f),
            GlyphShape("M9 1.75V5a.75.75 0 0 0 .75.75H13M5.75 8.75h4.5M5.75 11.25h3", false, true, 0.0f),
        )))
    put("power", Glyph(1.0f, listOf(
            GlyphShape("M8 2v5.5", false, true, 0.0f),
            GlyphShape("M4.8 4.3a5 5 0 1 0 6.4 0", false, true, 0.0f),
        )))
    put("claude", Glyph(0.6666667f, listOf(
            GlyphShape("m4.7144 15.9555 4.7174-2.6471.079-.2307-.079-.1275h-.2307l-.7893-.0486-2.6956-.0729-2.3375-.0971-2.2646-.1214-.5707-.1215-.5343-.7042.0546-.3522.4797-.3218.686.0608 1.5179.1032 2.2767.1578 1.6514.0972 2.4468.255h.3886l.0546-.1579-.1336-.0971-.1032-.0972L6.973 9.8356l-2.55-1.6879-1.3356-.9714-.7225-.4918-.3643-.4614-.1578-1.0078.6557-.7225.8803.0607.2246.0607.8925.686 1.9064 1.4754 2.4893 1.8336.3643.3035.1457-.1032.0182-.0728-.164-.2733-1.3539-2.4467-1.445-2.4893-.6435-1.032-.17-.6194c-.0607-.255-.1032-.4674-.1032-.7285L6.287.1335 6.6997 0l.9957.1336.419.3642.6192 1.4147 1.0018 2.2282 1.5543 3.0296.4553.8985.2429.8318.091.255h.1579v-.1457l.1275-1.706.2368-2.0947.2307-2.6957.0789-.7589.3764-.9107.7468-.4918.5828.2793.4797.686-.0668.4433-.2853 1.8517-.5586 2.9021-.3643 1.9429h.2125l.2429-.2429.9835-1.3053 1.6514-2.0643.7286-.8196.85-.9046.5464-.4311h1.0321l.759 1.1293-.34 1.1657-1.0625 1.3478-.8804 1.1414-1.2628 1.7-.7893 1.36.0729.1093.1882-.0183 2.8535-.607 1.5421-.2794 1.8396-.3157.8318.3886.091.3946-.3278.8075-1.967.4857-2.3072.4614-3.4364.8136-.0425.0304.0486.0607 1.5482.1457.6618.0364h1.621l3.0175.2247.7892.522.4736.6376-.079.4857-1.2142.6193-1.6393-.3886-3.825-.9107-1.3113-.3279h-.1822v.1093l1.0929 1.0686 2.0035 1.8092 2.5075 2.3314.1275.5768-.3218.4554-.34-.0486-2.2039-1.6575-.85-.7468-1.9246-1.621h-.1275v.17l.4432.6496 2.3436 3.5214.1214 1.0807-.17.3521-.6071.2125-.6679-.1214-1.3721-1.9246L14.38 17.959l-1.1414-1.9428-.1397.079-.674 7.2552-.3156.3703-.7286.2793-.6071-.4614-.3218-.7468.3218-1.4753.3886-1.9246.3157-1.53.2853-1.9004.17-.6314-.0121-.0425-.1397.0182-1.4328 1.9672-2.1796 2.9446-1.7243 1.8456-.4128.164-.7164-.3704.0667-.6618.4008-.5889 2.386-3.0357 1.4389-1.882.929-1.0868-.0062-.1579h-.0546l-6.3385 4.1164-1.1293.1457-.4857-.4554.0608-.7467.2307-.2429 1.9064-1.3114Z", true, false, 0.0f),
        )))
    put("codex", Glyph(0.6666667f, listOf(
            GlyphShape("M22.2819 9.8211a5.9847 5.9847 0 0 0-.5157-4.9108 6.0462 6.0462 0 0 0-6.5098-2.9A6.0651 6.0651 0 0 0 4.9807 4.1818a5.9847 5.9847 0 0 0-3.9977 2.9 6.0462 6.0462 0 0 0 .7427 7.0966 5.98 5.98 0 0 0 .511 4.9107 6.051 6.051 0 0 0 6.5146 2.9001A5.9847 5.9847 0 0 0 13.2599 24a6.0557 6.0557 0 0 0 5.7718-4.2058 5.9894 5.9894 0 0 0 3.9977-2.9001 6.0557 6.0557 0 0 0-.7475-7.0729zm-9.022 12.6081a4.4755 4.4755 0 0 1-2.8764-1.0408l.1419-.0804 4.7783-2.7582a.7948.7948 0 0 0 .3927-.6813v-6.7369l2.02 1.1686a.071.071 0 0 1 .038.052v5.5826a4.504 4.504 0 0 1-4.4945 4.4944zm-9.6607-4.1254a4.4708 4.4708 0 0 1-.5346-3.0137l.142.0852 4.783 2.7582a.7712.7712 0 0 0 .7806 0l5.8428-3.3685v2.3324a.0804.0804 0 0 1-.0332.0615L9.74 19.9502a4.4992 4.4992 0 0 1-6.1408-1.6464zM2.3408 7.8956a4.485 4.485 0 0 1 2.3655-1.9728V11.6a.7664.7664 0 0 0 .3879.6765l5.8144 3.3543-2.0201 1.1685a.0757.0757 0 0 1-.071 0l-4.8303-2.7865A4.504 4.504 0 0 1 2.3408 7.872zm16.5963 3.8558L13.1038 8.364 15.1192 7.2a.0757.0757 0 0 1 .071 0l4.8303 2.7913a4.4944 4.4944 0 0 1-.6765 8.1042v-5.6772a.79.79 0 0 0-.407-.667zm2.0107-3.0231l-.142-.0852-4.7735-2.7818a.7759.7759 0 0 0-.7854 0L9.409 9.2297V6.8974a.0662.0662 0 0 1 .0284-.0615l4.8303-2.7866a4.4992 4.4992 0 0 1 6.6802 4.66zM8.3065 12.863l-2.02-1.1638a.0804.0804 0 0 1-.038-.0567V6.0742a4.4992 4.4992 0 0 1 7.3757-3.4537l-.142.0805L8.704 5.459a.7948.7948 0 0 0-.3927.6813zm1.0976-2.3654 2.602-1.4998 2.6069 1.4998v2.9994l-2.5974 1.4997-2.6067-1.4997Z", true, false, 0.0f),
        )))
    put("terminal", Glyph(1.0f, listOf(
            GlyphShape("M3 4l4 4-4 4M8 12h5", false, true, 0.0f),
        )))
    put("code", Glyph(1.0f, listOf(
            GlyphShape("M5 4L1.5 8 5 12M11 4l3.5 4L11 12M9.5 2.5l-3 11", false, true, 0.0f),
        )))
    put("board", Glyph(1.0f, listOf(
            GlyphShape("M2 2h3.5v12h-3.5z", false, true, 0.0f),
            GlyphShape("M6.25 2h3.5v9h-3.5z", false, true, 0.0f),
            GlyphShape("M10.5 2h3.5v6h-3.5z", false, true, 0.0f),
        )))
    put("columns", Glyph(1.0f, listOf(
            GlyphShape("M2 2h3.5v12h-3.5z", false, true, 0.0f),
            GlyphShape("M6.25 2h3.5v9h-3.5z", false, true, 0.0f),
            GlyphShape("M10.5 2h3.5v6h-3.5z", false, true, 0.0f),
        )))
    put("modules", Glyph(1.0f, listOf(
            GlyphShape("M8 2l6 3-6 3-6-3z", false, true, 0.0f),
            GlyphShape("M2 8l6 3 6-3M2 11l6 3 6-3", false, true, 0.0f),
        )))
    put("layout", Glyph(1.0f, listOf(
            GlyphShape("M4 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1 -2 2h-8a2 2 0 0 1 -2 -2v-8a2 2 0 0 1 2 -2z", false, true, 0.0f),
            GlyphShape("M8 2v12M2 8h12", false, true, 0.0f),
        )))
    put("grid", Glyph(1.0f, listOf(
            GlyphShape("M4 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1 -2 2h-8a2 2 0 0 1 -2 -2v-8a2 2 0 0 1 2 -2z", false, true, 0.0f),
            GlyphShape("M8 2v12M2 8h12", false, true, 0.0f),
        )))
    put("focus", Glyph(1.0f, listOf(
            GlyphShape("M4.25 2h7.5a2.25 2.25 0 0 1 2.25 2.25v7.5a2.25 2.25 0 0 1 -2.25 2.25h-7.5a2.25 2.25 0 0 1 -2.25 -2.25v-7.5a2.25 2.25 0 0 1 2.25 -2.25z", false, true, 0.0f),
            GlyphShape("M6.25 5.25h3.5a1 1 0 0 1 1 1v3.5a1 1 0 0 1 -1 1h-3.5a1 1 0 0 1 -1 -1v-3.5a1 1 0 0 1 1 -1z", false, true, 0.0f),
        )))
    put("refresh", Glyph(1.0f, listOf(
            GlyphShape("M13 8a5 5 0 1 1-1.5-3.5", false, true, 0.0f),
            GlyphShape("M13 2.5V5h-2.5", false, true, 0.0f),
        )))
    put("download", Glyph(1.0f, listOf(
            GlyphShape("M8 2v8M4.5 7L8 10.5 11.5 7M3 13h10", false, true, 0.0f),
        )))
    put("user", Glyph(1.0f, listOf(
            GlyphShape("M5.5 5.5a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0 -5 0z", false, true, 0.0f),
            GlyphShape("M3 14a5 5 0 0 1 10 0", false, true, 0.0f),
        )))
    put("cpu", Glyph(1.0f, listOf(
            GlyphShape("M4 4h8v8h-8z", false, true, 0.0f),
            GlyphShape("M6.5 6.5h3v3h-3z", false, true, 0.0f),
            GlyphShape("M6 1.5v2.5M10 1.5v2.5M6 12v2.5M10 12v2.5M1.5 6h2.5M1.5 10h2.5M12 6h2.5M12 10h2.5", false, true, 0.0f),
        )))
    put("device", Glyph(1.0f, listOf(
            GlyphShape("M5.25 1.5h5.5a1 1 0 0 1 1 1v11a1 1 0 0 1 -1 1h-5.5a1 1 0 0 1 -1 -1v-11a1 1 0 0 1 1 -1z", false, true, 0.0f),
            GlyphShape("M6.5 3h3M7.25 12.5h1.5", false, true, 0.0f),
        )))
    put("save", Glyph(1.0f, listOf(
            GlyphShape("M3 3h8l2 2v8H3z", false, true, 0.0f),
            GlyphShape("M5 3v4h5V3M5 13V9h6v4", false, true, 0.0f),
        )))
    put("edit", Glyph(1.0f, listOf(
            GlyphShape("M3 13l1-3.5L11 2.5l2.5 2.5L6.5 12z", false, true, 0.0f),
            GlyphShape("M9.5 4l2.5 2.5", false, true, 0.0f),
        )))
    put("merge", Glyph(1.0f, listOf(
            GlyphShape("M3 3.5a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0 -3 0z", false, true, 0.0f),
            GlyphShape("M3 12.5a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0 -3 0z", false, true, 0.0f),
            GlyphShape("M10 12.5a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0 -3 0z", false, true, 0.0f),
            GlyphShape("M4.5 5v6M4.5 5a5 5 0 0 0 5 5h.5", false, true, 0.0f),
        )))
    put("history", Glyph(1.0f, listOf(
            GlyphShape("M2.5 8a5.5 5.5 0 1 0 11 0a5.5 5.5 0 1 0 -11 0z", false, true, 0.0f),
            GlyphShape("M8 5v3.5l2.5 1.5", false, true, 0.0f),
        )))
    put("spike", Glyph(1.0f, listOf(
            GlyphShape("M8 2v10M5 12h6M4 14h8", false, true, 0.0f),
        )))
    put("send", Glyph(1.0f, listOf(
            GlyphShape("M2 8l12-5.5L9.5 14 8 9z", false, true, 0.0f),
            GlyphShape("M8 9l6-6.5", false, true, 0.0f),
        )))
    put("notes", Glyph(1.0f, listOf(
            GlyphShape("M3 2.5h10v11H3z", false, true, 0.0f),
            GlyphShape("M5.5 5.5h5M5.5 8h5M5.5 10.5h3.5", false, true, 0.0f),
        )))
    put("pin", Glyph(1.0f, listOf(
            GlyphShape("M5 2.5h6M6 2.5v4l-2 2h8l-2-2v-4M8 8.5V14", false, true, 0.0f),
        )))
    put("more", Glyph(1.0f, listOf(
            GlyphShape("M2.7 8a0.8 0.8 0 1 0 1.6 0a0.8 0.8 0 1 0 -1.6 0z", true, false, 0.0f),
            GlyphShape("M7.2 8a0.8 0.8 0 1 0 1.6 0a0.8 0.8 0 1 0 -1.6 0z", true, false, 0.0f),
            GlyphShape("M11.7 8a0.8 0.8 0 1 0 1.6 0a0.8 0.8 0 1 0 -1.6 0z", true, false, 0.0f),
        )))
    put("sidebar", Glyph(1.0f, listOf(
            GlyphShape("M4.25 2.5h7.5a2.25 2.25 0 0 1 2.25 2.25v6.5a2.25 2.25 0 0 1 -2.25 2.25h-7.5a2.25 2.25 0 0 1 -2.25 -2.25v-6.5a2.25 2.25 0 0 1 2.25 -2.25z", false, true, 0.0f),
            GlyphShape("M6.25 2.5v11", false, true, 0.0f),
        )))
    put("graph", Glyph(1.0f, listOf(
            GlyphShape("M3.5 3a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0 -3 0z", false, true, 0.0f),
            GlyphShape("M9.5 8a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0 -3 0z", false, true, 0.0f),
            GlyphShape("M3.5 13a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0 -3 0z", false, true, 0.0f),
            GlyphShape("M5 4.5v7M6.5 4a5 5 0 0 1 4.5 2.5M11 9.5A5 5 0 0 1 6.5 12", false, true, 0.0f),
        )))
    put("shield", Glyph(1.0f, listOf(
            GlyphShape("M8 2l5 2v3.5c0 3.3-2 5.5-5 6.8-3-1.3-5-3.5-5-6.8V4z", false, true, 0.0f),
            GlyphShape("M5.5 8l1.5 1.5 3.5-4", false, true, 0.0f),
        )))
    put("fallback", Glyph(1.0f, listOf(
            GlyphShape("M5 8a3 3 0 1 0 6 0a3 3 0 1 0 -6 0z", false, true, 0.0f),
        )))
}
