package com.tally.app.ui.common

import androidx.compose.ui.unit.sp
import com.tally.app.ui.kit.CategoryGlyph
import com.tally.app.ui.kit.Glyph
import com.tally.app.ui.kit.softOf
import com.tally.app.ui.theme.categoryColor
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
    // At a large font four names to a row would be cut: three instead.
    val columns = if (androidx.compose.ui.platform.LocalDensity.current.fontScale >= 1.5f) 3 else GRID_COLUMNS
    val cells = gridCells(categories, selectedId, expanded, slots = columns * 2)
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
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        tiles.chunked(columns).forEach { row ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                row.forEach { tile -> tile(Modifier.weight(1f)) }
                repeat(columns - row.size) { Box(Modifier.weight(1f)) }
            }
        }
    }
}

/** One category's cell: a soft tile of its own hue, its icon over its name, ringed in ink when picked. */
@Composable
private fun CategoryTile(name: String, icon: String?, color: Int?, selected: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val c = Tally.colors
    val hue = categoryColor(color)
    Column(
        modifier
            .heightIn(min = 80.dp)
            .clip(Radii.card.let { androidx.compose.foundation.shape.RoundedCornerShape(18.dp) })
            .background(if (c.light) softOf(hue) else hue.copy(alpha = .22f))
            .then(if (selected) Modifier.border(2.dp, c.ink, androidx.compose.foundation.shape.RoundedCornerShape(18.dp)) else Modifier)
            .selectable(selected = selected, role = Role.RadioButton, onClick = onClick)
            .padding(horizontal = 4.dp, vertical = 12.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        CategoryGlyph(icon, color, size = 24.dp)
        T(
            name,
            style = Tally.type.caption.copy(fontSize = 12.sp),
            color = c.ink,
            maxLines = 1,
            weight = FontWeight.SemiBold,
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
            .heightIn(min = 80.dp)
            .clip(androidx.compose.foundation.shape.RoundedCornerShape(18.dp))
            .background(c.slab)
            .clickable(role = Role.Button, onClick = onClick)
            .padding(horizontal = 4.dp, vertical = 12.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Glyph(glyph, 22.dp, c.ink2)
        T(label, style = Tally.type.caption.copy(fontSize = 12.sp), color = c.ink, maxLines = 1, weight = FontWeight.SemiBold, align = TextAlign.Center)
    }
}
