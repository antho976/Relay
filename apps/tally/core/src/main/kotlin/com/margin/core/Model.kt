package com.margin.core

import kotlinx.serialization.Serializable

/** Stored by NAME (Room and the backup both); renaming a constant is a migration. */
enum class TxType { EXPENSE, INCOME, TRANSFER }

enum class AccountType { CASH, CHEQUING, SAVINGS, CREDIT }

enum class CategoryKind { EXPENSE, INCOME }

/** A category the app seeds on first run. [icon] is a key the UI maps to a drawn glyph. */
@Serializable
data class CategorySeed(val name: String, val kind: CategoryKind, val color: Int, val icon: String)

object Defaults {

    /** Twelve hues tuned for the warm near-black ground; [CategorySeed.color] indexes into them. */
    const val PALETTE_SIZE = 12

    val categories: List<CategorySeed> = listOf(
        CategorySeed("Groceries", CategoryKind.EXPENSE, 0, "cart"),
        CategorySeed("Dining", CategoryKind.EXPENSE, 6, "dining"),
        CategorySeed("Transport", CategoryKind.EXPENSE, 2, "transport"),
        CategorySeed("Housing", CategoryKind.EXPENSE, 9, "home"),
        CategorySeed("Utilities", CategoryKind.EXPENSE, 7, "bolt"),
        CategorySeed("Phone & internet", CategoryKind.EXPENSE, 1, "wifi"),
        CategorySeed("Shopping", CategoryKind.EXPENSE, 4, "bag"),
        CategorySeed("Health", CategoryKind.EXPENSE, 5, "health"),
        CategorySeed("Entertainment", CategoryKind.EXPENSE, 3, "ticket"),
        CategorySeed("Subscriptions", CategoryKind.EXPENSE, 10, "repeat"),
        CategorySeed("Travel", CategoryKind.EXPENSE, 8, "flight"),
        CategorySeed("Gifts", CategoryKind.EXPENSE, 11, "gift"),
        CategorySeed("Other", CategoryKind.EXPENSE, 10, "dots"),
        CategorySeed("Salary", CategoryKind.INCOME, 0, "work"),
        CategorySeed("Side income", CategoryKind.INCOME, 1, "spark"),
        CategorySeed("Refunds", CategoryKind.INCOME, 2, "refund"),
        CategorySeed("Other income", CategoryKind.INCOME, 10, "dots"),
    )

    /** Every icon key the UI must be able to draw. A test checks the app's map covers it. */
    val iconKeys: List<String> = listOf(
        "cart", "dining", "coffee", "transport", "car", "fuel", "home", "bolt", "wifi", "bag",
        "health", "ticket", "repeat", "flight", "gift", "dots", "work", "spark", "refund", "pets",
        "school", "child", "sport", "beauty", "savings", "bank", "card", "cash", "fees", "book",
    )
}
