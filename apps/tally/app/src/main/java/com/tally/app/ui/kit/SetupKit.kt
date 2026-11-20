package com.tally.app.ui.kit

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.tally.app.ui.theme.Hue
import com.tally.app.ui.theme.Palette
import com.tally.app.ui.theme.Tally

/*
 * Direction C's pieces for the setup and settings pages (design 40, "C · Setup & settings"): the
 * welcome sky, groups of rows on glass, progress dots, the appearance previews and the drawer's rows.
 */

/** A group of rows: a small-capitals label over a glass card. */
@Composable
fun Group(label: String?, modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        label?.let { Eyebrow(it, Modifier.padding(start = 4.dp, top = 6.dp)) }
        Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 12.dp, vertical = 4.dp), content = content)
    }
}

/** A white card: what is typed or chosen, set apart from the glass around it. */
@Composable
fun WhiteCard(modifier: Modifier = Modifier, padding: PaddingValues = PaddingValues(16.dp), spacing: Dp = 10.dp, content: @Composable ColumnScope.() -> Unit) {
    Slab(modifier.fillMaxWidth(), padding = padding, color = Tally.colors.raised, spacing = spacing, content = content)
}

/** The steps of a flow as dots: the current one a short ink bar, the done ones ink, the rest glass. */
@Composable
fun ProgressDots(count: Int, current: Int, modifier: Modifier = Modifier) {
    val c = Tally.colors
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
        repeat(count) { i ->
            Box(
                Modifier.height(8.dp).width(if (i == current) 22.dp else 8.dp).clip(CircleShape)
                    .background(if (i <= current) c.ink else c.slab),
            )
        }
    }
}

/** The welcome sky: the palette's gradient, a soft sun, two clouds and three violet hills. */
@Composable
fun WelcomeSky(palette: Palette, modifier: Modifier = Modifier) {
    Canvas(modifier) {
        drawRect(Brush.verticalGradient(palette.ground))
        val w = size.width
        val h = size.height
        val sun = Offset(w * .74f, h * .26f)
        drawCircle(Brush.radialGradient(listOf(Color(0xFFFFE7A3), Color(0xFFFFC25A), Color(0x00FFC25A)), sun, h * .2f), h * .2f, sun)
        drawOval(Color.White.copy(alpha = .7f), Offset(-w * .08f, h * .3f), Size(w * .5f, h * .1f))
        drawOval(Color.White.copy(alpha = .7f), Offset(w * .5f, h * .44f), Size(w * .56f, h * .11f))
        hill(Color(0xFFB8B4F5), 0f to .78f, .25f to .7f, .5f to .76f, .75f to .67f, 1f to .73f)
        hill(Color(0xFF8E86E8), 0f to .84f, .3f to .78f, .6f to .85f, 1f to .78f)
        hill(Color(0xFF6C4BF4), 0f to .92f, .35f to .88f, .67f to .93f, 1f to .89f)
    }
}

private fun DrawScope.hill(color: Color, vararg pts: Pair<Float, Float>) {
    val w = size.width
    val h = size.height
    val p = pts.map { Offset(it.first * w, it.second * h) }
    val path = Path()
    path.moveTo(p.first().x, p.first().y)
    for (i in 1 until p.size) {
        val prev = p[i - 1]
        val cur = p[i]
        path.quadraticTo(prev.x, prev.y, (prev.x + cur.x) / 2, (prev.y + cur.y) / 2)
    }
    path.lineTo(p.last().x, p.last().y)
    path.lineTo(w, h)
    path.lineTo(0f, h)
    path.close()
    drawPath(path, color)
}

/** A frosted circle with a glyph and a violet arc on it: Tally waiting on something. */
@Composable
fun WaitingRing(glyph: String, modifier: Modifier = Modifier, size: Dp = 128.dp) {
    Box(modifier.size(size).clip(CircleShape).background(Color.White.copy(alpha = .72f)), contentAlignment = Alignment.Center) {
        Canvas(Modifier.fillMaxSize()) {
            val s = 5.dp.toPx()
            drawArc(Hue.violet, 20f, 120f, false, Offset(s / 2, s / 2), Size(this.size.width - s, this.size.height - s), style = Stroke(s, cap = StrokeCap.Round))
        }
        Glyph(glyph, size * .36f, Hue.violet)
    }
}

