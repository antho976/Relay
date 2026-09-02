package com.forge.app.ui.common

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The durable-write boundary (M-22): a preference edit launched from a ViewModel must land even
 * when the destination that started it is popped while the edit is still suspended inside
 * DataStore. The "disk" here is a gate the test holds shut until after the scope is cancelled,
 * which is the deterministic form of "leave Settings before the file write finishes".
 */
@OptIn(ExperimentalCoroutinesApi::class)
class DurableWriteTest {

    @Test
    fun `a durable write in flight survives its scope being cancelled`() = runTest {
        val disk = CompletableDeferred<Unit>()
        var landed = false
        val viewModelScope = CoroutineScope(SupervisorJob() + StandardTestDispatcher(testScheduler))

        viewModelScope.launchDurable {
            disk.await()      // suspended inside the edit
            landed = true
        }
        runCurrent()          // the write is entered and parked on the slow disk
        viewModelScope.cancel() // the destination pops and clears the ViewModel
        disk.complete(Unit)
        advanceUntilIdle()

        assertTrue("the preference edit must finish despite the cancelled scope", landed)
    }

    @Test
    fun `the same edit under a bare launch is lost, which is the bug the boundary exists for`() = runTest {
        val disk = CompletableDeferred<Unit>()
        var landed = false
        val viewModelScope = CoroutineScope(SupervisorJob() + StandardTestDispatcher(testScheduler))

        viewModelScope.launch {
            disk.await()
            landed = true
        }
        runCurrent()
        viewModelScope.cancel()
        disk.complete(Unit)
        advanceUntilIdle()

        assertFalse(landed)
    }

    @Test
    fun `a durable write that needs no suspension completes at once`() = runTest {
        var landed = false
        val viewModelScope = CoroutineScope(SupervisorJob() + StandardTestDispatcher(testScheduler))
        viewModelScope.launchDurable { landed = true }
        runCurrent()
        assertTrue(landed)
    }
}
