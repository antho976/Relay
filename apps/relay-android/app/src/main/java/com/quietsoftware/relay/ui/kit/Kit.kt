package com.quietsoftware.relay.ui.kit

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.background
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.indication
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.layout.layout
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.quietsoftware.relay.core.model.Lamp
import com.quietsoftware.relay.ui.theme.Relay

/** The PC's radii (DESIGN.md "Shapes"). */
object Radii {
    val key = RoundedCornerShape(14.dp)
    val icon = RoundedCornerShape(7.dp)
    val keycap = RoundedCornerShape(6.dp)
    val tab = RoundedCornerShape(9.dp)
    val popover = RoundedCornerShape(12.dp)
    val card = RoundedCornerShape(16.dp)
    val sheet = RoundedCornerShape(topStart = 24.dp, topEnd = 24.dp)
    val pill = RoundedCornerShape(999.dp)
    /** Board cards, task pages, Git and Files rows. */
    val board = RoundedCornerShape(14.dp)
}

/** A touch target never shrinks below this, whatever the PC's row height was. */
val TOUCH = 44.dp

// ---- Text ----

@Composable
fun T(
    text: String,
    modifier: Modifier = Modifier,
    style: TextStyle = Relay.type.ui,
    color: Color = Relay.colors.ink,
    maxLines: Int = Int.MAX_VALUE,
    weight: FontWeight? = null,
) = androidx.compose.foundation.text.BasicText(
    text = text,
    modifier = modifier,
    style = style.merge(TextStyle(color = color, fontWeight = weight ?: style.fontWeight)),
    maxLines = maxLines,
    overflow = TextOverflow.Ellipsis,
)

