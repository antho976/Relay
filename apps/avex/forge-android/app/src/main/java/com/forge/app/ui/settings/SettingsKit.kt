@file:OptIn(
    androidx.compose.foundation.layout.ExperimentalLayoutApi::class,
    androidx.compose.material3.ExperimentalMaterial3Api::class
)

package com.forge.app.ui.settings

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
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.OpenInNew
import androidx.compose.material.icons.rounded.Remove
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.Typography
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
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
import com.forge.app.ui.common.ForgeSwitch
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenMeter
import com.forge.app.ui.common.OpenTopBarIcon
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.toggleableLabeled
import com.forge.app.ui.common.touchSlop
import com.forge.app.ui.common.window.DropdownMenu
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import com.forge.app.ui.theme.SettingsColors
import com.forge.app.ui.theme.SettingsType
import com.forge.app.ui.theme.openOnAccentLarge

/*
 * # The Settings kit, Open panel (2026-10-21)
 *
 * Settings moved to the redesign's "Open panel" look (Relay designs st, sa, sp, sw and lk5) with the
 * rest of the app. The kit keeps its API, so every page changed look at once; only the drawing moved:
 *
 * - **No slabs.** A row is its name (Noto Sans 16/600, ink) over its live value in the mono reading
 *   voice (10sp, tracked, upper case, muted), on the page at the 24dp gutter, 22dp to the next row.
 *   Its 48dp+ target comes from [touchSlop], not from a filled box.
 * - **Sections hold by air**: a mono label over the rows, ~44dp between sections, no rule.
 * - **Only actions are filled**: a pick is a 4dp chip with the accent wash under an accent edge, a
 *   switch is filled red when on and an outline when off, the primary button is the red one.
 * - The page title is the serif one every redesigned page opens on, under a bare back arrow.
 */

// ── Tokens ───────────────────────────────────────────────────────────────────────────────────────

/** Page margin: screen edge to content. The redesign's 24dp gutter. */
internal val KitGutter = 24.dp

/** Inside a row: its edge to its content. Rows sit straight on the gutter now, so none. */
internal val KitRowPad = 0.dp

/** Space between two sections on a page (the design's 44dp of air). */
internal val KitGroupSpacing = 44.dp

/** Space between two rows of a section. */
internal val KitRowGap = 22.dp

/** Space between two stacked pick boxes (radio options). */
internal val KitPickGap = 8.dp

private val Corner = RoundedCornerShape(4.dp)
internal val KitGroupShape: Shape = Corner

/** Kept for lazy rows that still ask for one; every row is the same open shape now. */
internal val LocalKitRowShape = compositionLocalOf<Shape> { Corner }

/** "Rear Delts" → "Rear delts": Settings labels are sentence case, whatever the data spells. */
internal fun String.sentenceCase(): String = lowercase().replaceFirstChar { it.uppercase() }

/** A row's shape by its position in a lazy group. One shape now; kept for its callers. */
@Suppress("UNUSED_PARAMETER")
internal fun kitRowShape(index: Int, count: Int): Shape = Corner

/** The spacing a lazy row leaves under itself: the row gap, or nothing after the last. */
internal fun kitRowSpacing(index: Int, count: Int): Dp = if (index == count - 1) 0.dp else KitRowGap

/** A row's ground: none. Rows are open on the page. */
@Composable
internal fun kitRowColor(): Color = Color.Transparent

/** The accent wash under a pick: the accent at 15%. */
@Composable
internal fun kitWash(): Color = MaterialTheme.colorScheme.primary.copy(alpha = 0.15f)

/**
 * Widens a full-bleed strip (the accent swatches, the icon strip) past the page gutter on both
 * sides, so it scrolls to the screen's edge while its first item still lines up with the text.
 */
internal fun Modifier.kitBleed(): Modifier = layout { measurable, constraints ->
    val g = KitGutter.roundToPx()
    val wide = if (constraints.hasBoundedWidth) constraints.maxWidth + 2 * g else constraints.maxWidth
    val p = measurable.measure(constraints.copy(minWidth = wide, maxWidth = wide))
    layout(constraints.maxWidth, p.height) { p.place(-g, 0) }
}

// ── Theme ────────────────────────────────────────────────────────────────────────────────────────

/** The app's own type scale, captured before [SettingsTheme] swaps the mono label rungs for sans. */
private val LocalAppTypography = staticCompositionLocalOf<Typography?> { null }

/**
 * The Settings voice: the app's theme with its three mono label styles swapped for sans. Material
 * draws button, chip, menu and dialog labels in `labelLarge`, which is mono app-wide, and that made
 * every control in a dialog here look typed. Scoped to Settings, so no other screen moves.
 */
@Composable
internal fun SettingsTheme(content: @Composable () -> Unit) {
    val base = MaterialTheme.typography
    val typography = remember(base) {
        val small = base.bodySmall.copy(fontWeight = FontWeight.Medium, letterSpacing = 0.3.sp)
        base.copy(labelLarge = base.titleSmall, labelMedium = small, labelSmall = small)
    }
    CompositionLocalProvider(LocalAppTypography provides (LocalAppTypography.current ?: base)) {
        MaterialTheme(
            colorScheme = MaterialTheme.colorScheme,
            shapes = MaterialTheme.shapes,
            typography = typography,
            content = content
        )
    }
}

