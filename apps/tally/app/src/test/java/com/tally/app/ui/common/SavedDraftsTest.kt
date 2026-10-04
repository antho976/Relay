package com.tally.app.ui.common

import androidx.lifecycle.SavedStateHandle
import com.tally.app.ui.accounts.AccountDraft
import com.tally.app.ui.categories.CategoryDraft
import com.tally.app.ui.entry.EntryDraft
import com.tally.app.ui.entry.SavedEntry
import com.tally.app.ui.onboarding.OnboardingState
import com.tally.app.ui.onboarding.OnboardingStep
import com.tally.app.ui.onboarding.SavedOnboarding
import com.tally.app.ui.plan.BillDraft
import com.tally.app.ui.plan.GoalDraft
import com.tally.app.ui.plan.SavedBill
import com.tally.app.ui.plan.SavedGoal
import com.tally.core.AccountType
import com.tally.core.AmountInput
import com.tally.core.Frequency
import com.tally.core.TxType
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.time.LocalDate

/**
 * Every editor's draft survives process death: kept in a SavedStateHandle as one string, it
 * reads back exactly as typed in a handle rebuilt from what the system saved.
 */
@RunWith(RobolectricTestRunner::class)
class SavedDraftsTest {

    private val today = LocalDate.of(2026, 10, 4)

    /** What a new process gets back: a fresh handle holding only the saved string. */
    private fun SavedStateHandle.afterProcessDeath(): SavedStateHandle =
        SavedStateHandle(mapOf(DRAFT_KEY to get<String>(DRAFT_KEY)))

    @Test fun anAccountDraftComesBackAsTyped() {
        val handle = SavedStateHandle()
        val typed = AccountDraft(
            name = "Visa",
            type = AccountType.CREDIT,
            openingText = "1,250.50",
            opening = 125_050,
            owe = true,
            isDefault = false,
            archived = false,
        )

        handle.keepDraft(AccountDraft.serializer(), typed)

        assertEquals(typed, handle.afterProcessDeath().savedDraft(AccountDraft.serializer()))
    }

    @Test fun anAmountThatDoesNotReadYetStaysAsTyped() {
        val handle = SavedStateHandle()
        val typed = AccountDraft(name = "Cash", openingText = "12,,5", opening = null)

        handle.keepDraft(AccountDraft.serializer(), typed)

        assertEquals(typed, handle.afterProcessDeath().savedDraft(AccountDraft.serializer()))
    }

    @Test fun noDraftOrOneThatNoLongerReadsGivesNull() {
        val handle = SavedStateHandle()
        assertNull(handle.savedDraft(AccountDraft.serializer()))

        handle[DRAFT_KEY] = "{not json"
        assertNull(handle.savedDraft(AccountDraft.serializer()))

        handle[DRAFT_KEY] = """{"type":"NOT_A_TYPE"}"""
        assertNull(handle.savedDraft(AccountDraft.serializer()))
    }

    @Test fun aFieldFromAnotherVersionIsIgnored() {
        val handle = SavedStateHandle(mapOf(DRAFT_KEY to """{"name":"Visa","addedLater":true}"""))

        assertEquals(AccountDraft(name = "Visa"), handle.savedDraft(AccountDraft.serializer()))
    }

    @Test fun aCategoryDraftComesBackAsPicked() {
        val handle = SavedStateHandle()
        val picked = CategoryDraft(name = "Streaming", icon = "repeat", color = 3, archived = true)

        handle.keepDraft(CategoryDraft.serializer(), picked)

        assertEquals(picked, handle.afterProcessDeath().savedDraft(CategoryDraft.serializer()))
    }

    @Test fun anEntryDraftComesBackWithItsKeypadTextAndDate() {
        val handle = SavedStateHandle()
        val typed = EntryDraft(
            id = 0,
            type = TxType.TRANSFER,
            amount = AmountInput("12.5", 2),
            date = today.minusDays(1),
            accountId = 1,
            toAccountId = 2,
            categoryId = 5,
            categoryChosen = true,
            note = "Rent, my half",
            repeat = true,
            frequency = Frequency.WEEKLY,
            recurringId = null,
        )

        handle.keepDraft(SavedEntry.serializer(), SavedEntry.of(typed))
        val back = handle.afterProcessDeath().savedDraft(SavedEntry.serializer())?.toDraft()

        assertEquals(typed, back)
        assertEquals(1_250L, back?.amount?.minor)
    }

    @Test fun aYenEntryKeepsItsDigits() {
        val typed = EntryDraft(amount = AmountInput("1200", 0), date = today, accountId = 3)

        assertEquals(typed, SavedEntry.of(typed).toDraft())
    }

    @Test fun aBillDraftComesBackWithItsSchedule() {
        val handle = SavedStateHandle()
        val typed = BillDraft(
            id = 7,
            name = "Hydro",
            type = TxType.EXPENSE,
            amountText = "78.40",
            amount = 7_840,
            accountId = 1,
            categoryId = 5,
            frequency = Frequency.MONTHLY,
            interval = 2,
            anchorDate = LocalDate.of(2026, 8, 18),
            autoPost = false,
            active = false,
        )

        handle.keepDraft(SavedBill.serializer(), SavedBill.of(typed))

        assertEquals(typed, handle.afterProcessDeath().savedDraft(SavedBill.serializer())?.toDraft())
    }

    @Test fun aGoalDraftComesBackWithOrWithoutItsDate() {
        val dated = GoalDraft(id = 1, name = "Lisbon in May", targetText = "3200", target = 320_000, targetDate = LocalDate.of(2027, 5, 1), color = 1)
        val undated = GoalDraft(name = "Cushion", targetText = "12,0", target = null)

        listOf(dated, undated).forEach { typed ->
            val handle = SavedStateHandle()
            handle.keepDraft(SavedGoal.serializer(), SavedGoal.of(typed))
            assertEquals(typed, handle.afterProcessDeath().savedDraft(SavedGoal.serializer())?.toDraft())
        }
    }

    @Test fun onboardingResumesOnTheSameQuestionWithItsAnswers() {
        val answered = OnboardingState(
            today = today,
            loaded = true,
            step = OnboardingStep.BUDGET,
            currency = "CAD",
            moreCurrencies = true,
            accountName = "Joint chequing",
            accountType = AccountType.SAVINGS,
            balanceText = "1250.50",
            balance = 125_050,
            budgetText = "2400",
            budget = 240_000,
        )
        val handle = SavedStateHandle()
        handle.keepDraft(SavedOnboarding.serializer(), SavedOnboarding.of(answered))

        val fresh = OnboardingState(today = today, loaded = true, currency = "CAD")
        val parse = { text: String -> AmountInput(text, 2).minor.takeIf { text.isNotBlank() } }
        val back = handle.afterProcessDeath().savedDraft(SavedOnboarding.serializer())?.applyTo(fresh, parse)

        assertEquals(answered, back)
    }
}
