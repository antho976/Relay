package com.tally.app.ui.settings

import androidx.compose.runtime.Immutable
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.NoteUse
import com.tally.app.data.repo.ImportLine
import com.tally.core.AccountType
import com.tally.core.BankStatements
import com.tally.core.CategoryKind
import com.tally.core.Payees
import com.tally.core.Statement
import com.tally.core.StatementRow
import com.tally.core.TxType
import java.time.LocalDate

/*
 * The bank import's pure half: which bank, what each statement line becomes, which lines are
 * already logged, and the totals the preview reads. No Android, so a JVM test holds all of it.
 */

/** The banks the import page names, with where their CSV is, and what a new account from them is called. */
enum class BankSource(val key: String, val label: String, val steps: String, val accountName: String, val accountType: AccountType) {
    DESJARDINS(
        "desjardins",
        "Desjardins",
        "On desjardins.com, open the account in AccèsD, pick the period, then export it as CSV",
        "Desjardins chequing",
        AccountType.CHEQUING,
    ),
    WEALTHSIMPLE(
        "wealthsimple",
        "Wealthsimple",
        "On wealthsimple.com, open Documents, then Statements, and download a month as CSV",
        "Wealthsimple Chequing",
        AccountType.CHEQUING,
    ),
    OTHER(
        "other",
        "Another bank",
        "Most banks export an account's transactions as CSV from their website",
        "Imported account",
        AccountType.CHEQUING,
    ),
    ;

    companion object {
        fun of(key: String?): BankSource? = entries.firstOrNull { it.key == key }
    }
}

/** What happens to the lines that move money between the owner's own accounts (card payments, transfers). */
enum class TransferMode(val label: String) { SKIP("Skip"), TRANSFERS("Transfers"), ENTRIES("Entries") }

/** The choices the preview offers, as the owner set them. */
@Immutable
data class ImportChoices(
    /** The account in the file to take, when it holds several; null takes them all. */
    val fileAccount: String? = null,
    /** True when the statement writes money out as positive (most card statements). */
    val flip: Boolean = false,
    val transfers: TransferMode = TransferMode.SKIP,
    /** The account at the other end of the moves, for [TransferMode.TRANSFERS]. */
    val counterpartId: Long? = null,
)

/** One statement line as the import reads it, before the owner says import. */
@Immutable
data class PlannedLine(
    val date: LocalDate,
    val note: String,
    /** Signed from the account's side: below zero leaves it. */
    val amount: Long,
    val categoryId: Long?,
    val categoryName: String?,
    val categoryIcon: String?,
    val categoryColor: Int?,
    /** A card payment or a move between accounts, by its words. */
    val transfer: Boolean,
    /** Its twin (same day, same amount) is already in the account. */
    val duplicate: Boolean,
    /** The category came from the owner's own entries, not from the merchant list. */
    val learned: Boolean,
)

/** Everything the preview shows and the import writes. */
@Immutable
data class ImportPlan(
    val lines: List<PlannedLine> = emptyList(),
    val toWrite: List<ImportLine> = emptyList(),
    val duplicates: Int = 0,
    val transferLike: Int = 0,
    /** Transfer-like lines left out under [TransferMode.SKIP] (or with no account at the other end). */
    val skipped: Int = 0,
    /** Lines going in with a category, and how many of those the owner's own entries filed. */
    val categorized: Int = 0,
    val learned: Int = 0,
    val uncategorized: Int = 0,
    val out: Long = 0,
    val inn: Long = 0,
    val first: LocalDate? = null,
    val last: LocalDate? = null,
    /** What the account held before the statement's first line, by the statement's own balances. */
    val openingFromStatement: Long? = null,
    /** What the statement says the account held after its last line. */
    val endBalance: Long? = null,
) {
    val count: Int get() = toWrite.size
}

/** Most card statements write a purchase as a positive amount; past this share of positive lines a card reads flipped. */
internal const val CARD_POSITIVE_SHARE = 0.6f

/** Whether a statement into an account of [type] most likely writes money out as positive. */
internal fun guessFlip(statement: Statement, type: AccountType?): Boolean =
    type == AccountType.CREDIT && statement.positiveShare > CARD_POSITIVE_SHARE

/**
 * The statement's lines in the order they happened. A file that lists the newest first is turned
 * round, so the first line is the oldest and its balance tells what came before it.
 */
internal fun chronological(rows: List<StatementRow>): List<StatementRow> {
    if (rows.size < 2) return rows
    val newestFirst = rows.first().date.isAfter(rows.last().date)
    return if (newestFirst) rows.asReversed() else rows
}

/**
 * The category each note was filed under most often, by kind and payee key: what the import
 * learns from the owner's own ledger, so "METRO PLUS #12" goes where "Metro Plus" always went.
 */
internal fun learnedCategories(uses: List<NoteUse>, categories: List<CategoryEntity>): Map<String, Long> {
    val live = categories.filter { !it.archived }.associateBy { it.id }
    return uses
        .mapNotNull { u ->
            val c = live[u.categoryId] ?: return@mapNotNull null
            val kind = if (u.type == TxType.INCOME) CategoryKind.INCOME else CategoryKind.EXPENSE
            if (c.kind != kind) return@mapNotNull null
            val key = Payees.key(u.note)
            if (key.isEmpty()) null else ("${kind.name}:$key" to c.id)
        }
        .groupBy({ it.first }, { it.second })
        .mapValues { (_, ids) -> ids.groupingBy { it }.eachCount().maxByOrNull { it.value }!!.key }
}

