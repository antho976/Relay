package com.forge.app.data.repo

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The archive-wide restore budget.
 *
 * The per-entry cap bounded each photo at 64 MiB and nothing bounded how many photos an archive
 * held, so a small, highly compressible ZIP full of distinct photo entries could write
 * tens of gigabytes into internal storage before anything noticed — and what noticed, eventually,
 * was the device running out of space in the middle of a restore.
 */
class ExtractionBudgetTest {

    @Test
    fun `entries inside the budget are allowed`() {
        val budget = ExtractionBudget(maxTotalBytes = 1_000, maxPhotos = 10)
        assertTrue(budget.spend(400))
        assertTrue(budget.spend(400))
        assertEquals(800L, budget.bytesSpent)
    }

    @Test
    fun `the budget is cumulative, not per entry`() {
        // The whole point: every one of these is comfortably under any single-entry cap.
        val budget = ExtractionBudget(maxTotalBytes = 1_000, maxPhotos = 10)
        assertTrue(budget.spend(400))
        assertTrue(budget.spend(400))
        assertFalse("the third small entry is what exceeds the archive's budget", budget.spend(400))
    }

    @Test
    fun `exactly the budget is still within it`() {
        val budget = ExtractionBudget(maxTotalBytes = 1_000, maxPhotos = 10)
        assertTrue(budget.spend(1_000))
        assertFalse(budget.spend(1))
    }

    @Test
    fun `an entry that blew its own cap fails the archive and adds nothing`() {
        // copyAtMost reports -1 there; adding it to the total would be meaningless.
        val budget = ExtractionBudget(maxTotalBytes = 1_000, maxPhotos = 10)
        assertFalse(budget.spend(-1))
        assertEquals(0L, budget.bytesSpent)
    }

    @Test
    fun `too many photos fails even when they are tiny`() {
        // The second failure mode: an archive of thousands of one-byte entries costs inodes and
        // time rather than space, and the byte budget would never notice.
        val budget = ExtractionBudget(maxTotalBytes = Long.MAX_VALUE, maxPhotos = 3)
        assertTrue(budget.countPhoto())
        assertTrue(budget.countPhoto())
        assertTrue(budget.countPhoto())
        assertFalse(budget.countPhoto())
    }

    @Test
    fun `the two limits are independent`() {
        val budget = ExtractionBudget(maxTotalBytes = 10, maxPhotos = 100)
        assertTrue(budget.countPhoto())
        assertFalse("the byte budget still applies to a photo the count allowed", budget.spend(11))
    }
}