/** A section heading in sentence case, as the sidebar's "Workspaces". */
@Composable
fun SectionLabel(text: String, modifier: Modifier = Modifier, trailing: @Composable RowScope.() -> Unit = {}) {
    Row(modifier.fillMaxWidth().padding(start = 4.dp, top = 14.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        T(text, Modifier.weight(1f), Relay.type.section, Relay.colors.ink3)
        trailing()
    }
}

/** Small capitals over a group, as the PC's settings and launch sheet ("ROLE"). */
@Composable
fun Eyebrow(text: String, modifier: Modifier = Modifier, color: Color = Relay.colors.ink3) =
    T(text.uppercase(), modifier, Relay.type.eyebrow, color)

// ---- Keys ----

enum class KeyKind { Plain, Primary, Quiet, Danger }

/**
 * A key (DESIGN.md "Buttons"): surface with a line-strong stroke; Primary is the accent with dark
 * text, the one per surface; Quiet is transparent ink-2; Danger reads in held red.
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
    val c = Relay.colors
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val (bg, fg, border) = when (kind) {
        KeyKind.Plain -> Triple(if (pressed) c.wash else c.slab, c.ink, c.strong)
        KeyKind.Primary -> Triple(if (pressed) c.accentText else c.accent, c.onAccent, Color.Transparent)
        KeyKind.Quiet -> Triple(if (pressed) c.wash else Color.Transparent, c.ink2, Color.Transparent)
        KeyKind.Danger -> Triple(if (pressed) c.wash else c.slab, c.heldText, c.strong)
    }
    Row(
        modifier
            .alpha(if (enabled) 1f else .4f)
            .heightIn(min = if (compact) 34.dp else 44.dp)
            .clip(Radii.key)
            .background(bg)
            .border(1.dp, border, Radii.key)
            .clickable(source, null, enabled = enabled, role = Role.Button, onClick = onClick)
            .padding(horizontal = if (kind == KeyKind.Primary) 16.dp else 12.dp),
        horizontalArrangement = Arrangement.spacedBy(7.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        glyph?.let { Glyph(it, 15.dp, fg) }
        T(text, style = Relay.type.uiMedium, color = fg, maxLines = 1, weight = if (kind == KeyKind.Primary) FontWeight.SemiBold else FontWeight.Medium)
    }
}

/**
 * An icon key: transparent, ink-3, a 44dp target around a 7dp-rounded face. [label] is what a
 * screen reader says for it; without one it says what the glyph shows ("Back", "More").
 */
@Composable
fun IconKey(
    glyph: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    size: Dp = 18.dp,
    tint: Color = Relay.colors.ink3,
    enabled: Boolean = true,
    selected: Boolean = false,
    onLongClick: (() -> Unit)? = null,
    badge: Int = 0,
    badgeColor: Color = Relay.colors.ink,
    label: String? = null,
) {
    val c = Relay.colors
    val spoken = label ?: spokenGlyph(glyph)
    Box(
        modifier
            .size(TOUCH)
            .alpha(if (enabled) 1f else .4f)
            .clip(Radii.icon)
            .background(if (selected) c.wash else Color.Transparent)
            .combinedClickable(enabled = enabled, role = Role.Button, onLongClick = onLongClick, onClick = onClick)
            .semantics { contentDescription = spoken },
        contentAlignment = Alignment.Center,
    ) {
        Glyph(glyph, size, if (selected) c.ink else tint)
        if (badge > 0) {
            Box(Modifier.align(Alignment.TopEnd).padding(top = 6.dp, end = 4.dp)) { CountBadge(badge, badgeColor) }
        }
    }
}

/** What an icon key with no label of its own is called: what its glyph shows. */
internal fun spokenGlyph(glyph: String): String = when (glyph) {
    "arrow-left" -> "Back"
    "sidebar" -> "Menu"
    "bell" -> "Inbox"
    "trash" -> "Delete"
    "plus" -> "Add"
    "minus" -> "Remove"
    "file-plus" -> "New file"
    "focus" -> "Show"
    else -> glyph.replace('-', ' ').replaceFirstChar { it.uppercase() }
}

/** A count on an ink pill, as the bell's (theme.css): Geist Mono, "99+" at most. */
@Composable
fun CountBadge(n: Int, color: Color = Relay.colors.ink) {
    Box(
        Modifier.defaultMinSize(minWidth = 16.dp, minHeight = 16.dp).clip(Radii.pill).background(color).padding(horizontal = 4.dp),
        contentAlignment = Alignment.Center,
    ) {
        T(if (n > 99) "99+" else n.toString(), style = Relay.type.mono.copy(fontSize = androidx.compose.ui.unit.TextUnit(10f, androidx.compose.ui.unit.TextUnitType.Sp)), color = Relay.colors.wall, weight = FontWeight.Medium)
    }
}

// ---- Fields ----

/** A field (DESIGN.md "Inputs"): raised fill, line-strong stroke, line-focus when focused. */
@Composable
fun Field(
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    placeholder: String = "",
    singleLine: Boolean = true,
    minLines: Int = 1,
    maxLines: Int = if (singleLine) 1 else 8,
    style: TextStyle = Relay.type.ui,
    leading: String? = null,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    keyboardActions: KeyboardActions = KeyboardActions.Default,
    visualTransformation: VisualTransformation = VisualTransformation.None,
    trailing: @Composable (RowScope.() -> Unit)? = null,
) {
    val c = Relay.colors
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
                    .heightIn(min = 42.dp)
                    .clip(Radii.key)
                    .background(c.raised)
                    .border(1.dp, if (focused) c.lineFocus else c.strong, Radii.key)
                    .padding(horizontal = 12.dp, vertical = 9.dp),
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

// ---- Surfaces ----

/** A card: slab, a thin edge, 14dp corners. */
@Composable
fun Slab(
    modifier: Modifier = Modifier,
    padding: PaddingValues = PaddingValues(14.dp),
    color: Color = Relay.colors.slab,
    edge: Color = Relay.colors.edge,
    shape: androidx.compose.ui.graphics.Shape = Radii.card,
    onClick: (() -> Unit)? = null,
    onLongClick: (() -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    Column(
        modifier
            .clip(shape)
            .background(color)
            .border(1.dp, edge, shape)
            .then(if (onClick != null || onLongClick != null) Modifier.combinedClickable(onLongClick = onLongClick, onClick = onClick ?: {}) else Modifier)
            .padding(padding),
        content = content,
    )
}

/** A row in a list: at least a touch target tall, wash when pressed or selected. */
@Composable
fun ListRow(
    modifier: Modifier = Modifier,
    selected: Boolean = false,
    onClick: (() -> Unit)? = null,
    onLongClick: (() -> Unit)? = null,
    padding: PaddingValues = PaddingValues(horizontal = 12.dp, vertical = 8.dp),
    shape: androidx.compose.ui.graphics.Shape = Radii.key,
    content: @Composable RowScope.() -> Unit,
) {
    val c = Relay.colors
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

/** A navigation row of the sidebar: icon, label, an optional count, wash when selected. */
@Composable
fun NavRow(label: String, glyph: String, selected: Boolean = false, count: String? = null, onClick: () -> Unit) {
    val c = Relay.colors
    ListRow(selected = selected, onClick = onClick) {
        Glyph(glyph, 18.dp, if (selected) c.ink else c.ink3)
        T(label, Modifier.weight(1f), Relay.type.nav, if (selected) c.ink else c.ink2, maxLines = 1)
        count?.let { T(it, style = Relay.type.mono, color = c.ink3) }
    }
}

@Composable
fun Hairline(modifier: Modifier = Modifier, color: Color = Relay.colors.lineSubtle) =
    Box(modifier.fillMaxWidth().height(1.dp).background(color))

// ---- State ----

/** True while the PC is out of reach: lamps show what was last known, so they dim. */
val LocalStaleLamps = compositionLocalOf { false }

/**
 * How bright a lamp is drawn: full while the PC is in reach, dimmed to its last known state
 * while it is not, eased so a quick first connection does not blink.
 */
@Composable
fun lampAlpha(): Float = animateFloatAsState(if (LocalStaleLamps.current) STALE_LAMP else 1f, tween(LAMP_FADE_MS), label = "lamp").value

private const val STALE_LAMP = .4f
private const val LAMP_FADE_MS = 400

/** A session lamp (DESIGN.md "Lamps"): a filled dot, or an outline when off; dimmed while the PC is away. */
@Composable
fun LampDot(lamp: Lamp, size: Dp = 7.dp, modifier: Modifier = Modifier) {
    val c = Relay.colors
    val fill = when (lamp) {
        Lamp.Live -> c.live
        Lamp.Held -> c.held
        Lamp.Waiting -> c.waiting
        Lamp.Off -> null
    }?.let { it.copy(alpha = it.alpha * lampAlpha()) }
    Box(
        modifier.size(size).clip(CircleShape)
            .then(if (fill != null) Modifier.background(fill) else Modifier.border(1.dp, c.ink3, CircleShape)),
    )
}

@Composable
fun Dot(color: Color, size: Dp = 6.dp, modifier: Modifier = Modifier) = Box(modifier.size(size).clip(CircleShape).background(color))

/**
 * A small pill: a filter chip, a label, a state badge. One that takes a tap is [PILL_TOUCH]
 * tall to the finger with its face as small as before, and the press is drawn on the face alone.
 */
@Composable
fun Pill(
    text: String,
    modifier: Modifier = Modifier,
    color: Color = Relay.colors.ink2,
    fill: Color = Relay.colors.slab,
    edge: Color = Relay.colors.strong,
    selected: Boolean = false,
    dot: Color? = null,
    glyph: String? = null,
    onClick: (() -> Unit)? = null,
) {
    val c = Relay.colors
    val press = remember { MutableInteractionSource() }
    val pressLook = LocalIndication.current
    Row(
        modifier
            .then(if (onClick != null) Modifier.clickable(interactionSource = press, indication = null, onClick = onClick).touchHeight(PILL_TOUCH) else Modifier)
            .heightIn(min = 28.dp)
            .clip(Radii.pill)
            .background(if (selected) c.ink else fill)
            .border(1.dp, if (selected) Color.Transparent else edge, Radii.pill)
            .then(if (onClick != null) Modifier.indication(press, pressLook) else Modifier)
            .then(if (selected) Modifier.semantics { this.selected = true } else Modifier)
            .padding(horizontal = 10.dp, vertical = 3.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        dot?.let { Dot(it) }
        glyph?.let { Glyph(it, 13.dp, if (selected) c.wall else color) }
        T(text, style = Relay.type.caption, color = if (selected) c.wall else color, maxLines = 1, weight = FontWeight.Medium)
    }
}

/** How tall a pill that takes a tap is to the finger. */
private val PILL_TOUCH = 40.dp

/** At least [min] tall, with what follows centred in it at its own height (a caller's taller face stays). */
private fun Modifier.touchHeight(min: Dp) = layout { measurable, constraints ->
    val face = measurable.measure(constraints)
    val height = maxOf(face.height, min.roundToPx()).coerceAtMost(constraints.maxHeight)
    layout(face.width, height) { face.place(0, (height - face.height) / 2) }
}

/**
 * The view switch (DESIGN.md "Navigation"): slab, 12dp outside, 9dp tabs; the showing one lit
 * with track and ink.
 */
@Composable
fun <T> Segmented(options: List<Pair<T, String>>, selected: T, onSelect: (T) -> Unit, modifier: Modifier = Modifier, fill: Boolean = false) {
    val c = Relay.colors
    Row(
        modifier.clip(Radii.popover).background(c.slab).border(1.dp, c.edge, Radii.popover).padding(3.dp).selectableGroup(),
        horizontalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        for ((value, label) in options) {
            val on = value == selected
            Box(
                (if (fill) Modifier.weight(1f) else Modifier)
                    .heightIn(min = 34.dp)
                    .clip(Radii.tab)
                    .background(if (on) c.track else Color.Transparent)
                    .selectable(selected = on, role = Role.Tab) { onSelect(value) }
                    .padding(horizontal = 12.dp),
                contentAlignment = Alignment.Center,
            ) {
                T(label, style = Relay.type.uiMedium, color = if (on) c.ink else c.ink3, maxLines = 1)
            }
        }
    }
}

/**
 * Tabs as a row of pills (design 14): the showing one filled with ink, the rest outlined; the row
 * scrolls sideways when it is wider than the screen.
 */
@Composable
fun <T> PillTabs(options: List<Pair<T, String>>, selected: T, onSelect: (T) -> Unit, modifier: Modifier = Modifier) {
    val c = Relay.colors
    Row(
        modifier.horizontalScroll(rememberScrollState()).selectableGroup(),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        for ((value, label) in options) {
            val on = value == selected
            Box(
                Modifier.heightIn(min = 36.dp).clip(Radii.pill)
                    .background(if (on) c.ink else Color.Transparent)
                    .border(1.dp, if (on) c.ink else c.strong, Radii.pill)
                    .selectable(selected = on, role = Role.Tab) { onSelect(value) }
                    .padding(horizontal = 14.dp),
                contentAlignment = Alignment.Center,
            ) {
                T(label, style = Relay.type.uiMedium, color = if (on) c.wall else c.ink, maxLines = 1, weight = if (on) FontWeight.SemiBold else FontWeight.Medium)
            }
        }
    }
}

/**
 * A switch (settings.css): 44×26, the accent when on. A row that is all one switch puts [switchRow] on
 * itself and passes no [onChange] here, so its words take the tap too and it is read as one switch.
 */
@Composable
fun Toggle(checked: Boolean, onChange: ((Boolean) -> Unit)?, modifier: Modifier = Modifier, enabled: Boolean = true) {
    val c = Relay.colors
    val x by animateFloatAsState(if (checked) 1f else 0f, label = "toggle")
    Box(
        modifier
            .alpha(if (enabled) 1f else .4f)
            .size(width = 44.dp, height = 26.dp)
            .clip(Radii.pill)
            .background(if (checked) c.accent else c.track)
            .then(if (onChange != null) Modifier.toggleable(value = checked, enabled = enabled, role = Role.Switch, onValueChange = onChange) else Modifier)
            .padding(3.dp),
    ) {
        Box(
            Modifier
                .padding(start = (18 * x).dp)
                .size(20.dp)
                .clip(CircleShape)
                .background(if (checked) Color.White else c.ink3),
        )
    }
}

/** A thin meter: amber from 70%, red from 90% (the usage card's rule) unless a colour is given. */
@Composable
fun Meter(fraction: Float, modifier: Modifier = Modifier, color: Color? = null, height: Dp = 4.dp) {
    val c = Relay.colors
    val f = fraction.coerceIn(0f, 1f)
    val fill = color ?: when {
        f >= .9f -> c.held
        f >= .7f -> c.waiting
        else -> c.ink
    }
    Canvas(modifier.fillMaxWidth().height(height)) {
        val r = androidx.compose.ui.geometry.CornerRadius(size.height / 2)
        drawRoundRect(c.track, cornerRadius = r)
        if (f > 0f) drawRoundRect(fill, size = Size(size.width * f, size.height), cornerRadius = r)
    }
}

/** A quiet empty state: a glyph, a sentence, an optional way forward. */
@Composable
fun Empty(glyph: String, title: String, body: String? = null, modifier: Modifier = Modifier, action: (@Composable () -> Unit)? = null) {
    Column(modifier.fillMaxWidth().padding(horizontal = 32.dp, vertical = 40.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Glyph(glyph, 28.dp, Relay.colors.ink3)
        T(title, style = Relay.type.title, color = Relay.colors.ink)
        body?.let { T(it, style = Relay.type.caption.copy(textAlign = androidx.compose.ui.text.style.TextAlign.Center), color = Relay.colors.ink3) }
        action?.let { Spacer(Modifier.height(4.dp)); it() }
    }
}

/**
 * A line saying the data shown is not the PC's answer just now, and how old it is: the phone's
 * copy, or what the server's copy answered while the PC was away (docs/CLOUD.md).
 */
@Composable
fun StaleNote(at: Long, modifier: Modifier = Modifier, cloud: Boolean = false) {
    if (at <= 0) return
    Row(modifier.padding(horizontal = 4.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Dot(Relay.colors.waiting, 5.dp)
        T(staleText(at, cloud), style = Relay.type.caption, color = Relay.colors.ink3)
    }
}

/** A whole row that is one switch: the tap anywhere on it, and one "on"/"off" for a screen reader. */
fun Modifier.switchRow(checked: Boolean, enabled: Boolean = true, onChange: (Boolean) -> Unit): Modifier =
    toggleable(value = checked, enabled = enabled, role = Role.Switch, onValueChange = onChange)

/** "1 task", "3 tasks". */
fun plural(n: Long, word: String): String = if (n == 1L) "1 $word" else "$n ${word}s"

fun plural(n: Int, word: String): String = plural(n.toLong(), word)

/** What [StaleNote] says, for a caption laid out its own way; [cloud] when the server's copy answered. */
fun staleText(at: Long, cloud: Boolean = false): String = (if (cloud) "From your server's copy" else "Saved on this phone") + " · ${ago(at)}"

/** "just now", "5m ago", "3h ago", "2d ago", "3w ago", then "on Mar 1": the desktop's (relative.rs `CompactAgo`). */
fun ago(epochMs: Long, now: Long = System.currentTimeMillis()): String {
    val s = maxOf(0L, (now - epochMs) / 1000)
    return when {
        s < 60 -> "just now"
        s < 3600 -> "${s / 60}m ago"
        s < 86_400 -> "${s / 3600}h ago"
        s < 7 * 86_400 -> "${s / 86_400}d ago"
        s < 35 * 86_400 -> "${s / (7 * 86_400)}w ago"
        else -> "on " + java.time.Instant.ofEpochMilli(epochMs).atZone(java.time.ZoneId.systemDefault())
            .format(java.time.format.DateTimeFormatter.ofPattern("MMM d", java.util.Locale.ENGLISH))
    }
}

/** RFC 3339 from the PC to epoch ms; 0 when it does not parse. */
fun epoch(ts: String?): Long = ts?.let { runCatching { java.time.Instant.parse(it).toEpochMilli() }.getOrNull() } ?: 0L

@Composable
fun Gap(h: Dp = 8.dp) = Spacer(Modifier.height(h))

@Composable
fun RowScope.Fill() = Spacer(Modifier.weight(1f))

@Composable
fun HGap(w: Dp = 8.dp) = Spacer(Modifier.width(w))

/** Constrains a page's content to a readable column on a tablet or a phone on its side. */
fun Modifier.column(max: Dp = 720.dp) = this.widthIn(max = max)
