@file:OptIn(
    androidx.compose.foundation.layout.ExperimentalLayoutApi::class,
    androidx.compose.material3.ExperimentalMaterial3Api::class
)

package com.forge.app.ui.settings

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.FlowRowScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.rounded.OpenInNew
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Remove
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.UnfoldMore
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.RadioButton
import androidx.compose.material3.RadioButtonDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.TopAppBarScrollBehavior
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.material3.rememberTopAppBarState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.toggleableLabeled
import com.forge.app.ui.common.window.DropdownMenu
import com.forge.app.ui.theme.ForgeMotion
import kotlin.math.roundToInt

/*
 * # The Settings kit (2026-09-26)
 *
 * Settings is an Operate surface: people come here to change one thing and leave. It used to speak
 * the editorial language of Home (flat rows on the page, tracked mono anchors, capsule buttons in
 * the mono label voice, `→ ▾ ✕` characters standing in for icons), and read like a terminal. This
 * kit is the native-grade replacement, kept inside Avex's warm world:
 *
 * - Rows live in **grouped containers**: each row is its own `surfaceContainerHigh` slab with 4dp
 *   corners, 2dp apart, and the group clip rounds the outer corners to 16dp. The gap separates rows
 *   without a single divider, and every slab is a tap target, so the fill is earned.
 * - **Sans everywhere.** [SettingsTheme] re-maps the mono label styles to sans for this screen only,
 *   so every Material control (buttons, chips, menus) reads in the same voice as the rows. The serif
 *   survives where it carries the brand: the collapsing page title and the live preview figures.
 * - **Accent means state**: a switch that is on, the picked radio, the primary button.
 * - Real controls: M3 switches, radios and filter chips, a sliding segmented control, drawn icons.
 */

// ── Tokens ───────────────────────────────────────────────────────────────────────────────────────

/** Page margin: screen edge to the group's edge. */
internal val KitGutter = 16.dp

/** Inside a row: its edge to its content. Group headers and footers align to the same inset. */
internal val KitRowPad = 16.dp

private val KitOuterRadius = 16.dp
private val KitInnerRadius = 4.dp
private val KitGap = 2.dp

/** Space between two groups on a page. */
internal val KitGroupSpacing = 28.dp

internal val KitGroupShape = RoundedCornerShape(KitOuterRadius)
private val KitInnerShape = RoundedCornerShape(KitInnerRadius)
private val PillShape = RoundedCornerShape(50)

/**
 * The shape a row clips itself to. Inside a [SettingsGroup] it is the small inner radius and the
 * group's clip supplies the outer corners; in a lazy list, where no container spans the rows, each
 * row is handed its full per-position shape through [kitRowShape].
 */
internal val LocalKitRowShape = compositionLocalOf<Shape> { KitInnerShape }

/** "Rear Delts" → "Rear delts": Settings labels are sentence case, whatever the data spells. */
internal fun String.sentenceCase(): String = lowercase().replaceFirstChar { it.uppercase() }

/** A row's shape by its position in a group of [count], for lazy lists. */
internal fun kitRowShape(index: Int, count: Int): Shape {
    val o = KitOuterRadius
    val i = KitInnerRadius
    return when {
        count <= 1 -> KitGroupShape
        index == 0 -> RoundedCornerShape(topStart = o, topEnd = o, bottomStart = i, bottomEnd = i)
        index == count - 1 -> RoundedCornerShape(topStart = i, topEnd = i, bottomStart = o, bottomEnd = o)
        else -> KitInnerShape
    }
}

/** The spacing a lazy row leaves under itself: the in-group gap, or a group break after the last. */
internal fun kitRowSpacing(index: Int, count: Int): Dp = if (index == count - 1) 0.dp else KitGap

@Composable
internal fun kitRowColor(): Color = MaterialTheme.colorScheme.surfaceContainerHigh

@Composable
private fun kitTileColor(): Color = MaterialTheme.colorScheme.surfaceContainerHighest

// ── Theme ────────────────────────────────────────────────────────────────────────────────────────

/**
 * The Settings voice: the app's theme with its three mono label styles swapped for sans. Material
 * draws button, chip, menu and segmented labels in `labelLarge`, which is mono app-wide, and that
 * alone made every control here look typed. Scoped to Settings, so no other screen moves.
 */
@Composable
internal fun SettingsTheme(content: @Composable () -> Unit) {
    val base = MaterialTheme.typography
    val typography = remember(base) {
        // Built from the sans rungs the theme already owns, so the scale stays one scale.
        val small = base.bodySmall.copy(fontWeight = FontWeight.Medium, letterSpacing = 0.3.sp)
        base.copy(labelLarge = base.titleSmall, labelMedium = small, labelSmall = small)
    }
    MaterialTheme(
        colorScheme = MaterialTheme.colorScheme,
        shapes = MaterialTheme.shapes,
        typography = typography,
        content = content
    )
}

// ── Page scaffolds ───────────────────────────────────────────────────────────────────────────────

/**
 * The large collapsing title every Settings page opens on: serif at full size, settling into the
 * sans bar title as the page scrolls under it.
 */
@Composable
internal fun SettingsTopBar(title: String, onBack: () -> Unit, scrollBehavior: TopAppBarScrollBehavior) {
    LargeTopAppBar(
        title = { SettingsTopBarTitle(title) },
        navigationIcon = {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
            }
        },
        colors = TopAppBarDefaults.topAppBarColors(
            containerColor = Color.Transparent,
            scrolledContainerColor = MaterialTheme.colorScheme.surfaceContainerLow,
            navigationIconContentColor = MaterialTheme.colorScheme.onSurface,
            titleContentColor = MaterialTheme.colorScheme.onSurface
        ),
        scrollBehavior = scrollBehavior
    )
}

@Composable
private fun SettingsTopBarTitle(title: String) {
    Text(title, modifier = Modifier.semantics { heading() })
}

