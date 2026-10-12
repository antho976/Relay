package com.tally.app.ui.kit

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.graphics.vector.group
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.tally.app.ui.theme.Tally

private val cache = HashMap<Pair<String, Float>, ImageVector>()

/**
 * One of Relay's glyphs on its 16-unit grid. The line is about 1.3px as drawn, like the PC's
 * (`min(1.5, 20.8 / size)`), so a larger icon gets finer lines, not heavier ones.
 */
fun glyph(name: String, sizeDp: Float): ImageVector = synchronized(cache) {
    val stroke = (20.8f / sizeDp).coerceAtMost(1.5f)
    cache.getOrPut(name to stroke) {
        val g = GLYPHS[name] ?: MONEY[name] ?: GLYPHS.getValue("fallback")
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
fun Glyph(name: String, size: Dp = 16.dp, tint: Color = Tally.colors.ink3, modifier: Modifier = Modifier) {
    val vector = remember(name, size) { glyph(name, size.value) }
    Image(rememberVectorPainter(vector), contentDescription = null, modifier = modifier.size(size), colorFilter = ColorFilter.tint(tint))
}

/** Whether [name] is a glyph Tally can draw (Relay's set or the money set below). */
fun hasGlyph(name: String): Boolean = name in GLYPHS || name in MONEY

private fun stroked(vararg d: String) = Glyph(1f, d.map { GlyphShape(it, false, true, 0f) })

/** Glyphs Tally needs that Relay has no use for, drawn on the same grid in the same hand. */
private val MONEY: Map<String, Glyph> = mapOf(
    "home" to stroked("M2.5 7.5L8 3l5.5 4.5V13.5h-11z", "M6.5 13.5V10h3v3.5"),
    "menu" to stroked("M2.5 4h11M2.5 8h11M2.5 12h11"),
    "list" to stroked("M5.5 4h8M5.5 8h8M5.5 12h8", "M2.5 4h.01M2.5 8h.01M2.5 12h.01"),
    "link" to stroked("M7 9a2.5 2.5 0 0 0 3.5 0l2.5-2.5a2.5 2.5 0 0 0-3.5-3.5l-1 1", "M9 7a2.5 2.5 0 0 0-3.5 0L3 9.5a2.5 2.5 0 0 0 3.5 3.5l1-1"),
    "wifi" to Glyph(1f, listOf(
        GlyphShape("M1.5 6a9.5 9.5 0 0 1 13 0M3.75 8.5a6.25 6.25 0 0 1 8.5 0M6 11a3 3 0 0 1 4 0", false, true, 0f),
        GlyphShape("M7.25 13.25a.75.75 0 1 0 1.5 0a.75.75 0 1 0-1.5 0z", true, false, 0f),
    )),
    "calendar" to stroked("M2.5 3.5h11v10h-11z", "M2.5 6.5h11", "M5.5 2v3M10.5 2v3"),
    "wallet" to stroked("M2.5 4.5h10a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1h-10z", "M2.5 4.5l7.5-2.5v2.5", "M10.5 8.5h3v2h-3z"),
    "card" to stroked("M1.5 4h13v8h-13z", "M1.5 6.5h13", "M3.5 10h3"),
    "bank" to stroked("M2 6.5L8 3l6 3.5z", "M3.5 6.5v5M6.5 6.5v5M9.5 6.5v5M12.5 6.5v5", "M2 13.5h12"),
    "repeat" to stroked("M3 6.5a4 4 0 0 1 7-2.5l1.5 1.5", "M11.5 2.5v3h-3", "M13 9.5a4 4 0 0 1-7 2.5L4.5 10.5", "M4.5 13.5v-3h3"),
    "target" to Glyph(1f, listOf(
        GlyphShape("M2.5 8a5.5 5.5 0 1 0 11 0a5.5 5.5 0 1 0 -11 0z", false, true, 0f),
        GlyphShape("M5 8a3 3 0 1 0 6 0a3 3 0 1 0 -6 0z", false, true, 0f),
        GlyphShape("M7.25 8a.75.75 0 1 0 1.5 0a.75.75 0 1 0-1.5 0z", true, false, 0f),
    )),
    "pie" to stroked("M7 2.5a5.5 5.5 0 1 0 6.5 6.5H7z", "M9.5 1.5v5h5a5 5 0 0 0-5-5z"),
    "bars" to stroked("M3 13.5V9M6.5 13.5V5M10 13.5V7.5M13.5 13.5V3"),
    "tag" to Glyph(1f, listOf(
        GlyphShape("M2.5 2.5h5l6 6-5 5-6-6z", false, true, 0f),
        GlyphShape("M4.75 5a.75.75 0 1 0 1.5 0a.75.75 0 1 0-1.5 0z", true, false, 0f),
    )),
    "money-in" to stroked("M8 2v7.5M5 6.5l3 3 3-3", "M2.5 11v2.5h11V11"),
    "money-out" to stroked("M8 9.5V2M5 5l3-3 3 3", "M2.5 11v2.5h11V11"),
    "transfer" to stroked("M2.5 5.5h10M10 3l2.5 2.5L10 8", "M13.5 10.5h-10M6 8l-2.5 2.5L6 13"),
    "coins" to stroked("M3 5c0-1.1 2.2-2 5-2s5 .9 5 2-2.2 2-5 2-5-.9-5-2z", "M3 5v3c0 1.1 2.2 2 5 2s5-.9 5-2V5", "M3 8v3c0 1.1 2.2 2 5 2s5-.9 5-2V8"),
    "receipt" to stroked("M3.5 1.5h9v13l-1.5-1-1.5 1-1.5-1-1.5 1-1.5-1-1.5 1z", "M5.5 5h5M5.5 8h5M5.5 11h3"),
    "trend" to stroked("M2 11.5l4-4 3 3 5-5.5", "M10.5 5h3.5v3.5"),
    "filter" to stroked("M2 3.5h12l-4.5 5.5v4l-3 1.5V9z"),
    "sync" to stroked("M13 8a5 5 0 0 1-8.5 3.5", "M3 8a5 5 0 0 1 8.5-3.5", "M11.5 2v2.5H9", "M4.5 14v-2.5H7"),
    "lock" to stroked("M4 7h8v6.5H4z", "M5.5 7V5a2.5 2.5 0 0 1 5 0v2"),
    "upload" to stroked("M8 10V2.5M5 5.5l3-3 3 3", "M2.5 10.5v3h11v-3"),
    "pulse" to stroked("M1.5 8h3l2-4.5 3 9 2-4.5h3"),
    "flag" to stroked("M3.5 14V2.5", "M3.5 3h8l-1.5 3 1.5 3h-8"),
    "eye" to stroked("M1.5 8s2.5-4.5 6.5-4.5S14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8z", "M6 8a2 2 0 1 0 4 0a2 2 0 1 0 -4 0z"),
    "chat" to stroked("M2 3.5h12v7.5H7l-3 2.5V11H2z"),
    "sparkle" to stroked("M8 2l1.3 3.7L13 7l-3.7 1.3L8 12l-1.3-3.7L3 7l3.7-1.3z"),
    "bolt" to stroked("M9 1.5L3.5 9H8l-1 5.5L12.5 7H8z"),
    "palette" to Glyph(1f, listOf(
        GlyphShape("M8 2a6 6 0 1 0 0 12c1 0 1.5-.6 1.5-1.4 0-.9-.8-1.3-.8-2.1 0-.8.7-1.5 1.5-1.5H12a2 2 0 0 0 2-2C14 4.2 11.3 2 8 2z", false, true, 0f),
        GlyphShape("M4.75 7.5a.75.75 0 1 0 1.5 0a.75.75 0 1 0-1.5 0z", true, false, 0f),
        GlyphShape("M7.25 4.75a.75.75 0 1 0 1.5 0a.75.75 0 1 0-1.5 0z", true, false, 0f),
        GlyphShape("M10.25 5.75a.75.75 0 1 0 1.5 0a.75.75 0 1 0-1.5 0z", true, false, 0f),
    )),
    "globe" to stroked("M2 8a6 6 0 1 0 12 0a6 6 0 1 0 -12 0z", "M2 8h12", "M8 2c1.6 1.7 2.4 3.7 2.4 6S9.6 12.3 8 14c-1.6-1.7-2.4-3.7-2.4-6S6.4 3.7 8 2z"),
)
