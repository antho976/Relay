package com.tally.app.ui.categories

import androidx.compose.runtime.Immutable
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.CategoryTotal
import com.tally.app.data.db.CategoryWithCount
import com.tally.app.data.repo.CategoryUse
import com.tally.core.BudgetPeriod
import com.tally.core.CategoryKind
import com.tally.core.Copy
import com.tally.core.Defaults
import com.tally.core.TxType
import java.time.LocalDate

/*
 * The pure half of Categories: the list's readings per kind, the editor's name check, the colour
 * a new category starts with and the lines a delete leaves behind. No Android, no Flow.
 */

/** A kind as the lens names it. */
internal fun kindLabel(kind: CategoryKind): String = if (kind == CategoryKind.INCOME) "Income" else "Spending"

/** The entries a category of [kind] files. */
internal fun CategoryKind.txType(): TxType = if (this == CategoryKind.INCOME) TxType.INCOME else TxType.EXPENSE

/** One category row: what it is, how many entries it holds, and this period's money in it. */
@Immutable
data class CategoryLine(
    val id: Long,
    val name: String,
    val kind: CategoryKind,
    val icon: String,
    val color: Int,
    val entries: Int,
    val periodTotal: Long,
    /** Its share of the kind's period total, rounded to a whole percent. */
    val percent: Int,
    val archived: Boolean,
)

/** One slice of the period's money by category. [color] null is the uncategorized remainder. */
@Immutable
data class ShareSegment(val name: String, val color: Int?, val total: Long, val share: Float, val percent: Int)

/** Everything one lens (spending or income) draws, summed here rather than in composition. */
@Immutable
data class KindSummary(
    val active: List<CategoryLine> = emptyList(),
    val archived: List<CategoryLine> = emptyList(),
    /** Every entry ever filed under this kind's categories. */
    val entries: Int = 0,
    /** This period's total for the kind, uncategorized entries included. */
    val periodTotal: Long = 0,
    /** This period's entries for the kind, uncategorized ones included. */
    val periodEntries: Int = 0,
    /** This period by category, largest first. */
    val segments: List<ShareSegment> = emptyList(),
    /** Active categories with money in them this period. */
    val usedThisPeriod: Int = 0,
    /** The active category holding the most entries. */
    val mostUsed: CategoryLine? = null,
    /** Active categories that have never held an entry. */
    val unused: Int = 0,
) {
    /** The period's typical entry for the kind, or 0 with none. */
    val averageEntry: Long get() = if (periodEntries > 0) periodTotal / periodEntries else 0L
}

@Immutable
data class CategoriesState(
    val today: LocalDate,
    val period: BudgetPeriod,
    val spending: KindSummary = KindSummary(),
    val income: KindSummary = KindSummary(),
    val loaded: Boolean = false,
) {
    fun of(kind: CategoryKind): KindSummary = if (kind == CategoryKind.INCOME) income else spending
}

internal fun summarize(kind: CategoryKind, categories: List<CategoryWithCount>, totals: List<CategoryTotal>): KindSummary {
    val periodTotal = totals.sumOf { it.total }
    val byId: Map<Long, Long> = totals
        .mapNotNull { t -> t.categoryId?.let { id -> id to t.total } }
        .groupBy({ it.first }, { it.second })
        .mapValues { (_, amounts) -> amounts.sum() }
    val lines = categories.filter { it.category.kind == kind }.map { c ->
        val total = byId[c.category.id] ?: 0L
        CategoryLine(
            id = c.category.id,
            name = c.category.name,
            kind = kind,
            icon = c.category.icon,
            color = c.category.color,
            entries = c.entryCount,
            periodTotal = total,
            percent = if (periodTotal > 0) Math.round(total * 100.0 / periodTotal).toInt() else 0,
            archived = c.category.archived,
        )
    }
    val (archived, active) = lines.partition { it.archived }
    val filed = lines.sumOf { it.periodTotal }
    val remainder = periodTotal - filed
    fun segment(name: String, color: Int?, total: Long): ShareSegment {
        val share = if (periodTotal > 0) (total.toDouble() / periodTotal).toFloat() else 0f
        return ShareSegment(name, color, total, share, Math.round(share * 100f))
    }
    val segments = buildList {
        lines.filter { it.periodTotal > 0 }.sortedByDescending { it.periodTotal }.forEach { add(segment(it.name, it.color, it.periodTotal)) }
        if (remainder > 0) add(segment("Uncategorized", null, remainder))
    }
    return KindSummary(
        active = active,
        archived = archived,
        entries = lines.sumOf { it.entries },
        periodTotal = periodTotal,
        periodEntries = totals.sumOf { it.count },
        segments = segments,
        usedThisPeriod = active.count { it.periodTotal > 0 },
        mostUsed = active.filter { it.entries > 0 }.maxByOrNull { it.entries },
        unused = active.count { it.entries == 0 },
    )
}

