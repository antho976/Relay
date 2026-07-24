package com.forge.app.service.wear

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The "follow the lifter" policy — the one place the watch's current exercise is decided, shared
 * by the mirror (display) and the set-write path so they can never disagree. Locked here because
 * a drift bug reads as "my watch is stuck on the last exercise" mid-workout.
 */
class CurrentSlotResolverTest {

    @Test
    fun nothingLoggedStartsAtFirstIncomplete() {
        assertEquals(
            0,
            CurrentSlotResolver.resolve(
                plannedSets = listOf(3, 3, 3),
                doneSets = listOf(0, 0, 0),
                lastLoggedIdx = null
            )
        )
    }

    @Test
    fun lastLoggedSlotWinsWhileIncomplete() {
        // Two of three sets done on slot 1 — the lifter is mid-exercise, stay there.
        assertEquals(
            1,
            CurrentSlotResolver.resolve(
                plannedSets = listOf(3, 3, 3),
                doneSets = listOf(3, 2, 0),
                lastLoggedIdx = 1
            )
        )
    }

    @Test
    fun completedSlotAdvancesToNextIncomplete() {
        assertEquals(
            2,
            CurrentSlotResolver.resolve(
                plannedSets = listOf(3, 3, 3),
                doneSets = listOf(3, 3, 0),
                lastLoggedIdx = 1
            )
        )
    }

    @Test
    fun advanceWrapsPastTheEndToSkippedEarlierWork() {
        // Slot 0 was skipped, slots 1–2 are done; wrapping lands back on 0.
        assertEquals(
            0,
            CurrentSlotResolver.resolve(
                plannedSets = listOf(3, 3, 3),
                doneSets = listOf(0, 3, 3),
                lastLoggedIdx = 2
            )
        )
    }

    @Test
    fun earlyDoneSlotIsSkippedWhenScanning() {
        // Slot 1 abandoned via "finish exercise early" — the scan must jump over it, not trap
        // the watch in the lifter's past (the reported sync bug).
        assertEquals(
            2,
            CurrentSlotResolver.resolve(
                plannedSets = listOf(3, 3, 3),
                doneSets = listOf(3, 1, 0),
                lastLoggedIdx = 0,
                earlyDoneIdx = setOf(1)
            )
        )
    }

    @Test
    fun freshLoggingOverridesEarlyDoneOnThatSlot() {
        // The lifter changed their mind and logged on the abandoned slot — following them wins.
        assertEquals(
            1,
            CurrentSlotResolver.resolve(
                plannedSets = listOf(3, 3, 3),
                doneSets = listOf(3, 2, 0),
                lastLoggedIdx = 1,
                earlyDoneIdx = setOf(1)
            )
        )
    }

    @Test
    fun fullyDoneDayPinsToLastLoggedSlot() {
        assertEquals(
            1,
            CurrentSlotResolver.resolve(
                plannedSets = listOf(3, 3),
                doneSets = listOf(3, 3),
                lastLoggedIdx = 1
            )
        )
    }

    @Test
    fun allSlotsEarlyDoneFallsBackToLastLogged() {
        // Everything abandoned or done — nowhere to advance, hold the lifter's last position.
        assertEquals(
            0,
            CurrentSlotResolver.resolve(
                plannedSets = listOf(3, 3),
                doneSets = listOf(3, 0),
                lastLoggedIdx = 0,
                earlyDoneIdx = setOf(1)
            )
        )
    }

    @Test
    fun emptyDayResolvesToNoSlot() {
        assertEquals(
            -1,
            CurrentSlotResolver.resolve(
                plannedSets = emptyList(),
                doneSets = emptyList(),
                lastLoggedIdx = null
            )
        )
    }
}
