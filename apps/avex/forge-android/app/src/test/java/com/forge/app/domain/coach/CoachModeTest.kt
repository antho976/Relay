package com.forge.app.domain.coach

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CoachModeTest {

    @Test
    fun `suggest applies nothing by itself and leaves the field on last session`() {
        assertFalse(CoachMode.appliesItself(CoachMode.SUGGEST))
        assertFalse(CoachMode.fillsTargets(CoachMode.SUGGEST))
    }

    @Test
    fun `auto applies what it earned but does not fill the field`() {
        assertTrue(CoachMode.appliesItself(CoachMode.AUTO))
        assertFalse(CoachMode.fillsTargets(CoachMode.AUTO))
    }

    @Test
    fun `hands-off applies changes and fills the field`() {
        assertTrue(CoachMode.appliesItself(CoachMode.HANDS_OFF))
        assertTrue(CoachMode.fillsTargets(CoachMode.HANDS_OFF))
    }

    @Test
    fun `an unknown stored mode behaves like suggest`() {
        assertFalse(CoachMode.appliesItself(""))
        assertFalse(CoachMode.fillsTargets("nonsense"))
    }
}
