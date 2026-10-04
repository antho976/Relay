package com.margin.app.ui.accounts

import androidx.compose.runtime.Immutable
import com.margin.app.data.db.AccountBalance
import com.margin.core.AccountType
import com.margin.core.Copy

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
}

/** The one line under each type in the editor's choice. */
internal fun typeHint(type: AccountType): String = when (type) {
    AccountType.CASH -> "Notes and coins in your wallet"
    AccountType.CHEQUING -> "The everyday bank account"
    AccountType.SAVINGS -> "Money set aside, earning or waiting"
    AccountType.CREDIT -> "A card you pay off later"
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

/** The editor's fields as typed. [opening] is [openingText] read as an amount; null when it is not one. */
@Immutable
data class AccountDraft(
    val name: String = "",
    val type: AccountType = AccountType.CHEQUING,
    val openingText: String = "",
    val opening: Long? = 0,
    /** On: the opening balance is money owed, stored below zero. */
    val owe: Boolean = false,
    val isDefault: Boolean = false,
    val archived: Boolean = false,
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

/** The delete dialog's sentence, with the real count, so the consequence is the number. */
internal fun deleteAccountLine(name: String, entries: Int): String =
    if (entries == 0) "Delete $name? It has no entries. This cannot be undone."
    else "Delete $name and its ${Copy.plural(entries, "entry", "entries")}? This cannot be undone."