/** A Settings page whose content is a short, fixed column of groups. */
@Composable
internal fun SettingsScaffold(
    title: String,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit
) {
    val behavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior(rememberTopAppBarState())
    Scaffold(
        modifier = modifier.nestedScroll(behavior.nestedScrollConnection),
        topBar = { SettingsTopBar(title, onBack, behavior) },
        containerColor = Color.Transparent
    ) { inner ->
        Column(
            Modifier
                .fillMaxSize()
                .padding(top = inner.calculateTopPadding())
                .verticalScroll(rememberScrollState())
                .padding(
                    start = KitGutter,
                    end = KitGutter,
                    top = 8.dp,
                    bottom = inner.calculateBottomPadding() + 40.dp
                ),
            verticalArrangement = Arrangement.spacedBy(KitGroupSpacing),
            content = content
        )
    }
}

/** A Settings page backed by a lazy list — the root list and the long exercise list. */
@Composable
internal fun SettingsLazyScaffold(
    title: String,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
    listState: LazyListState = rememberLazyListState(),
    verticalArrangement: Arrangement.Vertical = Arrangement.Top,
    content: LazyListScope.() -> Unit
) {
    val behavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior(rememberTopAppBarState())
    Scaffold(
        modifier = modifier.nestedScroll(behavior.nestedScrollConnection),
        topBar = { SettingsTopBar(title, onBack, behavior) },
        containerColor = Color.Transparent
    ) { inner ->
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize().padding(top = inner.calculateTopPadding()),
            contentPadding = PaddingValues(
                start = KitGutter,
                end = KitGutter,
                top = 8.dp,
                bottom = inner.calculateBottomPadding() + 40.dp
            ),
            verticalArrangement = verticalArrangement,
            content = content
        )
    }
}

// ── Groups ───────────────────────────────────────────────────────────────────────────────────────

/** The label over a group: sans, medium weight, aligned with the rows' content. */
@Composable
internal fun SettingsGroupHeader(title: String, modifier: Modifier = Modifier, trailing: String? = null) {
    Row(
        modifier.fillMaxWidth().padding(start = KitRowPad, end = KitRowPad, bottom = 10.dp),
        verticalAlignment = Alignment.Bottom,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text(
            title,
            style = MaterialTheme.typography.titleSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.weight(1f).semantics { heading() }
        )
        if (trailing != null) {
            Text(trailing, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/** The note under a group. Set [isError] for the quiet inline error line. */
@Composable
internal fun SettingsGroupFooter(text: String, modifier: Modifier = Modifier, isError: Boolean = false) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = if (isError) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier.fillMaxWidth().padding(start = KitRowPad, end = KitRowPad, top = 10.dp)
    )
}

/**
 * A titled group of rows. The rows draw their own 4dp slabs 2dp apart; this clip rounds the outer
 * corners, so conditional rows and expanding rows keep the right shape with no bookkeeping.
 */
@Composable
internal fun SettingsGroup(
    title: String? = null,
    modifier: Modifier = Modifier,
    footer: String? = null,
    footerIsError: Boolean = false,
    headerTrailing: String? = null,
    content: @Composable () -> Unit
) {
    Column(modifier.fillMaxWidth()) {
        if (title != null) SettingsGroupHeader(title, trailing = headerTrailing)
        KitStack(Modifier.fillMaxWidth().clip(KitGroupShape), content)
        if (footer != null) SettingsGroupFooter(footer, isError = footerIsError)
    }
}

/**
 * The group's column: children stacked [KitGap] apart, with no gap around a child that measures
 * zero high. A collapsed expander at the end of a group would otherwise leave a 2dp strip under the
 * last row, and the group's rounded clip would land on that strip instead of on the row.
 */
@Composable
private fun KitStack(modifier: Modifier, content: @Composable () -> Unit) {
    Layout(content, modifier) { measurables, constraints ->
        val gap = KitGap.roundToPx()
        val loose = constraints.copy(minHeight = 0)
        val placeables = measurables.map { it.measure(loose) }
        var height = 0
        var shown = 0
        placeables.forEach { p ->
            if (p.height > 0) {
                if (shown > 0) height += gap
                height += p.height
                shown++
            }
        }
        val width = (placeables.maxOfOrNull { it.width } ?: 0).coerceIn(constraints.minWidth, constraints.maxWidth)
        layout(width, height.coerceIn(constraints.minHeight, constraints.maxHeight)) {
            var y = 0
            var placed = 0
            placeables.forEach { p ->
                if (p.height > 0 && placed > 0) y += gap
                p.placeRelative(0, y)
                if (p.height > 0) {
                    y += p.height
                    placed++
                }
            }
        }
    }
}

/** A row-shaped slab for anything that is not a row: chip sets, tile grids, previews. */
@Composable
internal fun SettingsGroupBlock(
    modifier: Modifier = Modifier,
    padding: PaddingValues = PaddingValues(KitRowPad),
    content: @Composable ColumnScope.() -> Unit
) {
    Column(
        modifier
            .fillMaxWidth()
            .clip(LocalKitRowShape.current)
            .background(kitRowColor())
            .padding(padding),
        content = content
    )
}

// ── Rows ─────────────────────────────────────────────────────────────────────────────────────────

/** The base row: its slab, its tap, and a ≥56dp height that grows with the text inside it. */
@Composable
internal fun SettingsRowContainer(
    modifier: Modifier = Modifier,
    interaction: Modifier = Modifier,
    /** Extra start inset for a row that belongs to the row above it (a signal's write-back). */
    indent: Dp = 0.dp,
    content: @Composable RowScope.() -> Unit
) {
    Row(
        modifier
            .fillMaxWidth()
            .clip(LocalKitRowShape.current)
            .background(kitRowColor())
            .then(interaction)
            .heightIn(min = 56.dp)
            .padding(start = KitRowPad + indent, end = KitRowPad, top = 12.dp, bottom = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        content = content
    )
}

/** A row's title and its one supporting line. */
@Composable
internal fun RowScope.SettingsRowText(
    title: String,
    supporting: String? = null,
    titleColor: Color = MaterialTheme.colorScheme.onSurface,
    supportingColor: Color = MaterialTheme.colorScheme.onSurfaceVariant
) {
    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text(title, style = MaterialTheme.typography.bodyLarge, color = titleColor)
        if (supporting != null) {
            Text(supporting, style = MaterialTheme.typography.bodyMedium, color = supportingColor)
        }
    }
}

/**
 * A row with something at its end (a status pill, a stepper, a picked value) that keeps its text
 * readable at any font scale: the end element sits beside the text while the title fits unbroken
 * (and a description keeps half the row), and drops under the text when it would not, instead
 * of squeezing the words into a column one or two wide.
 */
@Composable
internal fun SettingsAdaptiveRow(
    title: String,
    supporting: String? = null,
    modifier: Modifier = Modifier,
    interaction: Modifier = Modifier,
    indent: Dp = 0.dp,
    enabled: Boolean = true,
    titleColor: Color = MaterialTheme.colorScheme.onSurface,
    leading: (@Composable () -> Unit)? = null,
    trailing: @Composable () -> Unit
) {
    val measurer = rememberTextMeasurer()
    val titleStyle = MaterialTheme.typography.bodyLarge
    val titlePx = remember(title, titleStyle) { measurer.measure(title, titleStyle).size.width }
    // One decision for every such row on the screen, so siblings never split apart: past 1.5x font
    // the end element always goes under the text. Below it, only a row that cannot fit stacks.
    val stackAll = kitStacksTrailing()
    val alpha = if (enabled) 1f else 0.35f
    Layout(
        contents = listOf(
            { if (leading != null) leading() },
            {
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(title, style = titleStyle, color = titleColor.copy(alpha = titleColor.alpha * alpha))
                    if (supporting != null) {
                        Text(supporting, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = alpha))
                    }
                }
            },
            { Box(Modifier.graphicsLayer { this.alpha = alpha }) { trailing() } }
        ),
        modifier = modifier
            .fillMaxWidth()
            .clip(LocalKitRowShape.current)
            .background(kitRowColor())
            .then(if (enabled) interaction else Modifier)
            .heightIn(min = 56.dp)
            .padding(start = KitRowPad + indent, end = KitRowPad, top = 12.dp, bottom = 12.dp)
    ) { (leadM, textM, trailM), c ->
        val gap = 16.dp.roundToPx()
        val width = c.maxWidth
        val lead = leadM.firstOrNull()?.measure(Constraints())
        val leadW = lead?.let { it.width + gap } ?: 0
        val trail = trailM.firstOrNull()?.measure(Constraints(maxWidth = (width - leadW).coerceAtLeast(0)))
        val trailW = trail?.let { it.width + gap } ?: 0
        val beside = width - leadW - trailW
        val needed = if (supporting != null) maxOf(titlePx, width / 2) else titlePx
        if (trail == null || (!stackAll && beside >= needed)) {
            val text = textM.first().measure(Constraints(maxWidth = beside.coerceAtLeast(0)))
            val h = maxOf(lead?.height ?: 0, text.height, trail?.height ?: 0).coerceAtLeast(c.minHeight)
            layout(width, h) {
                lead?.placeRelative(0, (h - lead.height) / 2)
                text.placeRelative(leadW, (h - text.height) / 2)
                trail?.placeRelative(width - trail.width, (h - trail.height) / 2)
            }
        } else {
            val text = textM.first().measure(Constraints(maxWidth = (width - leadW).coerceAtLeast(0)))
            val vGap = 10.dp.roundToPx()
            val block = text.height + vGap + trail.height
            val h = maxOf(block, lead?.height ?: 0).coerceAtLeast(c.minHeight)
            layout(width, h) {
                lead?.placeRelative(0, (h - lead.height) / 2)
                text.placeRelative(leadW, (h - block) / 2)
                trail.placeRelative(leadW, (h - block) / 2 + text.height + vGap)
            }
        }
    }
}

