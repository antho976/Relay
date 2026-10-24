package com.quietsoftware.relay.ui.term

import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The terminal repaints under steady output: frames arriving faster than one a frame still show. */
@OptIn(ExperimentalCoroutinesApi::class)
class FramePacingTest {
    @Test fun `output every 5 ms still paints about once a frame`() = runTest {
        val version = MutableStateFlow(0L)
        val shown = mutableListOf<Long>()
        val pacer = launch { paceFrames(version, { false }) { shown += it } }
        runCurrent()
        // Half a second of output, a PTY frame every 5 ms.
        repeat(100) {
            version.value++
            delay(5)
        }
        pacer.cancel()
        assertTrue("painted ${shown.size} times in 500 ms", shown.size in 25..40)
        assertTrue("never goes backwards", shown.zipWithNext().all { (a, b) -> b > a })
    }

    @Test fun `a change after a quiet spell shows at once, and the last one is not lost`() = runTest {
        val version = MutableStateFlow(0L)
        val shown = mutableListOf<Long>()
        val pacer = launch { paceFrames(version, { false }) { shown += it } }
        runCurrent()
        assertEquals(listOf(0L), shown)
        advanceTimeBy(100)
        version.value = 1
        version.value = 2
        runCurrent()
        assertEquals("the first change shows without waiting a frame", listOf(0L, 2L), shown)
        version.value = 3
        runCurrent()
        assertEquals(listOf(0L, 2L), shown)
        advanceTimeBy(FRAME_MS + 1)
        assertEquals("the change during the frame shows when it ends", listOf(0L, 2L, 3L), shown)
        pacer.cancel()
    }

    @Test fun `nothing shows while synchronized output holds the frame`() = runTest {
        val version = MutableStateFlow(0L)
        var held = true
        val shown = mutableListOf<Long>()
        val pacer = launch { paceFrames(version, { held }) { shown += it } }
        runCurrent()
        version.value = 1
        advanceTimeBy(FRAME_MS + 1)
        assertEquals(emptyList<Long>(), shown)
        held = false
        version.value = 2
        advanceTimeBy(FRAME_MS + 1)
        assertEquals(listOf(2L), shown)
        pacer.cancel()
    }
}
