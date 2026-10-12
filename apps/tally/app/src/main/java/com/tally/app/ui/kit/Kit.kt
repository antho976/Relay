package com.tally.app.ui.kit

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.tally.app.ui.theme.Palette
import com.tally.app.ui.theme.Tally

/** Relay's radii (DESIGN.md "Shapes"). */
object Radii {
    val key = RoundedCornerShape(10.dp)
    val icon = RoundedCornerShape(7.dp)
    val badge = RoundedCornerShape(9.dp)
    val tab = RoundedCornerShape(9.dp)
    val popover = RoundedCornerShape(12.dp)
    val card = RoundedCornerShape(14.dp)
    val sheet = RoundedCornerShape(topStart = 14.dp, topEnd = 14.dp)
    val pill = RoundedCornerShape(999.dp)
}

/** A touch target never shrinks below this. */
val TOUCH = 44.dp

// ---- Text ----

@Composable
fun T(
    text: String,
    modifier: Modifier = Modifier,
    style: TextStyle = Tally.type.ui,
    color: Color = Tally.colors.ink,
    maxLines: Int = Int.MAX_VALUE,
    weight: FontWeight? = null,
    align: TextAlign? = null,
) = BasicText(
    text = text,
    modifier = modifier,
    style = style.merge(TextStyle(color = color, fontWeight = weight ?: style.fontWeight, textAlign = align ?: style.textAlign)),
    maxLines = maxLines,
    overflow = TextOverflow.Ellipsis,
)

