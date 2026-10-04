@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package com.tally.app.ui.common

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.tally.app.ui.theme.TallyMotion

/*
 * The grouped-surface kit, from Avex: interactive things sit in ONE rounded group of filled slabs
 * joined by 2dp seams (a gap, never a rule), with a 16dp outer radius and a tight 4dp inner one.
 * Passive things sit bare on the page. A box is a promise of a tap.
 */

val GROUP_OUTER = 16.dp
val GROUP_INNER = 4.dp
val GROUP_SEAM = 2.dp
val ROW_PAD = 18.dp

fun rowShape(index: Int, count: Int): Shape {
    val top = if (index == 0) GROUP_OUTER else GROUP_INNER
    val bottom = if (index == count - 1) GROUP_OUTER else GROUP_INNER
    return RoundedCornerShape(topStart = top, topEnd = top, bottomStart = bottom, bottomEnd = bottom)
}

/** The sans group header over a slab group (the Settings voice), with an optional reading. */
@Composable
fun GroupHeader(title: String, modifier: Modifier = Modifier, trailing: String? = null) {
    EndsRow(
        start = {
            Text(
                title,
                style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.semantics { heading() },
            )
        },
        end = if (trailing != null) {
            { Text(trailing, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
        } else {
            null
        },
        modifier = modifier.fillMaxWidth().padding(start = ROW_PAD, end = ROW_PAD, bottom = 10.dp),
    )
}

/** The one note under a group. [isError] for the quiet inline error line. */
@Composable
fun GroupFooter(text: String, modifier: Modifier = Modifier, isError: Boolean = false) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = if (isError) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier.fillMaxWidth().padding(start = ROW_PAD, end = ROW_PAD, top = 10.dp),
    )
}

/**
 * A titled group. Rows are passed as a list so each can be given its corner shape; the group
 * rounds the outside and leaves 2dp seams between members.
 */
@Composable
fun Group(
    rows: List<@Composable (Shape) -> Unit>,
    modifier: Modifier = Modifier,
    title: String? = null,
    trailing: String? = null,
    footer: String? = null,
    footerIsError: Boolean = false,
) {
    Column(modifier.fillMaxWidth()) {
        if (title != null) GroupHeader(title, trailing = trailing)
        Column(verticalArrangement = Arrangement.spacedBy(GROUP_SEAM)) {
            rows.forEachIndexed { i, row -> row(rowShape(i, rows.size)) }
        }
        if (footer != null) GroupFooter(footer, isError = footerIsError)
    }
}

/** A slab: the raised fill of one group member. */
@Composable
fun Modifier.slab(shape: Shape): Modifier = this
    .fillMaxWidth()
    .clip(shape)
    .background(MaterialTheme.colorScheme.surfaceContainerHigh)

/** A free-form block inside a group (a picker grid, a chart, a control). */
@Composable
fun GroupBlock(shape: Shape, modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(modifier.slab(shape).padding(ROW_PAD), content = content)
}

/** A glyph on its rounded badge: the leading mark of a navigation or list row. */
@Composable
fun GlyphBadge(
    icon: ImageVector,
    modifier: Modifier = Modifier,
    tint: Color = MaterialTheme.colorScheme.onBackground,
    fill: Color = MaterialTheme.colorScheme.surfaceContainerHighest,
    size: Dp = 44.dp,
) {
    Box(
        modifier.size(size).clip(RoundedCornerShape(12.dp)).background(fill),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, contentDescription = null, tint = tint, modifier = Modifier.size(size * 0.5f))
    }
}

/**
 * The base row: a slab, its tap, and a ≥56dp height that grows with its text. [onClick] null
 * renders it passive (no press, no chevron): nothing looks tappable while doing nothing.
 */
