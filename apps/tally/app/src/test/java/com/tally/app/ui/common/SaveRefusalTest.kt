package com.tally.app.ui.common

import com.tally.app.ui.accounts.AccountDraft
import com.tally.app.ui.accounts.accountProblems
import com.tally.app.ui.plan.BillDraft
import com.tally.app.ui.plan.GoalDraft
import com.tally.app.ui.plan.billProblems
import com.tally.app.ui.plan.goalProblems
import com.tally.core.Copy
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Test
import java.time.LocalDate

/** A refused save says what stopped it, by name, next to the button that was pressed. */
class SaveRefusalTest {

    @Test fun theLineNamesEveryProblemInFormOrder() {
        val bill = billProblems(BillDraft(anchorDate = LocalDate.of(2026, 10, 4), accountId = 1, amountText = "", amount = null))
        assertEquals(
            "Not saved · Name the bill so you can find it later · Enter an amount above zero",
            refusalLine(listOf(bill.name, bill.amount, bill.account)),
        )
        val goal = goalProblems(GoalDraft(name = "Lisbon"))
        assertEquals("Not saved · Enter a target above zero", refusalLine(listOf(goal.name, goal.target)))
    }

    @Test fun nothingToSayGivesNoLine() {
        val fine = accountProblems(AccountDraft(name = "Visa", opening = 0))
        assertNull(refusalLine(listOf(fine.name, fine.opening)))
        assertNull(refusalLine(emptyList()))
    }

    @Test fun theLineKeepsTheVoice() {
        val account = accountProblems(AccountDraft(name = " ", opening = null))
        val line = refusalLine(listOf(account.name, account.opening)).orEmpty()
        assertEquals("Not saved · Give the account a name · Enter an amount, like 250 or 1,250.50", line)
        Copy.banned.forEach { bad -> assertFalse("\"$line\" has \"$bad\"", line.lowercase().contains(bad)) }
    }
}