/** True past 1.5x font, where every row puts its end element under its text (see [SettingsAdaptiveRow]). */
@Composable
internal fun kitStacksTrailing(): Boolean = LocalDensity.current.fontScale >= 1.5f

/** A state reading at a row's end: a filled dot and a word when live, a ring and a quieter word when not. */
@Composable
internal fun SettingsStatus(label: String, live: Boolean) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        // Live is a filled accent dot; quiet keeps the slot with a hollow ring, so a stacked
        // "Nothing yet" still reads as a status and not as one more line of description.
        if (live) Box(Modifier.size(8.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primary))
        else Box(Modifier.size(8.dp).border(1.5.dp, MaterialTheme.colorScheme.onSurfaceVariant, CircleShape))
        Text(
            label,
            style = MaterialTheme.typography.bodyMedium,
            color = if (live) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}

/**
 * A small tonal button at a row's end: the one action the row offers. With [onClick] it is its own
 * 48dp target; without, it is drawn only and the whole row it sits in takes the tap.
 */
@Composable
internal fun SettingsCompactButton(label: String, onClick: (() -> Unit)? = null) {
    Box(
        Modifier
            .then(if (onClick != null) Modifier.minimumInteractiveComponentSize() else Modifier)
            .clip(PillShape)
            .background(kitTileColor())
            .then(if (onClick != null) Modifier.clickableLabeled(label, onClick = onClick) else Modifier)
            .heightIn(min = 36.dp)
            .padding(horizontal = 16.dp, vertical = 8.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(label, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurface)
    }
}

/** How an icon tile is tinted. [Danger] is for the one row whose tap erases something. */
internal enum class TileTone { Neutral, Accent, Danger }

/** The 40dp icon tile a row leads with. Decorative: the row's title speaks for it. */
@Composable
internal fun SettingsIconTile(icon: ImageVector, tone: TileTone = TileTone.Neutral) {
    val (container, tint) = when (tone) {
        TileTone.Neutral -> kitTileColor() to MaterialTheme.colorScheme.onSurface
        TileTone.Accent -> MaterialTheme.colorScheme.primaryContainer to MaterialTheme.colorScheme.primary
        TileTone.Danger -> MaterialTheme.colorScheme.errorContainer to MaterialTheme.colorScheme.error
    }
    Box(
        Modifier.size(40.dp).clip(RoundedCornerShape(12.dp)).background(container),
        contentAlignment = Alignment.Center
    ) {
        Icon(icon, contentDescription = null, tint = tint, modifier = Modifier.size(22.dp))
    }
}

@Composable
private fun Chevron() {
    Icon(
        Icons.AutoMirrored.Rounded.KeyboardArrowRight,
        contentDescription = null,
        tint = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.size(24.dp)
    )
}

/**
 * A row that goes somewhere: icon tile, title, its live value, and a chevron. [external] swaps the
 * chevron for an out-arrow when the tap leaves Avex; [value] sits before the chevron instead of
 * under the title, for a short setting value.
 */
@Composable
internal fun SettingsNavigationRow(
    title: String,
    supporting: String? = null,
    icon: ImageVector? = null,
    tone: TileTone = TileTone.Neutral,
    value: String? = null,
    external: Boolean = false,
    enabled: Boolean = true,
    onClick: () -> Unit
) {
    SettingsRowContainer(
        interaction = if (enabled) Modifier.clickableLabeled(title, onClick = onClick) else Modifier
    ) {
        if (icon != null) SettingsIconTile(icon, tone)
        SettingsRowText(title, supporting)
        if (value != null) {
            Text(
                value,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.End,
                modifier = Modifier.widthIn(max = 160.dp)
            )
        }
        if (enabled) {
            if (external) {
                Icon(
                    Icons.AutoMirrored.Rounded.OpenInNew,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(20.dp)
                )
            } else Chevron()
        }
    }
}

/** A row that does something now. Same anatomy as navigation, no chevron: nothing opens. */
@Composable
internal fun SettingsActionRow(
    title: String,
    supporting: String? = null,
    icon: ImageVector? = null,
    tone: TileTone = TileTone.Neutral,
    enabled: Boolean = true,
    indent: Dp = 0.dp,
    onClick: () -> Unit
) {
    val alpha = if (enabled) 1f else 0.35f
    SettingsRowContainer(
        interaction = if (enabled) Modifier.clickableLabeled(title, onClick = onClick) else Modifier,
        indent = indent
    ) {
        if (icon != null) SettingsIconTile(icon, tone)
        SettingsRowText(
            title,
            supporting,
            titleColor = MaterialTheme.colorScheme.onSurface.copy(alpha = alpha),
            supportingColor = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = alpha)
        )
    }
}

