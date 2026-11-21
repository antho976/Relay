package com.forge.app.domain.adapt

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class E1rmTest {

    @Test
    fun singleRepIsTheWeightItself() {
        assertEquals(100.0, E1rm.epley(100.0, 1), 0.0001)
    }

    @Test
    fun zeroOrNegativeRepsFallBackToTheWeight() {
        assertEquals(100.0, E1rm.epley(100.0, 0), 0.0001)
        assertEquals(100.0, E1rm.epley(100.0, -3), 0.0001)
    }

    @Test
    fun epleyTenRepsIsOneThirdMore() {
        assertEquals(100.0 * (1 + 10 / 30.0), E1rm.epley(100.0, 10), 0.0001)
    }

    @Test
    fun monotonicInBothInputs() {
        assertTrue(E1rm.epley(50.0, 8) > E1rm.epley(45.0, 8))
        assertTrue(E1rm.epley(45.0, 10) > E1rm.epley(45.0, 8))
    }
}