/** The app's type scale under [SettingsTheme]: its label rungs are still mono here. */
@Composable
internal fun appTypography(): Typography = LocalAppTypography.current ?: MaterialTheme.typography

// ── Page scaffolds ───────────────────────────────────────────────────────────────────────────────

/** A one-word name takes the full 56 serif; a longer one ("Program & equipment") steps to 40. */
internal fun settingsTitleIsShort(title: String): Boolean = title.length <= 10

/**
 * The page's name: the serif title every redesigned page opens on, 56 for a short name and 40 for a
 * long one (which may wrap), with an optional mono line under it (the root's version).
 */
@Composable
internal fun SettingsPageTitle(title: String, subtitle: String? = null) {
    Column {
        Text(
            title,
            style = if (settingsTitleIsShort(title)) OpenType.title else SettingsType.titleM,
            color = MaterialTheme.colorScheme.onBackground,
            modifier = Modifier.semantics { heading() }
        )
        if (subtitle != null) {
            Spacer(Modifier.height(10.dp))
            OpenLabel(subtitle, style = OpenType.monoXS)
        }
    }
}

/**
 * The bar over every page: a bare back arrow on the gutter and, on the root, the search glyph at the
 * other end. [content] replaces the whole bar (the root's open search field).
 */
@Composable
internal fun SettingsTopBar(
    onBack: () -> Unit,
    trailing: (@Composable () -> Unit)? = null,
    content: (@Composable RowScope.() -> Unit)? = null
) {
    Row(
        Modifier
            .fillMaxWidth()
            .windowInsetsPadding(WindowInsets.statusBars)
            .padding(start = KitGutter, end = KitGutter, top = 4.dp)
            .height(48.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        if (content != null) {
            content()
        } else {
            OpenTopBarIcon(OpenIcons.ArrowLeft, "Back", onBack)
            Spacer(Modifier.weight(1f))
            trailing?.invoke()
        }
    }
}

/** Room under a page's last section: the gesture bar plus a little air. */
@Composable
private fun settingsBottomPad(): Dp =
    WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding() + 48.dp

/** A Settings page whose content is a short, fixed column of sections. */
@Composable
internal fun SettingsScaffold(
    title: String,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit
) {
    Column(modifier.fillMaxSize()) {
        SettingsTopBar(onBack)
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(start = KitGutter, end = KitGutter, top = 20.dp, bottom = settingsBottomPad())
        ) {
            SettingsPageTitle(title)
            Spacer(Modifier.height(32.dp))
            Column(verticalArrangement = Arrangement.spacedBy(KitGroupSpacing), content = content)
        }
    }
}

/** A Settings page backed by a lazy list: the root index and the long exercise list. */
@Composable
internal fun SettingsLazyScaffold(
    title: String,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
    /** A mono reading under the title (the root's version). */
    subtitle: String? = null,
    listState: LazyListState = rememberLazyListState(),
    verticalArrangement: Arrangement.Vertical = Arrangement.Top,
    /** The bar's far end (the root's search glyph). */
    topBarTrailing: (@Composable () -> Unit)? = null,
    /** Replaces the bar (the root's open search field). */
    topBarContent: (@Composable RowScope.() -> Unit)? = null,
    content: LazyListScope.() -> Unit
) {
    Column(modifier.fillMaxSize()) {
        SettingsTopBar(onBack, topBarTrailing, topBarContent)
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(start = KitGutter, end = KitGutter, top = 20.dp, bottom = settingsBottomPad()),
            verticalArrangement = verticalArrangement
        ) {
            item(key = "settings_page_title") {
                Box(Modifier.padding(bottom = 32.dp)) { SettingsPageTitle(title, subtitle) }
            }
            content()
        }
    }
}

// ── Sections ─────────────────────────────────────────────────────────────────────────────────────

/** A section's head: its mono label, and an optional mono reading at the far end. 18dp to its rows. */
@Composable
internal fun SettingsGroupHeader(title: String, modifier: Modifier = Modifier, trailing: String? = null) {
    Row(
        modifier.fillMaxWidth().padding(bottom = 18.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        OpenLabel(title, Modifier.weight(1f).semantics { heading() })
        if (trailing != null) HeadReading(trailing)
    }
}

/** A section head's reading at its far end, set to the end so a wrapped one stays right-aligned. */
@Composable
private fun HeadReading(text: String) {
    Text(text.uppercase(), style = OpenType.monoS, color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.End)
}

/**
 * The note under a section, in body. [isError] is the quiet failure line: a small accent dot and
 * the words in ink, never red text.
 */
@Composable
internal fun SettingsGroupFooter(text: String, modifier: Modifier = Modifier, isError: Boolean = false) {
    if (isError) {
        SettingsMarkedLine(text, modifier.fillMaxWidth().padding(top = 16.dp))
    } else {
        Text(
            text,
            style = OpenType.bodyS,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = modifier.fillMaxWidth().padding(top = 16.dp)
        )
    }
}

/** A line that needs noticing (too many tries, a failed copy): a 6dp accent dot, then ink words. */
@Composable
internal fun SettingsMarkedLine(text: String, modifier: Modifier = Modifier) {
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.Top) {
        Box(
            Modifier
                .padding(top = 7.dp)
                .size(6.dp)
                .clip(CircleShape)
                .background(MaterialTheme.colorScheme.primary)
        )
        Text(text, style = OpenType.bodyS, color = MaterialTheme.colorScheme.onBackground)
    }
}