// ── The editor ───────────────────────────────────────────────────────────────

internal const val MISSING_NAME = "Give the category a name"

/**
 * Why [name] cannot be saved as a [kind] category, or null when it can. Names are unique within a
 * kind, compared without case, archived categories included (they come back when unarchived).
 */
internal fun categoryNameProblem(name: String, kind: CategoryKind, selfId: Long, all: List<CategoryEntity>): String? {
    val clean = name.trim()
    if (clean.isEmpty()) return MISSING_NAME
    val clash = all.firstOrNull { it.kind == kind && it.id != selfId && it.name.trim().equals(clean, ignoreCase = true) }
        ?: return null
    val what = kindLabel(kind).lowercase()
    return if (clash.archived) "An archived $what category is already called ${clash.name}"
    else "You already have a $what category called ${clash.name}"
}

/** A new category's first hue: the first one its kind does not use yet, so it reads apart in a chart. */
internal fun suggestColor(used: List<Int>): Int {
    val taken = used.toSet()
    return (0 until Defaults.PALETTE_SIZE).firstOrNull { it !in taken } ?: (used.size % Defaults.PALETTE_SIZE)
}

/** Where a deleted category's entries can go: the other active categories of its kind. */
@Immutable
data class MoveTarget(val id: Long, val name: String, val icon: String, val color: Int, val entries: Int)

internal fun moveTargets(all: List<CategoryWithCount>, kind: CategoryKind, selfId: Long): List<MoveTarget> =
    all.filter { it.category.kind == kind && it.category.id != selfId && !it.category.archived }
        .map { MoveTarget(it.category.id, it.category.name, it.category.icon, it.category.color, it.entryCount) }

/** What a deleted category's move covers, as one phrase ("3 entries and 1 bill"); null when nothing moves. */
internal fun movingPhrase(use: CategoryUse): String? {
    val parts = listOfNotNull(
        if (use.entries > 0) Copy.plural(use.entries, "entry", "entries") else null,
        if (use.bills > 0) Copy.plural(use.bills, "bill") else null,
    )
    return if (parts.isEmpty()) null else parts.joinToString(" and ")
}

/** The move sheet's heading: "Move its 3 entries and 1 bill to". */
internal fun moveHeading(use: CategoryUse): String = "Move its ${movingPhrase(use).orEmpty()} to"

/** The move sheet's line under the heading: what goes for good, the budget included by its amount. */
internal fun moveNote(name: String, budget: String?): String =
    if (budget != null) "$name is deleted, with its $budget budget. This cannot be undone."
    else "$name is deleted. This cannot be undone."

/** The sheet's last row, the choice to move nothing: what then happens to the entries and the bills. */
internal fun leaveTitle(use: CategoryUse): String =
    if (use.entries + use.bills == 1) "Leave it uncategorized" else "Leave them uncategorized"

internal fun leaveSubtitle(use: CategoryUse): String = when {
    use.entries > 0 && use.bills > 0 -> "Entries stay in the ledger and bills keep posting, with no category"
    use.bills > 0 -> if (use.bills == 1) "It keeps posting, with no category" else "They keep posting, with no category"
    else -> "They stay in the ledger with no category"
}

/**
 * The one line the snackbar says after a delete: the budget that went with it by its amount, and
 * where the entries and bills went.
 */
internal fun deletedLine(name: String, use: CategoryUse, budget: String?, movedTo: String?): String {
    val head = if (budget != null) "$name deleted with its $budget budget" else "$name deleted"
    val moved = movingPhrase(use) ?: return head
    return if (movedTo != null) "$head · $moved moved to $movedTo" else "$head · $moved left uncategorized"
}

/** The spoken name of each icon key, for the picker's tiles. */
internal fun iconName(key: String): String = ICON_NAMES[key] ?: key.replaceFirstChar { it.uppercase() }