/** A section heading in sentence case, as Relay's sidebar's "Workspaces"; [trailing] holds its keys. */
@Composable
fun SectionLabel(text: String, modifier: Modifier = Modifier, trailing: @Composable RowScope.() -> Unit = {}) {
    Row(modifier.fillMaxWidth().padding(start = 4.dp, top = 14.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        T(text, Modifier.weight(1f), Tally.type.section, Tally.colors.ink3)
        trailing()
    }
}

/** Small capitals over a group, as Relay's settings ("ROLE"). */
@Composable
fun Eyebrow(text: String, modifier: Modifier = Modifier, color: Color = Tally.colors.ink3) =
    T(text.uppercase(), modifier, Tally.type.eyebrow, color, maxLines = 1)

/** A quiet line of mono text: a figure's caption, a status. */
@Composable
fun Mono(text: String, modifier: Modifier = Modifier, color: Color = Tally.colors.ink3) =
    T(text, modifier, Tally.type.mono, color, maxLines = 1)

// ---- Keys ----

enum class KeyKind { Plain, Primary, Quiet, Danger }

/**
 * A key (DESIGN.md "Buttons"): slab with a strong stroke; Primary is ink with ground text, the one
 * per surface; Quiet is transparent ink-2; Danger reads in held red.
 */
@Composable
fun Key(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    kind: KeyKind = KeyKind.Plain,
    glyph: String? = null,
    enabled: Boolean = true,
    compact: Boolean = false,
) {
    val c = Tally.colors
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val (bg, fg, border) = when (kind) {
        KeyKind.Plain -> Triple(if (pressed) c.wash else c.slab, c.ink, c.strong)
        KeyKind.Primary -> Triple(if (pressed) Color.White else c.ink, c.wall, Color.Transparent)
        KeyKind.Quiet -> Triple(if (pressed) c.wash else Color.Transparent, c.ink2, Color.Transparent)
        KeyKind.Danger -> Triple(if (pressed) c.wash else c.slab, c.heldText, c.strong)
    }
    Row(
        modifier
            .alpha(if (enabled) 1f else .4f)
            .heightIn(min = if (compact) 36.dp else 44.dp)
            .clip(Radii.key)
            .background(bg)
            .border(1.dp, border, Radii.key)
            .clickable(source, null, enabled = enabled, role = Role.Button, onClick = onClick)
            .padding(horizontal = if (kind == KeyKind.Primary) 16.dp else 12.dp),
        horizontalArrangement = Arrangement.spacedBy(7.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        glyph?.let { Glyph(it, 15.dp, fg) }
        T(text, style = Tally.type.uiMedium, color = fg, maxLines = 1, weight = if (kind == KeyKind.Primary) FontWeight.SemiBold else FontWeight.Medium)
    }
}

/** An icon key: transparent, ink-3, a 44dp target around a 7dp-rounded face. */
@Composable
fun IconKey(
    glyph: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    size: Dp = 18.dp,
    tint: Color = Tally.colors.ink3,
    enabled: Boolean = true,
    selected: Boolean = false,
    label: String? = null,
    badge: Int = 0,
) {
    val c = Tally.colors
    Box(
        modifier
            .size(TOUCH)
            .alpha(if (enabled) 1f else .4f)
            .clip(Radii.icon)
            .background(if (selected) c.wash else Color.Transparent)
            .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
            .then(if (label != null) Modifier.semantics { contentDescription = label } else Modifier),
        contentAlignment = Alignment.Center,
    ) {
        Glyph(glyph, size, if (selected) c.ink else tint)
        if (badge > 0) {
            Box(Modifier.align(Alignment.TopEnd).padding(top = 6.dp, end = 4.dp)) { CountBadge(badge) }
        }
    }
}

/** A count on an ink pill, as Relay's bell's: Geist Mono, "99+" at most. */
@Composable
fun CountBadge(n: Int, color: Color = Tally.colors.ink) {
    Box(
        Modifier.defaultMinSize(minWidth = 16.dp, minHeight = 16.dp).clip(Radii.pill).background(color).padding(horizontal = 4.dp),
        contentAlignment = Alignment.Center,
    ) {
        T(if (n > 99) "99+" else n.toString(), style = Tally.type.mono.copy(fontSize = 10.sp), color = Tally.colors.wall, weight = FontWeight.Medium)
    }
}

// ---- Fields ----

/** A field (DESIGN.md "Inputs"): raised fill, a strong stroke, line-focus when focused. */
@Composable
fun Field(
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    placeholder: String = "",
    singleLine: Boolean = true,
    minLines: Int = 1,
    maxLines: Int = if (singleLine) 1 else 6,
    style: TextStyle = Tally.type.ui,
    leading: String? = null,
    error: Boolean = false,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    keyboardActions: KeyboardActions = KeyboardActions.Default,
    visualTransformation: VisualTransformation = VisualTransformation.None,
    trailing: @Composable (RowScope.() -> Unit)? = null,
) {
    val c = Tally.colors
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    BasicTextField(
        value = value,
        onValueChange = onValueChange,
        modifier = modifier,
        textStyle = style.merge(TextStyle(color = c.ink)),
        singleLine = singleLine,
        minLines = minLines,
        maxLines = maxLines,
        cursorBrush = SolidColor(c.ink),
        interactionSource = source,
        keyboardOptions = keyboardOptions,
        keyboardActions = keyboardActions,
        visualTransformation = visualTransformation,
        decorationBox = { inner ->
            Row(
                Modifier
                    .heightIn(min = 44.dp)
                    .clip(Radii.key)
                    .background(c.raised)
                    .border(1.dp, if (error) c.held else if (focused) c.lineFocus else c.strong, Radii.key)
                    .padding(horizontal = 12.dp, vertical = 10.dp),
                verticalAlignment = if (singleLine) Alignment.CenterVertically else Alignment.Top,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                leading?.let { Glyph(it, 16.dp, c.ink3) }
                Box(Modifier.weight(1f)) {
                    if (value.isEmpty()) T(placeholder, style = style, color = c.ink3, maxLines = if (singleLine) 1 else 3)
                    inner()
                }
                trailing?.invoke(this)
            }
        },
    )
}

/** A labelled field: the label over it, a reason under it when there is one. */
@Composable
fun Labelled(label: String, modifier: Modifier = Modifier, problem: String? = null, hint: String? = null, content: @Composable () -> Unit) {
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        T(label, style = Tally.type.caption, color = Tally.colors.ink3, weight = FontWeight.Medium)
        content()
        when {
            problem != null -> T(problem, style = Tally.type.caption, color = Tally.colors.heldText)
            hint != null -> T(hint, style = Tally.type.caption, color = Tally.colors.ink3)
        }
    }
}

// ---- Surfaces ----

/** A card: slab, a thin edge, 14dp corners. */
@Composable
fun Slab(
    modifier: Modifier = Modifier,
    padding: PaddingValues = PaddingValues(14.dp),
    color: Color = Tally.colors.slab,
    edge: Color = Tally.colors.edge,
    shape: Shape = Radii.card,
    onClick: (() -> Unit)? = null,
    onLongClick: (() -> Unit)? = null,
    spacing: Dp = 0.dp,
    content: @Composable ColumnScope.() -> Unit,
) {
    Column(
        modifier
            .clip(shape)
            .background(color)
            .border(1.dp, edge, shape)
            .then(if (onClick != null || onLongClick != null) Modifier.combinedClickable(onLongClick = onLongClick, onClick = onClick ?: {}) else Modifier)
            .padding(padding),
        verticalArrangement = Arrangement.spacedBy(spacing),
        content = content,
    )
}

/**
 * A titled card, as the PC's panels: a glyph and a title over the content, an optional key at the
 * right ("All", "Edit"). Rows inside sit flush; the card holds the padding.
 */
@Composable
fun Panel(
    title: String,
    modifier: Modifier = Modifier,
    glyph: String? = null,
    action: String? = null,
    onAction: (() -> Unit)? = null,
    subtitle: String? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val c = Tally.colors
    Slab(modifier.fillMaxWidth(), padding = PaddingValues(start = 14.dp, end = 6.dp, top = 6.dp, bottom = 10.dp)) {
        Row(Modifier.fillMaxWidth().heightIn(min = 40.dp).padding(end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            glyph?.let { Glyph(it, 15.dp, c.ink3); HGap(8.dp) }
            Column(Modifier.weight(1f)) {
                T(title, style = Tally.type.section, color = c.ink2, maxLines = 1)
                subtitle?.let { T(it, style = Tally.type.caption, color = c.ink3, maxLines = 1) }
            }
            if (action != null && onAction != null) {
                T(action, Modifier.clip(Radii.key).clickable(role = Role.Button, onClick = onAction).padding(horizontal = 10.dp, vertical = 8.dp), Tally.type.caption, c.ink2, weight = FontWeight.Medium)
            }
        }
        Column(Modifier.padding(end = 8.dp)) { content() }
    }
}

/** A row in a list: at least a touch target tall, wash when pressed or selected. */
@Composable
fun ListRow(
    modifier: Modifier = Modifier,
    selected: Boolean = false,
    onClick: (() -> Unit)? = null,
    onLongClick: (() -> Unit)? = null,
    padding: PaddingValues = PaddingValues(horizontal = 12.dp, vertical = 8.dp),
    shape: Shape = Radii.key,
    content: @Composable RowScope.() -> Unit,
) {
    val c = Tally.colors
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = TOUCH)
            .clip(shape)
            .background(if (selected) c.wash else Color.Transparent)
            .then(if (onClick != null || onLongClick != null) Modifier.combinedClickable(onLongClick = onLongClick, onClick = onClick ?: {}) else Modifier)
            .padding(padding),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        content = content,
    )
}