/**
 * A titled section of rows, stacked [rowGap] apart with no box around them. A row that measures
 * zero high (a collapsed expander) takes no gap.
 */
@Composable
internal fun SettingsGroup(
    title: String? = null,
    modifier: Modifier = Modifier,
    footer: String? = null,
    footerIsError: Boolean = false,
    headerTrailing: String? = null,
    rowGap: Dp = KitRowGap,
    content: @Composable () -> Unit
) {
    Column(modifier.fillMaxWidth()) {
        if (title != null) SettingsGroupHeader(title, trailing = headerTrailing)
        KitStack(Modifier.fillMaxWidth(), rowGap, content)
        if (footer != null) SettingsGroupFooter(footer, isError = footerIsError)
    }
}

/** The section's column: children [gap] apart, with no gap around a child that measures zero. */
@Composable
private fun KitStack(modifier: Modifier, gap: Dp, content: @Composable () -> Unit) {
    Layout(content, modifier) { measurables, constraints ->
        val g = gap.roundToPx()
        val loose = constraints.copy(minHeight = 0)
        val placeables = measurables.map { it.measure(loose) }
        var height = 0
        var shown = 0
        placeables.forEach { p ->
            if (p.height > 0) {
                if (shown > 0) height += g
                height += p.height
                shown++
            }
        }
        val width = (placeables.maxOfOrNull { it.width } ?: 0).coerceIn(constraints.minWidth, constraints.maxWidth)
        layout(width, height.coerceIn(constraints.minHeight, constraints.maxHeight)) {
            var y = 0
            var placed = 0
            placeables.forEach { p ->
                if (p.height > 0 && placed > 0) y += g
                p.placeRelative(0, y)
                if (p.height > 0) {
                    y += p.height
                    placed++
                }
            }
        }
    }
}

/** Anything in a section that is not a row (chip sets, a preview): open, with optional inset. */
@Composable
internal fun SettingsGroupBlock(
    modifier: Modifier = Modifier,
    padding: PaddingValues = PaddingValues(0.dp),
    content: @Composable ColumnScope.() -> Unit
) {
    Column(modifier.fillMaxWidth().padding(padding), content = content)
}

// ── Rows ─────────────────────────────────────────────────────────────────────────────────────────

/** A row's tap reaches 11dp past it on each side (half the gap), so a 38dp row is a 60dp target. */
private fun Modifier.rowTarget(interaction: Modifier): Modifier =
    if (interaction === Modifier) this
    else this.touchSlop(vertical = 11.dp).then(interaction).padding(vertical = 11.dp)

/** The base row: open on the page, its tap reaching into the gap around it. */
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
            .rowTarget(interaction)
            .padding(start = indent),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        content = content
    )
}

/** A row's reading: the mono voice, upper case, tracked. */
@Composable
internal fun SettingsReading(text: String, color: Color = MaterialTheme.colorScheme.onSurfaceVariant, modifier: Modifier = Modifier) {
    Text(text.uppercase(), style = OpenType.monoXS, color = color, modifier = modifier)
}

/** A row's name over its one reading. */
@Composable
internal fun RowScope.SettingsRowText(
    title: String,
    supporting: String? = null,
    titleColor: Color = MaterialTheme.colorScheme.onBackground,
    supportingColor: Color = MaterialTheme.colorScheme.onSurfaceVariant
) {
    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
        Text(title, style = OpenType.rowTitleL, color = titleColor)
        if (supporting != null) SettingsReading(supporting, supportingColor)
    }
}

/**
 * A row with something at its end (a status, a stepper, a picked value) that keeps its text readable
 * at any font scale: the end element sits beside the text while the title fits unbroken (and a
 * reading keeps a bit under half the row), and drops under the text when it would not.
 */
