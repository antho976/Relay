package com.tally.app.ui.common

import com.tally.app.data.db.CategoryEntity
import com.tally.core.CategoryKind
import org.junit.Assert.assertEquals
import org.junit.Test

class CategoryGridTest {

    private val all = (1L..12L).map { CategoryEntity(id = it, name = "C$it", kind = CategoryKind.EXPENSE, color = 0, icon = "cart") }

    @Test fun twoRowsFoldTheRestBehindMore() {
        val cells = gridCells(all, selectedId = 1, expanded = false)
        // Seven categories and the More cell fill two rows of four.
        assertEquals((1L..7L).toList(), cells.shown.map { it.id })
        assertEquals(5, cells.hidden)
    }

    @Test fun thePickedCategoryIsAlwaysInSight() {
        val cells = gridCells(all, selectedId = 11, expanded = false)
        assertEquals(listOf(1L, 2L, 3L, 4L, 5L, 6L, 11L), cells.shown.map { it.id })
        assertEquals(5, cells.hidden)
    }

    @Test fun unfoldedOrSmallEveryCategoryShows() {
        assertEquals(12, gridCells(all, null, expanded = true).shown.size)
        assertEquals(0, gridCells(all, null, expanded = true).hidden)
        // Eight fit in two rows by themselves: no More cell is spent.
        assertEquals(8, gridCells(all.take(8), null, expanded = false).shown.size)
        assertEquals(0, gridCells(all.take(8), null, expanded = false).hidden)
    }
}
