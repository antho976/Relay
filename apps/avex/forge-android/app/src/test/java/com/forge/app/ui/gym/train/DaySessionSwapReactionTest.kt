package com.forge.app.ui.gym.train

import com.forge.app.data.db.SessionSwapResult
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The pure gate deciding what the day screen does once the repository has answered a session swap
 * (H-11). A refusal means a set landed under the row while the sheet was open — so the sheet must
 * close as stale, the existing "sets already logged" message must show, and "Make default" must NOT
 * persist a future default for a swap this session refused. The happy path is unchanged: apply,
 * close, no message, persist when asked.
 */
class DaySessionSwapReactionTest {

    @Test
    fun `an applied just-today swap closes the sheet quietly and persists nothing`() {
        val r = swapReaction(SessionSwapResult.APPLIED, makeDefault = false)
        assertTrue(r.applied)
        assertTrue(r.closeSheet)
        assertNull(r.message)
        assertFalse(r.persistDefault)
    }

    @Test
    fun `an applied make-default swap persists the future default`() {
        val r = swapReaction(SessionSwapResult.APPLIED, makeDefault = true)
        assertTrue(r.applied)
        assertTrue(r.closeSheet)
        assertNull(r.message)
        assertTrue(r.persistDefault)
    }

    @Test
    fun `a refused swap closes the stale sheet and shows the existing message`() {
        val r = swapReaction(SessionSwapResult.REFUSED_SETS_LOGGED, makeDefault = false)
        assertFalse(r.applied)
        assertTrue("the sheet is stale once a set exists under it", r.closeSheet)
        assertEquals(SWAP_AFTER_SETS_MESSAGE, r.message)
        assertFalse(r.persistDefault)
    }

    @Test
    fun `a refused make-default swap does not persist the future default`() {
        val r = swapReaction(SessionSwapResult.REFUSED_SETS_LOGGED, makeDefault = true)
        assertFalse(r.applied)
        assertTrue(r.closeSheet)
        assertEquals(SWAP_AFTER_SETS_MESSAGE, r.message)
        assertFalse("a swap this session refused must not become every future session's default", r.persistDefault)
    }

    @Test
    fun `a vanished row closes the sheet without a message or a default`() {
        val r = swapReaction(SessionSwapResult.NOT_FOUND, makeDefault = true)
        assertFalse(r.applied)
        assertTrue(r.closeSheet)
        assertNull("the sets-logged message would be a lie here", r.message)
        assertFalse(r.persistDefault)
    }

    @Test
    fun `the refusal message is the same one the picker uses up front`() {
        // One message for one condition, whichever side of the sheet it is caught on.
        assertEquals(
            SWAP_AFTER_SETS_MESSAGE,
            swapReaction(SessionSwapResult.REFUSED_SETS_LOGGED, makeDefault = false).message
        )
    }
}