@Composable
internal fun SettingsAdaptiveRow(
    title: String,
    supporting: String? = null,
    modifier: Modifier = Modifier,
    interaction: Modifier = Modifier,
    indent: Dp = 0.dp,
    enabled: Boolean = true,
    titleColor: Color = MaterialTheme.colorScheme.onBackground,
    leading: (@Composable () -> Unit)? = null,
    trailing: @Composable () -> Unit
) {
    val measurer = rememberTextMeasurer()
    val titleStyle = OpenType.rowTitleL
    val titlePx = remember(title, titleStyle) { measurer.measure(title, titleStyle).size.width }
    val stackAll = kitStacksTrailing()
    val alpha = if (enabled) 1f else 0.35f
    Layout(
        contents = listOf(
            { if (leading != null) leading() },
            {
                Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
                    Text(title, style = titleStyle, color = titleColor.copy(alpha = titleColor.alpha * alpha))
                    if (supporting != null) {
                        SettingsReading(supporting, MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = alpha))
                    }
                }
            },
            { Box(Modifier.graphicsLayer { this.alpha = alpha }) { trailing() } }
        ),
        modifier = modifier
            .fillMaxWidth()
            .rowTarget(if (enabled) interaction else Modifier)
            .padding(start = indent)
    ) { (leadM, textM, trailM), c ->
        val gap = 16.dp.roundToPx()
        val width = c.maxWidth
        val lead = leadM.firstOrNull()?.measure(Constraints())
        val leadW = lead?.let { it.width + gap } ?: 0
        val trail = trailM.firstOrNull()?.measure(Constraints(maxWidth = (width - leadW).coerceAtLeast(0)))
        val trailW = trail?.let { it.width + gap } ?: 0
        val beside = width - leadW - trailW
        val needed = if (supporting != null) maxOf(titlePx, width * 9 / 20) else titlePx
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
            val vGap = 12.dp.roundToPx()
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

/** A state at a row's end: a filled accent dot and the word in ink when live, a ring and muted when not. */
@Composable
internal fun SettingsStatus(label: String, live: Boolean) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        if (live) Box(Modifier.size(6.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primary))
        else Box(Modifier.size(6.dp).border(1.dp, OpenColors.Rule, CircleShape))
        SettingsReading(label, if (live) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/**
 * A small outlined button at a row's end (Open, Connect, Import): 34dp, 4dp corners, the rule-grey
 * edge. With [onClick] it is its own 48dp target; without, the row it sits in takes the tap.
 */
@Composable
internal fun SettingsCompactButton(label: String, height: Dp = 34.dp, onClick: (() -> Unit)? = null) {
    val slop = (48.dp - height) / 2
    Box(
        Modifier
            .then(if (onClick != null) Modifier.touchSlop(vertical = slop).clickableLabeled(label, onClick = onClick).padding(vertical = slop) else Modifier)
            .heightIn(min = height)
            .border(1.dp, OpenColors.Rule, Corner)
            .padding(horizontal = 12.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(label, style = SettingsType.smallButton, color = MaterialTheme.colorScheme.onBackground)
    }
}

/** How a row's glyph is tinted. [Danger] is for the one row whose tap erases something. */
internal enum class TileTone { Neutral, Accent, Danger }

/**
 * A row's glyph. The redesign draws rows without icon tiles, so the kit's rows no longer show one;
 * this remains for the few places a glyph carries meaning on its own. Decorative.
 */
@Composable
internal fun SettingsIconTile(icon: ImageVector, tone: TileTone = TileTone.Neutral) {
    val tint = when (tone) {
        TileTone.Neutral -> MaterialTheme.colorScheme.onSurfaceVariant
        TileTone.Accent -> MaterialTheme.colorScheme.primary
        TileTone.Danger -> SettingsColors.Danger
    }
    Icon(icon, contentDescription = null, tint = tint, modifier = Modifier.size(20.dp))
}

/** The 16dp rule-grey chevron at a row's end: the row opens a page. */
@Composable
internal fun SettingsChevron() {
    Icon(OpenIcons.ChevronRight, contentDescription = null, tint = OpenColors.Rule, modifier = Modifier.size(16.dp))
}

/**
 * A row that goes somewhere: its name, its live value, a chevron. [external] swaps the chevron for an
 * out-arrow when the tap leaves Avex; [value] sits before the chevron for a short setting value.
 * [icon] is accepted for the old call sites and not drawn: the redesign's rows carry no tiles.
 */
@Composable
internal fun SettingsNavigationRow(
    title: String,
    supporting: String? = null,
    @Suppress("UNUSED_PARAMETER") icon: ImageVector? = null,
    tone: TileTone = TileTone.Neutral,
    value: String? = null,
    external: Boolean = false,
    enabled: Boolean = true,
    onClick: () -> Unit
) {
    SettingsRowContainer(
        interaction = if (enabled) Modifier.clickableLabeled(title, onClick = onClick) else Modifier
    ) {
        SettingsRowText(title, supporting, titleColor = if (tone == TileTone.Danger) SettingsColors.Danger else MaterialTheme.colorScheme.onBackground)
        if (value != null) {
            SettingsReading(value, modifier = Modifier.widthIn(max = 160.dp))
        }
        if (enabled) {
            if (external) {
                Icon(
                    Icons.AutoMirrored.Rounded.OpenInNew,
                    contentDescription = null,
                    tint = OpenColors.Rule,
                    modifier = Modifier.size(16.dp)
                )
            } else SettingsChevron()
        }
    }
}

/** A row that does something now. Same anatomy as navigation, no chevron: nothing opens. */
@Composable
internal fun SettingsActionRow(
    title: String,
    supporting: String? = null,
    @Suppress("UNUSED_PARAMETER") icon: ImageVector? = null,
    tone: TileTone = TileTone.Neutral,
    enabled: Boolean = true,
    indent: Dp = 0.dp,
    onClick: () -> Unit
) {
    val alpha = if (enabled) 1f else 0.35f
    val ink = if (tone == TileTone.Danger) SettingsColors.Danger else MaterialTheme.colorScheme.onBackground
    SettingsRowContainer(
        interaction = if (enabled) Modifier.clickableLabeled(title, onClick = onClick) else Modifier,
        indent = indent
    ) {
        SettingsRowText(
            title,
            supporting,
            titleColor = ink.copy(alpha = alpha),
            supportingColor = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = alpha)
        )
    }
}

/** A static fact: name and reading, no tap. */
@Composable
internal fun SettingsInfoRow(
    title: String,
    supporting: String? = null,
    @Suppress("UNUSED_PARAMETER") icon: ImageVector? = null,
    value: String? = null
) {
    SettingsRowContainer {
        SettingsRowText(title, supporting)
        if (value != null) SettingsReading(value)
    }
}

/**
 * The redesign's switch: 40×24, filled red with a white knob when on, a rule-grey outline with a
 * grey knob when off. Drawn only: the row it sits in owns the tap and announces the state.
 */
@Composable
internal fun SettingsSwitch(checked: Boolean) {
    Box(Modifier.clearAndSetSemantics { }) {
        ForgeSwitch(
            checked = checked,
            onCheckedChange = null,
            checkedThumbColor = openOnAccentLarge(),
            uncheckedTrackColor = Color.Transparent,
            uncheckedThumbColor = OpenColors.Glyph,
            uncheckedBorderColor = OpenColors.Rule
        )
    }
}

/** Name, reading and a switch; the whole row toggles. */
@Composable
internal fun SettingsSwitchRow(
    title: String,
    supporting: String? = null,
    checked: Boolean,
    @Suppress("UNUSED_PARAMETER") icon: ImageVector? = null,
    indent: Dp = 0.dp,
    /** Off when something upstream makes the switch meaningless; its value stays shown. */
    enabled: Boolean = true,
    onCheckedChange: (Boolean) -> Unit
) {
    val alpha = if (enabled) 1f else 0.35f
    SettingsRowContainer(
        interaction = if (enabled) Modifier.toggleableLabeled(title, checked) { onCheckedChange(!checked) } else Modifier,
        indent = indent
    ) {
        SettingsRowText(
            title,
            supporting,
            titleColor = MaterialTheme.colorScheme.onBackground.copy(alpha = alpha),
            supportingColor = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = alpha)
        )
        Box(Modifier.graphicsLayer { this.alpha = alpha }) { SettingsSwitch(checked) }
    }
}

/** The one switch a page hangs off ("Use coach"): a row like the others, its name a step larger. */
@Composable
internal fun SettingsMainSwitch(title: String, checked: Boolean, onCheckedChange: (Boolean) -> Unit) {
    SettingsRowContainer(interaction = Modifier.toggleableLabeled(title, checked) { onCheckedChange(!checked) }) {
        Text(title, style = OpenType.figureS, color = MaterialTheme.colorScheme.onBackground, modifier = Modifier.weight(1f))
        SettingsSwitch(checked)
    }
}

/**
 * One option of a pick-one list: name and reading in a 4dp box with the rule-grey edge, the pick
 * with the accent wash under an accent edge (the chips' look, full width). Whole box selects. Stack
 * these with `rowGap = KitPickGap`.
 */
@Composable
internal fun SettingsRadioRow(
    title: String,
    selected: Boolean,
    supporting: String? = null,
    @Suppress("UNUSED_PARAMETER") icon: ImageVector? = null,
    onClick: () -> Unit
) {
    Row(
        Modifier
            .fillMaxWidth()
            .clip(Corner)
            .background(if (selected) kitWash() else Color.Transparent)
            .border(1.dp, if (selected) MaterialTheme.colorScheme.primary else OpenColors.Rule, Corner)
            .selectable(selected = selected, role = Role.RadioButton, onClick = onClick)
            .heightIn(min = 48.dp)
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        SettingsRowText(title, supporting)
    }
}

/**
 * A row whose value is picked from a short menu: the value and a down chevron at its end, the menu
 * anchored to the row, a check on the current pick.
 */
@Composable
internal fun SettingsDropdownRow(
    title: String,
    value: String,
    options: List<String>,
    selectedIndex: Int,
    supporting: String? = null,
    @Suppress("UNUSED_PARAMETER") icon: ImageVector? = null,
    valueColor: Color = MaterialTheme.colorScheme.onBackground,
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
            interaction = Modifier.clickableLabeled("$title, $value. Change") { open = true }
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(value, style = SettingsType.chip, color = valueColor)
                Icon(OpenIcons.ChevronDown, contentDescription = null, tint = OpenColors.Rule, modifier = Modifier.size(16.dp))
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
                    { Icon(OpenIcons.Check, contentDescription = "Selected", tint = MaterialTheme.colorScheme.primary) }
                } else null,
                onClick = { onSelect(i) }
            )
        }
    }
}