/** A navigation row of the drawer: icon, label, an optional count, wash when selected. */
@Composable
fun NavRow(label: String, glyph: String, selected: Boolean = false, count: String? = null, onClick: () -> Unit) {
    val c = Tally.colors
    ListRow(selected = selected, onClick = onClick) {
        Glyph(glyph, 18.dp, if (selected) c.ink else c.ink3)
        T(label, Modifier.weight(1f), Tally.type.nav, if (selected) c.ink else c.ink2, maxLines = 1)
        count?.let { T(it, style = Tally.type.mono, color = c.ink3) }
    }
}

/** A settings row: a glyph, a title over a line, and what sits at its end (a value, a switch, a chevron). */
@Composable
fun SettingRow(
    title: String,
    modifier: Modifier = Modifier,
    glyph: String? = null,
    subtitle: String? = null,
    value: String? = null,
    titleColor: Color = Tally.colors.ink,
    onClick: (() -> Unit)? = null,
    chevron: Boolean = onClick != null && value == null,
    trailing: (@Composable RowScope.() -> Unit)? = null,
) {
    val c = Tally.colors
    ListRow(modifier, onClick = onClick, padding = PaddingValues(horizontal = 4.dp, vertical = 10.dp)) {
        glyph?.let { Glyph(it, 17.dp, c.ink3) }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            T(title, style = Tally.type.ui, color = titleColor, maxLines = 2)
            subtitle?.let { T(it, style = Tally.type.caption, color = c.ink3, maxLines = 4) }
        }
        value?.let { T(it, style = Tally.type.ui, color = c.ink2, maxLines = 1) }
        trailing?.invoke(this)
        if (chevron) Glyph("chevron-right", 14.dp, c.ink3)
    }
}

/** A row with a switch at its end; the whole row toggles it. */
@Composable
fun ToggleRow(title: String, checked: Boolean, onChange: (Boolean) -> Unit, modifier: Modifier = Modifier, glyph: String? = null, subtitle: String? = null, enabled: Boolean = true) {
    SettingRow(title, modifier.alpha(if (enabled) 1f else .5f), glyph = glyph, subtitle = subtitle, onClick = if (enabled) ({ onChange(!checked) }) else null, chevron = false) {
        Toggle(checked, onChange, enabled = enabled)
    }
}

