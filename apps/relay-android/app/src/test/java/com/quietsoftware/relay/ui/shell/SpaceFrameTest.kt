package com.quietsoftware.relay.ui.shell

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** A page of one space, opened from the other, brings its space along; a shared page keeps the one picked. */
class SpaceFrameTest {
    @Test
    fun aPageNamesItsSpace() {
        for (r in listOf("threads", "thread/42", "tally", "arbiter", "avex")) assertEquals(r, "threads", spaceOf(r))
        for (r in listOf("agents", "board", "notes")) assertEquals(r, "dev", spaceOf(r))
        for (r in listOf("inbox", "outbox", "search", "settings", "pc", null)) assertNull(r, spaceOf(r))
    }
}