/** A static fact: title and reading, no tap. */
@Composable
internal fun SettingsInfoRow(title: String, supporting: String? = null, icon: ImageVector? = null, value: String? = null) {
    SettingsRowContainer {
        if (icon != null) SettingsIconTile(icon)
        SettingsRowText(title, supporting)
        if (value != null) {
            Text(value, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

@Composable
private fun kitSwitchColors() = SwitchDefaults.colors(
    checkedThumbColor = MaterialTheme.colorScheme.onPrimary,
    checkedTrackColor = MaterialTheme.colorScheme.primary,
    checkedBorderColor = MaterialTheme.colorScheme.primary,
    checkedIconColor = MaterialTheme.colorScheme.primary,
    uncheckedThumbColor = MaterialTheme.colorScheme.onSurfaceVariant,
    uncheckedTrackColor = MaterialTheme.colorScheme.surfaceContainerHighest,
    uncheckedBorderColor = MaterialTheme.colorScheme.outline,
    uncheckedIconColor = MaterialTheme.colorScheme.surfaceContainerHighest
)

/** The Material switch as Settings draws it. Drawn only: the row it sits in owns the tap. */
@Composable
internal fun SettingsSwitch(checked: Boolean) {
    Switch(
        checked = checked,
        onCheckedChange = null,
        thumbContent = if (checked) {
            { Icon(Icons.Rounded.Check, contentDescription = null, modifier = Modifier.size(SwitchDefaults.IconSize)) }
        } else null,
        colors = kitSwitchColors()
    )
}

/** Title, supporting line and a switch; the whole row toggles. */
@Composable
internal fun SettingsSwitchRow(
    title: String,
    supporting: String? = null,
    checked: Boolean,
    icon: ImageVector? = null,
    indent: Dp = 0.dp,
    /** Off when something upstream (a denied permission) makes the switch meaningless; its value stays shown. */
    enabled: Boolean = true,
    onCheckedChange: (Boolean) -> Unit
) {
    val alpha = if (enabled) 1f else 0.35f
    SettingsRowContainer(
        interaction = if (enabled) Modifier.toggleableLabeled(title, checked) { onCheckedChange(!checked) } else Modifier,
        indent = indent
    ) {
        if (icon != null) SettingsIconTile(icon)
        SettingsRowText(
            title,
            supporting,
            titleColor = MaterialTheme.colorScheme.onSurface.copy(alpha = alpha),
            supportingColor = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = alpha)
        )
        Box(Modifier.graphicsLayer { this.alpha = alpha }) { SettingsSwitch(checked) }
    }
}

/**
 * The one switch a page hangs off ("Use coach"): a larger slab that fills with the accent wash
 * while it is on, so the page's master state reads before any row under it.
 */
@Composable
internal fun SettingsMainSwitch(title: String, checked: Boolean, onCheckedChange: (Boolean) -> Unit) {
    val container = if (checked) MaterialTheme.colorScheme.primaryContainer else kitRowColor()
    Row(
        Modifier
            .fillMaxWidth()
            .clip(KitGroupShape)
            .background(container)
            .toggleableLabeled(title, checked) { onCheckedChange(!checked) }
            .heightIn(min = 72.dp)
            .padding(horizontal = 20.dp, vertical = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Text(
            title,
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.weight(1f)
        )
        SettingsSwitch(checked)
    }
}

/** One option of a pick-one list: optional tile, title, description, a radio. Whole row selects. */
@Composable
internal fun SettingsRadioRow(
    title: String,
    selected: Boolean,
    supporting: String? = null,
    icon: ImageVector? = null,
    onClick: () -> Unit
) {
    SettingsRowContainer(
        interaction = Modifier.selectable(selected = selected, role = Role.RadioButton, onClick = onClick)
    ) {
        if (icon != null) SettingsIconTile(icon, if (selected) TileTone.Accent else TileTone.Neutral)
        SettingsRowText(title, supporting)
        RadioButton(
            selected = selected,
            onClick = null,
            colors = RadioButtonDefaults.colors(
                selectedColor = MaterialTheme.colorScheme.primary,
                unselectedColor = MaterialTheme.colorScheme.onSurfaceVariant
            )
        )
    }
}

/**
 * A row whose value is picked from a short menu: the value and an up-down glyph on the right, the
 * menu anchored to the row, a check on the current pick.
 */
@Composable
internal fun SettingsDropdownRow(
    title: String,
    value: String,
    options: List<String>,
    selectedIndex: Int,
    supporting: String? = null,
    icon: ImageVector? = null,
    valueColor: Color = MaterialTheme.colorScheme.onSurface,
    indent: Dp = 0.dp,
    enabled: Boolean = true,
    onSelect: (Int) -> Unit
) {
    var open by remember { mutableStateOf(false) }
    Box {
        SettingsAdaptiveRow(
            title = title,
            supporting = supporting,
            indent = indent,
            enabled = enabled,
            interaction = Modifier.clickableLabeled("$title, $value. Change") { open = true },
            leading = icon?.let { { SettingsIconTile(it) } }
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(value, style = MaterialTheme.typography.bodyMedium, color = valueColor)
                Icon(
                    Icons.Rounded.UnfoldMore,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(20.dp)
                )
            }
        }
        SettingsMenu(open, options, selectedIndex, onDismiss = { open = false }) { onSelect(it); open = false }
    }
}

/** The dropdown menu every Settings picker opens: the options, a check on the current one. */
@Composable
internal fun SettingsMenu(
    expanded: Boolean,
    options: List<String>,
    selectedIndex: Int,
    onDismiss: () -> Unit,
    onSelect: (Int) -> Unit
) {
    DropdownMenu(expanded = expanded, onDismissRequest = onDismiss) {
        options.forEachIndexed { i, label ->
            val picked = i == selectedIndex
            DropdownMenuItem(
                text = {
                    Text(
                        label,
                        style = MaterialTheme.typography.bodyLarge,
                        color = if (picked) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant
                    )
                },
                trailingIcon = if (picked) {
                    { Icon(Icons.Rounded.Check, contentDescription = "Selected", tint = MaterialTheme.colorScheme.primary) }
                } else null,
                onClick = { onSelect(i) }
            )
        }
    }
}

// ── Segmented control ────────────────────────────────────────────────────────────────────────────

/**
 * Pick one of two to four short values. A dark inset track with a lifted thumb that slides to the
 * pick, so the change is felt as well as seen. Cells are equal width; [compact] sizes each cell to
 * the widest label (measured, so it holds at any font scale) instead of filling the width.
 */
@Composable
internal fun SettingsSegmented(
    options: List<String>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
    compact: Boolean = false,
    contentDescription: String? = null
) {
    val n = options.size.coerceAtLeast(1)
    // Kept as a State and read only in the thumb's layout: reading it here recomposed the whole
    // control on every frame of the slide.
    val position = animateFloatAsState(
        selectedIndex.coerceIn(0, n - 1).toFloat(),
        ForgeMotion.snappy(),
        label = "segment"
    )
    val labelStyle = MaterialTheme.typography.labelLarge
    val compactWidth = if (compact) segmentedCompactWidth(options) else null
    // The pick takes the app's selection formula: the accent wash under an accent edge.
    val thumb = MaterialTheme.colorScheme.primaryContainer
    if (!compact) {
        // Equal cells that cannot hold their widest label (a four-way choice at a large font
        // scale) would break words mid-label; the choice wraps into rows of cells instead.
        val measurer = rememberTextMeasurer()
        val density = LocalDensity.current
        // Text measurement is not free, and the labels only change with these.
        val widest = remember(options, labelStyle, measurer, density) {
            with(density) { (options.maxOfOrNull { measurer.measure(it, labelStyle).size.width } ?: 0).toDp() }
        }
        BoxWithConstraints(modifier.fillMaxWidth()) {
            val inner = maxWidth - 8.dp
            if (inner / n >= widest + 20.dp) {
                SegmentedTrack(options, selectedIndex, onSelect, Modifier, null, { position.value }, labelStyle, thumb, contentDescription)
            } else {
                val perRow = (inner / (widest + 24.dp)).toInt().coerceIn(1, (n - 1).coerceAtLeast(1))
                SegmentedGrid(options, selectedIndex, onSelect, perRow, labelStyle, thumb, contentDescription)
            }
        }
        return
    }
    SegmentedTrack(options, selectedIndex, onSelect, modifier, compactWidth, { position.value }, labelStyle, thumb, contentDescription)
}

/** The sliding-thumb track itself: one pill, equal cells, the thumb under the pick. */
@Composable
private fun SegmentedTrack(
    options: List<String>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier,
    compactWidth: Dp?,
    position: () -> Float,
    labelStyle: androidx.compose.ui.text.TextStyle,
    thumb: Color,
    contentDescription: String?
) {
    val n = options.size.coerceAtLeast(1)
    Box(
        modifier
            .then(if (compactWidth != null) Modifier.width(compactWidth) else Modifier.fillMaxWidth())
            .clip(PillShape)
            .background(MaterialTheme.colorScheme.surfaceContainerLowest)
            .padding(4.dp)
            .selectableGroup()
            .then(if (contentDescription != null) Modifier.semantics { this.contentDescription = contentDescription } else Modifier)
    ) {
        Box(
            Modifier
                .matchParentSize()
                .layout { measurable, constraints ->
                    val cell = constraints.maxWidth / n
                    val placeable = measurable.measure(Constraints.fixed(cell, constraints.maxHeight))
                    layout(constraints.maxWidth, constraints.maxHeight) {
                        placeable.placeRelative((position() * cell).roundToInt(), 0)
                    }
                }
                .clip(PillShape)
                .background(thumb)
                .border(1.dp, MaterialTheme.colorScheme.primary, PillShape)
        )
        Row(Modifier.fillMaxWidth()) {
            options.forEachIndexed { i, label ->
                val picked = i == selectedIndex
                Box(
                    Modifier
                        .weight(1f)
                        .heightIn(min = 40.dp)
                        .clip(PillShape)
                        .selectable(selected = picked, role = Role.RadioButton, onClick = { onSelect(i) })
                        .padding(horizontal = 8.dp, vertical = 8.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        label,
                        style = labelStyle,
                        color = if (picked) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = TextAlign.Center
                    )
                }
            }
        }
    }
}

/** The wrapped form of [SettingsSegmented]: rows of pill cells, the pick lifted like the thumb. */
@Composable
private fun SegmentedGrid(
    options: List<String>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    perRow: Int,
    labelStyle: androidx.compose.ui.text.TextStyle,
    thumb: Color,
    contentDescription: String?
) {
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(20.dp))
            .background(MaterialTheme.colorScheme.surfaceContainerLowest)
            .padding(4.dp)
            .selectableGroup()
            .then(if (contentDescription != null) Modifier.semantics { this.contentDescription = contentDescription } else Modifier),
        verticalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        options.indices.chunked(perRow).forEach { row ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                row.forEach { i ->
                    val picked = i == selectedIndex
                    Box(
                        Modifier
                            .weight(1f)
                            .heightIn(min = 44.dp)
                            .clip(PillShape)
                            .background(if (picked) thumb else Color.Transparent)
                            .then(if (picked) Modifier.border(1.dp, MaterialTheme.colorScheme.primary, PillShape) else Modifier)
                            .selectable(selected = picked, role = Role.RadioButton, onClick = { onSelect(i) })
                            .padding(horizontal = 8.dp, vertical = 8.dp),
                        contentAlignment = Alignment.Center
                    ) {
                        Text(
                            options[i],
                            style = labelStyle,
                            color = if (picked) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant,
                            textAlign = TextAlign.Center
                        )
                    }
                }
                repeat(perRow - row.size) { Spacer(Modifier.weight(1f)) }
            }
        }
    }
}

