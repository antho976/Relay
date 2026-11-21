package com.forge.app.ui.settings

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.touchSlop
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.OpenIcons

/**
 * A row of choices that never runs past the screen's edge (Antho, 2026-10-21): as many cells as fit
 * the width sit in one row, the current pick always among them, and a small "More ▾" slides the rest
 * down in rows of the same width ("Less ▴" folds them away again).
 *
 * [cellWidth] is each cell's footprint; the cells spread across the row so its edges line up with
 * the page's gutter. [noun] names the choices for the toggle's spoken label ("accent colours").
 */
@Composable
internal fun <T> SettingsFitGrid(
    items: List<T>,
    pickedIndex: Int,
    cellWidth: Dp,
    noun: String,
    cell: @Composable (T) -> Unit,
) {
    var expanded by rememberSaveable { mutableStateOf(false) }
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val minGap = 8.dp
        val cols = ((maxWidth + minGap) / (cellWidth + minGap)).toInt().coerceIn(1, items.size.coerceAtLeast(1))
        // The first row keeps the current pick in view: when it sits further along, it takes the
        // row's last place, and the rest keep their order below.
        val firstRow = if (pickedIndex < cols) items.take(cols)
            else items.take(cols - 1) + items[pickedIndex]
        val rest = items.filterIndexed { i, _ -> items[i] !in firstRow }
        Column {
            GridRow(firstRow, cols, cellWidth, cell)
            if (rest.isNotEmpty()) {
                AnimatedVisibility(
                    visible = expanded,
                    enter = expandVertically(ForgeMotion.enterTween()) + fadeIn(ForgeMotion.enterTween()),
                    exit = shrinkVertically(ForgeMotion.exitTween()) + fadeOut(ForgeMotion.exitTween()),
                ) {
                    Column {
                        rest.chunked(cols).forEach { row ->
                            Spacer(Modifier.height(16.dp))
                            GridRow(row, cols, cellWidth, cell)
                        }
                    }
                }
                Spacer(Modifier.height(10.dp))
                Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                    Row(
                        Modifier
                            .touchSlop(vertical = 12.dp, horizontal = 12.dp)
                            .clickableLabeled(if (expanded) "Show fewer $noun" else "Show all ${items.size} $noun") { expanded = !expanded }
                            .padding(vertical = 12.dp, horizontal = 12.dp),
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        OpenLabel(if (expanded) "Less" else "More · ${rest.size}")
                        Icon(
                            if (expanded) OpenIcons.ChevronUp else OpenIcons.ChevronDown,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.size(14.dp),
                        )
                    }
                }
            }
        }
    }
}

/** One row of the grid: [cols] slots spread edge to edge, a short last row keeping the same slots. */
@Composable
private fun <T> GridRow(row: List<T>, cols: Int, cellWidth: Dp, cell: @Composable (T) -> Unit) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
        repeat(cols) { i ->
            Box(Modifier.width(cellWidth), contentAlignment = Alignment.TopCenter) {
                row.getOrNull(i)?.let { cell(it) }
            }
        }
    }
}
