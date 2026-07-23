package com.forge.app.service.wear

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CommandDeduperTest {

    @Test
    fun `a command id runs once and never again`() {
        val d = CommandDeduper()
        assertTrue(d.isNew("a"))
        assertFalse(d.isNew("a"))
        assertFalse(d.isNew("a"))
    }

    @Test
    fun `distinct ids are independent`() {
        val d = CommandDeduper()
        assertTrue(d.isNew("a"))
        assertTrue(d.isNew("b"))
        assertFalse(d.isNew("a"))
        assertFalse(d.isNew("b"))
    }

    @Test
    fun `capacity eviction only forgets the oldest`() {
        val d = CommandDeduper()
        repeat(200) { assertTrue(d.isNew("cmd-$it")) }
        // The most recent ids are still remembered even after older ones were evicted.
        assertFalse(d.isNew("cmd-199"))
        assertFalse(d.isNew("cmd-150"))
    }
}