/** How wide a compact [SettingsSegmented] draws: equal cells as wide as the widest label, measured. */
@Composable
private fun segmentedCompactWidth(options: List<String>): Dp {
    val style = MaterialTheme.typography.labelLarge
    val measurer = rememberTextMeasurer()
    val density = LocalDensity.current
    return remember(options, style, measurer, density) {
        val widest = options.maxOfOrNull { measurer.measure(it, style).size.width } ?: 0
        (with(density) { widest.toDp() } + 28.dp) * options.size + 8.dp
    }
}

/**
 * A setting with two to four short values. [stacked] puts a full-width control under the text (a
 * four-way choice, or long labels); otherwise the control sits at the row's end and drops under
 * the text only when the two no longer fit side by side.
 */
@Composable
internal fun SettingsSegmentedRow(
    title: String,
    options: List<String>,
    selectedIndex: Int,
    supporting: String? = null,
    stacked: Boolean = false,
    onSelect: (Int) -> Unit
) {
    Column(
        Modifier
            .fillMaxWidth()
            .clip(LocalKitRowShape.current)
            .background(kitRowColor())
            .padding(horizontal = KitRowPad, vertical = 12.dp)
    ) {
        if (stacked) {
            Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(title, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurface)
                if (supporting != null) {
                    Text(supporting, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            Spacer(Modifier.height(12.dp))
            SettingsSegmented(options, selectedIndex, onSelect, contentDescription = title)
        } else {
            // Side by side while the text keeps a sensible column beside the control; past that
            // (a long label, or a large font scale) the control takes its own full-width line.
            val measurer = rememberTextMeasurer()
            val titleStyle = MaterialTheme.typography.bodyLarge
            val density = LocalDensity.current
            BoxWithConstraints(Modifier.fillMaxWidth()) {
                val controlWidth = segmentedCompactWidth(options)
                // The title must fit unbroken, and a description keeps half the row.
                val titleWidth = with(density) { measurer.measure(title, titleStyle).size.width.toDp() }
                val needed = if (supporting != null) maxOf(titleWidth, maxWidth / 2) else titleWidth
                if (!kitStacksTrailing() && maxWidth - controlWidth - 16.dp >= needed) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f).padding(end = 16.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                            Text(title, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurface)
                            if (supporting != null) {
                                Text(supporting, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                        SettingsSegmented(options, selectedIndex, onSelect, compact = true, contentDescription = title)
                    }
                } else {
                    Column {
                        Text(title, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurface)
                        if (supporting != null) {
                            Text(supporting, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        Spacer(Modifier.height(12.dp))
                        SettingsSegmented(options, selectedIndex, onSelect, contentDescription = title)
                    }
                }
            }
        }
    }
}

// ── Stepper ──────────────────────────────────────────────────────────────────────────────────────

/** A round tonal icon button: 40dp drawn, 48dp to touch, dimmed and inert at the end of its range. */
@Composable
internal fun SettingsRoundButton(icon: ImageVector, label: String, enabled: Boolean = true, onClick: () -> Unit) {
    val alpha = if (enabled) 1f else 0.35f
    Box(
        Modifier
            .minimumInteractiveComponentSize()
            .size(40.dp)
            .clip(CircleShape)
            .background(kitTileColor().copy(alpha = alpha))
            .then(if (enabled) Modifier.clickableLabeled(label, onClick = onClick) else Modifier.semantics { contentDescription = label }),
        contentAlignment = Alignment.Center
    ) {
        Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.onSurface.copy(alpha = alpha), modifier = Modifier.size(20.dp))
    }
}

/** Title with the value between − and + at its end, or under it when the title needs the room. */
@Composable
internal fun SettingsStepperRow(
    title: String,
    value: String,
    canDecrease: Boolean,
    canIncrease: Boolean,
    onDecrease: () -> Unit,
    onIncrease: () -> Unit,
    supporting: String? = null,
    indent: Dp = 0.dp
) {
    SettingsAdaptiveRow(title = title, supporting = supporting, indent = indent) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            SettingsRoundButton(Icons.Rounded.Remove, "Less $title", canDecrease, onDecrease)
            Text(
                value,
                style = MaterialTheme.typography.titleMedium.copy(fontFeatureSettings = "tnum"),
                color = MaterialTheme.colorScheme.onSurface,
                textAlign = TextAlign.Center,
                modifier = Modifier.widthIn(min = 56.dp).semantics { contentDescription = "$title $value" }
            )
            SettingsRoundButton(Icons.Rounded.Add, "More $title", canIncrease, onIncrease)
        }
    }
}

// ── Chips ────────────────────────────────────────────────────────────────────────────────────────

/** A wrapping set of chips inside a group slab. */
@Composable
internal fun SettingsChipBlock(content: @Composable FlowRowScope.() -> Unit) {
    SettingsGroupBlock(padding = PaddingValues(horizontal = KitRowPad, vertical = 12.dp)) {
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(0.dp),
            content = content
        )
    }
}

