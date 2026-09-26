package com.forge.app.ui.gym.train

import com.forge.app.ui.common.ForgeHapticType
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * The set-log / PR cue on the live screen. A fresh screen first composes while the session read is
 * still suspended (no exercises), so arming then made a reopened session's existing PR sets read as
 * 0 → N: the whole PR celebration replayed on every reopen (audit 2026-09-26).
 */
class SetLogHapticCuesTest {

    @Test
    fun `a reopened session with PR sets fires nothing when it loads`() {
        val cues = SetLogHapticCues()
        assertNull(cues.advance(loading = true, totalSets = 0, totalPrSets = 0))
        assertNull(cues.advance(loading = false, totalSets = 6, totalPrSets = 2))
    }

    @Test
    fun `a PR logged after loading still fires`() {
        val cues = SetLogHapticCues()
        cues.advance(loading = true, totalSets = 0, totalPrSets = 0)
        cues.advance(loading = false, totalSets = 6, totalPrSets = 2)
        assertEquals(ForgeHapticType.PR_OR_FINISH, cues.advance(loading = false, totalSets = 7, totalPrSets = 3))
    }

    @Test
    fun `a plain set fires the set-logged cue`() {
        val cues = SetLogHapticCues()
        cues.advance(loading = false, totalSets = 0, totalPrSets = 0)
        assertEquals(ForgeHapticType.SET_LOGGED, cues.advance(loading = false, totalSets = 1, totalPrSets = 0))
        // Deleting a set or an unchanged reading is silent.
        assertNull(cues.advance(loading = false, totalSets = 0, totalPrSets = 0))
        assertNull(cues.advance(loading = false, totalSets = 0, totalPrSets = 0))
    }
}