/** Every line of [statement] as the import would write it, with the choices applied. */
internal fun planImport(
    statement: Statement,
    choices: ImportChoices,
    categories: List<CategoryEntity>,
    learned: Map<String, Long>,
    existing: List<Pair<LocalDate, Long>>,
): ImportPlan {
    val rows = chronological(statement.rows.filter { choices.fileAccount == null || it.account == choices.fileAccount })
    val sign = if (choices.flip) -1L else 1L
    val byId = categories.associateBy { it.id }
    val live = categories.filter { !it.archived }
    // Each logged entry can stand twin to one statement line only.
    val logged = existing.groupingBy { it }.eachCount().toMutableMap()
    val lines = rows.map { r ->
        val amount = r.amount * sign
        val note = Payees.clean(r.description).ifBlank { "Bank entry" }
        val transfer = Payees.looksLikeTransfer(r.description, r.code)
        val twin = date(r) to amount
        val duplicate = (logged[twin] ?: 0) > 0
        if (duplicate) logged[twin] = logged.getValue(twin) - 1
        val kind = if (amount < 0) CategoryKind.EXPENSE else CategoryKind.INCOME
        val fromLedger = learned["${kind.name}:${Payees.key(note)}"]?.takeIf { byId[it]?.kind == kind }
        val guessName = if (fromLedger == null) Payees.guessCategory(r.description, income = amount > 0, code = r.code) else null
        val guessed = guessName?.let { name -> live.firstOrNull { it.kind == kind && it.name.equals(name, ignoreCase = true) }?.id }
        val categoryId = if (transfer && choices.transfers != TransferMode.ENTRIES) null else fromLedger ?: guessed
        val c = categoryId?.let { byId[it] }
        PlannedLine(
            date = r.date,
            note = note,
            amount = amount,
            categoryId = categoryId,
            categoryName = c?.name,
            categoryIcon = c?.icon,
            categoryColor = c?.color,
            transfer = transfer,
            duplicate = duplicate,
            learned = fromLedger != null && categoryId == fromLedger,
        )
    }
    val writing = lines.filter { l ->
        !l.duplicate && when {
            !l.transfer -> true
            choices.transfers == TransferMode.ENTRIES -> true
            choices.transfers == TransferMode.TRANSFERS -> choices.counterpartId != null
            else -> false
        }
    }
    val toWrite = writing.map { l ->
        ImportLine(
            date = l.date,
            note = l.note,
            amount = l.amount,
            categoryId = l.categoryId,
            counterpartId = if (l.transfer && choices.transfers == TransferMode.TRANSFERS) choices.counterpartId else null,
        )
    }
    val entries = writing.filter { !(it.transfer && choices.transfers == TransferMode.TRANSFERS) }
    val firstWithBalance = rows.firstOrNull { it.balance != null }
    val lastWithBalance = rows.lastOrNull { it.balance != null }
    return ImportPlan(
        lines = lines,
        toWrite = toWrite,
        duplicates = lines.count { it.duplicate },
        transferLike = lines.count { it.transfer },
        skipped = lines.count { it.transfer && !it.duplicate } - writing.count { it.transfer },
        categorized = entries.count { it.categoryId != null },
        learned = entries.count { it.learned },
        uncategorized = entries.count { it.categoryId == null },
        out = entries.filter { it.amount < 0 }.sumOf { -it.amount },
        inn = entries.filter { it.amount > 0 }.sumOf { it.amount },
        first = rows.firstOrNull()?.date,
        last = rows.lastOrNull()?.date,
        openingFromStatement = firstWithBalance?.let { f -> sign * (f.balance!! - f.amount) },
        endBalance = lastWithBalance?.balance?.let { sign * it },
    )
}

private fun date(r: StatementRow): LocalDate = r.date

/** Whether a numeric date like 03/04 reads day first: French and most of the world do, English Canada and the US do not. */
internal fun dayFirstFor(language: String, country: String): Boolean =
    !(country == "US" || (language == "en" && country == "CA"))

/** The words under the hero: "1 Sep to 30 Sep · $2,140 out · $3,200 in". */
internal fun planSpan(plan: ImportPlan, short: (LocalDate) -> String, money: (Long) -> String): String = listOfNotNull(
    if (plan.first != null && plan.last != null) {
        if (plan.first == plan.last) short(plan.first) else short(plan.first) + " to " + short(plan.last)
    } else null,
    money(plan.out) + " out",
    money(plan.inn) + " in",
).joinToString(" · ")

/** The import's outcome, for the notice: "84 entries into Desjardins chequing · 12 already there". */
internal fun importedLine(count: Int, into: String, duplicates: Int): String =
    (if (count == 1) "1 entry" else "$count entries") + " imported into $into" +
        (if (duplicates > 0) " · $duplicates already there" else "")

/** True when [name] reads like an account of [source]'s ("Desjardins chequing" for Desjardins). */
internal fun belongsTo(name: String, source: BankSource?): Boolean =
    source != null && source != BankSource.OTHER && BankStatements.normalize(name).contains(source.key)
