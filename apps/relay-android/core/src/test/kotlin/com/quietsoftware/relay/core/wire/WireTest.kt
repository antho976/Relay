package com.quietsoftware.relay.core.wire

import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class WireTest {
    @Test
    fun `proof is the same digest the door computes`() {
        // sha256("c1:t1"), as crates/relay-remote/src/wire.rs `proof` makes it.
        assertEquals("00844fe554f30ed60c079f308be1e44ab0159f9c3070d55f857e9004f01c6345", Wire.proof("c1", "t1"))
    }

    @Test
    fun `hellos and requests carry only the fields the PC accepts`() {
        val pair = Wire.parse(Wire.pairHello(" ABCD-EFGH ", "Pixel"))!!
        assertEquals(setOf("v", "pair", "device_name"), pair.keys)
        assertEquals("ABCD-EFGH", pair["pair"]!!.jsonPrimitive.content)
        val proof = Wire.parse(Wire.proofHello("dev", "c1", "t1"))!!
        assertEquals(setOf("v", "device", "proof"), proof.keys)
        // The envelope is deny_unknown_fields on the PC (envelope.rs).
        val req = Wire.parse(Wire.request("task.list", JsonObject(emptyMap()), "7f2b5d2e-2f1a-4b7c-9a6c-1c9d1e6a0001"))!!
        assertEquals(setOf("v", "id", "actor", "op", "payload"), req.keys)
        assertEquals("user", req["actor"]!!.jsonPrimitive.content)
    }

    @Test
    fun `a greeting is read leniently and an offline rendezvous is told apart`() {
        val g = Wire.greeting("""{"v":1,"relay":"remote","host":"desk","host_id":"h","instance":"dev","version":"4","challenge":"ab","future":1,"wake":[{"mac":"aa:bb:cc:dd:ee:ff","broadcast":"192.168.1.255"}],"engine":"stopped"}""")!!
        assertEquals("desk", g.host)
        assertEquals("ab", g.challenge)
        assertEquals("stopped", g.engine)
        assertEquals(listOf(WakeTarget("aa:bb:cc:dd:ee:ff", "192.168.1.255")), g.wake)
        assertNull(Wire.greeting("""{"v":1,"ok":false,"error":"host.offline"}"""))
        assertEquals("host.offline", Wire.offline("""{"v":1,"ok":false,"error":"host.offline"}"""))
    }

    @Test
    fun `a welcome says which door let the phone in, and an older one says nothing`() {
        val g = Wire.greeting("""{"v":1,"relay":"remote","host":"","host_id":"","instance":"stable","version":"","challenge":"ab"}""")!!
        val cloud = Wire.welcome("""{"v":1,"ok":true,"device":"d1","host":"server","ops":"cloud"}""")!!
        assertEquals(Wire.OPS_CLOUD, cloud.ops)
        assertEquals(Wire.OPS_CLOUD, g.admittedBy(cloud).ops)
        val older = Wire.welcome("""{"v":1,"ok":true,"device":"d1"}""")!!
        assertNull(older.ops)
        assertNull(g.admittedBy(older).ops)
    }

    @Test
    fun `lines are classified by their second key`() {
        assertTrue(Wire.classify("""{"v":1,"ev":"task.changed","ts":"t","actor":"user","payload":{"task_id":3}}""") is Incoming.Ev)
        assertTrue(Wire.classify("""{"v":1,"stream":"pty","session":"a","epoch":1,"seq":2,"data":"aGk="}""") is Incoming.Fr)
        val res = Wire.classify("""{"v":1,"id":"x","ok":false,"error":{"kind":"conflict","code":"task.edit_conflict","message":"m"},"replayed":true}""") as Incoming.Res
        assertEquals("task.edit_conflict", res.response.error!!.code)
        assertTrue(res.response.replayed)
        assertNull(Wire.classify("not json"))
    }

    @Test
    fun `a magic packet is six FFs and the MAC sixteen times`() {
        val p = Wake.magicPacket("AA-BB-CC-DD-EE-FF")!!
        assertEquals(102, p.size)
        assertTrue(p.take(6).all { it == 0xFF.toByte() })
        assertEquals(0xAA.toByte(), p[6])
        assertEquals(0xFF.toByte(), p[101])
        assertNull(Wake.magicPacket("nope"))
    }

    @Test
    fun `errors without a body still parse`() {
        val e = BusError.from(Wire.parse("""{"kind":"unavailable","code":"app.quitting","message":"bye"}""")!!.jsonObject)
        assertTrue(e.transient)
    }

    @Test
    fun `only the link, a quitting PC and a busy one are worth waiting for`() {
        assertTrue(BusError.link("link.down", "gone").transient)
        assertTrue(BusError.link("link.timeout", "slow").transient)
        assertTrue(BusError("unavailable", "design.render.busy", "busy").transient)
        assertTrue(BusError("unavailable", "arbiter.rate_limited", "slow down").transient)
        assertFalse(BusError("unavailable", "provider.not_installed", "no codex").transient)
        assertFalse(BusError("unavailable", "session.spawn_failed", "failed").transient)
        assertFalse(BusError("unavailable", "git.head", "no head").transient)
        assertFalse(BusError("conflict", "thread.busy", "answering").transient)
    }
}