@Composable
fun GroupRow(
    title: String,
    shape: Shape,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    leading: (@Composable () -> Unit)? = null,
    trailing: (@Composable RowScope.() -> Unit)? = null,
    chevron: Boolean = true,
    titleColor: Color = MaterialTheme.colorScheme.onBackground,
    onClick: (() -> Unit)? = null,
) {
    Row(
        modifier
            .slab(shape)
            .then(if (onClick != null) Modifier.bounceClick(label = title, focusShape = shape, onClick = onClick) else Modifier)
            .heightIn(min = 56.dp)
            .padding(horizontal = ROW_PAD, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        leading?.invoke()
        // The title and the trailing reading share the line until a word of the title would break;
        // then the reading moves under it (200% font, long amounts).
        EndsRow(
            start = {
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(title, style = MaterialTheme.typography.bodyLarge, color = titleColor)
                    if (subtitle != null) {
                        Text(subtitle, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            },
            end = if (trailing != null) {
                { Row(verticalAlignment = Alignment.CenterVertically) { trailing(this) } }
            } else {
                null
            },
            modifier = Modifier.weight(1f),
            gap = 16.dp,
        )
        if (chevron && onClick != null) {
            Icon(
                Icons.AutoMirrored.Rounded.KeyboardArrowRight,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

/**
 * A boolean row. The WHOLE row toggles; the switch is drawn, not a second tap target, and the
 * row announces the state.
 */
@Composable
fun SwitchRow(
    title: String,
    checked: Boolean,
    onToggle: (Boolean) -> Unit,
    shape: Shape,
    subtitle: String? = null,
    enabled: Boolean = true,
) {
    val alpha = if (enabled) 1f else 0.35f
    Row(
        Modifier
            .slab(shape)
            .then(if (enabled) Modifier.bounceClick(label = title, role = Role.Switch, focusShape = shape) { onToggle(!checked) } else Modifier)
            .semantics { stateDescription = if (checked) "On" else "Off" }
            .heightIn(min = 56.dp)
            .padding(horizontal = ROW_PAD, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground.copy(alpha = alpha))
            if (subtitle != null) {
                Text(subtitle, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = alpha))
            }
        }
        Box(Modifier.clearAndSetSemantics { }) {
            Switch(
                checked = checked,
                onCheckedChange = null,
                enabled = enabled,
                thumbContent = if (checked) {
                    { Icon(Icons.Rounded.Check, contentDescription = null, modifier = Modifier.size(SwitchDefaults.IconSize)) }
                } else null,
                colors = SwitchDefaults.colors(
                    checkedTrackColor = MaterialTheme.colorScheme.primary,
                    checkedThumbColor = MaterialTheme.colorScheme.background,
                    checkedIconColor = MaterialTheme.colorScheme.primary,
                    uncheckedTrackColor = MaterialTheme.colorScheme.surfaceContainerHighest,
                    uncheckedThumbColor = MaterialTheme.colorScheme.onSurfaceVariant,
                    uncheckedBorderColor = MaterialTheme.colorScheme.outline,
                ),
            )
        }
    }
}

/** The selectable formula: an accent ring over an accent wash when picked, the outline rung when not. */
@Composable
fun selectableColors(selected: Boolean): Pair<Color, Color> {
    val border by animateColorAsState(
        if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline.copy(alpha = 0.35f),
        TallyMotion.standard(TallyMotion.Fast), label = "sel_border",
    )
    val fill by animateColorAsState(
        if (selected) MaterialTheme.colorScheme.primary.copy(alpha = 0.15f) else Color.Transparent,
        TallyMotion.standard(TallyMotion.Fast), label = "sel_fill",
    )
    return border to fill
}

/**
 * A sliding segmented control: recessed track, equal cells, one thumb that glides to the pick.
 * Past 1.3x font scale worded options wrap into separate capsules so no word breaks; they still
 * announce as tabs, so the control reads the same at every size.
 */
@Composable
fun SlidingSegments(
    options: List<String>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    if (LocalDensity.current.fontScale > 1.3f && options.size > 2) {
        androidx.compose.foundation.layout.FlowRow(
            modifier.selectableGroup(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            options.forEachIndexed { i, label -> ChoiceChip(label, i == selectedIndex, role = Role.Tab) { onSelect(i) } }
        }
        return
    }
    val track = RoundedCornerShape(12.dp)
    val thumb = RoundedCornerShape(10.dp)
    BoxWithConstraints(modifier.clip(track).background(MaterialTheme.colorScheme.surfaceContainerLowest).selectableGroup()) {
        val cell = maxWidth / options.size
        val (border, fill) = selectableColors(selected = selectedIndex >= 0)
        val x by animateDpAsState(cell * selectedIndex.coerceAtLeast(0), TallyMotion.snappy(), label = "thumb")
        if (selectedIndex >= 0) {
            Box(Modifier.matchParentSize()) {
                Box(
                    Modifier
                        .offset(x = x)
                        .width(cell)
                        .fillMaxHeight()
                        .padding(3.dp)
                        .clip(thumb)
                        .background(fill)
                        .border(1.5.dp, border, thumb)
                )
            }
        }
        Row(Modifier.fillMaxWidth()) {
            options.forEachIndexed { i, label ->
                val picked = i == selectedIndex
                Box(
                    Modifier
                        .weight(1f)
                        .heightIn(min = 48.dp)
                        .bounceClick(label = label, role = Role.Tab) { onSelect(i) }
                        .semantics { selected = picked }
                        .padding(horizontal = 4.dp, vertical = 12.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        label,
                        style = MaterialTheme.typography.bodyLarge,
                        color = if (picked) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = TextAlign.Center,
                    )
                }
            }
        }
    }
}

/**
 * A capsule choice chip: outline when idle, ring and wash when picked. ≥48dp touch. A chip is one
 * of several and only one is picked, so it announces as a radio button; put its row in a
 * `Modifier.selectableGroup()` so TalkBack reads its place in the set. A chip that acts rather
 * than picks (a note suggestion) passes [Role.Button].
 */
@Composable
fun ChoiceChip(
    label: String,
    selected: Boolean,
    modifier: Modifier = Modifier,
    role: Role = Role.RadioButton,
    onClick: () -> Unit,
) {
    val (border, fill) = selectableColors(selected)
    val shape = RoundedCornerShape(50)
    Box(
        modifier
            .heightIn(min = 48.dp)
            .padding(vertical = 4.dp)
            .clip(shape)
            .background(fill)
            .border(1.dp, border, shape)
            .bounceClick(label = label, role = role, focusShape = shape, onClick = onClick)
            .semantics { if (role != Role.Button) this.selected = selected }
            .padding(horizontal = 16.dp, vertical = 9.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            label,
            style = MaterialTheme.typography.bodyMedium,
            color = if (selected) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/** A row label with a compact segmented control at its end; stacked at large font scales. */
@Composable
fun ChoiceRow(title: String, options: List<String>, selectedIndex: Int, onSelect: (Int) -> Unit, shape: Shape) {
    val stacked = LocalDensity.current.fontScale > 1.3f || options.size > 3
    if (stacked) {
        Column(Modifier.slab(shape).padding(horizontal = ROW_PAD, vertical = 14.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
            SlidingSegments(options, selectedIndex, onSelect, Modifier.fillMaxWidth())
        }
    } else {
        Row(
            Modifier.slab(shape).padding(start = ROW_PAD, end = 8.dp, top = 8.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground, modifier = Modifier.weight(1f))
            Spacer(Modifier.width(12.dp))
            SlidingSegments(options, selectedIndex, onSelect, Modifier.width(if (options.size > 2) 220.dp else 150.dp))
        }
    }
}
