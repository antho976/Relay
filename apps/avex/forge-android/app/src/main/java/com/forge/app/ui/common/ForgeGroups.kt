@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.common

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.theme.ForgeMotion

/**
 * The grouped-surface kit: one visual language for forms, pickers, lists and chrome across the app
 * (born in onboarding 2026-09-26, promoted here 2026-09-27 so every screen can use it).
 *
 * Answers and rows sit in ONE rounded group of filled members (`surfaceContainerHigh`), joined by
 * 2dp seams, with a large outer radius and a tight inner one: the shape current Android settings
 * use, instead of separately outlined boxes or bare rows split by hairlines. Picking something
 * washes it in the accent and rings it; in a pick-one group the chosen member also rounds its inner
 * corners out so it lifts from the group. One-of-few values are a [ForgeSlidingSegments] control,
 * text is typed inline on its row ([ForgeFieldRow]), glyphs sit on [ForgeGlyphBadge]s, and the top
 * of a screen is a [ForgeTopBar] of filled capsules.
 *
 * Everything reads its colours from the theme, so AMOLED and monochrome follow for free.
 */

/** Small mono anchor over a group, with an optional reading on the right. */
@Composable
internal fun ForgeGroupLabel(text: String, meta: String? = null) {
    Row(
        Modifier.fillMaxWidth().semantics { heading() },
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            text.uppercase(),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            letterSpacing = 1.sp
        )
        if (meta != null) {
            Text(
                meta.uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

/** One quiet explainer line, inside a row or under a group. */
@Composable
internal fun ForgeGroupCaption(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        modifier = modifier,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f)
    )
}

/** Outer corners of a group, and the tight inner corners where two of its members meet. */
// Tightened 2026-09-27 from 20 / 6: at 20dp every list, panel and control read as a pill, and the
// rounding became the look instead of the grouping.
internal val GROUP_OUTER = 12.dp
internal val GROUP_INNER = 4.dp

/** The seam between two members of one group: a gap, never a rule. */
internal val GROUP_SEAM = 2.dp

/** Row gutter inside a group, shared by every row so their text starts on one edge. */
internal val ROW_H = 18.dp

/** Static shape for row [index] of [count] stacked rows. */
internal fun rowShape(index: Int, count: Int): Shape {
    val top = if (index == 0) GROUP_OUTER else GROUP_INNER
    val bottom = if (index == count - 1) GROUP_OUTER else GROUP_INNER
    return RoundedCornerShape(topStart = top, topEnd = top, bottomStart = bottom, bottomEnd = bottom)
}

/**
 * Which corners of a grid member sit on the group's outside edge. Ragged last rows are handled: the
 * member above an empty cell owns the bottom-end corner there.
 */
internal data class Corners(val topStart: Boolean, val topEnd: Boolean, val bottomStart: Boolean, val bottomEnd: Boolean) {
    companion object {
        fun ofRow(index: Int, count: Int) = Corners(index == 0, index == 0, index == count - 1, index == count - 1)

        fun ofGrid(index: Int, count: Int, cols: Int): Corners {
            val rows = (count + cols - 1) / cols
            val r = index / cols
            val c = index % cols
            val lastRowCols = count - (rows - 1) * cols
            val rowCols = if (r == rows - 1) lastRowCols else cols
            val lastRow = r == rows - 1
            // The bottom edge of a column is exposed on the last row, or on the row above it when
            // the last row stops short of this column.
            val bottomExposed = lastRow || (r == rows - 2 && c >= lastRowCols)
            return Corners(
                topStart = r == 0 && c == 0,
                topEnd = r == 0 && c == rowCols - 1,
                bottomStart = lastRow && c == 0,
                bottomEnd = bottomExposed && (c == rowCols - 1 || (r == rows - 2 && c == cols - 1))
            )
        }
    }
}

/** The member's shape, rounding every corner out when it is picked. Animated, so a pick morphs.
 *  Pick-one groups only: in a pick-many grid every lit tile would pop out and the group would
 *  dissolve into loose pills, so those pass `selected = false` here and keep the ring alone. */
@Composable
internal fun memberShape(corners: Corners, selected: Boolean): Shape {
    @Composable
    fun corner(outer: Boolean, label: String): Dp {
        val target = if (selected || outer) GROUP_OUTER else GROUP_INNER
        val r by animateDpAsState(target, ForgeMotion.snappy(), label = label)
        return r
    }
    return RoundedCornerShape(
        topStart = corner(corners.topStart, "ts"),
        topEnd = corner(corners.topEnd, "te"),
        bottomStart = corner(corners.bottomStart, "bs"),
        bottomEnd = corner(corners.bottomEnd, "be")
    )
}

/**
 * The fill every group member wears: the raised surface, the accent wash over it when picked, and an
 * accent ring. The ring and wash come from [selectableColors], the same formula the segmented
 * thumb and the Settings selectables use, so "picked" looks one way across the app.
 */
@Composable
internal fun Modifier.memberFill(shape: Shape, selected: Boolean, enabled: Boolean = true): Modifier {
    val (border, wash) = selectableColors(selected, enabled)
    val ring by animateColorAsState(
        if (selected) border else Color.Transparent,
        ForgeMotion.standardTween(ForgeMotion.DurationFast),
        label = "member_ring"
    )
    return this
        .clip(shape)
        .background(MaterialTheme.colorScheme.surfaceContainerHigh)
        .background(wash)
        .border(1.5.dp, ring, shape)
}

/** A glyph on its own rounded badge, accent when its member is picked. */
@Composable
internal fun ForgeGlyphBadge(icon: ImageVector, selected: Boolean, size: Dp = 40.dp) {
    val badge by animateColorAsState(
        if (selected) MaterialTheme.colorScheme.primary.copy(alpha = 0.15f)
        else MaterialTheme.colorScheme.surfaceContainerHighest,
        ForgeMotion.standardTween(ForgeMotion.DurationFast),
        label = "badge_fill"
    )
    val glyph by animateColorAsState(
        if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
        ForgeMotion.standardTween(ForgeMotion.DurationFast),
        label = "badge_glyph"
    )
    Box(
        Modifier
            .size(size)
            .clip(RoundedCornerShape(10.dp))
            .background(badge),
        contentAlignment = Alignment.Center
    ) {
        Icon(icon, contentDescription = null, tint = glyph, modifier = Modifier.size(size * 0.55f))
    }
}

/** A mono anchor over a group, and an optional footnote under it. */
@Composable
internal fun ForgeGroupSection(
    label: String?,
    meta: String? = null,
    footer: (@Composable () -> Unit)? = null,
    content: @Composable () -> Unit
) {
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        if (label != null) Box(Modifier.padding(horizontal = 4.dp)) { ForgeGroupLabel(label, meta) }
        content()
        if (footer != null) Box(Modifier.padding(horizontal = 4.dp)) { footer() }
    }
}