/** A Material filter chip in the Settings palette: a check and the accent wash when picked. */
@Composable
internal fun SettingsFilterChip(label: String, selected: Boolean, onClick: () -> Unit) {
    FilterChip(
        selected = selected,
        onClick = onClick,
        label = { Text(label) },
        leadingIcon = if (selected) {
            { Icon(Icons.Rounded.Check, contentDescription = null, modifier = Modifier.size(FilterChipDefaults.IconSize)) }
        } else null,
        colors = FilterChipDefaults.filterChipColors(
            containerColor = Color.Transparent,
            labelColor = MaterialTheme.colorScheme.onSurfaceVariant,
            selectedContainerColor = MaterialTheme.colorScheme.primaryContainer,
            selectedLabelColor = MaterialTheme.colorScheme.onSurface,
            selectedLeadingIconColor = MaterialTheme.colorScheme.primary
        ),
        border = FilterChipDefaults.filterChipBorder(
            enabled = true,
            selected = selected,
            borderColor = MaterialTheme.colorScheme.outline,
            selectedBorderColor = MaterialTheme.colorScheme.primary
        )
    )
}

/** A small rounded label: a status reading or a drawn call to action inside a tappable row. */
internal enum class PillTone { Neutral, Accent, Quiet }

@Composable
internal fun SettingsPill(text: String, tone: PillTone = PillTone.Neutral, dot: Boolean = false) {
    val (container, content) = when (tone) {
        PillTone.Neutral -> kitTileColor() to MaterialTheme.colorScheme.onSurface
        PillTone.Accent -> MaterialTheme.colorScheme.primaryContainer to MaterialTheme.colorScheme.onSurface
        PillTone.Quiet -> Color.Transparent to MaterialTheme.colorScheme.onSurfaceVariant
    }
    Row(
        Modifier
            .clip(PillShape)
            .background(container)
            .then(if (tone == PillTone.Quiet) Modifier.border(1.dp, MaterialTheme.colorScheme.outline, PillShape) else Modifier)
            .padding(horizontal = 12.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        if (dot) Box(Modifier.size(6.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primary))
        Text(text, style = MaterialTheme.typography.labelMedium, color = content)
    }
}

// ── Buttons ──────────────────────────────────────────────────────────────────────────────────────

internal enum class ButtonKind { Primary, Tonal, Outlined, Danger }

/**
 * The Settings button. [ButtonKind.Primary] is the page's one do-it-now and takes the accent;
 * [ButtonKind.Tonal] is its sidekick; [ButtonKind.Danger] erases or replaces something.
 */
@Composable
internal fun SettingsButton(
    label: String,
    modifier: Modifier = Modifier,
    kind: ButtonKind = ButtonKind.Primary,
    enabled: Boolean = true,
    icon: ImageVector? = null,
    onClick: () -> Unit
) {
    val padding = PaddingValues(horizontal = 22.dp, vertical = 14.dp)
    val content: @Composable RowScope.() -> Unit = {
        if (icon != null) {
            Icon(icon, contentDescription = null, modifier = Modifier.size(18.dp))
            Spacer(Modifier.width(8.dp))
        }
        Text(label, textAlign = TextAlign.Center)
    }
    val scheme = MaterialTheme.colorScheme
    when (kind) {
        ButtonKind.Primary -> Button(
            onClick = onClick, enabled = enabled, modifier = modifier.heightIn(min = 48.dp), contentPadding = padding,
            colors = ButtonDefaults.buttonColors(
                containerColor = scheme.primary,
                contentColor = scheme.onPrimary,
                disabledContainerColor = scheme.onSurface.copy(alpha = 0.15f),
                disabledContentColor = scheme.onSurface.copy(alpha = 0.35f)
            ),
            content = content
        )
        ButtonKind.Tonal, ButtonKind.Danger -> FilledTonalButton(
            onClick = onClick, enabled = enabled, modifier = modifier.heightIn(min = 48.dp), contentPadding = padding,
            colors = ButtonDefaults.filledTonalButtonColors(
                containerColor = if (kind == ButtonKind.Danger) scheme.errorContainer else kitTileColor(),
                contentColor = scheme.onSurface,
                disabledContainerColor = scheme.onSurface.copy(alpha = 0.15f),
                disabledContentColor = scheme.onSurface.copy(alpha = 0.35f)
            ),
            content = content
        )
        ButtonKind.Outlined -> OutlinedButton(
            onClick = onClick, enabled = enabled, modifier = modifier.heightIn(min = 48.dp), contentPadding = padding,
            colors = ButtonDefaults.outlinedButtonColors(
                contentColor = scheme.onSurface,
                disabledContentColor = scheme.onSurface.copy(alpha = 0.35f)
            ),
            content = content
        )
    }
}

/** Buttons that belong together, wrapping to a second line at large font scales. */
@Composable
internal fun SettingsButtonBar(modifier: Modifier = Modifier, content: @Composable FlowRowScope.() -> Unit) {
    FlowRow(
        modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        content = content
    )
}

// ── Search ───────────────────────────────────────────────────────────────────────────────────────

/** The search pill: 56dp, drawn magnifier, a clear button once there is something to clear. */
@Composable
internal fun SettingsSearchBar(
    query: String,
    placeholder: String,
    onQueryChange: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    val onSurface = MaterialTheme.colorScheme.onSurface
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .clip(PillShape)
            .background(kitRowColor())
            .padding(start = 18.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Icon(Icons.Rounded.Search, contentDescription = null, tint = muted, modifier = Modifier.size(24.dp))
        BasicTextField(
            value = query,
            onValueChange = onQueryChange,
            textStyle = MaterialTheme.typography.bodyLarge.copy(color = onSurface),
            cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
            singleLine = true,
            modifier = Modifier.weight(1f).padding(vertical = 16.dp),
            decorationBox = { inner ->
                Box {
                    if (query.isEmpty()) {
                        Text(placeholder, style = MaterialTheme.typography.bodyLarge, color = muted)
                    }
                    inner()
                }
            }
        )
        if (query.isNotEmpty()) {
            IconButton(onClick = { onQueryChange("") }) {
                Icon(Icons.Rounded.Close, contentDescription = "Clear search", tint = muted)
            }
        } else {
            Spacer(Modifier.width(8.dp))
        }
    }
}

// ── Notices and empty states ─────────────────────────────────────────────────────────────────────

/**
 * A page-top notice for a state that stops the page working (notifications blocked, Health Connect
 * missing): what is wrong, and the one button that fixes it.
 */
@Composable
internal fun SettingsNotice(
    icon: ImageVector,
    title: String,
    body: String,
    action: String? = null,
    /** A fault (tinted with the error wash) rather than a missing piece (the plain row colour). */
    critical: Boolean = true,
    onAction: () -> Unit = {}
) {
    Column(
        Modifier
            .fillMaxWidth()
            .clip(KitGroupShape)
            .background(if (critical) MaterialTheme.colorScheme.errorContainer else kitRowColor())
            .padding(KitRowPad),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.onSurface, modifier = Modifier.size(24.dp))
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(title, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurface)
                Text(body, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        if (action != null) {
            SettingsButton(action, Modifier.padding(start = 38.dp), kind = ButtonKind.Tonal, onClick = onAction)
        }
    }
}

/** A list with nothing in it yet: a drawn icon, what would be here, and how to add the first. */
@Composable
internal fun SettingsEmptyBlock(icon: ImageVector, title: String, body: String) {
    SettingsGroupBlock(padding = PaddingValues(horizontal = 24.dp, vertical = 28.dp)) {
        Column(
            Modifier.fillMaxWidth(),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            Box(
                Modifier.size(48.dp).clip(CircleShape).background(kitTileColor()),
                contentAlignment = Alignment.Center
            ) {
                Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(24.dp))
            }
            Spacer(Modifier.height(4.dp))
            Text(title, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurface, textAlign = TextAlign.Center)
            Text(body, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.Center)
        }
    }
}

