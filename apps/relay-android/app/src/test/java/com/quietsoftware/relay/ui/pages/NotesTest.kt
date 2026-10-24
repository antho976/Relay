package com.quietsoftware.relay.ui.pages

import androidx.compose.runtime.saveable.SaverScope
import com.quietsoftware.relay.core.model.Note
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

    @Test
    fun aDraftSurvivesTheProcessBeingReclaimed() {
        val scope = SaverScope { true }
        fun roundTrip(d: Draft?): Draft? = DraftSaver.restore(with(DraftSaver) { scope.save(d) }!!)
        val base = Note(id = "12", projectId = 1, title = "Phone app", body = "The phone keeps a copy.", updatedAt = "2026-10-24T10:00:00Z")
        assertEquals(Draft("Phone app", "The phone keeps a copy, and more.", base), roundTrip(Draft("Phone app", "The phone keeps a copy, and more.", base)))
        assertEquals("a new note's draft", Draft("", "Written on the bus", null), roundTrip(Draft("", "Written on the bus", null)))
        assertEquals("a note open for reading stays so", null, roundTrip(null))
    }
}
