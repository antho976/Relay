package com.tally.link

import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.longOrNull
import kotlinx.serialization.json.put
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test

/**
 * The whole way in against a real server: ask, wait for the owner's Allow, collect the link, pair
 * through the rendezvous, sync, and come back with a proof. Skipped unless TALLY_E2E_SERVER names
 * the bot (`http://127.0.0.1:3901`); TALLY_E2E_RENDEZVOUS replaces the link's public address with
 * a local one (`ws://127.0.0.1:7430`) for a server run on this machine. Someone has to tap Allow
 * (or mark the request allowed in the bot's database) within two minutes.
 */
class CloudLiveTest {

    @Test fun aPhoneConnectsSyncsAndComesBack() = runBlocking {
        val server = System.getenv("TALLY_E2E_SERVER").orEmpty()
        assumeTrue("TALLY_E2E_SERVER is not set", server.isNotBlank())
        val api = CloudApi()
        assertTrue("the server runs Relay's cloud copy", api.hello(server).cloud)
        val enrollment = api.enroll(server, "Tally on the JVM")
        println("enrolled ${enrollment.id}; allow code ${enrollment.code}")

        var link: PairLink? = null
        val until = System.currentTimeMillis() + 120_000
        while (link == null && System.currentTimeMillis() < until) {
            when (val reply = api.claim(enrollment)) {
                is ClaimReply.Ready -> link = reply.link
                ClaimReply.Waiting -> delay(1_000)
                else -> error("the request ended: $reply")
            }
        }
        val pairing = checkNotNull(link) { "nobody allowed the request in two minutes" }
        val local = System.getenv("TALLY_E2E_RENDEZVOUS").orEmpty()
        val routes = if (local.isBlank()) pairing.routes else pairing.routes.map { local + it.substring(it.indexOf("/join/")) }

        val client = RelayClient()
        val first = client.connect(routes, { RelayWire.pairHello(pairing.code, "Tally on the JVM") }, RelayClient.PAIR_WELCOME_TIMEOUT_MS, oneShot = true)
        val device = checkNotNull(first.welcome.device)
        val token = checkNotNull(first.welcome.token)
        val now = System.currentTimeMillis()
        val change = buildJsonObject {
            put("table", "accounts")
            put("uid", "live-${now}")
            put("updated_at", now)
            put("deleted", false)
            put("row", buildJsonObject {
                put("name", "Live test")
                put("type", "CASH")
                put("openingBalance", 0)
                put("archived", false)
                put("sortOrder", 99)
            })
        }
        val merged = first.use {
            it.request("money.sync", buildJsonObject {
                put("device", "Tally on the JVM")
                put("replace", false)
                put("since", 0)
                put("changes", JsonArray(listOf(change)))
            }) as JsonObject
        }
        val cursor = (merged["cursor"] as JsonPrimitive).longOrNull!!
        assertEquals(1L, (merged["applied"] as JsonPrimitive).longOrNull)

        val again = client.connect(routes, { g -> RelayWire.proofHello(device, g.challenge, token) })
        again.use {
            val next = it.request("money.sync", buildJsonObject {
                put("device", "Tally on the JVM")
                put("replace", false)
                put("since", cursor)
                put("generation", (merged["generation"] as JsonPrimitive).content)
                put("changes", JsonArray(emptyList()))
            }) as JsonObject
            assertTrue((next["cursor"] as JsonPrimitive).longOrNull!! >= cursor)
        }
    }
}
