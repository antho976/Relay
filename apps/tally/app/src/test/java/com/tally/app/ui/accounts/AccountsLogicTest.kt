package com.tally.app.ui.accounts

import com.tally.app.data.db.AccountBalance
import com.tally.app.data.repo.AccountUse
import com.tally.app.data.repo.OtherAccountChange
import com.tally.core.AccountType
import com.tally.core.Copy
import com.tally.core.MoneyFormatter
import com.tally.core.Registration
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.Locale

class AccountsLogicTest {

    private fun account(
        id: Long,
        type: AccountType,
        balance: Long,
        opening: Long = 0,
        entries: Int = 0,
        archived: Boolean = false,
    ) = AccountBalance(id, "Account $id", type, opening, archived, id.toInt(), balance, entries)

    private val chequing = account(1, AccountType.CHEQUING, 300_000, opening = 240_000, entries = 40)
    private val visa = account(2, AccountType.CREDIT, -40_000, entries = 61)
    private val savings = account(3, AccountType.SAVINGS, 700_000, opening = 650_000, entries = 3)
    private val old = account(4, AccountType.CREDIT, -5_000, entries = 12, archived = true)

    @Test fun activeAndArchivedSplitArchivedNeverCounts() {
        val s = buildAccountsState(listOf(chequing, visa, savings, old), defaultAccountId = 1, entries = 100)
        assertEquals(listOf(1L, 2L, 3L), s.active.map { it.account.id })
        assertEquals(listOf(4L), s.archived.map { it.account.id })
        assertEquals(960_000L, s.net)
        assertEquals(1_000_000L, s.held)
        assertEquals(40_000L, s.owed)
        assertEquals(960_000L - 890_000L, s.sinceOpening)
        assertEquals(100, s.entries)
        assertTrue(s.loaded)
        assertTrue(s.canTransfer)
    }

    @Test fun sharesAreOfTheirOwnSide() {
        val s = buildAccountsState(listOf(chequing, visa, savings), defaultAccountId = 0, entries = 0)
        assertEquals(0.3f, s.active.first { it.account.id == 1L }.share, 0.0001f)
        assertEquals(0.7f, s.active.first { it.account.id == 3L }.share, 0.0001f)
        assertEquals(1f, s.active.first { it.account.id == 2L }.share, 0.0001f)
        assertEquals("30% of what you hold", shareLine(s.active.first { it.account.id == 1L }))
        assertEquals("100% of what you owe", shareLine(s.active.first { it.account.id == 2L }))
    }

    @Test fun largestBusiestAndDefaultAreFound() {
        val s = buildAccountsState(listOf(chequing, visa, savings), defaultAccountId = 1, entries = 0)
        assertEquals(3L, s.largest?.account?.id)
        assertEquals(2L, s.busiest?.account?.id)
        assertEquals("Account 1", s.defaultName)
        assertTrue(s.active.first { it.account.id == 1L }.isDefault)
    }

    @Test fun zeroAccountsIsAnHonestZero() {
        val s = buildAccountsState(emptyList(), defaultAccountId = 0, entries = 0)
        assertEquals(0L, s.net)
        assertNull(s.largest)
        assertNull(s.busiest)
        assertNull(s.defaultName)
        assertFalse(s.canTransfer)
    }

    @Test fun anArchivedDefaultIsNotNamed() {
        val s = buildAccountsState(listOf(chequing, old), defaultAccountId = 4, entries = 0)
        assertNull(s.defaultName)
    }

    @Test fun openingBalanceReadsBlankAsZeroAndKeepsItsSign() {
        assertEquals(0L, readOpening("", null))
        assertEquals(0L, readOpening("   ", null))
        assertNull(readOpening("abc", null))
        assertEquals(1_250L, readOpening("12.50", 1_250L))
        val card = AccountDraft(type = AccountType.CREDIT, opening = 41_280, owe = true)
        assertEquals(-41_280L, card.signedOpening)
        assertEquals(41_280L, card.copy(owe = false).signedOpening)
        assertNull(card.copy(opening = null).signedOpening)
    }

    @Test fun theOweSwitchShowsForACardOrWhileItIsOn() {
        assertTrue(AccountDraft(type = AccountType.CREDIT).showsOwe)
        assertFalse(AccountDraft(type = AccountType.CHEQUING).showsOwe)
        assertTrue(AccountDraft(type = AccountType.CHEQUING, owe = true).showsOwe)
    }

    @Test fun anArchivedAccountIsNeverTheDefault() {
        assertTrue(AccountDraft(isDefault = true).effectiveDefault)
        assertFalse(AccountDraft(isDefault = true, archived = true).effectiveDefault)
    }

