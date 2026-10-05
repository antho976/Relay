package com.tally.app.ui.onboarding

import com.tally.core.AccountType
import com.tally.core.Copy
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate

class OnboardingLogicTest {

    private val today = LocalDate.of(2026, 10, 14)
    private val ready = OnboardingState(today = today, loaded = true)

    @Test fun eachTypeHasItsOwnName() {
        assertEquals("Cash", defaultAccountName(AccountType.CASH))
        assertEquals("Chequing", defaultAccountName(AccountType.CHEQUING))
        assertEquals("Savings", defaultAccountName(AccountType.SAVINGS))
        assertEquals("Credit card", defaultAccountName(AccountType.CREDIT))
        assertEquals("Chequing", ready.accountName)
    }

    @Test fun theNameFollowsTheTypeUntilTyped() {
        assertEquals("Savings", nameForType("Chequing", AccountType.CHEQUING, AccountType.SAVINGS))
        assertEquals("Credit card", nameForType("  Chequing ", AccountType.CHEQUING, AccountType.CREDIT))
        assertEquals("Cash", nameForType("", AccountType.CHEQUING, AccountType.CASH))
        assertEquals("Joint account", nameForType("Joint account", AccountType.CHEQUING, AccountType.SAVINGS))
    }

    @Test fun aCardStartsInDebt() {
        assertEquals(-41_220L, openingBalance(AccountType.CREDIT, 41_220))
        assertEquals(318_450L, openingBalance(AccountType.CHEQUING, 318_450))
        assertEquals(0L, openingBalance(AccountType.CREDIT, 0))
        assertEquals(-500L, ready.copy(accountType = AccountType.CREDIT, balance = 500).storedBalance)
        assertEquals(0L, ready.storedBalance)
    }

    @Test fun aClearedNameSavesAsTheType() {
        assertEquals("Cash", accountNameToSave("   ", AccountType.CASH))
        assertEquals("Joint", accountNameToSave("  Joint ", AccountType.CHEQUING))
    }

    @Test fun onlyTextThatIsNotAnAmountIsAProblem() {
        assertNull(amountProblem("", null))
        assertNull(amountProblem("  ", null))
        assertNull(amountProblem("12.50", 1_250))
        assertNotNull(amountProblem("about a grand", null))
    }

    @Test fun stepsWalkInOrder() {
        assertEquals(OnboardingStep.ACCOUNT, nextStep(OnboardingStep.CURRENCY))
        assertEquals(OnboardingStep.BUDGET, nextStep(OnboardingStep.ACCOUNT))
        assertNull(nextStep(OnboardingStep.BUDGET))
        assertNull(previousStep(OnboardingStep.CURRENCY))
        assertEquals(OnboardingStep.CURRENCY, previousStep(OnboardingStep.ACCOUNT))
    }

    @Test fun continueWaitsForAReadableAmount() {
        assertFalse(OnboardingState(today = today).canContinue)
        assertTrue(ready.canContinue)
        val account = ready.copy(step = OnboardingStep.ACCOUNT)
        assertTrue(account.canContinue)
        assertFalse(account.copy(balanceText = "abc", balance = null).canContinue)
        assertTrue(account.copy(balanceText = "100", balance = 10_000).canContinue)
        assertFalse(ready.copy(step = OnboardingStep.BUDGET, budgetText = "x", budget = null).canContinue)
        assertFalse(ready.copy(busy = true).canContinue)
    }

    @Test fun copyKeepsTheVoice() {
        val lines = OnboardingStep.entries.flatMap { listOf(it.question, it.caption) } +
            listOfNotNull(amountProblem("x", null))
        lines.forEach { line -> Copy.banned.forEach { bad -> assertFalse("\"$line\" contains \"$bad\"", line.contains(bad, ignoreCase = true)) } }
    }

    // ── Several accounts ─────────────────────────────────────────────────────

    private val chequing = OnboardAccount("Chequing", AccountType.CHEQUING, "3184.50", 318_450)
    private val visa = OnboardAccount("Visa", AccountType.CREDIT, "412.20", 41_220)

    @Test fun theNextFormSuggestsTheNextUsualType() {
        assertEquals(AccountType.CHEQUING, suggestedType(emptyList()))
        assertEquals(AccountType.CREDIT, suggestedType(listOf(chequing)))
        assertEquals(AccountType.SAVINGS, suggestedType(listOf(chequing, visa)))
        assertEquals(AccountType.CHEQUING, suggestedType(AccountType.entries.map { OnboardAccount(it.name, it) }))
    }

    @Test fun theOpenFormIsOneMoreAccountToCreate() {
        val step = ready.copy(step = OnboardingStep.ACCOUNT, accounts = listOf(chequing, visa))
        // A form closed after adding: only what was added.
        val closed = step.copy(editing = false)
        assertFalse(closed.formOpen)
        assertEquals(listOf("Chequing", "Visa"), closed.toCreate.map { it.name })
        assertEquals(318_450L - 41_220L, closed.net)
        assertTrue("Nothing to read in a closed form", closed.copy(balanceText = "abc", balance = null).canContinue)
        // An open form adds its own account, named for its type when the name was cleared.
        val open = step.copy(editing = true, accountType = AccountType.SAVINGS, accountName = " ", balanceText = "100", balance = 10_000)
        assertEquals(listOf("Chequing", "Visa", "Savings"), open.toCreate.map { it.name })
        assertEquals(318_450L - 41_220L + 10_000L, open.net)
        // With nothing added yet the form is always open.
        assertTrue(ready.copy(editing = false).formOpen)
    }

    @Test fun theNamesReadAsOneLine() {
        assertEquals("", accountNames(emptyList()))
        assertEquals("Chequing", accountNames(listOf(chequing)))
        assertEquals("Chequing and Visa", accountNames(listOf(chequing, visa)))
        val savings = OnboardAccount("Savings", AccountType.SAVINGS)
        assertEquals("Chequing, Visa and Savings", accountNames(listOf(chequing, visa, savings)))
        assertEquals("Chequing, Visa and 2 more", accountNames(listOf(chequing, visa, savings, savings)))
    }

    @Test fun addedAccountsSurviveAProcessDeathAndReadAgainAtTheCurrency() {
        val answered = ready.copy(step = OnboardingStep.ACCOUNT, accounts = listOf(chequing, visa), editing = false)
        val saved = SavedOnboarding.of(answered)
        val back = saved.applyTo(ready) { text -> text.takeIf { it.isNotBlank() }?.toBigDecimal()?.movePointRight(2)?.toLong() }
        assertEquals(listOf("Chequing", "Visa"), back.accounts.map { it.name })
        assertEquals(listOf(318_450L, 41_220L), back.accounts.map { it.balance })
        assertFalse(back.editing)
    }
}