// ── Chips ────────────────────────────────────────────────────────────────────────────────────────

/**
 * The redesign's chip: a 4dp box with the rule-grey edge, the pick with the accent wash under an
 * accent edge, its label in Noto Sans 14/600 ink. [height] 40 for a full-width choice, 34 for a
 * compact one beside a row's name.
 */
@Composable
internal fun SettingsChip(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    height: Dp = 40.dp,
    role: Role = Role.RadioButton,
    /** The label's inset from the edge; a crowded row (large font, narrow phone) tightens it. */
    hPad: Dp = 10.dp,
    contentDescription: String? = null
) {
    val select = if (role == Role.Checkbox) {
        Modifier.toggleable(value = selected, role = role, onValueChange = { onClick() })
    } else {
        Modifier.selectable(selected = selected, role = role, onClick = onClick)
    }
    val slop = ((48.dp - height) / 2).coerceAtLeast(0.dp)
    Box(
        modifier
            .touchSlop(vertical = slop)
            .then(select)
            .padding(vertical = slop)
            .heightIn(min = height)
            .clip(Corner)
            .background(if (selected) kitWash() else Color.Transparent)
            .border(1.dp, if (selected) MaterialTheme.colorScheme.primary else OpenColors.Rule, Corner)
            .padding(horizontal = hPad, vertical = 6.dp)
            .then(if (contentDescription != null) Modifier.semantics { this.contentDescription = contentDescription } else Modifier),
        contentAlignment = Alignment.Center
    ) {
        Text(label, style = SettingsType.chip, color = MaterialTheme.colorScheme.onBackground, textAlign = TextAlign.Center)
    }
}

