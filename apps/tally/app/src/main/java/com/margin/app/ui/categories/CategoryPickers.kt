package com.margin.app.ui.categories

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.LabelOff
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.margin.app.ui.common.CategoryBadge
import com.margin.app.ui.common.CategoryIcons
import com.margin.app.ui.common.GUTTER
import com.margin.app.ui.common.GlyphBadge
import com.margin.app.ui.common.Group
import com.margin.app.ui.common.GroupRow
import com.margin.app.ui.common.bounceClick
import com.margin.app.ui.common.selectableColors
import com.margin.app.ui.theme.categoryColor
import com.margin.core.Copy
import com.margin.core.Defaults

/*
 * The category editor's pickers: the glyph grid, the twelve hue swatches and the sheet that asks
 * where a deleted category's entries go.
 */

private val TILE_GAP = 6.dp

/** The smallest cell either grid lays out: a 48dp target. */
private val MIN_CELL = 48.dp

/** Six columns when six cells of [MIN_CELL] fit, otherwise [fallback], so no target drops under 48dp. */
private fun columnsFor(width: Dp, fallback: Int): Int =
    if ((width - TILE_GAP * 5) / 6 >= MIN_CELL) 6 else fallback

/**
 * Every glyph in [CategoryIcons.all] as one radio group of tiles, six to a row. The picked tile
 * wears the accent ring and wash and shows its glyph in the category's own hue.
 */
@Composable
internal fun IconGrid(selected: String, hue: Color, onPick: (String) -> Unit, modifier: Modifier = Modifier) {
    val keys = remember { CategoryIcons.all.keys.toList() }
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val columns = columnsFor(maxWidth, fallback = 5)
        val rows = remember(columns) { keys.chunked(columns) }
        Column(Modifier.fillMaxWidth().selectableGroup(), verticalArrangement = Arrangement.spacedBy(TILE_GAP)) {
            rows.forEach { row ->
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(TILE_GAP)) {
                    row.forEach { key ->
                        IconTile(key, key == selected, hue, Modifier.weight(1f)) { onPick(key) }
                    }
                    repeat(columns - row.size) { Spacer(Modifier.weight(1f)) }
                }
            }
        }
    }
}

@Composable
private fun IconTile(key: String, selected: Boolean, hue: Color, modifier: Modifier, onClick: () -> Unit) {
    val (ring, wash) = selectableColors(selected)
    val shape = RoundedCornerShape(14.dp)
    val name = iconName(key)
    Box(
        modifier
            .aspectRatio(1f)
            .heightIn(min = 48.dp)
            .clip(shape)
            .background(wash)
            .border(2.dp, if (selected) ring else Color.Transparent, shape)
            .bounceClick(label = name, role = Role.RadioButton, onClick = onClick)
            .semantics {
                contentDescription = name
                this.selected = selected
            },
        contentAlignment = Alignment.Center,
    ) {
        GlyphBadge(
            CategoryIcons.of(key),
            tint = if (selected) hue else MaterialTheme.colorScheme.onBackground,
            fill = if (selected) hue.copy(alpha = 0.15f) else MaterialTheme.colorScheme.surfaceContainerHighest,
            size = 38.dp,
        )
    }
}

/**
 * The twelve category hues, six to a row (four on a narrow screen), Avex's accent-picker look:
 * the picked one wears a ring and a check. One radio group; each swatch says its number and name.
 */
@Composable
internal fun HueSwatches(selected: Int, onPick: (Int) -> Unit, modifier: Modifier = Modifier) {
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val columns = columnsFor(maxWidth, fallback = 4)
        val rows = remember(columns) { (0 until Defaults.PALETTE_SIZE).toList().chunked(columns) }
        Column(Modifier.fillMaxWidth().selectableGroup(), verticalArrangement = Arrangement.spacedBy(TILE_GAP)) {
            rows.forEach { row ->
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(TILE_GAP)) {
                    row.forEach { i ->
                        Box(Modifier.weight(1f), contentAlignment = Alignment.Center) {
                            Swatch(i, i == selected) { onPick(i) }
                        }
                    }
                    repeat(columns - row.size) { Spacer(Modifier.weight(1f)) }
                }
            }
        }
    }
}

@Composable
private fun Swatch(index: Int, selected: Boolean, onClick: () -> Unit) {
    val ring = MaterialTheme.colorScheme.onBackground
    val label = "Colour ${index + 1}, " + hueName(index).lowercase()
    Box(
        Modifier
            .size(48.dp)
            .clip(CircleShape)
            .bounceClick(label = label, role = Role.RadioButton, onClick = onClick)
            .semantics {
                contentDescription = label
                this.selected = selected
            }
            .border(2.dp, if (selected) ring else Color.Transparent, CircleShape)
            .padding(5.dp)
            .clip(CircleShape)
            .background(categoryColor(index)),
        contentAlignment = Alignment.Center,
    ) {
        if (selected) {
            Icon(
                Icons.Rounded.Check,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.background,
                modifier = Modifier.size(20.dp),
            )
        }
    }
}

/**
 * Deleting a category that holds entries: they move to another category of its kind, or stay as
 * uncategorized. Picking a row is the commit; dismissing keeps the category.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun MoveSheet(
    name: String,
    entries: Int,
    targets: List<MoveTarget>,
    onMove: (Long?) -> Unit,
    onDismiss: () -> Unit,
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
    ) {
        Column(
            Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .navigationBarsPadding()
                .padding(start = GUTTER, end = GUTTER, bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(
                "Move its ${Copy.plural(entries, "entry", "entries")} to",
                style = MaterialTheme.typography.headlineMedium,
                color = MaterialTheme.colorScheme.onBackground,
                modifier = Modifier.semantics { heading() },
            )
            Text(
                "$name is deleted, with its budget if it has one. This cannot be undone.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            val rows: List<@Composable (Shape) -> Unit> = targets.map { t -> moveRow(t, onMove) } + leaveRow(onMove)
            Group(rows = rows)
        }
    }
}

private fun moveRow(t: MoveTarget, onMove: (Long?) -> Unit): @Composable (Shape) -> Unit = { shape ->
    GroupRow(
        t.name,
        shape,
        subtitle = Copy.plural(t.entries, "entry", "entries"),
        leading = { CategoryBadge(t.icon, t.color, size = 40.dp) },
        chevron = false,
        onClick = { onMove(t.id) },
    )
}

private fun leaveRow(onMove: (Long?) -> Unit): @Composable (Shape) -> Unit = { shape ->
    GroupRow(
        "Leave them uncategorized",
        shape,
        subtitle = "They stay in the ledger with no category",
        leading = { GlyphBadge(Icons.AutoMirrored.Rounded.LabelOff, size = 40.dp) },
        chevron = false,
        onClick = { onMove(null) },
    )
}