    @Test fun problemsNameWhatIsMissing() {
        val blank = accountProblems(AccountDraft(name = "  ", opening = 0))
        assertTrue(blank.any)
        assertEquals("Give the account a name", blank.name)
        assertNull(blank.opening)
        val bad = accountProblems(AccountDraft(name = "Visa", opening = null))
        assertNull(bad.name)
        assertTrue(bad.opening != null)
        assertFalse(accountProblems(AccountDraft(name = "Visa", opening = 0)).any)
    }

    @Test fun thePreviewShareLeavesTheAccountItselfOutOfTheOthers() {
        val all = listOf(chequing, visa, savings)
        val p = previewShare(id = 1, balance = 300_000, archived = false, all = all)
        assertEquals(1_000_000L, p.total)
        assertEquals(0.3f, p.share, 0.0001f)
        val fresh = previewShare(id = 0, balance = 100_000, archived = false, all = all)
        assertEquals(1_100_000L, fresh.total)
        val owed = previewShare(id = 0, balance = -10_000, archived = false, all = all)
        assertEquals(50_000L, owed.total)
        assertEquals(0.2f, owed.share, 0.0001f)
        assertEquals(SharePreview(), previewShare(id = 1, balance = 300_000, archived = true, all = all))
        assertEquals(SharePreview(), previewShare(id = 0, balance = 0, archived = false, all = all))
    }

    private val money = MoneyFormatter("CAD", Locale.CANADA)

    @Test fun theDeleteLineCarriesTheRealCounts() {
        assertEquals(
            "Delete Visa? Its 61 entries go with it. This cannot be undone.",
            deleteAccountLine("Visa", AccountUse(entries = 61), money, canArchive = false),
        )
        assertEquals(
            "Delete Visa? Its 1 entry goes with it. This cannot be undone.",
            deleteAccountLine("Visa", AccountUse(entries = 1), money, canArchive = false),
        )
        assertEquals(
            "Delete Visa? It has no entries and no bills. This cannot be undone.",
            deleteAccountLine("Visa", AccountUse(), money, canArchive = false),
        )
    }

    /** The sample's Visa: its bills and the card payments from Chequing go too, and Chequing reads higher after. */
    @Test fun theDeleteLineNamesTheBillsAndEveryAccountWhoseBalanceMoves() {
        val use = AccountUse(
            entries = 24,
            bills = 5,
            others = listOf(
                OtherAccountChange("Chequing", transfers = 2, change = 180_000),
                OtherAccountChange("Savings", transfers = 1, change = -1_000),
            ),
        )
        assertEquals(
            "Delete Visa? Its 24 entries and 5 bills go with it. " +
                "Chequing goes up by $1,800.00, since 2 transfers between them go too. " +
                "Savings goes down by $10.00, since 1 transfer between them goes too. " +
                "This cannot be undone. Archiving keeps all of it and takes the account out of the pickers.",
            deleteAccountLine("Visa", use, money, canArchive = true),
        )
        assertEquals(
            "Delete Visa? Its 1 bill goes with it. This cannot be undone.",
            deleteAccountLine("Visa", AccountUse(bills = 1), money, canArchive = false),
        )
        assertEquals(
            "2 transfers with Savings go too, which leaves its balance where it is.",
            otherAccountLine(OtherAccountChange("Savings", transfers = 2, change = 0), money),
        )
    }

    @Test fun theDeleteNoticeSaysWhatWentWithIt() {
        assertEquals("Visa deleted with its 24 entries and 5 bills", accountDeletedLine("Visa", AccountUse(entries = 24, bills = 5)))
        assertEquals("Visa deleted with its 1 bill", accountDeletedLine("Visa", AccountUse(bills = 1)))
        assertEquals("Visa deleted", accountDeletedLine("Visa", AccountUse()))
    }

    @Test fun anInvestmentAccountKeepsItsInstitutionOrTakesItFromItsName() {
        assertEquals("Wealthsimple", institutionFor("Wealthsimple TFSA", ""))
        assertEquals("Questrade", institutionFor("Wealthsimple TFSA", "Questrade"))
        assertEquals("", institutionFor("Desjardins REER", " "))
        val tfsa = freeTemplates(emptyList()).first { it.name == "Wealthsimple TFSA" }
        assertEquals(Registration.TFSA, tfsa.registration)
    }

    @Test fun everyLineKeepsTheVoice() {
        val use = AccountUse(3, 2, listOf(OtherAccountChange("Chequing", 1, 5_000), OtherAccountChange("Cash", 1, -5_000)))
        val lines = AccountType.entries.flatMap { listOf(typeLabel(it), typeHint(it)) } +
            listOf(
                deleteAccountLine("Visa", use, money, canArchive = true),
                accountDeletedLine("Visa", use),
                accountProblems(AccountDraft(opening = null)).opening.orEmpty(),
            )
        lines.forEach { line -> Copy.banned.forEach { bad -> assertFalse("\"$line\" has \"$bad\"", line.lowercase().contains(bad)) } }
    }
}
