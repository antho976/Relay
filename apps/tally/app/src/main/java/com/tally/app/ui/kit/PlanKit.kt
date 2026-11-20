package com.tally.app.ui.kit

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.tally.app.ui.common.CategoryIcons
import com.tally.app.ui.theme.CategoryPalette
import com.tally.app.ui.theme.Tally
import com.tally.app.ui.theme.categoryColor

/*
 * Direction C pieces for the Plan and Money pages: choice tiles, colour swatches, icon tiles,
 * gradient meters and category tiles. They read the same tokens as the kit.
 */

/** True when an account's name names a bank Tally draws a mark for, so its row shows that mark. */
fun knownBank(name: String): Boolean {
    val n = name.lowercase()
    return listOf("desjardins", "wealthsimple", "rbc", "royal bank", "national bank", "bmo", "scotia", "tangerine").any { it in n } ||
        n.split(' ', '-', '_').any { it == "td" }
}

/**
 * A choice as a tile: a glyph on a soft tint over a title, an optional hint under it, an ink edge
 * and a check when it is the one picked. Used two or three to a row.
 */
@Composable
fun ChoiceTile(
    title: String,
    glyph: String,
    hue: Color,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    hint: String? = null,
) {
    val c = Tally.colors
    Box(
        modifier
            .clip(Radii.card)
            .background(if (selected) c.raised else c.slab)
            .border(if (selected) 2.dp else 0.dp, if (selected) c.ink else Color.Transparent, Radii.card)
            .clickable(role = Role.RadioButton, onClick = onClick)
            .semantics { contentDescription = title }
            .padding(horizontal = 12.dp, vertical = 14.dp),
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            GlyphTile(glyph, hue, size = 38.dp, fill = softOf(hue))
            T(title, style = Tally.type.uiMedium, color = c.ink, weight = FontWeight.SemiBold, maxLines = 2)
            hint?.let { T(it, style = Tally.type.caption.copy(fontSize = 11.5.sp, lineHeight = 15.sp), color = c.ink3, maxLines = 3) }
        }
        if (selected) {
            Box(
                Modifier.align(Alignment.TopEnd).size(20.dp).clip(CircleShape).background(c.ink),
                contentAlignment = Alignment.Center,
            ) { Glyph("check", 12.dp, c.onInk) }
        }
    }
}

/** Tiles laid [columns] to a row, each the height of the tallest in its row. */
@Composable
fun TileGrid(count: Int, columns: Int, modifier: Modifier = Modifier, gap: Dp = 10.dp, tile: @Composable (Int, Modifier) -> Unit) {
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(gap)) {
        (0 until count).chunked(columns).forEach { row ->
            Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(gap)) {
                row.forEach { i -> tile(i, Modifier.weight(1f).fillMaxHeight()) }
                repeat(columns - row.size) { Box(Modifier.weight(1f)) }
            }
        }
    }
}

/** The twelve category hues as circles; the picked one wears an ink ring and a check. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun SwatchRow(selected: Int, onPick: (Int) -> Unit, modifier: Modifier = Modifier, name: (Int) -> String = { "Colour ${it + 1}" }) {
    val c = Tally.colors
    Slab(modifier.fillMaxWidth(), padding = androidx.compose.foundation.layout.PaddingValues(horizontal = 14.dp, vertical = 14.dp)) {
        FlowRow(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            CategoryPalette.forEachIndexed { i, hue ->
                val on = selected == i
                Box(
                    Modifier.size(36.dp).clip(CircleShape)
                        .border(if (on) 3.dp else 0.dp, if (on) c.ink else Color.Transparent, CircleShape)
                        .padding(if (on) 5.dp else 0.dp)
                        .clip(CircleShape).background(hue)
                        .clickable(role = Role.RadioButton) { onPick(i) }
                        .semantics { contentDescription = name(i) },
                    contentAlignment = Alignment.Center,
                ) {
                    if (on) Glyph("check", 14.dp, Color.White)
                }
            }
        }
    }
}

/** A category icon in a tile: white, or filled with the category's hue when picked. */
@Composable
fun IconChoice(key: String, colorIndex: Int, selected: Boolean, onClick: () -> Unit, label: String, modifier: Modifier = Modifier) {
    val c = Tally.colors
    val hue = categoryColor(colorIndex)
    Box(
        modifier.size(48.dp).clip(RoundedCornerShape(15.dp)).background(if (selected) hue else c.raised)
            .clickable(role = Role.RadioButton, onClick = onClick)
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        Image(CategoryIcons.of(key), contentDescription = null, modifier = Modifier.size(22.dp), colorFilter = ColorFilter.tint(if (selected) Color.White else c.ink2))
    }
}

/** A meter whose fill runs between two hues, as the C mockups' room rows. */
@Composable
fun GradientMeter(fraction: Float, from: Color, to: Color, modifier: Modifier = Modifier, height: Dp = 9.dp, over: Boolean = false, pace: Float? = null) {
    val c = Tally.colors
    val f = fraction.coerceIn(0f, 1f)
    Canvas(modifier.fillMaxWidth().height(height + 6.dp)) {
        val bar = height.toPx()
        val top = (size.height - bar) / 2
        val r = CornerRadius(bar / 2)
        drawRoundRect(c.track, topLeft = androidx.compose.ui.geometry.Offset(0f, top), size = Size(size.width, bar), cornerRadius = r)
        if (f > 0f) {
            val w = size.width * f
            val at = androidx.compose.ui.geometry.Offset(0f, top)
            if (over) drawRoundRect(c.held, topLeft = at, size = Size(w, bar), cornerRadius = r)
            else drawRoundRect(Brush.horizontalGradient(listOf(from, to), endX = w), topLeft = at, size = Size(w, bar), cornerRadius = r)
        }
        pace?.let { p ->
            val x = size.width * p.coerceIn(0f, 1f)
            drawLine(c.ink, androidx.compose.ui.geometry.Offset(x, 0f), androidx.compose.ui.geometry.Offset(x, size.height), strokeWidth = 2.dp.toPx(), cap = androidx.compose.ui.graphics.StrokeCap.Round)
        }
    }
}

/**
 * A category as a tinted tile: its glyph, its name in a deep shade of its hue, this month's money
 * and a line under it. Tap opens, hold edits.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun CategoryTile(
    name: String,
    icon: String?,
    colorIndex: Int?,
    figure: String,
    line: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    onLongClick: (() -> Unit)? = null,
) {
    val hue = categoryColor(colorIndex)
    val words = deepOf(hue)
    Column(
        modifier.clip(Radii.card).background(softOf(hue))
            .combinedClickable(onLongClick = onLongClick, onClick = onClick)
            .heightIn(min = 120.dp)
            .padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        Image(CategoryIcons.of(icon), contentDescription = null, modifier = Modifier.size(24.dp), colorFilter = ColorFilter.tint(hue))
        T(name, Modifier.padding(top = 4.dp), Tally.type.uiMedium, words, maxLines = 1, weight = FontWeight.SemiBold)
        T(figure, style = Tally.type.figure.copy(fontSize = 22.sp), color = Color(0xFF1A1A2E), maxLines = 1)
        T(line, style = Tally.type.caption, color = words, maxLines = 2)
    }
}
