package com.quietsoftware.relay.data

import com.quietsoftware.relay.data.link.ConnectFailure
import com.quietsoftware.relay.data.link.Reply
import com.quietsoftware.relay.data.link.ServerConnect
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

/** Pairing through a server the person typed, as Tally 2 connects (docs/CLOUD.md). */
class ServerConnectTest {
    private val web = MockWebServer()

    @After fun down() = web.shutdown()

    @Test fun `whatever is typed or pasted names the server`() {
        val host = "https://my-bot.onrender.com"
        for (typed in listOf(
            "my-bot.onrender.com", " my-bot.onrender.com/ ", "https://my-bot.onrender.com",
            "wss://my-bot.onrender.com/relay", "https://my-bot.onrender.com/api/connect",
            "https://my-bot.onrender.com/api/tally", "https://my-bot.onrender.com/dashboard/devices",
        )) {
            assertEquals(typed, host, ServerConnect.normalize(typed))
        }
        assertEquals("http://192.168.1.20:8080", ServerConnect.normalize("ws://192.168.1.20:8080/relay"))
        assertEquals("", ServerConnect.normalize("  "))
        assertEquals("my-bot.onrender.com", ServerConnect("wss://my-bot.onrender.com/relay").host)
    }

    @Test fun `asks the typed server and collects the PC's link from it`() = runBlocking {
        web.start()
        val api = ServerConnect(web.url("/relay").toString())
        web.enqueue(json(200, """{"service":"relay-connect","pc":true}"""))
        assertTrue(api.pcConnected())
        web.enqueue(json(201, """{"id":"r1","code":"AB12","notified":true,"expiresAt":99}"""))
        val asked = api.ask("Pixel 8")
        assertEquals("AB12", asked.code)
        web.enqueue(json(200, """{"status":"ready","link":"relay://pair?v=1&code=X"}"""))
        assertEquals(Reply.Ready("relay://pair?v=1&code=X"), api.claim(asked))

        assertEquals("/api/connect", web.takeRequest().path)
        val enroll = web.takeRequest()
        assertEquals("/api/connect/enroll", enroll.path)
        val body = Json.parseToJsonElement(enroll.body.readUtf8()).jsonObject
        assertEquals("relay", body["kind"]!!.jsonPrimitive.content)
        // Only the secret's hash leaves the phone; the secret collects the link.
        assertEquals(ServerConnect.challengeOf(asked.verifier), body["challenge"]!!.jsonPrimitive.content)
        val claim = web.takeRequest()
        assertEquals("/api/connect/claim", claim.path)
        assertTrue(claim.body.readUtf8().contains(asked.verifier))
    }

    @Test fun `an address that is not the server says so`() = runBlocking {
        web.start()
        web.enqueue(json(200, """{"service":"tally-cloud","cloud":true}"""))
        try {
            ServerConnect(web.url("/").toString()).pcConnected()
            fail("a Tally-only answer is not a Relay server")
        } catch (e: ConnectFailure) {
            assertTrue(e.message!!, e.message!!.startsWith("That address does not answer as your server"))
        }
        try {
            ServerConnect("not a server").pcConnected()
            fail("not an address")
        } catch (e: ConnectFailure) {
            assertFalse(e.unsupported)
        }
    }

    @Test fun `no PC tells the person what to run, with their server`() = runBlocking {
        web.start()
        val api = ServerConnect(web.url("/").toString())
        web.enqueue(json(409, """{"error":"no_pc"}"""))
        try {
            api.ask("Pixel 8")
            fail("no PC")
        } catch (e: ConnectFailure) {
            assertEquals(ServerConnect.noPc(api.host), e.message)
            assertTrue(e.message!!.endsWith("relay cloud connect ${web.hostName}:${web.port}"))
        }
    }

    private fun json(status: Int, body: String) =
        MockResponse().setResponseCode(status).setHeader("Content-Type", "application/json").setBody(body)
}
