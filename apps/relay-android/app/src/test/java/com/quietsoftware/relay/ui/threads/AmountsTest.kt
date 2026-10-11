package com.quietsoftware.relay.ui.threads

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** A card reads an amount the PC's MCP door gave the agent as dollars, or an older thread's cents. */
class AmountsTest {
    private fun row(json: String) = Json.parseToJsonElement(json).jsonObject

    @Test
    fun decimalStringsAndMinorUnitsReadTheSame() {
        assertEquals(15799L, row("""{"amount": "157.99"}""").minor("amount", 2))
        assertEquals(15799L, row("""{"amount": 15799}""").minor("amount", 2))
        assertEquals(150000000L, row("""{"quantity": "1.5"}""").minor("quantity", 8))
        assertEquals(1500L, row("""{"amount": "1500"}""").minor("amount", 0))
        assertEquals("−$157.99", txAmount(row("""{"type": "EXPENSE", "amount": "157.99"}"""), Money("CAD", 2)))
    }

    @Test
    fun anythingElseIsNoAmount() {
        assertNull(row("""{"amount": "abc"}""").minor("amount", 2))
        assertNull(row("""{"amount": null}""").minor("amount", 2))
        assertNull(row("""{}""").minor("amount", 2))
    }
}
