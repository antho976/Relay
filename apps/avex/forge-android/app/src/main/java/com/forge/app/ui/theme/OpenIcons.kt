package com.forge.app.ui.theme

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.unit.dp

/**
 * The redesign's icon family: 24-unit outline glyphs, 2-unit stroke, round caps and joins.
 *
 * These are the exact shapes the Relay design canvas draws for its built-in icon names (the
 * redesign in designs 7 and 8 was drawn with them), so a screen built from a design uses the same
 * glyph it shows. Paint is black like the Material set; `Icon(tint = …)` recolours the stroke.
 * Generated from Relay's icon table: add a name by copying its path data, never by redrawing it.
 */
object OpenIcons {
    val Activity: ImageVector by lazy { openIcon("Activity", "M22 12h-4l-3 9L9 3l-3 9H2") }
    val ArrowLeft: ImageVector by lazy { openIcon("ArrowLeft", "M19 12H5M11 6l-6 6 6 6") }
    val ArrowRight: ImageVector by lazy { openIcon("ArrowRight", "M5 12h14M13 6l6 6-6 6") }
    val ArrowUp: ImageVector by lazy { openIcon("ArrowUp", "M12 19V5M6 11l6-6 6 6") }
    val Award: ImageVector by lazy { openIcon("Award", "M6 8a6 6 0 1 0 12 0a6 6 0 1 0-12 0M8.5 13.5 7 22l5-3 5 3-1.5-8.5") }
    val BarChart: ImageVector by lazy { openIcon("BarChart", "M4 20h16M7 16v-5M12 16V6M17 16v-8") }
    val Bell: ImageVector by lazy { openIcon("Bell", "M6 16v-5a6 6 0 0 1 12 0v5l2 2H4zM10 21h4") }
    val Book: ImageVector by lazy { openIcon("Book", "M4 4.5A1.5 1.5 0 0 1 5.5 3H20v15H5.5A1.5 1.5 0 0 0 4 19.5zM4 19.5A1.5 1.5 0 0 0 5.5 21H20v-3") }
    val Bookmark: ImageVector by lazy { openIcon("Bookmark", "M6 3h12v18l-6-4-6 4z") }
    val Calendar: ImageVector by lazy { openIcon("Calendar", "M4 6h16v15H4zM4 10h16M8 3v4M16 3v4") }
    val Camera: ImageVector by lazy { openIcon("Camera", "M3 8h4l2-3h6l2 3h4v12H3zM8.5 13.5a3.5 3.5 0 1 0 7 0a3.5 3.5 0 1 0-7 0") }
    val Check: ImageVector by lazy { openIcon("Check", "M5 12.5l4.5 4.5L19 7") }
    val ChevronDown: ImageVector by lazy { openIcon("ChevronDown", "M6 9l6 6 6-6") }
    val ChevronLeft: ImageVector by lazy { openIcon("ChevronLeft", "M15 6l-6 6 6 6") }
    val ChevronRight: ImageVector by lazy { openIcon("ChevronRight", "M9 6l6 6-6 6") }
    val ChevronUp: ImageVector by lazy { openIcon("ChevronUp", "M6 15l6-6 6 6") }
    val Compass: ImageVector by lazy { openIcon("Compass", "M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0M16 8l-2 6-6 2 2-6z") }
    val Copy: ImageVector by lazy { openIcon("Copy", "M8 8h12v12H8zM16 8V4H4v12h4") }
    val Dumbbell: ImageVector by lazy { openIcon("Dumbbell", "M6 6v12M18 6v12M3 9v6M21 9v6M6 12h12") }
    val Edit: ImageVector by lazy { openIcon("Edit", "M4 20h4L19 9l-4-4L4 16zM13.5 6.5l4 4") }
    val Fingerprint: ImageVector by lazy { openIcon("Fingerprint", "M12 10 a2 2 0 0 0 -2 2 c0 1.02 -0.1 2.51 -0.26 4 M14 13.12 c0 2.38 0 6.38 -1 8.88 M17.29 21.02 c0.12 -0.6 0.43 -2.3 0.5 -3.02 M2 12 a10 10 0 0 1 18 -6 M2 16 h0.01 M21.8 16 c0.2 -2 0.131 -5.354 0 -6 M5 19.5 C5.5 18 6 15 6 12 a6 6 0 0 1 0.34 -2 M8.65 22 c0.21 -0.66 0.45 -1.32 0.57 -2 M9 6.8 a6 6 0 0 1 9 5.2 v2") }
    val Flame: ImageVector by lazy { openIcon("Flame", "M12 22a7 7 0 0 0 7-7c0-4-3-6-4-9-1 2-2 3-4 3 0-2-1-4-2-5-1 4-4 7-4 11a7 7 0 0 0 7 7z") }
    val GraduationCap: ImageVector by lazy { openIcon("GraduationCap", "M2 9l10-5 10 5-10 5zM6 11v5c3 2.5 9 2.5 12 0v-5M22 9v6") }
    val Heart: ImageVector by lazy { openIcon("Heart", "M12 20s-8-4.5-8-10a4.5 4.5 0 0 1 8-2.8A4.5 4.5 0 0 1 20 10c0 5.5-8 10-8 10z") }
    val History: ImageVector by lazy { openIcon("History", "M3 12a9 9 0 1 0 3-6.7L3 8M3 3v5h5M12 7v5l3 2") }
    val Home: ImageVector by lazy { openIcon("Home", "M3 10.5 12 3l9 7.5M5 9v11h5v-6h4v6h5V9") }
    val Image: ImageVector by lazy { openIcon("Image", "M3 5h18v14H3zM3 16l5-5 4 4 3-3 6 6M14.5 9a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0-3 0") }
    val Lock: ImageVector by lazy { openIcon("Lock", "M5 11h14v10H5zM8 11V7a4 4 0 0 1 8 0v4") }
    val MapPin: ImageVector by lazy { openIcon("MapPin", "M12 21s-7-6.5-7-12a7 7 0 0 1 14 0c0 5.5-7 12-7 12zM9.5 9a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0-5 0") }
    val Menu: ImageVector by lazy { openIcon("Menu", "M4 6h16M4 12h16M4 18h16") }
    val Moon: ImageVector by lazy { openIcon("Moon", "M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z") }
    val MoreVertical: ImageVector by lazy { openIcon("MoreVertical", "M11 5a1 1 0 1 0 2 0a1 1 0 1 0-2 0M11 12a1 1 0 1 0 2 0a1 1 0 1 0-2 0M11 19a1 1 0 1 0 2 0a1 1 0 1 0-2 0") }
    val Navigation: ImageVector by lazy { openIcon("Navigation", "M3 11l19-9-9 19-2-8z") }
    val Plus: ImageVector by lazy { openIcon("Plus", "M12 5v14M5 12h14") }
    val Refresh: ImageVector by lazy { openIcon("Refresh", "M20 11a8 8 0 0 0-14.7-4.3L4 9M4 4v5h5M4 13a8 8 0 0 0 14.7 4.3L20 15M20 20v-5h-5") }
    val Search: ImageVector by lazy { openIcon("Search", "M4 11a7 7 0 1 0 14 0a7 7 0 1 0-14 0M21 21l-5-5") }
    val Settings: ImageVector by lazy { openIcon("Settings", "M9 12a3 3 0 1 0 6 0a3 3 0 1 0-6 0M5 12a7 7 0 1 0 14 0a7 7 0 1 0-14 0M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.2 2.2M16.9 16.9l2.2 2.2M4.9 19.1l2.2-2.2M16.9 7.1l2.2-2.2") }
    val Share: ImageVector by lazy { openIcon("Share", "M15.5 5a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0-5 0M3.5 12a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0-5 0M15.5 19a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0-5 0M8.2 10.8l7.6-4.6M8.2 13.2l7.6 4.6") }
    val Sliders: ImageVector by lazy { openIcon("Sliders", "M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h.01M14 4v4M8 10v4M16 16v4") }
    val Smartphone: ImageVector by lazy { openIcon("Smartphone", "M6 2h12v20H6zM11 18h2") }
    val Sparkles: ImageVector by lazy { openIcon("Sparkles", "M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8zM19 16l.8 2.2L22 19l-2.2.8L19 22l-.8-2.2L16 19l2.2-.8z") }
    val Store: ImageVector by lazy { openIcon("Store", "M4 3h16l1 5a3 3 0 0 1-6 0a3 3 0 0 1-6 0a3 3 0 0 1-6 0zM5 11v10h14V11M10 21v-5h4v5") }
    val Timer: ImageVector by lazy { openIcon("Timer", "M5 13a7 7 0 1 0 14 0a7 7 0 1 0-14 0M12 13V9M10 2h4M19 6l-1.5 1.5") }
    val Trash: ImageVector by lazy { openIcon("Trash", "M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3") }
    val User: ImageVector by lazy { openIcon("User", "M8 7a4 4 0 1 0 8 0a4 4 0 1 0-8 0M4 21v-1a6 6 0 0 1 6-6h4a6 6 0 0 1 6 6v1") }
    val X: ImageVector by lazy { openIcon("X", "M6 6l12 12M18 6 6 18") }
}

private fun openIcon(name: String, pathData: String): ImageVector =
    ImageVector.Builder(
        name = "Open.$name",
        defaultWidth = 24.dp,
        defaultHeight = 24.dp,
        viewportWidth = 24f,
        viewportHeight = 24f,
    ).addPath(
        pathData = addPathNodes(pathData),
        fill = null,
        stroke = SolidColor(Color.Black),
        strokeLineWidth = 2f,
        strokeLineCap = StrokeCap.Round,
        strokeLineJoin = StrokeJoin.Round,
    ).build()
