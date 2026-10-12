package com.tally.app.ui.accounts

import androidx.compose.runtime.Immutable
import com.tally.app.data.db.AccountBalance
import com.tally.app.data.repo.AccountUse
import com.tally.app.data.repo.OtherAccountChange
import com.tally.core.AccountType
import com.tally.core.BankStatements
import com.tally.core.Copy
import com.tally.core.MoneyFormatter
import com.tally.core.Registration
import kotlinx.serialization.Serializable

/*
 * The pure half of Accounts: the list's readings, the editor's draft and its checks. No Android,
 * no Flow, so a plain JVM test holds every number the screens print.
 */

/** An account type as words. */
internal fun typeLabel(type: AccountType): String = when (type) {
    AccountType.CASH -> "Cash"
    AccountType.CHEQUING -> "Chequing"
    AccountType.SAVINGS -> "Savings"
    AccountType.CREDIT -> "Credit card"
    AccountType.INVESTMENT -> "Investment"
}

/** The one line under each type in the editor's choice. */
internal fun typeHint(type: AccountType): String = when (type) {
    AccountType.CASH -> "Notes and coins in your wallet"
    AccountType.CHEQUING -> "The everyday bank account"
    AccountType.SAVINGS -> "Money set aside, earning or waiting"
    AccountType.CREDIT -> "A card you pay off later"
    AccountType.INVESTMENT -> "A TFSA, an RRSP or a brokerage account, valued by its Wealthsimple files or by hand"
}

/** A ready-made account for the banks the owner uses: one tap fills the name, the type and, for an investment account, its kind. */
@Immutable
data class AccountTemplate(val name: String, val type: AccountType, val registration: Registration? = null)

/** Desjardins and Wealthsimple first, as the owner banks there; cash last. */
internal val ACCOUNT_TEMPLATES: List<AccountTemplate> = listOf(
    AccountTemplate("Desjardins chequing", AccountType.CHEQUING),
    AccountTemplate("Desjardins savings", AccountType.SAVINGS),
    AccountTemplate("Desjardins Visa", AccountType.CREDIT),
    AccountTemplate("Wealthsimple Chequing", AccountType.CHEQUING),
    AccountTemplate("Wealthsimple TFSA", AccountType.INVESTMENT, Registration.TFSA),
    AccountTemplate("Wealthsimple RRSP", AccountType.INVESTMENT, Registration.RRSP),
    AccountTemplate("Wealthsimple FHSA", AccountType.INVESTMENT, Registration.FHSA),
    AccountTemplate("Wealthsimple credit card", AccountType.CREDIT),
    AccountTemplate("Cash", AccountType.CASH),
)

/** The templates not yet taken: an account by that name already exists, in any case. */
internal fun freeTemplates(existing: List<String>): List<AccountTemplate> {
    val taken = existing.map { it.trim().lowercase() }.toSet()
    return ACCOUNT_TEMPLATES.filter { it.name.lowercase() !in taken }
}

/** One account row: its balance, and the share it is of its side (what you hold, or what you owe). */
@Immutable
data class AccountLine(
    val account: AccountBalance,
    /** 0..1 of the held total for a balance above zero, of the owed total below it. */
    val share: Float,
    val isDefault: Boolean,
)

/** Everything the Accounts list draws, summed once here rather than in composition. */
@Immutable
data class AccountsState(
    val active: List<AccountLine> = emptyList(),
    val archived: List<AccountLine> = emptyList(),
    /** What the active accounts add up to, cards owed counted against it. */
    val net: Long = 0,
    /** The active balances above zero, summed. */
    val held: Long = 0,
    /** The active balances below zero, as a positive amount. */
    val owed: Long = 0,
    /** How far [net] has moved from the active accounts' opening balances. */
    val sinceOpening: Long = 0,
    /** Every entry in the ledger (a transfer counts once, though it touches two accounts). */
    val entries: Int = 0,
    /** The active account holding the most. */
    val largest: AccountLine? = null,
    /** The active account with the most entries. */
    val busiest: AccountLine? = null,
    val defaultName: String? = null,
    val loaded: Boolean = false,
) {
    /** A transfer needs two places for the money to move between. */
    val canTransfer: Boolean get() = active.size >= 2
}

internal fun buildAccountsState(balances: List<AccountBalance>, defaultAccountId: Long, entries: Int): AccountsState {
    val (archived, active) = balances.partition { it.archived }
    val held = active.filter { it.balance > 0 }.sumOf { it.balance }
    val owed = active.filter { it.balance < 0 }.sumOf { -it.balance }
    val lines = active.map { a ->
        AccountLine(
            account = a,
            share = when {
                a.balance > 0 && held > 0 -> (a.balance.toDouble() / held).toFloat()
                a.balance < 0 && owed > 0 -> (-a.balance.toDouble() / owed).toFloat()
                else -> 0f
            },
            isDefault = a.id == defaultAccountId,
        )
    }
    val net = active.sumOf { it.balance }
    return AccountsState(
        active = lines,
        archived = archived.map { AccountLine(it, 0f, it.id == defaultAccountId) },
        net = net,
        held = held,
        owed = owed,
        sinceOpening = net - active.sumOf { it.openingBalance },
        entries = entries,
        largest = lines.filter { it.account.balance > 0 }.maxByOrNull { it.account.balance },
        busiest = lines.filter { it.account.entryCount > 0 }.maxByOrNull { it.account.entryCount },
        defaultName = lines.firstOrNull { it.isDefault }?.account?.name,
        loaded = true,
    )
}

