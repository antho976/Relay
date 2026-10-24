package com.quietsoftware.relay.core.link

import com.quietsoftware.relay.core.testing.Pipe
import com.quietsoftware.relay.core.wire.Event
import com.quietsoftware.relay.core.wire.Greeting
import com.quietsoftware.relay.core.wire.Route
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.concurrent.CopyOnWriteArrayList

/** Events dropped by a burst are reported even when nothing follows the burst. */
class LaggedTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    @After
    fun tearDown() = scope.cancel()

    @Test
    fun `a burst that overflows and goes quiet still says events were lost`() = runBlocking {
        val pipe = Pipe()
        val session = BusSession(pipe, Route("ws://192.168.1.20:7420", Route.Kind.Lan), Greeting("desk", "h1", "dev", "4.0", "c"), "dev1", scope)
        val gate = CompletableDeferred<Unit>()
        val seen = CopyOnWriteArrayList<Event>()
        scope.launch {
            session.events.collect { e ->
                seen += e
                gate.await() // a reader held up: the burst fills the buffer behind it
            }
        }.also { delay(100) }
        repeat(BusSession.EVENT_BUFFER + 50) { i ->
            pipe.toPhone.send("""{"v":1,"ev":"task.changed","ts":"2026-10-24T00:00:00Z","actor":"user","payload":{"task_id":$i}}""")
        }
        // The burst is read in full, then nothing else comes.
        withTimeout(10_000) { while (!pipe.toPhone.isEmpty) delay(10) }
        delay(100)
        gate.complete(Unit)
        withTimeout(10_000) { while (seen.none { it.ev == "bus.lagged" }) delay(20) }
        assertTrue(seen.last().ev == "bus.lagged")
        session.close()
    }
}