/**
 * A ground's preview card for Appearance: a small sky in the palette's gradient with the month's
 * gauge and two cards on it, the name and a line under it; the chosen one outlined in ink with a check.
 */
@Composable
fun PalettePreview(p: Palette, name: String, hint: String, selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val c = Tally.colors
    Column(
        modifier.clip(Radii.card).background(c.raised)
            .border(if (selected) 2.dp else 0.dp, if (selected) c.ink else Color.Transparent, Radii.card)
            .clickable(role = Role.RadioButton, onClick = onClick).padding(10.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Box(Modifier.fillMaxWidth().height(116.dp).clip(RoundedCornerShape(16.dp))) {
            Canvas(Modifier.fillMaxSize()) {
                drawRect(Brush.verticalGradient(p.ground))
                val w = size.width
                val d = w * .58f
                val stroke = 7.dp.toPx()
                val tl = Offset((w - d) / 2, 12.dp.toPx())
                drawArc(Color.White.copy(alpha = if (p.light) .75f else .18f), 180f, 180f, false, tl, Size(d, d), style = Stroke(stroke, cap = StrokeCap.Round))
                drawArc(Brush.horizontalGradient(listOf(Hue.mint, Hue.sun, Hue.coral), startX = tl.x, endX = tl.x + d), 180f, 106f, false, tl, Size(d, d), style = Stroke(stroke, cap = StrokeCap.Round))
                val cardTop = size.height * .72f
                drawRoundRect(p.slab, Offset(8.dp.toPx(), cardTop), Size(w / 2 - 12.dp.toPx(), size.height - cardTop - 8.dp.toPx()), androidx.compose.ui.geometry.CornerRadius(8.dp.toPx()))
                drawRoundRect(p.slab, Offset(w / 2 + 4.dp.toPx(), cardTop), Size(w / 2 - 12.dp.toPx(), size.height - cardTop - 8.dp.toPx()), androidx.compose.ui.geometry.CornerRadius(8.dp.toPx()))
            }
            T("$1,240", Modifier.align(Alignment.TopCenter).padding(top = 40.dp), Tally.type.title.copy(fontSize = 15.sp), p.ink, weight = FontWeight.Bold)
            if (selected) {
                Box(Modifier.align(Alignment.TopEnd).padding(8.dp).size(24.dp).clip(CircleShape).background(c.ink), contentAlignment = Alignment.Center) {
                    Glyph("check", 14.dp, c.onInk)
                }
            }
        }
        Column(Modifier.padding(horizontal = 4.dp, vertical = 2.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            T(name, style = Tally.type.ui, color = c.ink, weight = FontWeight.SemiBold, maxLines = 1)
            T(hint, style = Tally.type.caption, color = c.ink3, maxLines = 2)
        }
    }
}

/** A row of the drawer: an icon and a label, the open page on a lilac tint in violet. */
@Composable
fun DrawerRow(label: String, glyph: String, selected: Boolean = false, onClick: () -> Unit) {
    val c = Tally.colors
    Row(
        Modifier.fillMaxWidth().heightIn(min = 46.dp).clip(Radii.key)
            .background(if (selected) Hue.lilacSoft else Color.Transparent)
            .clickable(role = Role.Button, onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Glyph(glyph, 19.dp, if (selected) Hue.violet else c.ink2)
        T(label, Modifier.weight(1f), Tally.type.nav, if (selected) Hue.lilacInk else c.ink, maxLines = 1, weight = if (selected) FontWeight.SemiBold else FontWeight.Medium)
    }
}

/** A choice row of a flow: a tinted glyph tile, a title over a line, and a chevron. */
@Composable
fun ChoiceRow(title: String, sub: String?, glyph: String, tint: Color, onClick: () -> Unit) {
    SettingRow(title, glyph = glyph, glyphTint = tint, subtitle = sub, onClick = onClick)
}