@Composable
fun Hairline(modifier: Modifier = Modifier, color: Color = Tally.colors.lineSubtle) =
    Box(modifier.fillMaxWidth().height(1.dp).background(color))

@Composable
fun Dot(color: Color, size: Dp = 6.dp, modifier: Modifier = Modifier) = Box(modifier.size(size).clip(CircleShape).background(color))

/** A small pill: a filter chip, a choice, a state badge. */
@Composable
fun Pill(
    text: String,
    modifier: Modifier = Modifier,
    color: Color = Tally.colors.ink2,
    fill: Color = Tally.colors.slab,
    edge: Color = Tally.colors.strong,
    selected: Boolean = false,
    dot: Color? = null,
    glyph: String? = null,
    onClick: (() -> Unit)? = null,
) {
    val c = Tally.colors
    Row(
        modifier
            .heightIn(min = 32.dp)
            .clip(Radii.pill)
            .background(if (selected) c.ink else fill)
            .border(1.dp, if (selected) Color.Transparent else edge, Radii.pill)
            .then(if (onClick != null) Modifier.clickable(role = Role.Button, onClick = onClick) else Modifier)
            .padding(horizontal = 12.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        dot?.let { Dot(it) }
        glyph?.let { Glyph(it, 13.dp, if (selected) c.wall else color) }
        T(text, style = Tally.type.caption, color = if (selected) c.wall else color, maxLines = 1, weight = FontWeight.Medium)
    }
}

/**
 * The view switch (DESIGN.md "Navigation"): slab, 12dp outside, 9dp tabs; the showing one lit with
 * track and ink.
 */
@Composable
fun <V> Segmented(options: List<Pair<V, String>>, selected: V, onSelect: (V) -> Unit, modifier: Modifier = Modifier, fill: Boolean = false) {
    val c = Tally.colors
    Row(
        modifier.clip(Radii.popover).background(c.slab).border(1.dp, c.edge, Radii.popover).padding(3.dp),
        horizontalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        for ((value, label) in options) {
            val on = value == selected
            Box(
                (if (fill) Modifier.weight(1f) else Modifier)
                    .heightIn(min = 36.dp)
                    .clip(Radii.tab)
                    .background(if (on) c.track else Color.Transparent)
                    .clickable(role = Role.Tab) { onSelect(value) }
                    .padding(horizontal = 12.dp),
                contentAlignment = Alignment.Center,
            ) {
                T(label, style = Tally.type.uiMedium, color = if (on) c.ink else c.ink3, maxLines = 1)
            }
        }
    }
}

/** A switch (settings.css): 44×26, ink when on. */
@Composable
fun Toggle(checked: Boolean, onChange: (Boolean) -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true) {
    val c = Tally.colors
    val x by animateFloatAsState(if (checked) 1f else 0f, label = "toggle")
    Box(
        modifier
            .alpha(if (enabled) 1f else .4f)
            .size(width = 44.dp, height = 26.dp)
            .clip(Radii.pill)
            .background(if (checked) c.ink else c.track)
            .clickable(enabled = enabled, role = Role.Switch) { onChange(!checked) }
            .padding(3.dp),
    ) {
        Box(
            Modifier
                .padding(start = (18 * x).dp)
                .size(20.dp)
                .clip(CircleShape)
                .background(if (checked) c.wall else c.ink3),
        )
    }
}

/** A thin meter: ink, amber from 85%, red past 100% unless a colour is given. */
@Composable
fun Meter(fraction: Float, modifier: Modifier = Modifier, color: Color? = null, height: Dp = 5.dp) {
    val c = Tally.colors
    val f = fraction.coerceIn(0f, 1f)
    val fill = color ?: when {
        fraction > 1f -> c.held
        fraction >= .85f -> c.waiting
        else -> c.ink
    }
    Canvas(modifier.fillMaxWidth().height(height)) {
        val r = CornerRadius(size.height / 2)
        drawRoundRect(c.track, cornerRadius = r)
        if (f > 0f) drawRoundRect(fill, size = Size(size.width * f, size.height), cornerRadius = r)
    }
}

/**
 * A budget's meter with its pace tick: the fill is what was spent, the tick is where an even spend
 * would be today. Over the tick reads amber, over the budget red, under it ink.
 */
@Composable
fun PaceMeter(spent: Float, pace: Float, modifier: Modifier = Modifier, height: Dp = 6.dp) {
    val c = Tally.colors
    val fill = when {
        spent > 1f -> c.held
        spent > pace + 0.02f -> c.waiting
        else -> c.ink
    }
    Canvas(modifier.fillMaxWidth().height(height + 6.dp)) {
        val bar = height.toPx()
        val top = (size.height - bar) / 2
        val r = CornerRadius(bar / 2)
        drawRoundRect(c.track, topLeft = Offset(0f, top), size = Size(size.width, bar), cornerRadius = r)
        val f = spent.coerceIn(0f, 1f)
        if (f > 0f) drawRoundRect(fill, topLeft = Offset(0f, top), size = Size(size.width * f, bar), cornerRadius = r)
        val x = size.width * pace.coerceIn(0f, 1f)
        drawLine(c.ink2, Offset(x, 0f), Offset(x, size.height), strokeWidth = 2.dp.toPx(), cap = StrokeCap.Round)
    }
}

/** A figure in a small card: caption, figure, a line under it. */
@Composable
fun Stat(caption: String, figure: String, modifier: Modifier = Modifier, sub: String? = null, glyph: String? = null, figureColor: Color = Tally.colors.ink, subColor: Color = Tally.colors.ink3, onClick: (() -> Unit)? = null) {
    val c = Tally.colors
    Column(
        modifier.clip(Radii.card).background(c.slab).border(1.dp, c.edge, Radii.card)
            .then(if (onClick != null) Modifier.clickable(role = Role.Button, onClick = onClick) else Modifier)
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            glyph?.let { Glyph(it, 13.dp, c.ink3) }
            T(caption, style = Tally.type.caption, color = c.ink3, maxLines = 1)
        }
        T(figure, style = Tally.type.figure, color = figureColor, maxLines = 1)
        if (!sub.isNullOrEmpty()) T(sub, style = Tally.type.caption, color = subColor, maxLines = 2)
    }
}

