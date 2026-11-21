package com.forge.app

import androidx.datastore.core.DataStore
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.async
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.*
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class StartupGateTest {
    private class Store : DataStore<String> {
        var value = "original"
        override val data: Flow<String> get() = flowOf(value)
        override suspend fun updateData(transform: suspend (String) -> String): String {
            value = transform(value)
            return value
        }
    }

    @Test fun `early constructor read does not initialize live storage before recovery`() = runTest {
        val gate = StartupGate()
        val store = Store()
        var opened = 0
        val guarded = GatedDataStore(gate::await) { opened++; store }
        val reading = async { guarded.data.first() }
        runCurrent()
        assertEquals(0, opened)
        assertFalse(reading.isCompleted)
        // Restore can still replace the contents before any DataStore gets to cache them.
        store.value = "restored"
        gate.complete()
        assertEquals("restored", reading.await())
        assertEquals(1, opened)
    }

    @Test fun `early writer waits without blocking other work`() = runTest {
        val gate = StartupGate()
        val store = Store()
        val guarded = GatedDataStore(gate::await) { store }
        val writing = async { guarded.updateData { "$it edited" } }
        runCurrent()
        assertEquals("original",store.value)
        assertFalse(writing.isCompleted)
        store.value = "restored"
        gate.complete()
        assertEquals("restored edited",writing.await())
    }

    @Test fun `failed recovery never initializes a store`() = runTest {
        val gate = StartupGate()
        var opened = false
        val guarded = GatedDataStore(gate::await) { opened=true; Store() }
        val reading = async { runCatching { guarded.data.first() } }
        runCurrent()
        gate.fail(IllegalStateException("rollback incomplete"))
        assertTrue(reading.await().isFailure)
        assertFalse(opened)
    }
}
