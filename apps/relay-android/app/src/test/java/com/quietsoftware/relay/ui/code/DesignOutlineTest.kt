package com.quietsoftware.relay.ui.code

import org.junit.Assert.assertEquals
import org.junit.Test

/** What a design's page lists under its outline. */
class DesignOutlineTest {
    private val page = DesignNode(id = "page-1", kind = "page", name = "Home")

    private fun frame(id: String, hidden: Boolean? = null, parent: String = "page-1") =
        DesignNode(id = id, parent = parent, kind = "frame", name = id, w = 390.0, h = 844.0, hidden = hidden)

    @Test
    fun `the frames list leaves out what the outline does not draw`() {
        val nodes = listOf(
            page,
            frame("shown"),
            frame("hidden", hidden = true),
            frame("unhidden", hidden = false),
            DesignNode(id = "card", parent = "page-1", kind = "rect", w = 40.0, h = 40.0),
            // Frames inside frames are not listed: the outline draws them inside their parent.
            frame("nested", parent = "shown"),
        )
        val frames = pageFrames(page, nodes.groupBy { it.parent })
        assertEquals(listOf("shown", "unhidden"), frames.map { it.id })
    }
}
