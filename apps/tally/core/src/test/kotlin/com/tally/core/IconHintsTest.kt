package com.tally.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class IconHintsTest {

    @Test fun `names suggest their glyph in english and french`() {
        assertEquals("coffee", IconHints.suggest("Coffee"))
        assertEquals("coffee", IconHints.suggest("Café"))
        assertEquals("cart", IconHints.suggest("Épicerie"))
        assertEquals("home", IconHints.suggest("Loyer"))
        assertEquals("pets", IconHints.suggest("Dog food"))
        assertEquals("invest", IconHints.suggest("TFSA"))
        assertNull(IconHints.suggest("Zzyzx"))
        assertNull(IconHints.suggest(""))
    }

    @Test fun `every suggestion is a key the app can draw`() {
        val words = listOf(
            "coffee", "tea", "pizza", "takeout", "lunch", "bakery", "beer", "wine", "alcohol", "bars", "dining", "groceries",
            "gas", "parking", "charging", "garage", "car", "taxi", "bike", "train", "metro", "transit", "flights", "hotel",
            "travel", "rent", "condo", "hydro", "water", "heating", "phone", "internet", "tv", "insurance", "furniture",
            "repairs", "cleaning", "laundry", "garden", "clothes", "computer", "audio", "toys", "flowers", "jewels",
            "shopping", "medication", "pharmacy", "hospital", "dentist", "glasses", "therapy", "haircut", "beauty", "yoga",
            "gym", "health", "movies", "games", "music", "party", "theatre", "art", "camping", "hiking", "pool", "casino",
            "tickets", "baby", "kids", "school", "books", "pets", "family", "parents", "charity", "church", "birthday",
            "gifts", "subscriptions", "investments", "crypto", "taxes", "loans", "credit card", "fees", "lawyer", "salary",
            "business", "bonus", "refunds", "sold", "cash", "shipping", "mail", "online", "cloud",
        )
        words.forEach { w ->
            val key = IconHints.suggest(w)
            assertTrue("$w suggested nothing", key != null)
            assertTrue("$w suggested $key, which is not an icon key", key in Defaults.iconKeys)
        }
    }

    @Test fun `icon keys are unique and keep the original thirty first`() {
        assertEquals(Defaults.iconKeys.size, Defaults.iconKeys.toSet().size)
        assertEquals("cart", Defaults.iconKeys.first())
        assertEquals("book", Defaults.iconKeys[29])
    }
}