/** Stats two to a row, each the height of the taller. */
@Composable
fun StatGrid(stats: List<@Composable (Modifier) -> Unit>, modifier: Modifier = Modifier) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(10.dp)) {
        for (pair in stats.chunked(2)) {
            Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                for (s in pair) s(Modifier.weight(1f).fillMaxHeight())
                if (pair.size == 1) Box(Modifier.weight(1f))
            }
        }
    }
}

/**
 * Tally's mark in Relay's hand: four ink tally strokes, the fourth dimmed, crossed by the fifth in
 * Relay's signal green (the launcher icon draws the same).
 */
@Composable
fun TallyMark(size: Dp, modifier: Modifier = Modifier, ink: Color = Tally.colors.ink, dim: Color = Palette.DIM, signal: Color = Tally.colors.signal) {
    Canvas(modifier.size(size)) {
        val u = this.size.minDimension / 48f
        val w = 3.2f * u
        for ((i, x) in listOf(14f, 20f, 26f, 32f).withIndex()) {
            drawLine(if (i == 3) dim else ink, Offset(x * u, 12f * u), Offset(x * u, 36f * u), strokeWidth = w, cap = StrokeCap.Round)
        }
        drawLine(signal, Offset(8.5f * u, 30.5f * u), Offset(39.5f * u, 17.5f * u), strokeWidth = w * 1.1f, cap = StrokeCap.Round)
    }
}

/** A quiet empty state: a glyph, a sentence, an optional way forward. */
@Composable
fun Empty(glyph: String, title: String, body: String? = null, modifier: Modifier = Modifier, action: (@Composable () -> Unit)? = null) {
    Column(modifier.fillMaxWidth().padding(horizontal = 28.dp, vertical = 32.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Glyph(glyph, 28.dp, Tally.colors.ink3)
        T(title, style = Tally.type.title, color = Tally.colors.ink, align = TextAlign.Center)
        body?.let { T(it, style = Tally.type.caption, color = Tally.colors.ink3, align = TextAlign.Center) }
        action?.let { Spacer(Modifier.height(4.dp)); it() }
    }
}

/** "just now", "5m ago", "3h ago", "2d ago". */
fun ago(epochMs: Long, now: Long = System.currentTimeMillis()): String {
    val s = (now - epochMs) / 1000
    return when {
        s < 45 -> "just now"
        s < 3600 -> "${s / 60}m ago"
        s < 86400 -> "${s / 3600}h ago"
        else -> "${s / 86400}d ago"
    }
}

@Composable
fun Gap(h: Dp = 8.dp) = Spacer(Modifier.height(h))

@Composable
fun RowScope.Fill() = Spacer(Modifier.weight(1f))

@Composable
fun HGap(w: Dp = 8.dp) = Spacer(Modifier.width(w))

/** Constrains a page's content to a readable column on a tablet or a phone on its side. */
fun Modifier.column(max: Dp = 720.dp) = this.widthIn(max = max)
