package com.quietsoftware.relay.core.wire

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class PairLinkTest {
    @Test
    fun `reads the link the PC prints`() {
        // pairlink.rs's own test link.
        val link = PairLink.parse("relay://pair?v=1&code=ABCD-EFGH&host=antho%20desktop&id=abc&instance=dev&direct=ws%3A%2F%2F192.168.1.20%3A7420&via=wss%3A%2F%2Frelay.example.org%2Fjoin%2Froom1")!!
        assertEquals("ABCD-EFGH", link.code)
        assertEquals("antho desktop", link.host)
        assertEquals(listOf("ws://192.168.1.20:7420"), link.direct)
        assertEquals("wss://relay.example.org/join/room1", link.via)
        assertEquals(listOf(Route.Kind.Lan, Route.Kind.Rendezvous), link.routes.map { it.kind })
    }

    @Test
    fun `refuses links it cannot use`() {
        assertNull(PairLink.parse("relay://pair?v=2&code=X&direct=ws%3A%2F%2Fa%3A1"))
        assertNull(PairLink.parse("relay://pair?v=1&code=X"))
        assertNull(PairLink.parse("https://example.org"))
    }

    @Test
    fun `typed addresses become WebSocket urls`() {
        assertEquals("ws://192.168.1.20:7420", PairLink.address("192.168.1.20"))
        assertEquals("ws://my-pc.tail1234.ts.net:7420", PairLink.address("my-pc.tail1234.ts.net"))
        assertEquals("ws://host:9000", PairLink.address("host:9000"))
        assertEquals("wss://relay.example.org/join/r", PairLink.address("https://relay.example.org/join/r/"))
        assertNull(PairLink.address("two words"))
    }

    @Test
    fun `routes are tried home network first and the server last`() {
        val routes = listOf(
            Route("wss://relay.example.org/join/r", Route.Kind.Rendezvous),
            Route("ws://100.101.102.103:7420", Route.Kind.of("ws://100.101.102.103:7420")),
            Route("ws://10.0.0.4:7420", Route.Kind.of("ws://10.0.0.4:7420")),
            Route("ws://192.168.1.20:7420", Route.Kind.of("ws://192.168.1.20:7420")),
        )
        assertEquals(Route.Kind.Tailscale, routes[1].kind)
        val auto = orderRoutes(routes, RoutePolicy.Auto, preferred = "ws://192.168.1.20:7420").map { it.url }
        assertEquals(listOf("ws://192.168.1.20:7420", "ws://10.0.0.4:7420", "ws://100.101.102.103:7420", "wss://relay.example.org/join/r"), auto)
        assertEquals(1, orderRoutes(routes, RoutePolicy.ServerOnly).size)
        assertEquals(3, orderRoutes(routes, RoutePolicy.DirectOnly).size)
    }

    @Test
    fun `a link cut mid-escape or with a bad escape is no link, not a crash`() {
        assertNull(PairLink.parse("relay://pair?v=1&code=X&direct=ws%3A%2F%2F192.168.1.20%3"))
        assertNull(PairLink.parse("relay://pair?v=1&code=X&direct=ws%3A%2F%2F192.168.1.20%zz7420"))
    }
}
