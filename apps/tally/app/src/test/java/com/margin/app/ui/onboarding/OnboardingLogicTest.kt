package com.margin.app.ui.onboarding

import com.margin.core.AccountType
import com.margin.core.Copy
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
}
