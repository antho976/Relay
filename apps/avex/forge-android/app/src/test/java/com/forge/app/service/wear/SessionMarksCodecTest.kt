package com.forge.app.service.wear

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class SessionMarksCodecTest {

    @Test fun `marks round-trip`() {
        val m = WearFocusHolder.Marks(
            sessionId = 42,
            earlyDone = setOf("upper-a-0", "custom_Bench, close grip=narrow"),
            bonusSets = mapOf("upper-a-1" to 2, "custom_x=y" to 1)
        )
        assertEquals(m, decodeMarks(encodeMarks(m)))
    }

    @Test fun `empty marks round-trip`() {
        val m = WearFocusHolder.Marks(sessionId = 7)
        assertEquals(m, decodeMarks(encodeMarks(m)))
    }

    @Test fun `nothing stored or a damaged line reads as nothing`() {
        assertNull(decodeMarks(null))
        assertNull(decodeMarks(""))
        assertNull(decodeMarks("not-a-number\na\nb=1"))
    }

    @Test fun `a bad bonus pair is dropped, not fatal`() {
        assertEquals(mapOf("ok" to 1), decodeMarks("3\n\nok=1,bad=x,=2,zero=0")!!.bonusSets)
    }
}