/** What the share bar under a row says, as words. */
internal fun shareLine(line: AccountLine): String {
    val percent = Math.round(line.share * 100f)
    return when {
        line.account.balance > 0 -> "$percent% of what you hold"
        line.account.balance < 0 -> "$percent% of what you owe"
        else -> "Holds nothing"
    }
}

// ── The editor ───────────────────────────────────────────────────────────────

/**
 * The editor's fields as typed. [opening] is [openingText] read as an amount; null when it is not
 * one. Serializable so the editor can keep it in its SavedStateHandle across process death.
 */
@Immutable
@Serializable
data class AccountDraft(
    val name: String = "",
    val type: AccountType = AccountType.CHEQUING,
    val openingText: String = "",
    val opening: Long? = 0,
    /** On: the opening balance is money owed, stored below zero. */
    val owe: Boolean = false,
    val isDefault: Boolean = false,
    val archived: Boolean = false,
    /** An investment account's kind (TFSA, RRSP...); null for none, and on every other type. */
    val registration: Registration? = null,
) {
    /** The owe switch shows for a card, and for any account that opens below zero, so its sign can be undone. */
    val showsOwe: Boolean get() = type == AccountType.CREDIT || owe

    /** The opening balance as stored: a card you owe on opens below zero. */
    val signedOpening: Long? get() = opening?.let { if (owe) -it else it }

    /** An archived account is never the one new entries start in. */
    val effectiveDefault: Boolean get() = isDefault && !archived
}

@Immutable
data class AccountProblems(val name: String? = null, val opening: String? = null) {
    val any: Boolean get() = name != null || opening != null
}

internal fun accountProblems(d: AccountDraft): AccountProblems = AccountProblems(
    name = if (d.name.isBlank()) "Give the account a name" else null,
    opening = if (d.opening == null) "Enter an amount, like 250 or 1,250.50" else null,
)

/**
 * The institution an investment account is kept under: the stored one, else Wealthsimple when the
 * name says so ("Wealthsimple TFSA"), else none.
 */
internal fun institutionFor(name: String, stored: String): String = when {
    stored.isNotBlank() -> stored
    "wealthsimple" in BankStatements.normalize(name).split(" ") -> "Wealthsimple"
    else -> ""
}

/** A typed opening balance: blank reads as zero, anything else must parse. */
internal fun readOpening(text: String, parsed: Long?): Long? = if (text.isBlank()) 0L else parsed

/** The editor's share bar: this account against the other active ones on its side. */
@Immutable
data class SharePreview(val share: Float = 0f, val total: Long = 0)

internal fun previewShare(id: Long, balance: Long, archived: Boolean, all: List<AccountBalance>): SharePreview {
    if (archived || balance == 0L) return SharePreview()
    val others = all.filter { it.id != id && !it.archived }
    return if (balance > 0) {
        val total = balance + others.filter { it.balance > 0 }.sumOf { it.balance }
        SharePreview((balance.toDouble() / total).toFloat(), total)
    } else {
        val total = -balance + others.filter { it.balance < 0 }.sumOf { -it.balance }
        SharePreview((-balance.toDouble() / total).toFloat(), total)
    }
}

/** What goes with a deleted account, as one phrase ("24 entries and 5 bills"); null when nothing does. */
internal fun goesWith(use: AccountUse): String? {
    val parts = listOfNotNull(
        if (use.entries > 0) Copy.plural(use.entries, "entry", "entries") else null,
        if (use.bills > 0) Copy.plural(use.bills, "bill") else null,
    )
    return if (parts.isEmpty()) null else parts.joinToString(" and ")
}

/** An account that stays, and how far its balance moves once the transfers with the deleted one are gone. */
internal fun otherAccountLine(other: OtherAccountChange, money: MoneyFormatter): String {
    val transfers = Copy.plural(other.transfers, "transfer")
    val go = if (other.transfers == 1) "goes" else "go"
    return when {
        other.change > 0 -> "${other.name} goes up by ${money.format(other.change)}, since $transfers between them $go too."
        other.change < 0 -> "${other.name} goes down by ${money.format(-other.change)}, since $transfers between them $go too."
        else -> "$transfers with ${other.name} $go too, which leaves its balance where it is."
    }
}

/**
 * The delete dialog's text, with the real counts, so the consequence is the number: the entries
 * and bills that go, every account that stays but reads differently without its transfers with
 * this one, and, when there is anything to keep, that archiving keeps it.
 */
internal fun deleteAccountLine(name: String, use: AccountUse, money: MoneyFormatter, canArchive: Boolean): String {
    val what = goesWith(use)
    val go = if (use.entries + use.bills == 1) "goes" else "go"
    val sentences = buildList {
        add(if (what == null) "Delete $name? It has no entries and no bills." else "Delete $name? Its $what $go with it.")
        use.others.forEach { add(otherAccountLine(it, money)) }
        add("This cannot be undone.")
        if (canArchive) add("Archiving keeps all of it and takes the account out of the pickers.")
    }
    return sentences.joinToString(" ")
}

/** The snackbar after the delete. */
internal fun accountDeletedLine(name: String, use: AccountUse): String =
    goesWith(use)?.let { "$name deleted with its $it" } ?: "$name deleted"
