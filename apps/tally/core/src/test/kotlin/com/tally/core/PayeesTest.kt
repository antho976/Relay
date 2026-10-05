package com.tally.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class PayeesTest {

    @Test fun `channel words and references go, shouting is set in title case`() {
        assertEquals("Metro Plus Montreal QC", Payees.clean("ACHAT - METRO PLUS #123 MONTREAL QC"))
        assertEquals("Hydro-Quebec", Payees.clean("Paiement facture - AccèsD Internet /HYDRO-QUEBEC"))
        assertEquals("IGA Extra", Payees.clean("Paiement direct - IGA EXTRA 0045123"))
        assertEquals("Spotify", Payees.clean("POS Purchase SPOTIFY ****4821"))
    }

    @Test fun `a description that is all channel words keeps its text`() {
        assertEquals("Retrait", Payees.clean("Retrait"))
        assertEquals("Café Olimpico", Payees.clean("  Café   Olimpico "))
    }

    @Test fun `keys group the same payee across stores`() {
        assertEquals("metro plus", Payees.key("Metro Plus Montreal QC"))
        assertEquals("metro plus", Payees.key("METRO PLUS #4411"))
        assertEquals("", Payees.key("12345"))
    }

    @Test fun `known merchants file themselves`() {
        assertEquals("Groceries", Payees.guessCategory("METRO PLUS MONTREAL", income = false))
        assertEquals("Dining", Payees.guessCategory("Uber Eats", income = false))
        assertEquals("Transport", Payees.guessCategory("UBER TRIP", income = false))
        assertEquals("Subscriptions", Payees.guessCategory("Amazon Prime Video", income = false))
        assertEquals("Shopping", Payees.guessCategory("AMZN Mktp CA", income = false))
        assertEquals("Utilities", Payees.guessCategory("Hydro-Québec", income = false))
        assertEquals("Phone & internet", Payees.guessCategory("VIDEOTRON LTEE", income = false))
        assertEquals("Health", Payees.guessCategory("Jean Coutu #12", income = false))
        assertEquals("Salary", Payees.guessCategory("Dépôt de paie", income = true))
        assertEquals("Other income", Payees.guessCategory("Interest", income = true, code = "INT"))
    }

    @Test fun `a whole word key never matches inside a longer word`() {
        assertNull(Payees.guessCategory("Metropolis Books", income = false))
        assertNull(Payees.guessCategory("Paiement reçu", income = true))
    }

    @Test fun `card payments and transfers are moves, an e-transfer to a person is not`() {
        assertTrue(Payees.looksLikeTransfer("Paiement carte VISA"))
        assertTrue(Payees.looksLikeTransfer("Transfer out to Chequing"))
        assertTrue(Payees.looksLikeTransfer("Virement entre comptes"))
        assertTrue(Payees.looksLikeTransfer("anything", code = "TRFOUT"))
        assertFalse(Payees.looksLikeTransfer("Interac e-Transfer to Sam"))
        assertFalse(Payees.looksLikeTransfer("Virement Interac reçu"))
        assertFalse(Payees.looksLikeTransfer("Metro"))
    }
}
