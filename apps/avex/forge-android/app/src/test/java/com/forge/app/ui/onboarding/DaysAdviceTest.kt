package com.forge.app.ui.onboarding

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test

class DaysAdviceTest {

    @Test
    fun `three to five days carries no warning`() {
        (3..5).forEach { assertNull(daysAdvice(it, "intermediate")) }
    }

    @Test
    fun `every count outside three to five is warned`() {
        listOf(1, 2, 6, 7).forEach { assertNotNull(daysAdvice(it, "beginner")) }
    }

    @Test
    fun `no answer yet says nothing`() {
        assertNull(daysAdvice(0, ""))
    }

    @Test
    fun `six days reads differently for an experienced lifter`() {
        assertNotEquals(daysAdvice(6, "beginner"), daysAdvice(6, "advanced"))
        assertEquals(daysAdvice(6, "beginner"), daysAdvice(6, "intermediate"))
    }
}
