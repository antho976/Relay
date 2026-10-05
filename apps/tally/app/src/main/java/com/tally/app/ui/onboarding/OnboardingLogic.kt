package com.tally.app.ui.onboarding

import androidx.compose.runtime.Immutable
import com.tally.core.AccountType
import com.tally.core.BudgetPeriod
import kotlinx.serialization.Serializable
import java.time.LocalDate

/*
 * First run asks only what the plan needs: the currency, the first account, the month's budget.
 * The pure rules live here so a plain JVM test holds them.
 */

enum class OnboardingStep(val question: String, val caption: String) {
    CURRENCY("Which currency do you spend in?", "Every amount in the app reads in it. You can change it later."),
    ACCOUNT("Where does your money sit?", "The account most of your spending comes from. Add the rest later."),
    BUDGET("How much can you spend in a month?", "One number for the whole month. Budgets per category can come later."),
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
    val balanceProblem: String? get() = amountProblem(balanceText, balance)
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
    val accountName: String = defaultAccountName(AccountType.CHEQUING),
    val accountType: AccountType = AccountType.CHEQUING,
    val balanceText: String = "",
    val budgetText: String = "",
) {
    /** [into] with these answers; [parse] reads a typed amount in the current currency. */
    fun applyTo(into: OnboardingState, parse: (String) -> Long?): OnboardingState = into.copy(
        step = step,
        moreCurrencies = moreCurrencies,
        accountName = accountName,
        accountType = accountType,
        balanceText = balanceText,
        balance = parse(balanceText),
        budgetText = budgetText,
        budget = parse(budgetText),
    )

    companion object {
        fun of(s: OnboardingState): SavedOnboarding =
            SavedOnboarding(s.step, s.moreCurrencies, s.accountName, s.accountType, s.balanceText, s.budgetText)
    }
}

internal fun nextStep(step: OnboardingStep): OnboardingStep? = OnboardingStep.entries.getOrNull(step.ordinal + 1)

internal fun previousStep(step: OnboardingStep): OnboardingStep? = OnboardingStep.entries.getOrNull(step.ordinal - 1)
