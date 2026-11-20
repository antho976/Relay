package com.tally.app.ui.kit

import androidx.annotation.DrawableRes
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.tally.app.R
import com.tally.app.ui.theme.Hue
import com.tally.app.ui.theme.Tally
import java.time.LocalDate
import java.time.format.TextStyle
import java.util.Locale
import kotlin.math.abs

/*
 * Direction C's own pieces (DESIGN.md, design 40): the month's gauge, postcards with drawn scenes,
 * date badges, a donut, and soft tinted tiles. Colour here is data or state, as everywhere.
 */

/** A soft wash of a hue for a tile's ground, and a deep shade of it for words on that wash. */
fun softOf(hue: Color): Color = lerp(Color.White, hue, .16f)
fun deepOf(hue: Color): Color = lerp(hue, Color(0xFF1A1A2E), .55f)

/**
 * The month's half-circle gauge: a frosted track, then what was spent from green through amber to
 * coral. [spent] is the share of the budget gone (0..1+), [pace] where an even month would be.
 */
@Composable
fun Gauge(spent: Float, modifier: Modifier = Modifier, pace: Float? = null, stroke: Dp = 18.dp, content: @Composable BoxScope.() -> Unit = {}) {
    val c = Tally.colors
    Box(modifier.fillMaxWidth().height(190.dp), contentAlignment = Alignment.BottomCenter) {
        Canvas(Modifier.fillMaxSize()) {
            val w = stroke.toPx()
            val d = minOf(size.width - w, (size.height - w / 2) * 2f)
            val topLeft = Offset((size.width - d) / 2, w / 2 + 4.dp.toPx())
            val arc = Size(d, d)
            drawArc(Color.White.copy(alpha = if (c.light) .7f else .14f), 180f, 180f, false, topLeft, arc, style = Stroke(w, cap = StrokeCap.Round))
            val f = spent.coerceIn(0f, 1f)
            if (f > 0f) {
                val brush = Brush.sweepGradient(
                    0f to Hue.coral, .5f to Hue.mint, .66f to Color(0xFF8CCB4A), .82f to Hue.sun, 1f to Hue.coral,
                    center = Offset(topLeft.x + d / 2, topLeft.y + d / 2),
                )
                drawArc(brush, 180f, 180f * f, false, topLeft, arc, style = Stroke(w, cap = StrokeCap.Round))
            }
            pace?.let { p ->
                val a = Math.toRadians((180.0 + 180.0 * p.coerceIn(0f, 1f)))
                val r = d / 2
                val cx = topLeft.x + r
                val cy = topLeft.y + r
                val inner = r - w * .9f
                val outer = r + w * .9f
                drawLine(c.ink, Offset(cx + inner * kotlin.math.cos(a).toFloat(), cy + inner * kotlin.math.sin(a).toFloat()), Offset(cx + outer * kotlin.math.cos(a).toFloat(), cy + outer * kotlin.math.sin(a).toFloat()), strokeWidth = 3.dp.toPx(), cap = StrokeCap.Round)
            }
        }
        Column(Modifier.padding(bottom = 4.dp), horizontalAlignment = Alignment.CenterHorizontally, content = { Box(content = content) })
    }
}

/**
 * The scenes a postcard can show: pictures made for Tally on the owner's PC (Qwen-Image, 2026-11-19),
 * in the C sky's own pastels, so they sit on its ground without a frame.
 */
enum class Scene(@DrawableRes val picture: Int) {
    MOUNTAIN(R.drawable.scene_mountain),
    BEACH(R.drawable.scene_beach),
    GROWTH(R.drawable.scene_growth),
    HILLS(R.drawable.scene_hills),
    CITY(R.drawable.scene_city),
    ROAD(R.drawable.scene_road),
    BANKS(R.drawable.scene_banks),
}

/** A scene for a goal, from what its name says, falling back on its kind. */
fun sceneFor(name: String, invest: Boolean = false): Scene {
    val n = name.lowercase()
    return when {
        invest -> Scene.GROWTH
        listOf("trip", "japan", "travel", "vacation", "voyage", "lisbon", "flight", "ski").any { it in n } -> Scene.MOUNTAIN
        listOf("emergency", "cushion", "fund", "rainy", "safety", "beach").any { it in n } -> Scene.BEACH
        listOf("tfsa", "rrsp", "fhsa", "invest", "stock", "etf", "retire").any { it in n } -> Scene.GROWTH
        listOf("car", "truck", "bike", "road", "drive").any { it in n } -> Scene.ROAD
        listOf("house", "home", "condo", "rent", "apartment", "move").any { it in n } -> Scene.CITY
        else -> Scene.values()[abs(n.hashCode()) % 4]
    }
}

/**
 * A postcard: a white card with a drawn scene across its top and its words under it. [tag] sits on
 * the scene in a small frosted chip ("Savings pot").
 */