/** Passive rows stacked into one group: a surface each, 2dp seams, no selection. */
@Composable
internal fun ForgeRowGroup(vararg rows: @Composable () -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(GROUP_SEAM)) {
        rows.forEachIndexed { i, row ->
            Box(
                Modifier
                    .fillMaxWidth()
                    .clip(rowShape(i, rows.size))
                    .background(MaterialTheme.colorScheme.surfaceContainerHigh)
            ) { row() }
        }
    }
}

// ── ForgeChoice list ──────────────────────────────────────────────────────────────

/** One answer of a pick-one list. */
internal data class ForgeChoice(
    val key: String,
    val label: String,
    val description: String? = null,
    val meta: String? = null,
    val icon: ImageVector? = null
)

/**
 * A pick-one question as one group of full-width rows: optional glyph badge, label over a
 * one-line description, and a mono reading at the end. [topContent] draws above a row's text (the
 * plan-mode vignettes).
 */
@Composable
internal fun ForgeChoiceList(
    choices: List<ForgeChoice>,
    selected: String,
    onSelect: (String) -> Unit,
    topContent: (@Composable (ForgeChoice) -> Unit)? = null
) {
    val largeFont = LocalDensity.current.fontScale > 1.3f
    Column(verticalArrangement = Arrangement.spacedBy(GROUP_SEAM)) {
        choices.forEachIndexed { i, choice ->
            val picked = choice.key == selected
            val shape = memberShape(Corners.ofRow(i, choices.size), picked)
            val muted = MaterialTheme.colorScheme.onSurfaceVariant
            Column(
                Modifier
                    .fillMaxWidth()
                    .memberFill(shape, picked)
                    .bounceClick { onSelect(choice.key) }
                    .semantics { this.selected = picked; role = Role.RadioButton }
                    .padding(horizontal = ROW_H, vertical = 16.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp)
            ) {
                topContent?.invoke(choice)
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                    choice.icon?.let { ForgeGlyphBadge(it, picked) }
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        Text(choice.label, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                        if (choice.description != null) {
                            Text(choice.description, style = MaterialTheme.typography.bodySmall, color = muted)
                        }
                        // At a large font scale the reading drops under the text instead of taking
                        // a column the label needs.
                        if (choice.meta != null && largeFont) {
                            Text(
                                choice.meta.uppercase(),
                                style = MaterialTheme.typography.labelSmall,
                                color = if (picked) MaterialTheme.colorScheme.primary else muted,
                                modifier = Modifier.padding(top = 4.dp)
                            )
                        }
                    }
                    if (choice.meta != null && !largeFont) {
                        Text(
                            choice.meta.uppercase(),
                            style = MaterialTheme.typography.labelSmall,
                            color = if (picked) MaterialTheme.colorScheme.primary else muted,
                            textAlign = TextAlign.End
                        )
                    }
                }
            }
        }
    }
}

