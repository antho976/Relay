package com.tally.link

import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The handshake and the pairing link, against what crates/relay-remote writes and expects. */
class WireTest {

    @Test fun theProofIsSha256OfChallengeColonTokenInLowercaseHex() {
        // printf 'c1:t1' | sha256sum, the digest wire.rs's proof("c1", "t1") computes.
        assertEquals("00844fe554f30ed60c079f308be1e44ab0159f9c3070d55f857e9004f01c6345", RelayWire.proof("c1", "t1"))
        assertFalse(RelayWire.proof("c2", "t1") == RelayWire.proof("c1", "t1"))
    }

    @Test fun helloCarriesACodeOnceAndAProofAfter() {
        val pair = RelayWire.parse(RelayWire.pairHello(" ABCD-EFGH ", "Tally on Pixel"))!!
        assertEquals("ABCD-EFGH", pair["pair"]!!.jsonPrimitive.content)
        assertEquals("Tally on Pixel", pair["device_name"]!!.jsonPrimitive.content)
        assertEquals(1, pair["v"]!!.jsonPrimitive.content.toInt())

        val proof = RelayWire.parse(RelayWire.proofHello("dev1", "c1", "t1"))!!
        assertEquals("dev1", proof["device"]!!.jsonPrimitive.content)
        assertEquals(RelayWire.proof("c1", "t1"), proof["proof"]!!.jsonPrimitive.content)
        assertNull("The token itself never goes out", proof["token"])
    }

    @Test fun aRequestIsTheUsersAndItsResponseIsFoundById() {
        val line = RelayWire.request("money.sync", buildJsonObject { put("since", 0) }, id = "id-1")
        val o = RelayWire.parse(line)!!
        assertEquals("user", o["actor"]!!.jsonPrimitive.content)
        assertEquals("money.sync", o["op"]!!.jsonPrimitive.content)
        assertFalse("A request is one line", '\n' in line)

        val ok = """{"v":1,"id":"id-1","ok":true,"result":{"cursor":3}}"""
        assertNull("Another request's answer is not this one's", RelayWire.response(ok, "id-2"))
        assertNull("An event is not an answer", RelayWire.response("""{"v":1,"ev":"money.changed","payload":{}}""", "id-1"))
        assertTrue(RelayWire.response(ok, "id-1")!!.ok)
        val refused = RelayWire.response("""{"v":1,"id":"id-1","ok":false,"error":{"code":"remote.op","message":"no"}}""", "id-1")!!
        assertEquals("remote.op", refused.errorCode)
    }

    @Test fun theGreetingAndTheWelcomeRead() {
        val g = RelayWire.greeting("""{"v":1,"relay":"remote","host":"antho desktop","host_id":"h","instance":"dev","version":"0.4","challenge":"abc"}""")!!
        assertEquals("antho desktop", g.host)
        assertEquals("abc", g.challenge)
        assertNull(g.refusal)
        assertEquals("host.offline", RelayWire.greeting("""{"ok":false,"error":"host.offline"}""")!!.refusal)
        assertNull("Not Relay", RelayWire.greeting("""{"hello":"world"}"""))

        val w = RelayWire.welcome("""{"v":1,"ok":true,"device":"d1","token":"t"}""")!!
        assertEquals("t", w.token)
        assertEquals("pair.invalid", RelayWire.welcome("""{"v":1,"ok":false,"error":"pair.invalid"}""")!!.error)
    }

    @Test fun thePairLinkCarriesTheCodeAndEveryRouteDirectFirst() {
        // As pairlink.rs's own test builds it.
        val url = "relay://pair?v=1&code=ABCD-EFGH&host=antho%20desktop&id=abc&instance=dev" +
            "&direct=ws%3A%2F%2F192.168.1.20%3A7420%2Cws%3A%2F%2F100.64.0.2%3A7420&via=wss%3A%2F%2Frelay.example.org%2Fjoin%2Froom"
        val link = PairLink.parse("  $url\n")!!
        assertEquals("ABCD-EFGH", link.code)
        assertEquals("antho desktop", link.host)
        assertEquals("dev", link.instance)
        assertEquals(
            listOf("ws://192.168.1.20:7420", "ws://100.64.0.2:7420", "wss://relay.example.org/join/room"),
            link.routes,
        )
        assertNull("Another app's link", PairLink.parse("https://example.org/pair?code=X"))
        assertNull("No route", PairLink.parse("relay://pair?v=1&code=X&host=h"))
        assertNull("A newer link version", PairLink.parse("relay://pair?v=2&code=X&direct=ws%3A%2F%2Fa%3A1"))
    }

    @Test fun anAddressTypedByHandGetsTheDoorsPort() {
        assertEquals("ws://192.168.1.20:7420", PairLink.address("192.168.1.20"))
        assertEquals("ws://192.168.1.20:9000", PairLink.address("192.168.1.20:9000"))
        assertEquals("ws://pc.tail.ts.net:7420", PairLink.address(" pc.tail.ts.net "))
        assertEquals("wss://relay.example.org/join/r", PairLink.address("https://relay.example.org/join/r/"))
        assertEquals("ws://10.0.0.2:7420", PairLink.address("ws://10.0.0.2:7420"))
        assertNull(PairLink.address("not an address"))
        assertNull(PairLink.address(""))
        assertNotNull(PairLink.manual("192.168.1.20", "abcd-efgh"))
        assertNull("A code is needed", PairLink.manual("192.168.1.20", " - "))
    }
}
