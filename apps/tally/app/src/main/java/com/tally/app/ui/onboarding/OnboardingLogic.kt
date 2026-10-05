package com.tally.app.ui.onboarding

import androidx.compose.runtime.Immutable
import com.tally.core.AccountType
import com.tally.core.BudgetPeriod
import kotlinx.serialization.Serializable
import kotlinx.serialization.Transient
import java.time.LocalDate

/*
 * First run asks only what the plan needs: the currency, the accounts, the month's budget. The
 * pure rules live here so a plain JVM test holds them.
 */

enum class OnboardingStep(val question: String, val caption: String) {
    CURRENCY("Which currency do you spend in?", "Every amount in the app reads in it. You can change it later."),
    ACCOUNT("Where does your money sit?", "Add each account you spend from. More can come later in Accounts."),
    BUDGET("How much can you spend in a month?", "One number for the whole month. Budgets per category can come later."),
}

/**
 * One account added on the account step, kept as typed until the end. [name] is the name it will
 * be saved under; [balance] is read again from [balanceText] whenever the currency changes, so it
 * is not kept across a process death.
 */
@Immutable
@Serializable
data class OnboardAccount(
    val name: String,
    val type: AccountType,
    val balanceText: String = "",
    @Transient val balance: Long = 0L,
) {
    /** As it will be stored: what a card owes is negative. */
    val storedBalance: Long get() = openingBalance(type, balance)
}

/** The draft the three steps fill in. Nothing is written as an account or budget until the end. */
@Immutable
data class OnboardingState(
    val today: LocalDate,
    val period: BudgetPeriod = BudgetPeriod.containing(today),
    val step: OnboardingStep = OnboardingStep.CURRENCY,
    val currency: String = "CAD",
    /** The phone's own currency, offered first. */
    val deviceCurrency: String = "CAD",
    val moreCurrencies: Boolean = false,
    /** The accounts added so far; the form below them is the next one. */
    val accounts: List<OnboardAccount> = emptyList(),
    /** Whether the form for another account is open. It always is while no account is added. */
    val editing: Boolean = true,
    val accountName: String = defaultAccountName(AccountType.CHEQUING),
    val accountType: AccountType = AccountType.CHEQUING,
    val balanceText: String = "",
    /** The typed balance as minor units; null when the text is not an amount. */
    val balance: Long? = null,
    val budgetText: String = "",
    val budget: Long? = null,
    val busy: Boolean = false,
    /** A failed finish or sample load, said once over the action. */
    val error: String? = null,
    val loaded: Boolean = false,
) {
    val balanceProblem: String? get() = if (formOpen) amountProblem(balanceText, balance) else null

    /** The account form shows: always for the first account, then only when asked for. */
    val formOpen: Boolean get() = editing || accounts.isEmpty()

    /** The account the form holds, as it would be added. */
    val draftAccount: OnboardAccount
        get() = OnboardAccount(accountNameToSave(accountName, accountType), accountType, balanceText, balance ?: 0L)

    /** Every account the finish writes: those added, and the open form's. */
    val toCreate: List<OnboardAccount> get() = if (formOpen) accounts + draftAccount else accounts

    /** What the accounts to create add up to: held less owed. */
    val net: Long get() = toCreate.sumOf { it.storedBalance }
    val budgetProblem: String? get() = amountProblem(budgetText, budget)

    /** Whether the step's main act can run: an amount field holding something that is not an amount blocks it. */
    val canContinue: Boolean
        get() = loaded && !busy && when (step) {
            OnboardingStep.CURRENCY -> true
            OnboardingStep.ACCOUNT -> balanceProblem == null
            OnboardingStep.BUDGET -> budgetProblem == null
        }

    /** The balance as it will be stored: what a card owes is negative, so its spending reads as debt. */
    val storedBalance: Long get() = openingBalance(accountType, balance ?: 0L)
}

/** The type a new form starts on: the next of the usual set not added yet (a card after the chequing account). */
internal fun suggestedType(added: List<OnboardAccount>): AccountType =
    listOf(AccountType.CHEQUING, AccountType.CREDIT, AccountType.SAVINGS, AccountType.CASH, AccountType.INVESTMENT)
        .firstOrNull { t -> added.none { it.type == t } } ?: AccountType.CHEQUING

/** "Chequing, Credit card and Savings", or "Chequing, Visa and 3 more" past three. */
internal fun accountNames(accounts: List<OnboardAccount>): String {
    val names = accounts.map { it.name }
    return when {
        names.isEmpty() -> ""
        names.size == 1 -> names[0]
        names.size <= 3 -> names.dropLast(1).joinToString(", ") + " and " + names.last()
        else -> names.take(2).joinToString(", ") + " and ${names.size - 2} more"
    }
}

/** Each type's own name, used as the default account name and as the tile's label. */
internal fun defaultAccountName(type: AccountType): String = when (type) {
    AccountType.CASH -> "Cash"
    AccountType.CHEQUING -> "Chequing"
    AccountType.SAVINGS -> "Savings"
    AccountType.CREDIT -> "Credit card"
    AccountType.INVESTMENT -> "Investments"
}

/** Follows the type with the name while the owner has not typed one of their own. */
internal fun nameForType(current: String, from: AccountType, to: AccountType): String =
    if (current.isBlank() || current.trim() == defaultAccountName(from)) defaultAccountName(to) else current

internal fun openingBalance(type: AccountType, typed: Long): Long = if (type == AccountType.CREDIT) -typed else typed

/** The name that will be saved: what was typed, or the type's own name when the field was cleared. */
internal fun accountNameToSave(typed: String, type: AccountType): String = typed.trim().ifEmpty { defaultAccountName(type) }

/** An amount field's one problem: text that is there but is not an amount. Empty is fine (zero). */
internal fun amountProblem(text: String, parsed: Long?): String? =
    if (text.isNotBlank() && parsed == null) "That is not an amount. Type digits, like 1250.50" else null

/**
 * What the owner has filled in so far, as the flow keeps it in its SavedStateHandle so a process
 * death does not send them back to the first question. The currency is not here: it is written
 * to settings as it is picked. Amounts are kept as typed and read again at that currency.
 */
@Serializable
internal data class SavedOnboarding(
    val step: OnboardingStep = OnboardingStep.CURRENCY,
    val moreCurrencies: Boolean = false,
    val accounts: List<OnboardAccount> = emptyList(),
    val editing: Boolean = true,
    val accountName: String = defaultAccountName(AccountType.CHEQUING),
    val accountType: AccountType = AccountType.CHEQUING,
    val balanceText: String = "",
    val budgetText: String = "",
) {
    /** [into] with these answers; [parse] reads a typed amount in the current currency. */
    fun applyTo(into: OnboardingState, parse: (String) -> Long?): OnboardingState = into.copy(
        step = step,
        moreCurrencies = moreCurrencies,
        accounts = accounts.map { it.copy(balance = parse(it.balanceText) ?: 0L) },
        editing = editing,
        accountName = accountName,
        accountType = accountType,
        balanceText = balanceText,
        balance = parse(balanceText),
        budgetText = budgetText,
        budget = parse(budgetText),
    )

    companion object {
        fun of(s: OnboardingState): SavedOnboarding =
            SavedOnboarding(s.step, s.moreCurrencies, s.accounts, s.editing, s.accountName, s.accountType, s.balanceText, s.budgetText)
    }
}

internal fun nextStep(step: OnboardingStep): OnboardingStep? = OnboardingStep.entries.getOrNull(step.ordinal + 1)

internal fun previousStep(step: OnboardingStep): OnboardingStep? = OnboardingStep.entries.getOrNull(step.ordinal - 1)