/** Supplies a lazy row with its per-position shape. */
@Composable
internal fun KitLazyRow(index: Int, count: Int, content: @Composable () -> Unit) {
    CompositionLocalProvider(LocalKitRowShape provides kitRowShape(index, count)) {
        Box(Modifier.padding(bottom = kitRowSpacing(index, count))) { content() }
    }
}

// ── Sheets and dialogs ───────────────────────────────────────────────────────────────────────────

/**
 * A Settings bottom sheet: the same serif title voice as a page, one supporting line, then groups.
 * Opens fully (these hold a list of choices, not a peek) and scrolls at large font scales.
 */
@Composable
internal fun SettingsSheet(
    title: String,
    supporting: String? = null,
    onDismiss: () -> Unit,
    content: @Composable ColumnScope.() -> Unit
) {
    com.forge.app.ui.common.window.ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = androidx.compose.material3.rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = MaterialTheme.colorScheme.surfaceContainerLow
    ) {
        Column(
            Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(start = KitGutter, end = KitGutter, bottom = 32.dp),
            verticalArrangement = Arrangement.spacedBy(24.dp)
        ) {
            Column(Modifier.padding(horizontal = 8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(
                    title,
                    style = MaterialTheme.typography.headlineSmall,
                    color = MaterialTheme.colorScheme.onSurface,
                    modifier = Modifier.semantics { heading() }
                )
                if (supporting != null) {
                    Text(supporting, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            content()
        }
    }
}

/**
 * The one confirmation every irreversible Settings act goes through: an icon naming the kind of
 * act, a question for a title, what it does, and a filled button that says the act, not "OK".
 * [destructive] paints that button in the error colour; [extra] holds a warning line or the
 * type-to-confirm field.
 */
@Composable
internal fun SettingsConfirmDialog(
    title: String,
    body: String,
    confirmLabel: String,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
    icon: ImageVector? = null,
    destructive: Boolean = true,
    confirmEnabled: Boolean = true,
    dismissLabel: String = "Cancel",
    extra: (@Composable () -> Unit)? = null
) {
    val scheme = MaterialTheme.colorScheme
    com.forge.app.ui.common.window.AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = scheme.surfaceContainerHigh,
        icon = icon?.let { { Icon(it, contentDescription = null, tint = if (destructive) scheme.error else scheme.onSurface) } },
        // A dialog's question, not a screen name: the top-bar rule does not reach it.
        title = { Text(title, textAlign = TextAlign.Center) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                Text(body, style = MaterialTheme.typography.bodyMedium, color = scheme.onSurfaceVariant)
                extra?.invoke()
            }
        },
        confirmButton = {
            Button(
                onClick = onConfirm,
                enabled = confirmEnabled,
                colors = ButtonDefaults.buttonColors(
                    containerColor = if (destructive) scheme.error else scheme.primary,
                    contentColor = if (destructive) scheme.onError else scheme.onPrimary,
                    disabledContainerColor = scheme.onSurface.copy(alpha = 0.15f),
                    disabledContentColor = scheme.onSurface.copy(alpha = 0.35f)
                )
            ) { Text(confirmLabel) }
        },
        dismissButton = {
            androidx.compose.material3.TextButton(onClick = onDismiss) {
                Text(dismissLabel, color = scheme.onSurface)
            }
        }
    )
}

/** A text field in the Settings palette: accent when focused, the outline rung when not. */
@Composable
internal fun settingsFieldColors() = androidx.compose.material3.OutlinedTextFieldDefaults.colors(
    focusedBorderColor = MaterialTheme.colorScheme.primary,
    unfocusedBorderColor = MaterialTheme.colorScheme.outline,
    cursorColor = MaterialTheme.colorScheme.primary,
    focusedLabelColor = MaterialTheme.colorScheme.onSurface,
    unfocusedLabelColor = MaterialTheme.colorScheme.onSurfaceVariant
)

/** A thin determinate bar: a share of something done (an export's progress). */
@Composable
internal fun SettingsProgressBar(fraction: Float, modifier: Modifier = Modifier) {
    Box(
        modifier
            .fillMaxWidth()
            .height(4.dp)
            .clip(PillShape)
            .background(MaterialTheme.colorScheme.outline.copy(alpha = 0.35f))
    ) {
        Box(
            Modifier
                .fillMaxWidth(fraction.coerceIn(0f, 1f))
                .height(4.dp)
                .clip(PillShape)
                .background(MaterialTheme.colorScheme.primary)
        )
    }
}
