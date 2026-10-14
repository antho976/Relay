package com.tally.app.ui.common

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.selection.selectable
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.tally.app.data.db.CategoryEntity
import com.tally.app.ui.kit.CategoryBadge
import com.tally.app.ui.kit.GlyphBadge
import com.tally.app.ui.kit.Radii
import com.tally.app.ui.kit.T
import com.tally.app.ui.theme.Tally

/** Four to a row: a phone's width holds four names at caption size without crowding them. */
const val GRID_COLUMNS = 4

/** What the grid shows: the categories in their cells, and how many wait behind More. */
@Immutable
internal data class GridCells(val shown: List<CategoryEntity>, val hidden: Int)

/**
 * The cells of a folded grid: the first [slots] less one (that cell is More), the picked category
 * always among them, swapped in for the last one when it sits further down. Unfolded, or when
 * everything fits, every category shows.
 */
internal fun gridCells(all: List<CategoryEntity>, selectedId: Long?, expanded: Boolean, slots: Int = GRID_COLUMNS * 2): GridCells {
    if (expanded || all.size <= slots) return GridCells(all, 0)
    var shown = all.take(slots - 1)
    val picked = all.firstOrNull { it.id == selectedId }
    if (picked != null && shown.none { it.id == picked.id }) shown = shown.dropLast(1) + picked
    return GridCells(shown, all.size - shown.size)
}

/**
 * Categories as an even grid: each one's badge over its name, in cells of one size, so the
 * choice reads as a set of places rather than a run of chips. Folded to two rows with a More cell
 * when there are more; unfolded, it ends on New (when [onNew] is given) and Fewer.
 */
@Composable
fun CategoryGrid(
    categories: List<CategoryEntity>,
    selectedId: Long?,
    onPick: (Long) -> Unit,
    modifier: Modifier = Modifier,
    onNew: (() -> Unit)? = null,
) {
    var expanded by rememberSaveable { mutableStateOf(false) }
    val cells = gridCells(categories, selectedId, expanded)
    val tiles = buildList<@Composable (Modifier) -> Unit> {
        cells.shown.forEach { cat ->
            add { m -> CategoryTile(cat.name, cat.icon, cat.color, cat.id == selectedId, m) { onPick(cat.id) } }
        }
        if (cells.hidden > 0) {
            add { m -> ActionTile("more", "${cells.hidden} more", m) { expanded = true } }
        } else {
            if (onNew != null) add { m -> ActionTile("plus", "New", m, onNew) }
            if (expanded) add { m -> ActionTile("chevron-up", "Fewer", m) { expanded = false } }
        }
    }
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(2.dp)) {
        tiles.chunked(GRID_COLUMNS).forEach { row ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                row.forEach { tile -> tile(Modifier.weight(1f)) }
                repeat(GRID_COLUMNS - row.size) { Box(Modifier.weight(1f)) }
            }
        }
    }
}

/** One category's cell: its badge, ringed in ink when it is the one, over its name. */
@Composable
private fun CategoryTile(name: String, icon: String?, color: Int?, selected: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val c = Tally.colors
    Column(
        modifier
            .heightIn(min = 76.dp)
            .clip(Radii.key)
            .background(if (selected) c.wash else Color.Transparent)
            .selectable(selected = selected, role = Role.RadioButton, onClick = onClick)
            .padding(horizontal = 2.dp, vertical = 8.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        CategoryBadge(
            icon,
            color,
            size = 38.dp,
            modifier = if (selected) Modifier.border(1.5.dp, c.ink, Radii.badge) else Modifier,
        )
        T(
            name,
            style = Tally.type.caption,
            color = if (selected) c.ink else c.ink2,
            maxLines = 1,
            weight = if (selected) FontWeight.Medium else null,
            align = TextAlign.Center,
        )
    }
}

/** A cell that acts on the grid itself: More, New, Fewer. */
@Composable
private fun ActionTile(glyph: String, label: String, modifier: Modifier, onClick: () -> Unit) {
    val c = Tally.colors
    Column(
        modifier
            .heightIn(min = 76.dp)
            .clip(Radii.key)
            .clickable(role = Role.Button, onClick = onClick)
            .padding(horizontal = 2.dp, vertical = 8.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        GlyphBadge(glyph, c.ink2, c.raised, size = 38.dp)
        T(label, style = Tally.type.caption, color = c.ink3, maxLines = 1, align = TextAlign.Center)
    }
}
