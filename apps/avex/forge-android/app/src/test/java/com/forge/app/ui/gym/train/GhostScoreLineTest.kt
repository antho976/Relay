package com.forge.app.ui.gym.train

import org.junit.Assert.assertEquals
import org.junit.Test

/** The session header's "vs last session" line says what its count is, with a real plural. */
class GhostScoreLineTest {

    @Test
    fun `a single tied set names the duel and takes the singular`() {
        assertEquals("VS LAST SESSION · BEAT 0 OF 1 SET", ghostScoreLine(beats = 0, total = 1))
    }

    @Test
    fun `half or more beaten leads with winning`() {
        assertEquals("BEATING LAST SESSION · 2 OF 4 SETS", ghostScoreLine(beats = 2, total = 4))
    }

    @Test
    fun `under half stays on the duel`() {
        assertEquals("VS LAST SESSION · BEAT 1 OF 3 SETS", ghostScoreLine(beats = 1, total = 3))
    }
}
