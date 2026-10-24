package com.quietsoftware.relay.core.link

import com.quietsoftware.relay.core.testing.FakeDoor
import com.quietsoftware.relay.core.testing.Pipe
import com.quietsoftware.relay.core.wire.Route
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Racing routes: the one that greets first is kept, and the others are closed, not left open. */
class HandshakeTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    @After
    fun tearDown() = scope.cancel()

    @Test
    fun `a route still waiting for its greeting is closed when another wins`() = runBlocking {
        val door = FakeDoor(scope).also { it.reachable = setOf("ws://192.168.1.20:7420") }
        // A LAN address that connects and then says nothing, as a stale address on another network can.
        val silent = Pipe()
        val dialer = Dialer { url, timeout -> if (url == "ws://10.9.9.9:7420") silent else door.dial(url, timeout) }
        val greeted = Handshake.greet(dialer, listOf(Route("ws://10.9.9.9:7420", Route.Kind.Lan), Route("ws://192.168.1.20:7420", Route.Kind.Lan))).getOrThrow()
        assertEquals("ws://192.168.1.20:7420", greeted.route.url)
        assertTrue("the silent route's socket was closed", silent.toPhone.isClosedForSend)
        greeted.socket.close()
    }
}