/**
 * Pick one of two to seven short values as a row of equal chips, 8dp apart. Chips that cannot hold
 * their widest label (a large font scale) wrap into rows rather than break words. [compact] hugs each
 * chip to its label (34dp tall) instead of filling the width.
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
    val groupSemantics = Modifier
        .selectableGroup()
        .then(if (contentDescription != null) Modifier.semantics { this.contentDescription = contentDescription } else Modifier)
    if (compact) {
        Row(modifier.then(groupSemantics), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            options.forEachIndexed { i, label ->
                SettingsChip(label, i == selectedIndex, { onSelect(i) }, Modifier.widthIn(min = 40.dp), height = 34.dp)
            }
        }
        return
    }
    val measurer = rememberTextMeasurer()
    val density = LocalDensity.current
    val style = SettingsType.chip
    val widths = remember(options, style, measurer, density) {
        with(density) { options.map { measurer.measure(it, style).size.width.toDp() } }
    }
    val widest = widths.maxOrNull() ?: 0.dp
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val gap = 8.dp
        val pad = 22.dp
        val fits = (maxWidth - gap * (n - 1)) / n >= widest + pad
        // Equal cells cannot hold the longest word, but the labels side by side still fit: one row,
        // each chip as wide as its own label (a large font scale on a narrow phone).
        val tight = 16.dp
        val hugGap = 6.dp
        val hugFits = widths.fold(0.dp) { acc, w -> acc + w + tight } + hugGap * (n - 1) <= maxWidth
        if (!fits && hugFits) {
            Row(groupSemantics.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(hugGap)) {
                options.forEachIndexed { i, label ->
                    SettingsChip(label, i == selectedIndex, { onSelect(i) }, Modifier.weight((widths[i] + tight).value), hPad = 7.dp)
                }
            }
            return@BoxWithConstraints
        }
        val perRow = if (fits) n else ((maxWidth + gap) / (widest + pad + gap)).toInt().coerceIn(1, n)
        Column(groupSemantics, verticalArrangement = Arrangement.spacedBy(8.dp)) {
            options.indices.chunked(perRow).forEach { row ->
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(gap)) {
                    row.forEach { i ->
                        SettingsChip(options[i], i == selectedIndex, { onSelect(i) }, Modifier.weight(1f))
                    }
                    repeat(perRow - row.size) { Spacer(Modifier.weight(1f)) }
                }
            }
        }
    }
}

/** How wide a compact [SettingsSegmented] draws, measured, so a row knows whether it fits beside text. */
@Composable
private fun segmentedCompactWidth(options: List<String>): Dp {
    val style = SettingsType.chip
    val measurer = rememberTextMeasurer()
    val density = LocalDensity.current
    return remember(options, style, measurer, density) {
        val total = options.sumOf { measurer.measure(it, style).size.width }
        with(density) { total.toDp() } + 20.dp * options.size + 8.dp * (options.size - 1)
    }
}

/**
 * A setting with two to seven short values. [stacked] puts full-width chips under the name;
 * otherwise compact chips sit at the row's end (the design's Session length) and drop under the
 * name only when the two no longer fit side by side.
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
    val nameBlock: @Composable (Modifier) -> Unit = { m ->
        Column(m, verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Text(title, style = OpenType.rowTitleL, color = MaterialTheme.colorScheme.onBackground)
            if (supporting != null) SettingsReading(supporting)
        }
    }
    if (stacked) {
        Column(Modifier.fillMaxWidth()) {
            nameBlock(Modifier)
            Spacer(Modifier.height(12.dp))
            SettingsSegmented(options, selectedIndex, onSelect, contentDescription = title)
        }
        return
    }
    val measurer = rememberTextMeasurer()
    val density = LocalDensity.current
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val controlWidth = segmentedCompactWidth(options)
        val titleWidth = with(density) { measurer.measure(title, OpenType.rowTitleL).size.width.toDp() }
        val needed = if (supporting != null) maxOf(titleWidth, maxWidth * 0.4f) else titleWidth
        if (!kitStacksTrailing() && maxWidth - controlWidth - 16.dp >= needed) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                nameBlock(Modifier.weight(1f).padding(end = 16.dp))
                SettingsSegmented(options, selectedIndex, onSelect, compact = true, contentDescription = title)
            }
        } else {
            Column {
                nameBlock(Modifier)
                Spacer(Modifier.height(12.dp))
                SettingsSegmented(options, selectedIndex, onSelect, contentDescription = title)
            }
        }
    }
}

// ── Stepper ──────────────────────────────────────────────────────────────────────────────────────

/** A 36dp outlined square with a glyph: 48dp to touch, dimmed and inert at the end of its range. */
@Composable
internal fun SettingsRoundButton(icon: ImageVector, label: String, enabled: Boolean = true, onClick: () -> Unit) {
    val alpha = if (enabled) 1f else 0.35f
    Box(
        Modifier
            .touchSlop(vertical = 6.dp, horizontal = 6.dp)
            .then(if (enabled) Modifier.clickableLabeled(label, onClick = onClick) else Modifier.semantics { contentDescription = label })
            .padding(6.dp)
            .size(36.dp)
            .border(1.dp, OpenColors.Rule.copy(alpha = alpha), Corner),
        contentAlignment = Alignment.Center
    ) {
        Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.onBackground.copy(alpha = alpha), modifier = Modifier.size(18.dp))
    }
}