/** Every icon key's spoken name; a test holds it to [com.tally.app.ui.common.CategoryIcons.all]. */
internal val ICON_NAMES = mapOf(
    "cart" to "Shopping cart",
    "dining" to "Knife and fork",
    "coffee" to "Coffee cup",
    "transport" to "Bus",
    "car" to "Car",
    "fuel" to "Fuel pump",
    "home" to "House",
    "bolt" to "Lightning bolt",
    "wifi" to "Wi-Fi",
    "bag" to "Shopping bag",
    "health" to "Heart",
    "ticket" to "Ticket",
    "repeat" to "Repeat arrows",
    "flight" to "Plane",
    "gift" to "Gift",
    "dots" to "Three dots",
    "work" to "Briefcase",
    "spark" to "Sparkle",
    "refund" to "Return arrow",
    "pets" to "Paw",
    "school" to "Graduation cap",
    "child" to "Child",
    "sport" to "Dumbbell",
    "beauty" to "Lotus",
    "savings" to "Piggy bank",
    "bank" to "Bank",
    "card" to "Card",
    "cash" to "Banknotes",
    "fees" to "Receipt",
    "book" to "Book",
    "pizza" to "Pizza slice",
    "fastfood" to "Burger and drink",
    "lunch" to "Lunch plate",
    "ramen" to "Noodle bowl",
    "bakery" to "Croissant",
    "icecream" to "Ice cream",
    "bar" to "Cocktail",
    "beer" to "Beer",
    "wine" to "Wine glass",
    "liquor" to "Bottle",
    "tea" to "Tea cup",
    "kitchen" to "Fridge",
    "train" to "Train",
    "subway" to "Subway",
    "taxi" to "Taxi",
    "bike" to "Bike",
    "scooter" to "Scooter",
    "ev" to "Electric car",
    "parking" to "Parking",
    "carrepair" to "Car repair",
    "boat" to "Boat",
    "apartment" to "Apartment",
    "water" to "Water drop",
    "heat" to "Flame",
    "furniture" to "Chair",
    "repairs" to "Wrench",
    "cleaning" to "Cleaning",
    "laundry" to "Washing machine",
    "garden" to "Garden",
    "insurance" to "Shield",
    "phone" to "Phone",
    "tv" to "Television",
    "clothes" to "Hanger",
    "computer" to "Computer",
    "headphones" to "Headphones",
    "toys" to "Toys",
    "flowers" to "Flower",
    "jewelry" to "Diamond",
    "store" to "Storefront",
    "watch" to "Watch",
    "camera" to "Camera",
    "hospital" to "Hospital",
    "medication" to "Medication",
    "pharmacy" to "Pharmacy",
    "medical" to "Medical bag",
    "haircut" to "Scissors",
    "wellness" to "Meditation",
    "therapy" to "Mind",
    "eyes" to "Eye",
    "movie" to "Film",
    "games" to "Game controller",
    "music" to "Music note",
    "party" to "Party",
    "outdoors" to "Tree",
    "soccer" to "Ball",
    "hiking" to "Hiker",
    "pool" to "Swimmer",
    "casino" to "Dice",
    "theatre" to "Theatre masks",
    "art" to "Palette",
    "hotel" to "Bed",
    "luggage" to "Suitcase",
    "beach" to "Umbrella",
    "baby" to "Stroller",
    "family" to "Family",
    "elderly" to "Walking cane",
    "charity" to "Giving hand",
    "church" to "Church",
    "birthday" to "Cake",
    "invest" to "Rising line",
    "crypto" to "Bitcoin",
    "tax" to "Tax form",
    "wallet" to "Wallet",
    "loan" to "Handshake",
    "legal" to "Gavel",
    "business" to "Office building",
    "atm" to "Cash machine",
    "sale" to "Price tag",
    "star" to "Star",
    "idea" to "Light bulb",
    "online" to "Globe",
    "cloud" to "Cloud",
    "delivery" to "Delivery truck",
    "mail" to "Envelope",
)

/** The twelve hues by name, in palette order, for the swatches' spoken labels. */
internal val HUE_NAMES = listOf(
    "Sage", "Teal", "Sky", "Iris", "Orchid", "Rose", "Coral", "Amber", "Olive", "Sand", "Slate", "Brick",
)

internal fun hueName(index: Int): String = HUE_NAMES.getOrElse(index) { "Colour ${index + 1}" }
