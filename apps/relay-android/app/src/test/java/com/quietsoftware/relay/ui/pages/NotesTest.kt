package com.quietsoftware.relay.ui.pages

import org.junit.Assert.assertEquals
import org.junit.Test

/** A note's preview reads as words, not as the Markdown that draws them. */
class NotesTest {
    @Test
    fun aPreviewDropsTheMarksAndKeepsTheWords() {
        val body = "# Phone app\n\nThe phone keeps **a copy**.\n\n```kotlin\nhub.change(op)\n```\n\n| route | private |\n|---|---|\n| WiFi | yes |\n\n- [x] ordered\n2. see [docs](https://x.y)"
        assertEquals(
            listOf("Phone app", "The phone keeps a copy.", "hub.change(op)", "route · private", "WiFi · yes", "ordered", "see docs"),
            plainLines(body),
        )
    }
}