/** Name with the value, in serif, between − and + at its end, or under it when the name needs the room. */
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
                style = OpenType.figureS,
                color = MaterialTheme.colorScheme.onBackground,
                textAlign = TextAlign.Center,
                modifier = Modifier.widthIn(min = 64.dp).semantics { contentDescription = "$title $value" }
            )
            SettingsRoundButton(OpenIcons.Plus, "More $title", canIncrease, onIncrease)
        }
    }
}

/** A wrapping set of chips, 8dp apart, open on the page. */
@Composable
internal fun SettingsChipBlock(content: @Composable FlowRowScope.() -> Unit) {
    FlowRow(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        content = content
    )
}

/** A pick-any chip (a priority muscle, a plate size): [SettingsChip] at 36dp with a checkbox role. */
@Composable
internal fun SettingsFilterChip(label: String, selected: Boolean, onClick: () -> Unit) {
    SettingsChip(label, selected, onClick, height = 36.dp, role = Role.Checkbox)
}

/** A small label: a status reading, or a drawn call to action inside a tappable row. */
internal enum class PillTone { Neutral, Accent, Quiet }

@Composable
internal fun SettingsPill(text: String, tone: PillTone = PillTone.Neutral, dot: Boolean = false) {
    when (tone) {
        PillTone.Neutral -> Box(
            Modifier.border(1.dp, OpenColors.Rule, Corner).padding(horizontal = 10.dp, vertical = 6.dp)
        ) { SettingsReading(text, MaterialTheme.colorScheme.onBackground) }
        PillTone.Accent -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Box(Modifier.size(6.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primary))
            SettingsReading(text, MaterialTheme.colorScheme.onBackground)
        }
        PillTone.Quiet -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (dot) Box(Modifier.size(6.dp).border(1.dp, OpenColors.Rule, CircleShape))
            SettingsReading(text)
        }
    }
}

// ── Buttons ──────────────────────────────────────────────────────────────────────────────────────

internal enum class ButtonKind { Primary, Tonal, Outlined, Danger }

/**
 * The Settings button: 40dp, 4dp corners. [ButtonKind.Primary] is the page's one do-it-now, filled
 * red with dark text as the redesign's small filled button is; the rest are outlined in the rule
 * grey ([ButtonKind.Danger] with its words in the soft danger red).
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
    val alpha = if (enabled) 1f else 0.35f
    val ink = when (kind) {
        ButtonKind.Primary -> MaterialTheme.colorScheme.onPrimary
        ButtonKind.Danger -> SettingsColors.Danger
        else -> MaterialTheme.colorScheme.onBackground
    }
    Row(
        modifier
            .touchSlop(vertical = 4.dp)
            .then(if (enabled) Modifier.clickableLabeled(label, onClick = onClick) else Modifier.semantics { contentDescription = label })
            .padding(vertical = 4.dp)
            .heightIn(min = 40.dp)
            .graphicsLayer { this.alpha = alpha }
            .clip(Corner)
            .then(
                if (kind == ButtonKind.Primary) Modifier.background(MaterialTheme.colorScheme.primary)
                else Modifier.border(1.dp, OpenColors.Rule, Corner)
            )
            .padding(horizontal = 16.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically
    ) {
        if (icon != null) Icon(icon, contentDescription = null, tint = ink, modifier = Modifier.size(18.dp))
        Text(label, style = SettingsType.chip, color = ink, textAlign = TextAlign.Center)
    }
}

/** Buttons that belong together, wrapping to a second line at large font scales. */
@Composable
internal fun SettingsButtonBar(modifier: Modifier = Modifier, content: @Composable FlowRowScope.() -> Unit) {
    FlowRow(
        modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
        content = content
    )
}

// ── Search ───────────────────────────────────────────────────────────────────────────────────────

/**
 * The root's search, opened from the top bar's glyph: the field takes the bar itself (the back arrow
 * stays at the start), a hairline under it, and an ✕ that clears it, then closes it.
 */
@Composable
internal fun RowScope.SettingsSearchField(
    query: String,
    placeholder: String,
    onQueryChange: (String) -> Unit,
    onBack: () -> Unit,
    onClose: () -> Unit
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    OpenTopBarIcon(OpenIcons.ArrowLeft, "Back", onBack)
    Column(Modifier.weight(1f)) {
        BasicTextField(
            value = query,
            onValueChange = onQueryChange,
            textStyle = OpenType.rowTitleL.copy(color = MaterialTheme.colorScheme.onBackground, fontWeight = FontWeight.Normal),
            cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
            singleLine = true,
            modifier = Modifier.fillMaxWidth().focusRequester(focus).padding(vertical = 8.dp),
            decorationBox = { inner ->
                Box {
                    if (query.isEmpty()) Text(placeholder, style = OpenType.rowTitleL.copy(fontWeight = FontWeight.Normal), color = muted)
                    inner()
                }
            }
        )
        Box(Modifier.fillMaxWidth().height(1.dp).background(OpenColors.Rule))
    }
    OpenTopBarIcon(OpenIcons.X, if (query.isEmpty()) "Close search" else "Clear search", { if (query.isEmpty()) onClose() else onQueryChange("") })
}

