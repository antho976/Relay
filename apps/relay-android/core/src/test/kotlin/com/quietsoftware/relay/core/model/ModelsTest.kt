package com.quietsoftware.relay.core.model

import com.quietsoftware.relay.core.sync.Kind
import com.quietsoftware.relay.core.sync.Row
import com.quietsoftware.relay.core.wire.Wire
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ModelsTest {
    private fun row(kind: Kind, json: String) = Row.of(kind, Wire.parse(json)!!)!!

    @Test
    fun `numeric ids and temp ids both decode`() {
        val real = row(Kind.Task, """{"id":42,"project_id":1,"title":"T","column":"ready","rollup":{"total":2,"done":1},"future":true}""").decode<Task>()!!
        assertEquals("42", real.id)
        assertEquals(42L, real.num)
        assertEquals("#42", real.ref)
        val temp = row(Kind.Task, """{"id":"tmp:abc","title":"New","column":null}""").decode<Task>()!!
        assertNull(temp.num)
        assertEquals("backlog", temp.column)
    }

    @Test
    fun `thread messages read text from either shape`() {
        val user = Wire.parse("""{"id":1,"thread_id":2,"role":"user","body":{"text":"hi"}}""")!!.decodeAs<ThreadMessage>()!!
        assertEquals("hi", user.text)
        val reply = Wire.parse("""{"id":2,"thread_id":2,"role":"assistant","body":{"blocks":[{"type":"text","text":"a"},{"type":"tool_use","id":"t","name":"money_summary","input":{}},{"type":"text","text":"b"}]}}""")!!.decodeAs<ThreadMessage>()!!
        assertEquals("a\n\nb", reply.text)
        assertEquals(listOf("money_summary"), reply.tools)
    }

    @Test
    fun `sessions say which lamp they light`() {
        val s = row(Kind.Session, """{"id":3,"name":"brisk-otter","project_id":1,"state":"blocked"}""").decode<Session>()!!
        assertEquals(Lamp.Held, s.lamp)
        assertEquals("NEEDS YOU", s.stateLabel)
    }

    @Test
    fun `working and blocked are one definition each`() {
        fun session(state: String, provider: String = "claude") = Session(name = "a", state = state, provider = provider)
        assertEquals(listOf("running", "spawning"), listOf("running", "spawning", "idle", "blocked", "created").filter { session(it).working })
        assertEquals(true, session("blocked").blocked)
        // The owner's own shell is never at work on a turn.
        assertEquals(false, session("running", provider = "shell").working)
    }

    @Test
    fun `a title is the first line with words, without its heading marks`() {
        assertEquals("Groceries", titleOf("\n  \n# Groceries\nmilk", 60))
        assertEquals("Plan", titleOf("## Plan", 60))
        assertEquals("abc", titleOf("abcdef", 3))
        assertNull(titleOf("\n \n", 60))
        assertNull(titleOf(null, 60))
    }
}
