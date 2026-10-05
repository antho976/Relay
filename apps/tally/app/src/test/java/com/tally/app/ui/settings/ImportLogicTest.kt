package com.tally.app.ui.settings

import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.NoteUse
import com.tally.core.AccountType
import com.tally.core.CategoryKind
import com.tally.core.Statement
import com.tally.core.StatementFormat
import com.tally.core.StatementRow
import com.tally.core.TxType
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate

class ImportLogicTest {

    private val day = LocalDate.of(2026, 9, 1)
    private val groceries = CategoryEntity(1, "Groceries", CategoryKind.EXPENSE, 0, "cart")
    private val dining = CategoryEntity(2, "Dining", CategoryKind.EXPENSE, 6, "dining")
    private val salary = CategoryEntity(3, "Salary", CategoryKind.INCOME, 0, "work")
    private val treats = CategoryEntity(4, "Treats", CategoryKind.EXPENSE, 3, "cake")
    private val categories = listOf(groceries, dining, salary, treats)

    private fun statement(vararg rows: StatementRow) = Statement(StatementFormat.DESJARDINS, rows.toList())

    private val month = statement(
        StatementRow(day, "PAIE EMPLOYEUR INC", 215_000, balance = 315_000),
        StatementRow(day.plusDays(1), "Achat - METRO PLUS #12", -6_842, balance = 308_158),
        StatementRow(day.plusDays(3), "Paiement carte VISA", -90_000, balance = 218_158),
        StatementRow(day.plusDays(4), "Achat - PATISSERIE ROLLAND", -1_250, balance = 216_908),
    )

    @Test fun linesAreCategorizedByMerchantAndTransfersAreSkipped() {
        val plan = planImport(month, ImportChoices(), categories, emptyMap(), emptyList())
        assertEquals(4, plan.lines.size)
        assertEquals(3, plan.count)
        assertEquals(1, plan.transferLike)
        assertEquals(1, plan.skipped)
        assertEquals(groceries.id, plan.lines[1].categoryId)
        assertEquals(salary.id, plan.lines[0].categoryId)
        assertNull("A card payment is not an expense", plan.lines[2].categoryId)
        assertEquals(6_842L + 1_250, plan.out)
        assertEquals(215_000L, plan.inn)
        // The account stood at 1,000 before the pay came in.
        assertEquals(100_000L, plan.openingFromStatement)
        assertEquals(216_908L, plan.endBalance)
    }

    @Test fun theOwnersOwnEntriesTeachThePayee() {
        val learned = learnedCategories(
            listOf(NoteUse("Patisserie Rolland", treats.id, TxType.EXPENSE), NoteUse("Patisserie Rolland", treats.id, TxType.EXPENSE)),
            categories,
        )
        val plan = planImport(month, ImportChoices(), categories, learned, emptyList())
        val line = plan.lines.last()
        assertEquals(treats.id, line.categoryId)
        assertTrue(line.learned)
        assertEquals(1, plan.learned)
    }

    @Test fun aLineAlreadyInTheAccountIsADuplicateOnce() {
        val twice = statement(
            StatementRow(day, "Achat - METRO", -6_842),
            StatementRow(day, "Achat - METRO", -6_842),
        )
        val plan = planImport(twice, ImportChoices(), categories, emptyMap(), listOf(day to -6_842L))
        assertEquals(1, plan.duplicates)
        assertEquals(1, plan.count)
    }

    @Test fun transfersCanGoInAsMovesToAnotherAccount() {
        val plan = planImport(month, ImportChoices(transfers = TransferMode.TRANSFERS, counterpartId = 9), categories, emptyMap(), emptyList())
        assertEquals(4, plan.count)
        assertEquals(9L, plan.toWrite.single { it.amount == -90_000L }.counterpartId)
        // A move is neither spending nor income.
        assertEquals(6_842L + 1_250, plan.out)
    }

    @Test fun aCardStatementOfPositivePurchasesReadsFlipped() {
        val card = statement(
            StatementRow(day, "Achat - METRO", 6_842),
            StatementRow(day.plusDays(1), "Achat - PHO LIEN", 2_310),
            StatementRow(day.plusDays(2), "Paiement - merci", -90_000),
        )
        assertTrue(guessFlip(card, AccountType.CREDIT))
        assertFalse(guessFlip(card, AccountType.CHEQUING))
        val plan = planImport(card, ImportChoices(flip = true), categories, emptyMap(), emptyList())
        assertEquals(-6_842L, plan.lines.first().amount)
    }

    @Test fun newestFirstFilesAreTurnedRound() {
        val rows = listOf(StatementRow(day.plusDays(2), "b", -1), StatementRow(day, "a", -1))
        assertEquals(listOf("a", "b"), chronological(rows).map { it.description })
    }

    @Test fun numericDatesReadDayFirstOutsideEnglishCanadaAndTheUs() {
        assertTrue(dayFirstFor("fr", "CA"))
        assertFalse(dayFirstFor("en", "CA"))
        assertFalse(dayFirstFor("en", "US"))
        assertTrue(dayFirstFor("en", "GB"))
    }

    @Test fun anAccountBelongsToTheBankItIsNamedFor() {
        assertTrue(belongsTo("Desjardins chequing", BankSource.DESJARDINS))
        assertFalse(belongsTo("Desjardins chequing", BankSource.WEALTHSIMPLE))
        assertFalse(belongsTo("Anything", BankSource.OTHER))
        assertEquals("84 entries imported into Chequing · 12 already there", importedLine(84, "Chequing", 12))
        assertEquals("1 entry imported into Chequing", importedLine(1, "Chequing", 0))
    }
}
