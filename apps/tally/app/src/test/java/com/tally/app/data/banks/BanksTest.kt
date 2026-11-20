package com.tally.app.data.banks

import com.tally.app.data.db.AccountEntity
import com.tally.app.ui.banks.BanksViewModel
import com.tally.core.AccountType
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class BanksTest {

    private val answer = Json.parseToJsonElement(
        """{"banks":[{"id":"b1","institution":"Desjardins","last_sync":1763500000000,
            "accounts":[{"id":"a1","name":"Visa Classique","mask":"4417","kind":"credit card","follow":true,"into":"u-visa"},
                        {"id":"a2","name":"Épargne","mask":"3302","kind":"savings","follow":false}]},
           {"institution":"no id, dropped"}]}"""
    )

    @Test fun readsTheServersBanks() {
        val banks = Banks.parseBanks(answer)
        assertEquals(1, banks.size)
        val b = banks.single()
        assertEquals("Desjardins", b.institution)
        assertEquals(1763500000000L, b.lastSync)
        assertNull(b.problem)
        assertEquals(listOf("a1", "a2"), b.accounts.map { it.id })
        assertTrue(b.accounts[0].follow)
        assertEquals("u-visa", b.accounts[0].into)
        assertNull(b.accounts[1].into)
    }

    @Test fun spendingAccountsAreFollowedByDefaultSavingsAreNot() {
        assertTrue(BanksViewModel.followsByDefault(BankAccount("a", "Visa", "1", "credit card", false, null)))
        assertTrue(BanksViewModel.followsByDefault(BankAccount("a", "Compte", "1", "checking", false, null)))
        assertFalse(BanksViewModel.followsByDefault(BankAccount("a", "Épargne", "1", "savings", false, null)))
        assertFalse(BanksViewModel.followsByDefault(BankAccount("a", "TFSA", "1", "tfsa", false, null)))
    }

    @Test fun matchesTheTallyAccountOfTheSameBankAndKind() {
        val mine = listOf(
            AccountEntity(name = "Desjardins Visa", type = AccountType.CREDIT, uid = "u-visa"),
            AccountEntity(name = "Desjardins chequing", type = AccountType.CHEQUING, uid = "u-cheq"),
            AccountEntity(name = "Wealthsimple Cash", type = AccountType.CHEQUING, uid = "u-ws"),
        )
        assertEquals("u-visa", BanksViewModel.match("Desjardins", BankAccount("a", "Visa Classique", "4417", "credit card", true, null), mine)?.uid)
        assertEquals("u-cheq", BanksViewModel.match("Desjardins", BankAccount("a", "Compte courant", "0921", "checking", true, null), mine)?.uid)
        assertNull(BanksViewModel.match("RBC", BankAccount("a", "Chequing", "1", "checking", true, null), mine))
    }
}
