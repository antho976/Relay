package com.margin.app.ui.categories

import com.margin.app.data.db.CategoryEntity
import com.margin.app.data.db.CategoryTotal
import com.margin.app.data.db.CategoryWithCount
import com.margin.app.ui.common.CategoryIcons
import com.margin.core.BudgetPeriod
import com.margin.core.CategoryKind
import com.margin.core.Copy
import com.margin.core.Defaults
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate

class CategoriesLogicTest {

    private fun entity(id: Long, name: String, kind: CategoryKind = CategoryKind.EXPENSE, color: Int = 0, archived: Boolean = false) =
        CategoryEntity(id = id, name = name, kind = kind, color = color, icon = "dots", archived = archived, sortOrder = id.toInt())

    private fun counted(id: Long, name: String, entries: Int, kind: CategoryKind = CategoryKind.EXPENSE, color: Int = 0, archived: Boolean = false) =
        CategoryWithCount(entity(id, name, kind, color, archived), entries)

    private val all = listOf(
        counted(1, "Groceries", 48, color = 0),
        counted(2, "Dining", 61, color = 6),
        counted(3, "Travel", 0, color = 8),
        counted(4, "Gifts", 3, color = 11, archived = true),
        counted(20, "Salary", 18, CategoryKind.INCOME),
    )

    @Test fun summaryKeepsToItsKindAndSplitsArchived() {
        val s = summarize(CategoryKind.EXPENSE, all, emptyList())
        assertEquals(listOf(1L, 2L, 3L), s.active.map { it.id })
        assertEquals(listOf(4L), s.archived.map { it.id })
        assertEquals(48 + 61 + 0 + 3, s.entries)
        assertEquals(2L, s.mostUsed?.id)
        assertEquals(1, s.unused)
        assertEquals(0L, s.periodTotal)
        assertTrue(s.segments.isEmpty())
        assertEquals(0L, s.averageEntry)
    }

    @Test fun periodMoneyIsSplitLargestFirstWithTheRemainderUncategorized() {
        val totals = listOf(CategoryTotal(1, 30_000, 3), CategoryTotal(2, 60_000, 6), CategoryTotal(null, 10_000, 1))
        val s = summarize(CategoryKind.EXPENSE, all, totals)
        assertEquals(100_000L, s.periodTotal)
        assertEquals(10, s.periodEntries)
        assertEquals(10_000L, s.averageEntry)
        assertEquals(listOf("Dining", "Groceries", "Uncategorized"), s.segments.map { it.name })
        assertEquals(listOf(60, 30, 10), s.segments.map { it.percent })
        assertNull(s.segments.last().color)
        assertEquals(2, s.usedThisPeriod)
        assertEquals(30, s.active.first { it.id == 1L }.percent)
        assertEquals(30_000L, s.active.first { it.id == 1L }.periodTotal)
    }

    @Test fun anEmptyLedgerIsAnHonestZero() {
        val s = summarize(CategoryKind.INCOME, emptyList(), emptyList())
        assertTrue(s.active.isEmpty())
        assertNull(s.mostUsed)
        assertEquals(0, s.unused)
        assertEquals(0L, s.periodTotal)
    }

    @Test fun aNameIsRequired() {
        assertEquals(MISSING_NAME, categoryNameProblem("   ", CategoryKind.EXPENSE, 0, all.map { it.category }))
    }

    @Test fun namesAreUniqueWithinAKindIgnoringCase() {
        val entities = all.map { it.category }
        assertEquals(
            "You already have a spending category called Dining",
            categoryNameProblem(" dining ", CategoryKind.EXPENSE, 0, entities),
        )
        assertEquals(
            "An archived spending category is already called Gifts",
            categoryNameProblem("GIFTS", CategoryKind.EXPENSE, 0, entities),
        )
        // The category itself, and the other kind, never clash.
        assertNull(categoryNameProblem("Dining", CategoryKind.EXPENSE, 2, entities))
        assertNull(categoryNameProblem("Dining", CategoryKind.INCOME, 0, entities))
        assertNull(categoryNameProblem("Coffee", CategoryKind.EXPENSE, 0, entities))
    }

    @Test fun aMissingNameShowsOnlyAfterASaveIsTried() {
        val today = LocalDate.of(2026, 10, 14)
        val base = CategoryEditState(today = today, period = BudgetPeriod.containing(today))
        assertNull(base.copy(nameProblem = MISSING_NAME).shownNameProblem)
        assertEquals(MISSING_NAME, base.copy(nameProblem = MISSING_NAME, showErrors = true).shownNameProblem)
        assertEquals("clash", base.copy(nameProblem = "clash").shownNameProblem)
    }

    @Test fun aNewColourIsTheFirstHueItsKindDoesNotUse() {
        assertEquals(0, suggestColor(emptyList()))
        assertEquals(2, suggestColor(listOf(0, 1, 3)))
        assertEquals(1, suggestColor((0 until Defaults.PALETTE_SIZE).toList() + 0))
    }

    @Test fun movesGoToOtherActiveCategoriesOfTheSameKind() {
        val targets = moveTargets(all, CategoryKind.EXPENSE, selfId = 2)
        assertEquals(listOf(1L, 3L), targets.map { it.id })
        assertEquals(48, targets.first().entries)
    }

    @Test fun theDeleteNoticeSaysWhereTheEntriesWent() {
        assertEquals("Category deleted", deletedLine("Dining", 0, null))
        assertEquals("Dining deleted · 61 entries moved to Groceries", deletedLine("Dining", 61, "Groceries"))
        assertEquals("Dining deleted · 1 entry left uncategorized", deletedLine("Dining", 1, null))
    }

    @Test fun everyIconAndHueHasASpokenName() {
        CategoryIcons.all.keys.forEach { key -> assertTrue(key, key in ICON_NAMES) }
        assertEquals(Defaults.PALETTE_SIZE, HUE_NAMES.size)
        assertEquals("Colour 13", hueName(12))
    }

    @Test fun everyLineKeepsTheVoice() {
        val lines = listOf(
            deletedLine("Dining", 3, "Groceries"),
            deletedLine("Dining", 3, null),
            MISSING_NAME,
            categoryNameProblem("Dining", CategoryKind.EXPENSE, 0, all.map { it.category }).orEmpty(),
        ) + CategoryIcons.all.keys.map { iconName(it) }
        lines.forEach { line -> Copy.banned.forEach { bad -> assertFalse("\"$line\" has \"$bad\"", line.lowercase().contains(bad)) } }
    }
}