@Composable
fun Postcard(
    scene: Scene,
    modifier: Modifier = Modifier,
    sceneHeight: Dp = 128.dp,
    tag: String? = null,
    tagGlyph: String? = null,
    onClick: (() -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val c = Tally.colors
    Column(
        modifier.fillMaxWidth().shadow(10.dp, Radii.card, ambientColor = Color(0x331A1A2E), spotColor = Color(0x331A1A2E)).clip(Radii.card)
            .background(c.raised)
            .then(if (onClick != null) Modifier.clickable(role = Role.Button, onClick = onClick) else Modifier),
    ) {
        Box(Modifier.fillMaxWidth().height(sceneHeight)) {
            SceneArt(scene, Modifier.fillMaxSize())
            tag?.let {
                androidx.compose.foundation.layout.Row(
                    Modifier.padding(12.dp).clip(Radii.pill).background(Color.White.copy(alpha = .9f)).padding(horizontal = 10.dp, vertical = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(5.dp),
                ) {
                    tagGlyph?.let { g -> Glyph(g, 12.dp, Color(0xFF1A1A2E)) }
                    T(it, style = Tally.type.caption.copy(fontSize = 12.sp), color = Color(0xFF1A1A2E), weight = FontWeight.SemiBold, maxLines = 1)
                }
            }
        }
        Column(Modifier.fillMaxWidth().padding(PaddingValues(start = 16.dp, end = 16.dp, top = 14.dp, bottom = 16.dp)), verticalArrangement = Arrangement.spacedBy(8.dp), content = content)
    }
}

/** A scene alone, cropped to fill its box; dimmed a little on a dark palette so it does not glare. */
@Composable
fun SceneArt(scene: Scene, modifier: Modifier = Modifier) {
    val c = Tally.colors
    Box(modifier, contentAlignment = Alignment.Center) {
        Image(painterResource(scene.picture), contentDescription = null, modifier = Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
        if (!c.light) Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = .22f)))
        if (scene == Scene.BANKS) {
            // Two banks linked: the owner's own, as their initials, never their logos.
            androidx.compose.foundation.layout.Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                BankTile("D", Color(0xFF187A48), -8f)
                Box(Modifier.size(26.dp).clip(RoundedCornerShape(13.dp)).background(Color(0xFF1A1A2E)), contentAlignment = Alignment.Center) { Glyph("link", 14.dp, Color.White) }
                BankTile("W", Color(0xFF111111), 8f)
            }
        }
    }
}

@Composable
private fun BankTile(letter: String, fill: Color, tilt: Float) {
    Box(
        Modifier.rotate(tilt).size(56.dp).shadow(8.dp, RoundedCornerShape(18.dp)).clip(RoundedCornerShape(18.dp)).background(fill),
        contentAlignment = Alignment.Center,
    ) {
        T(letter, style = Tally.type.brand.copy(fontSize = 24.sp), color = Color.White)
    }
}


/** A date as a badge: the month's three letters over the day, on a tint of its own. */
@Composable
fun DateBadge(date: LocalDate, modifier: Modifier = Modifier, hue: Color = Hue.sky) {
    Column(
        modifier.size(width = 46.dp, height = 50.dp).clip(RoundedCornerShape(14.dp)).background(softOf(hue)),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        T(date.month.getDisplayName(TextStyle.SHORT, Locale.getDefault()).uppercase().take(3), style = Tally.type.caption.copy(fontSize = 10.sp, letterSpacing = 0.6.sp), color = deepOf(hue), weight = FontWeight.Bold, maxLines = 1)
        T(date.dayOfMonth.toString(), style = Tally.type.title.copy(fontSize = 18.sp), color = Color(0xFF1A1A2E), weight = FontWeight.Bold, maxLines = 1)
    }
}

/** A hue per date badge, so a list of bills reads as colour without meaning more than "a date". */
fun dateHue(index: Int): Color = listOf(Hue.sky, Hue.pink, Hue.violet, Hue.coral, Hue.teal, Hue.sun)[((index % 6) + 6) % 6]

/** A donut of parts, with words in its hole. */
@Composable
fun Donut(parts: List<Pair<Color, Long>>, modifier: Modifier = Modifier, size: Dp = 124.dp, thickness: Dp = 22.dp, center: @Composable BoxScope.() -> Unit = {}) {
    val c = Tally.colors
    val total = parts.sumOf { it.second }.coerceAtLeast(1)
    Box(modifier.size(size), contentAlignment = Alignment.Center) {
        Canvas(Modifier.fillMaxSize()) {
            val w = thickness.toPx()
            val inset = w / 2
            val arc = Size(this.size.width - w, this.size.height - w)
            if (parts.isEmpty()) {
                drawArc(c.track, 0f, 360f, false, Offset(inset, inset), arc, style = Stroke(w))
                return@Canvas
            }
            var start = -90f
            val gap = if (parts.size > 1) 2f else 0f
            for ((color, v) in parts) {
                val sweep = 360f * v / total
                drawArc(color, start + gap / 2, (sweep - gap).coerceAtLeast(.5f), false, Offset(inset, inset), arc, style = Stroke(w))
                start += sweep
            }
        }
        center()
    }
}

/** A tile on a soft tint: a stat with colour, as the C mockups' Today and Each day tiles. */
@Composable
fun TintTile(
    caption: String,
    figure: String,
    hue: Color,
    modifier: Modifier = Modifier,
    sub: String? = null,
    glyph: String? = null,
    figureColor: Color = Color(0xFF1A1A2E),
    onClick: (() -> Unit)? = null,
) {
    val words = deepOf(hue)
    Column(
        modifier.clip(Radii.card).background(softOf(hue))
            .then(if (onClick != null) Modifier.clickable(role = Role.Button, onClick = onClick) else Modifier)
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        glyph?.let {
            Box(Modifier.size(34.dp).clip(RoundedCornerShape(17.dp)).background(hue), contentAlignment = Alignment.Center) { Glyph(it, 17.dp, Color.White) }
            Gap(2.dp)
        }
        T(caption, style = Tally.type.caption.copy(fontSize = 13.sp), color = words, weight = FontWeight.Medium, maxLines = 2)
        T(figure, style = Tally.type.figure.copy(fontSize = 24.sp), color = figureColor, maxLines = 1)
        if (!sub.isNullOrEmpty()) T(sub, style = Tally.type.caption, color = words, maxLines = 2)
    }
}
