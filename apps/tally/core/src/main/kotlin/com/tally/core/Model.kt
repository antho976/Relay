package com.tally.core

import kotlinx.serialization.Serializable

/** Stored by NAME (Room and the backup both); renaming a constant is a migration. */
enum class TxType { EXPENSE, INCOME, TRANSFER }

/** [INVESTMENT] holds money put to work (a TFSA, an RRSP, a brokerage account); its value can be updated by hand. */
enum class AccountType { CASH, CHEQUING, SAVINGS, CREDIT, INVESTMENT }

enum class CategoryKind { EXPENSE, INCOME }

/**
 * What a goal measures. Stored by NAME.
 *
 * - [SAVINGS]: a pot filled by contributions, toward a target and an optional date.
 * - [BALANCE]: an account (or everything you own, net of what you owe) reaching an amount by a date.
 * - [INVEST]: each month, money moved into investment accounts: a share of what came in, or a set amount.
 * - [SAVE]: each month, what is kept of what came in: a share of it, or a set amount.
 */
enum class GoalKind { SAVINGS, BALANCE, INVEST, SAVE }

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
        // Added with the grouped picker. Appended, never reordered: the keys are stored.
        "pizza", "fastfood", "lunch", "ramen", "bakery", "icecream", "bar", "beer", "wine", "liquor", "tea", "kitchen",
        "train", "subway", "taxi", "bike", "scooter", "ev", "parking", "carrepair", "boat",
        "apartment", "water", "heat", "furniture", "repairs", "cleaning", "laundry", "garden", "insurance", "phone", "tv",
        "clothes", "computer", "headphones", "toys", "flowers", "jewelry", "store", "watch", "camera",
        "hospital", "medication", "pharmacy", "medical", "haircut", "wellness", "therapy", "eyes",
        "movie", "games", "music", "party", "outdoors", "soccer", "hiking", "pool", "casino", "theatre", "art", "hotel", "luggage", "beach",
        "baby", "family", "elderly", "charity", "church", "birthday",
        "invest", "crypto", "tax", "wallet", "loan", "legal", "business", "atm", "sale",
        "star", "idea", "online", "cloud", "delivery", "mail",
    )
}
