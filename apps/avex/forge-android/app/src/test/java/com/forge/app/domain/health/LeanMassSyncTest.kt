package com.forge.app.domain.health

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class LeanMassSyncTest {

    @Test
    fun `nothing in health connect imports nothing`() {
        assertFalse(LeanMassSync.shouldImport(null, null, null))
        assertFalse(LeanMassSync.shouldImport(1000L, null, null))
        assertFalse(LeanMassSync.shouldImport(null, 150.0, null))
    }

    @Test
    fun `first reading imports when no local history`() {
        assertTrue(LeanMassSync.shouldImport(1000L, 150.0, null))
    }

    @Test
    fun `only strictly newer readings import`() {
        assertTrue(LeanMassSync.shouldImport(2000L, 150.0, 1000L))
        assertFalse(LeanMassSync.shouldImport(1000L, 150.0, 1000L)) // equal = idempotent no-op
        assertFalse(LeanMassSync.shouldImport(500L, 150.0, 1000L))
    }

    @Test
    fun `implausible values are rejected as corrupt`() {
        assertFalse(LeanMassSync.shouldImport(2000L, 0.0, null))
        assertFalse(LeanMassSync.shouldImport(2000L, -5.0, null))
        assertFalse(LeanMassSync.shouldImport(2000L, 900.0, null))
    }
}
