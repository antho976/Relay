package com.tally.app.ui.categories

import androidx.compose.ui.test.junit4.createComposeRule
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.CategoryTotal
import com.tally.app.data.db.CategoryWithCount
import com.tally.app.testing.Fixtures
import com.tally.app.testing.shoot
import com.tally.core.CategoryKind
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = RobolectricDeviceQualifiers.Pixel7)
class CategoriesScreenshotTest {

    @get:Rule val compose = createComposeRule()

    private fun category(
        id: Long,
        name: String,
        color: Int,
        icon: String,
        entries: Int,
        kind: CategoryKind = CategoryKind.EXPENSE,
        archived: Boolean = false,
    ) = CategoryWithCount(
        CategoryEntity(id = id, name = name, kind = kind, color = color, icon = icon, archived = archived, sortOrder = id.toInt()),
        entries,
    )

    private val all = listOf(
        category(1, "Groceries", 0, "cart", 48),
        category(2, "Dining", 6, "dining", 61),
        category(3, "Transport", 2, "transport", 22),
        category(4, "Housing", 9, "home", 9),
        category(5, "Utilities", 7, "bolt", 12),
        category(6, "Phone & internet", 1, "wifi", 9),
        category(7, "Shopping", 4, "bag", 17),
        category(8, "Health", 5, "health", 4),
        category(9, "Entertainment", 3, "ticket", 0),
        category(10, "Subscriptions", 10, "repeat", 27),
        category(11, "Travel", 8, "flight", 0),
        category(12, "Gifts", 11, "gift", 3, archived = true),
        category(20, "Salary", 0, "work", 18, CategoryKind.INCOME),
        category(21, "Side income", 1, "spark", 4, CategoryKind.INCOME),
        category(22, "Refunds", 2, "refund", 2, CategoryKind.INCOME),
    )

    private val spent = listOf(
        CategoryTotal(1, 25_440, 7),
        CategoryTotal(2, 21_800, 9),
        CategoryTotal(3, 12_400, 4),
        CategoryTotal(4, 140_000, 1),
        CategoryTotal(5, 7_840, 1),
        CategoryTotal(7, 19_999, 2),
        CategoryTotal(10, 6_497, 3),
        CategoryTotal(null, 4_200, 2),
    )

    private val earned = listOf(CategoryTotal(20, 430_000, 2), CategoryTotal(21, 18_000, 1))

    private val populated = CategoriesState(
        today = Fixtures.TODAY,
        period = Fixtures.PERIOD,
        spending = summarize(CategoryKind.EXPENSE, all, spent),
        income = summarize(CategoryKind.INCOME, all, earned),
        loaded = true,
    )

    private val zero = CategoriesState(
        today = Fixtures.TODAY,
        period = Fixtures.PERIOD,
        spending = summarize(CategoryKind.EXPENSE, emptyList(), emptyList()),
        income = summarize(CategoryKind.INCOME, emptyList(), emptyList()),
        loaded = true,
    )

    @Test fun categories() = compose.shoot("categories") { CategoriesScreen(populated, CategoryKind.EXPENSE, CategoriesActions()) }
    @Test fun categories200() = compose.shoot("categories-200", fontScale = 2f) {
        CategoriesScreen(populated, CategoryKind.EXPENSE, CategoriesActions())
    }
    @Test fun categoriesIncome() = compose.shoot("categories-income") { CategoriesScreen(populated, CategoryKind.INCOME, CategoriesActions()) }
    @Test fun categoriesZero() = compose.shoot("categories-zero") { CategoriesScreen(zero, CategoryKind.EXPENSE, CategoriesActions()) }

    // ── The editor ───────────────────────────────────────────────────────────

    private val dining = CategoryDraft(name = "Dining", icon = "dining", color = 6)

    private val editing = CategoryEditState(
        today = Fixtures.TODAY,
        period = Fixtures.PERIOD,
        kind = CategoryKind.EXPENSE,
        draft = dining,
        loaded = true,
        stored = true,
        entryCount = 61,
        periodTotal = 21_800,
        kindTotal = 238_176,
        nameProblem = categoryNameProblem(dining.name, CategoryKind.EXPENSE, 2, all.map { it.category }),
        targets = moveTargets(all, CategoryKind.EXPENSE, 2),
    )

    /** A new category typed with a name its kind already has: the clash reads at once. */
    private val clashing = CategoryDraft(name = "groceries", icon = "cart", color = 3)

    private val creating = CategoryEditState(
        today = Fixtures.TODAY,
        period = Fixtures.PERIOD,
        kind = CategoryKind.EXPENSE,
        draft = clashing,
        loaded = true,
        nameProblem = categoryNameProblem(clashing.name, CategoryKind.EXPENSE, 0, all.map { it.category }),
    )

    @Test fun categoryEdit() = compose.shoot("category-edit") { CategoryEditScreen(editing, CategoryEditActions()) }
    @Test fun categoryEdit200() = compose.shoot("category-edit-200", fontScale = 2f) { CategoryEditScreen(editing, CategoryEditActions()) }
    @Test fun categoryNew() = compose.shoot("category-new") { CategoryEditScreen(creating, CategoryEditActions()) }
}
