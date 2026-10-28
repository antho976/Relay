package com.quietsoftware.relay.ui.kit

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.graphics.vector.group
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.quietsoftware.relay.ui.theme.Relay
import androidx.compose.foundation.Image
import androidx.compose.ui.graphics.ColorFilter

private val cache = HashMap<Pair<String, Float>, ImageVector>()

/**
 * One of the desktop's glyphs (icons.rs) on its 16-unit grid. The line is about 1.3px as drawn,
 * like the PC's (`min(1.5, 20.8 / size)`), so a larger icon gets finer lines, not heavier ones.
 */
fun glyph(name: String, sizeDp: Float): ImageVector = synchronized(cache) {
    val stroke = (20.8f / sizeDp).coerceAtMost(1.5f)
    cache.getOrPut(name to stroke) {
        val g = GLYPHS[name] ?: EXTRA[name] ?: GLYPHS.getValue("fallback")
        ImageVector.Builder(name = name, defaultWidth = 16.dp, defaultHeight = 16.dp, viewportWidth = 16f, viewportHeight = 16f).apply {
            group(scaleX = g.scale, scaleY = g.scale) {
                for (s in g.shapes) {
                    group(rotate = s.rotate, pivotX = 8f / g.scale, pivotY = 8f / g.scale) {
                        addPath(
                            pathData = PathParser().parsePathString(s.d).toNodes(),
                            fill = if (s.fill) SolidColor(Color.White) else null,
                            stroke = if (s.stroke) SolidColor(Color.White) else null,
                            strokeLineWidth = stroke / g.scale,
                            strokeLineCap = StrokeCap.Round,
                            strokeLineJoin = StrokeJoin.Round,
                        )
                    }
                }
            }
        }.build()
    }
}

@Composable
fun Glyph(name: String, size: Dp = 16.dp, tint: Color = Relay.colors.ink3, modifier: Modifier = Modifier) {
    val vector = remember(name, size) { glyph(name, size.value) }
    Image(rememberVectorPainter(vector), contentDescription = null, modifier = modifier.size(size), colorFilter = ColorFilter.tint(tint))
}

/** Glyphs the phone needs that the desktop has no use for, drawn on the same grid. */
private val EXTRA: Map<String, Glyph> = mapOf(
    // A QR code's three finder squares and a scatter of modules.
    "qr" to Glyph(1f, listOf(
        GlyphShape("M2.5 2.5h4v4h-4zM9.5 2.5h4v4h-4zM2.5 9.5h4v4h-4z", false, true, 0f),
        GlyphShape("M9.5 9.5h1.5v1.5M13.5 9.5v1.5M9.5 13.5h4", false, true, 0f),
    )),
    "link" to Glyph(1f, listOf(
        GlyphShape("M7 9a2.5 2.5 0 0 0 3.5 0l2.5-2.5a2.5 2.5 0 0 0-3.5-3.5l-1 1", false, true, 0f),
        GlyphShape("M9 7a2.5 2.5 0 0 0-3.5 0L3 9.5a2.5 2.5 0 0 0 3.5 3.5l1-1", false, true, 0f),
    )),
    "wifi" to Glyph(1f, listOf(
        GlyphShape("M1.5 6a9.5 9.5 0 0 1 13 0M3.75 8.5a6.25 6.25 0 0 1 8.5 0M6 11a3 3 0 0 1 4 0", false, true, 0f),
        GlyphShape("M7.25 13.25a.75.75 0 1 0 1.5 0a.75.75 0 1 0-1.5 0z", true, false, 0f),
    )),
    // The outbox: a tray with an arrow leaving it.
    "outbox" to Glyph(1f, listOf(
        GlyphShape("M2 9.5h3l1 2h4l1-2h3V13.5H2z", false, true, 0f),
        GlyphShape("M8 8V2M5.5 4.5L8 2l2.5 2.5", false, true, 0f),
    )),
    "keyboard" to Glyph(1f, listOf(
        GlyphShape("M1.5 4h13v8h-13z", false, true, 0f),
        GlyphShape("M4 6.5h.01M6.5 6.5h.01M9 6.5h.01M11.5 6.5h.01M4 9.5h8", false, true, 0f),
    )),
    // Two speech bubbles: the Threads space (start.rs draws the same idea).
    "threads" to Glyph(1f, listOf(
        GlyphShape("M2 3.5h8v5.5H5.5L3 11V9H2z", false, true, 0f),
        GlyphShape("M10 6h4v5.5h-1V13.5l-2-2H7.5V9", false, true, 0f),
    )),
    "home" to Glyph(1f, listOf(
        GlyphShape("M2.5 7.5L8 3l5.5 4.5V13.5h-11z", false, true, 0f),
        GlyphShape("M6.5 13.5V10h3v3.5", false, true, 0f),
    )),
    "inbox" to Glyph(1f, listOf(
        GlyphShape("M2 9.5h3l1 2h4l1-2h3V13.5H2z", false, true, 0f),
        GlyphShape("M3.5 9.5L5 3h6l1.5 6.5", false, true, 0f),
    )),
    "mail" to Glyph(1f, listOf(
        GlyphShape("M2 3.5h12v9H2z", false, true, 0f),
        GlyphShape("M2 4l6 5 6-5", false, true, 0f),
    )),
    "menu" to Glyph(1f, listOf(GlyphShape("M2.5 4h11M2.5 8h11M2.5 12h11", false, true, 0f))),
    "arbiter" to Glyph(1f, listOf(GlyphShape("M2 12l3.5-4 3 2.5L14 4M10.5 4H14v3.5", false, true, 0f))),
    "avex" to Glyph(1f, listOf(GlyphShape("M1.5 6.5v3M4 4.5v7M4 8h8M12 4.5v7M14.5 6.5v3", false, true, 0f))),
    // Aide: a spark, the agent that speaks first.
    "aide" to Glyph(1f, listOf(GlyphShape("M8 1.5v4M8 10.5v4M1.5 8h4M10.5 8h4M4.2 4.2l1.6 1.6M10.2 10.2l1.6 1.6M11.8 4.2l-1.6 1.6M5.8 10.2l-1.6 1.6", false, true, 0f))),
)