// ── Tile grid ────────────────────────────────────────────────────────────────

/**
 * Members laid out [cols] across as one connected group. Each cell gets its [Corners] so only the
 * group's own outside corners are fully round, and a short last row leaves its cells empty rather
 * than stretching the remaining tiles.
 */
@Composable
internal fun <T> ForgeTileGrid(
    items: List<T>,
    cols: Int,
    tile: @Composable (item: T, corners: Corners, modifier: Modifier) -> Unit
) {
    Column(verticalArrangement = Arrangement.spacedBy(GROUP_SEAM)) {
        items.chunked(cols).forEachIndexed { r, rowItems ->
            // One height per row: at a large font scale a wrapped name would otherwise leave its
            // neighbour short, and a ragged row reads as broken.
            Row(
                Modifier.fillMaxWidth().height(IntrinsicSize.Min),
                horizontalArrangement = Arrangement.spacedBy(GROUP_SEAM)
            ) {
                rowItems.forEachIndexed { c, item ->
                    tile(item, Corners.ofGrid(r * cols + c, items.size, cols), Modifier.weight(1f).fillMaxHeight())
                }
                repeat(cols - rowItems.size) { Box(Modifier.weight(1f)) }
            }
        }
    }
}

/** A toggleable gear tile: badge over a two-line name. */
@Composable
internal fun ForgeGearTile(
    icon: ImageVector,
    label: String,
    selected: Boolean,
    corners: Corners,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val shape = memberShape(corners, selected = false)
    Column(
        modifier
            .memberFill(shape, selected)
            .bounceClick(onClick = onClick)
            .semantics { this.selected = selected; role = Role.Checkbox }
            .padding(horizontal = 6.dp, vertical = 14.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        ForgeGlyphBadge(icon, selected)
        // minLines keeps a row of tiles one height; no maxLines, so a long name wraps at 200%.
        Text(
            label,
            style = MaterialTheme.typography.bodySmall,
            color = if (selected) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            minLines = 2
        )
    }
}

/** A text tile: a name and a mono reading under it, with an optional leading badge. */
@Composable
internal fun ForgeLabelTile(
    label: String,
    meta: String,
    selected: Boolean,
    corners: Corners,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    role: Role = Role.Checkbox,
    icon: ImageVector? = null
) {
    val shape = memberShape(corners, selected && role == Role.RadioButton)
    Column(
        modifier
            .memberFill(shape, selected)
            .bounceClick(onClick = onClick)
            .semantics { this.selected = selected; this.role = role }
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        icon?.let { ForgeGlyphBadge(it, selected) }
        Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Text(
                label,
                style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.onBackground,
                minLines = if (icon != null) 2 else 1
            )
            Text(
                meta.uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

// ── Segmented control ────────────────────────────────────────────────────────

/**
 * A sliding segmented control: a recessed track, equal cells, and one thumb that glides to the
 * picked cell. The thumb is the app's selectable formula (accent ring over an accent wash).
 * [selectedIndex] of -1 draws no thumb: a question nobody has answered yet.
 *
 * Past 1.3x font scale equal cells would break words mid-label, so worded options wrap as separate
 * capsules instead; the selectable drawing is the same either way.
 */
@Composable
internal fun ForgeSlidingSegments(
    options: List<String>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier
) {
    // Short numeric cells ("1" to "7") never break, so only worded options fall back.
    if (LocalDensity.current.fontScale > 1.3f && options.size > 2 && options.any { it.length > 3 }) {
        FlowRow(
            modifier,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            options.forEachIndexed { i, label -> ForgeChoiceChip(label, i == selectedIndex, { onSelect(i) }) }
        }
        return
    }
    // A rounded rectangle, not a pill: the track shares the group's corner and the thumb nests
    // inside it, inset by its 3dp gap.
    val track = RoundedCornerShape(GROUP_OUTER)
    val thumb = RoundedCornerShape(GROUP_OUTER - 3.dp)
    BoxWithConstraints(
        modifier
            .clip(track)
            .background(MaterialTheme.colorScheme.surfaceContainerLowest)
    ) {
        val cell = maxWidth / options.size
        val (border, fill) = selectableColors(selected = selectedIndex >= 0)
        val thumbX by animateDpAsState(
            cell * selectedIndex.coerceAtLeast(0),
            ForgeMotion.snappy(),
            label = "segment_thumb"
        )
        if (selectedIndex >= 0) {
            Box(Modifier.matchParentSize()) {
                Box(
                    Modifier
                        .offset(x = thumbX)
                        .width(cell)
                        .fillMaxHeight()
                        .padding(3.dp)
                        .clip(thumb)
                        .background(fill)
                        .border(1.dp, border, thumb)
                )
            }
        }
        Row(Modifier.fillMaxWidth()) {
            options.forEachIndexed { i, label ->
                val picked = i == selectedIndex
                Box(
                    Modifier
                        .weight(1f)
                        .heightIn(min = 44.dp)
                        .bounceClick { onSelect(i) }
                        .semantics { selected = picked; role = Role.RadioButton }
                        .padding(horizontal = 4.dp, vertical = 10.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        label,
                        style = MaterialTheme.typography.bodyMedium,
                        color = if (picked) MaterialTheme.colorScheme.onBackground
                        else MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = TextAlign.Center
                    )
                }
            }
        }
    }
}

// ── Rows ─────────────────────────────────────────────────────────────────────

/**
 * A label with its text typed inline at the row's end, where a settings value sits. Tapping
 * anywhere on the row puts the cursor in the field, so the whole row is the target rather than the
 * few characters of the value.
 */
@Composable
internal fun ForgeFieldRow(
    label: String,
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    keyboardOptions: KeyboardOptions,
    keyboardActions: KeyboardActions = KeyboardActions.Default,
    suffix: String? = null,
    isError: Boolean = false,
    focusRequester: FocusRequester = remember { FocusRequester() }
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val textStyle = MaterialTheme.typography.bodyLarge.copy(
        color = if (isError) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onBackground,
        textAlign = TextAlign.End
    )
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .pointerInput(Unit) { detectTapGestures { focusRequester.requestFocus() } }
            .padding(horizontal = ROW_H, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Text(label, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            modifier = Modifier
                .weight(1f)
                .focusRequester(focusRequester)
                .semantics { contentDescription = label },
            textStyle = textStyle,
            singleLine = true,
            cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
            keyboardOptions = keyboardOptions,
            keyboardActions = keyboardActions,
            decorationBox = { field ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.weight(1f), contentAlignment = Alignment.CenterEnd) {
                        if (value.isEmpty()) {
                            Text(
                                placeholder,
                                style = textStyle.copy(color = muted.copy(alpha = 0.6f))
                            )
                        }
                        field()
                    }
                    if (suffix != null) {
                        Spacer(Modifier.width(6.dp))
                        Text(suffix, style = MaterialTheme.typography.bodyLarge, color = muted)
                    }
                }
            }
        )
    }
}

/** A label with a compact segmented control at its end. At a large font scale the control drops
 *  under the label rather than squeezing it (both must stay whole at 200%). */
@Composable
internal fun ForgeChoiceRow(label: String, options: List<String>, selectedIndex: Int, onSelect: (Int) -> Unit) {
    val stacked = LocalDensity.current.fontScale > 1.3f
    val labelText: @Composable () -> Unit = {
        Text(label, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
    }
    if (stacked) {
        Column(
            Modifier.fillMaxWidth().padding(horizontal = ROW_H, vertical = 14.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            labelText()
            ForgeSlidingSegments(options, selectedIndex, onSelect, Modifier.fillMaxWidth())
        }
    } else {
        Row(
            Modifier.fillMaxWidth().padding(start = ROW_H, end = 8.dp, top = 8.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Box(Modifier.weight(1f)) { labelText() }
            ForgeSlidingSegments(
                options, selectedIndex, onSelect,
                // Two short units sit as a compact pill; a three-way choice takes more of the row.
                modifier = Modifier.width(if (options.size > 2) 248.dp else 132.dp)
            )
        }
    }
}

/** A label, its reading on the right, one explainer line, and a full-width control under them. */
@Composable
internal fun ForgeBlockRow(label: String, meta: String?, explainer: String, content: @Composable () -> Unit) {
    Column(
        Modifier.fillMaxWidth().padding(horizontal = ROW_H, vertical = 16.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                label,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onBackground,
                modifier = Modifier.weight(1f)
            )
            if (meta != null) {
                Text(
                    meta.uppercase(),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
        ForgeGroupCaption(explainer)
        Spacer(Modifier.height(8.dp))
        content()
    }
}

/**
 * A boolean with its one-line explainer and a drawn [ForgeSwitch]. The WHOLE row is the tap target
 * and the switch is passive, never a nested tap. A row that cannot run renders inert.
 */
@Composable
internal fun ForgeSwitchRow(
    label: String,
    description: String,
    checked: Boolean,
    onToggle: (Boolean) -> Unit,
    enabled: Boolean = true
) {
    val alpha = if (enabled) 1f else 0.35f
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .then(if (enabled) Modifier.clickableLabeled(label) { onToggle(!checked) } else Modifier)
            // The switch is drawn, not focusable, so the row announces the state.
            .semantics { stateDescription = if (checked && enabled) "On" else "Off" }
            .padding(horizontal = ROW_H, vertical = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                label,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onBackground.copy(alpha = alpha)
            )
            ForgeGroupCaption(description)
        }
        Box(Modifier.clearAndSetSemantics { }) {
            ForgeSwitch(checked = checked && enabled, onCheckedChange = null, enabled = enabled)
        }
    }
}


// ── Chrome ───────────────────────────────────────────────────────────────────

/**
 * The chrome's two controls, back and skip, as small raised capsules on the flow's surface: 44dp
 * tall so they read as buttons at a glance, with the 48dp target coming from the interaction box.
 */
@Composable
internal fun ForgeChromeButton(
    onClick: () -> Unit,
    label: String,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit
) {
    Box(
        modifier = modifier
            .minimumInteractiveComponentSize()
            .heightIn(min = 44.dp)
            .widthIn(min = 44.dp)
            .clip(RoundedCornerShape(50))
            .background(MaterialTheme.colorScheme.surfaceContainerHigh)
            .clickableLabeled(label, onClick = onClick),
        contentAlignment = Alignment.Center
    ) { content() }
}


/**
 * A filled secondary capsule with a text label: the sidekick beside a [ForgePrimaryCapsule] (Close,
 * Cancel, Re-roll), replacing the outlined capsule in the grouped-surface look. [destructive] tints
 * the label with `error` so a delete still reads as one.
 */
@Composable
internal fun ForgeSecondaryCapsule(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    destructive: Boolean = false
) {
    ForgeChromeButton(onClick = onClick, label = label, modifier = modifier) {
        Text(
            label,
            style = MaterialTheme.typography.titleSmall,
            color = if (destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onBackground,
            textAlign = TextAlign.Center,
            // Same padding as ForgePrimaryCapsule so a primary and its sidekick stand one height.
            modifier = Modifier.padding(horizontal = 18.dp, vertical = 13.dp)
        )
    }
}

/** A glyph-only chrome capsule (an add, an edit, a share) for the end of a [ForgeTopBar]. */
@Composable
internal fun ForgeChromeIconButton(
    icon: ImageVector,
    label: String,
    onClick: () -> Unit,
    tint: Color = MaterialTheme.colorScheme.onBackground
) {
    ForgeChromeButton(onClick = onClick, label = label) {
        // The glyph carries the capsule's name: a click label alone gives TalkBack an action with
        // nothing to call it.
        Icon(icon, contentDescription = label, tint = tint, modifier = Modifier.size(20.dp))
    }
}

/**
 * The app's top bar: a round filled back capsule at the start and optional chrome capsules at the
 * end, on the page itself (no bar fill), inset below the status bar. It never names the screen; the
 * page's own serif title does that. [title] exists only for the rare bar that must carry a live
 * value (a selection count), not for the screen's name.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun ForgeTopBar(
    onBack: (() -> Unit)?,
    modifier: Modifier = Modifier,
    backLabel: String = "Back",
    title: String? = null,
    actions: @Composable RowScope.() -> Unit = {}
) {
    Row(
        modifier
            .fillMaxWidth()
            .windowInsetsPadding(TopAppBarDefaults.windowInsets)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        if (onBack != null) {
            ForgeChromeIconButton(Icons.AutoMirrored.Filled.ArrowBack, backLabel, onBack)
        }
        Box(Modifier.weight(1f)) {
            if (title != null) {
                Text(
                    title,
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onBackground,
                    modifier = Modifier.padding(start = 4.dp)
                )
            }
        }
        actions()
    }
}