/** The older full-width search bar, kept for the recipe gallery: the same field, open on the page. */
@Composable
internal fun SettingsSearchBar(
    query: String,
    placeholder: String,
    onQueryChange: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier.fillMaxWidth().heightIn(min = 48.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Icon(OpenIcons.Search, contentDescription = null, tint = muted, modifier = Modifier.size(20.dp))
        Column(Modifier.weight(1f)) {
            BasicTextField(
                value = query,
                onValueChange = onQueryChange,
                textStyle = OpenType.rowTitleL.copy(color = MaterialTheme.colorScheme.onBackground, fontWeight = FontWeight.Normal),
                cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                singleLine = true,
                modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp),
                decorationBox = { inner ->
                    Box {
                        if (query.isEmpty()) Text(placeholder, style = OpenType.rowTitleL.copy(fontWeight = FontWeight.Normal), color = muted)
                        inner()
                    }
                }
            )
            Box(Modifier.fillMaxWidth().height(1.dp).background(OpenColors.Rule))
        }
    }
}

// ── Notices and empty states ─────────────────────────────────────────────────────────────────────

/**
 * A page-top notice for a state that stops the page working (notifications blocked, Health Connect
 * missing): a dot (accent for a fault, a ring for a missing piece), what is wrong in a row's voice,
 * why in body, and the one outlined button that fixes it.
 */
@Composable
internal fun SettingsNotice(
    @Suppress("UNUSED_PARAMETER") icon: ImageVector,
    title: String,
    body: String,
    action: String? = null,
    critical: Boolean = true,
    onAction: () -> Unit = {}
) {
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(14.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Box(
                Modifier
                    .padding(top = 7.dp)
                    .size(10.dp)
                    .clip(CircleShape)
                    .then(
                        if (critical) Modifier.background(MaterialTheme.colorScheme.primary)
                        else Modifier.border(1.5.dp, OpenColors.Rule, CircleShape)
                    )
            )
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(title, style = OpenType.rowTitleL, color = MaterialTheme.colorScheme.onBackground)
                Text(body, style = OpenType.bodyS, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        if (action != null) {
            SettingsButton(action, Modifier.padding(start = 22.dp), kind = ButtonKind.Outlined, onClick = onAction)
        }
    }
}

/** A list with nothing in it yet: what would be here, and how to add the first, in plain words. */
@Composable
internal fun SettingsEmptyBlock(@Suppress("UNUSED_PARAMETER") icon: ImageVector, title: String, body: String) {
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(title, style = OpenType.rowTitleL, color = MaterialTheme.colorScheme.onBackground)
        Text(body, style = OpenType.bodyS, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** Spaces a lazy row from the next one in its section. */
@Composable
internal fun KitLazyRow(index: Int, count: Int, content: @Composable () -> Unit) {
    CompositionLocalProvider(LocalKitRowShape provides kitRowShape(index, count)) {
        Box(Modifier.padding(bottom = kitRowSpacing(index, count))) { content() }
    }
}

// ── Sheets and dialogs ───────────────────────────────────────────────────────────────────────────

/**
 * A Settings bottom sheet: a serif title, one supporting line, then sections. Opens fully (these
 * hold a list of choices, not a peek) and scrolls at large font scales.
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
            verticalArrangement = Arrangement.spacedBy(28.dp)
        ) {
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(
                    title,
                    style = OpenType.sheetTitle,
                    color = MaterialTheme.colorScheme.onBackground,
                    modifier = Modifier.semantics { heading() }
                )
                if (supporting != null) {
                    Text(supporting, style = OpenType.bodyS, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            content()
        }
    }
}

/**
 * The one confirmation every irreversible Settings act goes through: an icon naming the kind of
 * act, a question for a title, what it does, and a 4dp button that says the act, not "OK".
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
                shape = Corner,
                colors = ButtonDefaults.buttonColors(
                    containerColor = if (destructive) scheme.error else scheme.primary,
                    contentColor = if (destructive) scheme.onError else scheme.onPrimary,
                    disabledContainerColor = scheme.onSurface.copy(alpha = 0.15f),
                    disabledContentColor = scheme.onSurface.copy(alpha = 0.35f)
                )
            ) { Text(confirmLabel) }
        },
        dismissButton = { SettingsDialogDismiss(dismissLabel, onDismiss) }
    )
}

/** A dialog's way out: outlined, square-cornered like the action beside it, never a pill. */
@Composable
internal fun SettingsDialogDismiss(label: String, onClick: () -> Unit) {
    androidx.compose.material3.OutlinedButton(
        onClick = onClick,
        shape = Corner,
        border = androidx.compose.foundation.BorderStroke(1.dp, OpenColors.Rule),
        colors = ButtonDefaults.outlinedButtonColors(contentColor = MaterialTheme.colorScheme.onSurface)
    ) { Text(label) }
}

/** A text field in the Settings palette: accent when focused, the rule grey when not. */
@Composable
internal fun settingsFieldColors() = androidx.compose.material3.OutlinedTextFieldDefaults.colors(
    focusedBorderColor = MaterialTheme.colorScheme.primary,
    unfocusedBorderColor = OpenColors.Rule,
    cursorColor = MaterialTheme.colorScheme.primary,
    focusedLabelColor = MaterialTheme.colorScheme.onSurface,
    unfocusedLabelColor = MaterialTheme.colorScheme.onSurfaceVariant
)

/** A thin determinate bar: a share of something done (an export's progress). */
@Composable
internal fun SettingsProgressBar(fraction: Float, modifier: Modifier = Modifier) {
    OpenMeter(fraction, modifier)
}
