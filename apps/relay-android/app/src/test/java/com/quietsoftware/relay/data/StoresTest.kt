package com.quietsoftware.relay.data

import android.content.Context
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import com.quietsoftware.relay.core.sync.Kind
import com.quietsoftware.relay.core.sync.Ledger
import com.quietsoftware.relay.core.sync.OutboxEntry
import com.quietsoftware.relay.core.sync.Row
import com.quietsoftware.relay.core.sync.Scope
import com.quietsoftware.relay.core.wire.BusError
import com.quietsoftware.relay.core.wire.Wire
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.data.db.RelayDb
import com.quietsoftware.relay.data.db.RoomOutbox
import com.quietsoftware.relay.data.db.RoomReplica
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** The replica and outbox on a real SQLite: the scopes a snapshot replaces, and the outbox's order. */
@RunWith(RobolectricTestRunner::class)
class StoresTest {
    private val db = Room.inMemoryDatabaseBuilder(ApplicationProvider.getApplicationContext<Context>(), RelayDb::class.java)
        .allowMainThreadQueries()
        .build()
    private val replica = RoomReplica(db)
    private val outbox = RoomOutbox(db)

    @After
    fun close() = db.close()

    private fun task(id: Long, project: Long, title: String) =
        Row.of(Kind.Task, Wire.parse("""{"id":$id,"project_id":$project,"title":"$title","column":"backlog"}""")!!)!!

    @Test
    fun `a snapshot replaces its own project's rows and no other's`() = runBlocking {
        replica.upsert(listOf(task(1, 1, "a"), task(2, 1, "b"), task(3, 2, "c")))
        replica.replace(Kind.Task, Scope(projectId = 1), listOf(task(4, 1, "d")))
        assertEquals(listOf("3", "4"), replica.all(Kind.Task).map { it.id }.sorted())
        replica.replace(Kind.Task, Scope.All, listOf(task(5, 3, "e")))
        assertEquals(listOf("5"), replica.all(Kind.Task).map { it.id })
    }

    @Test
    fun `rows keep the PC's version beside what the phone shows`() = runBlocking {
        val ledger = Ledger(replica, outbox) { 1_760_000_000_000 }
        ledger.upsert(listOf(task(7, 1, "old")))
        ledger.enqueue("task.update", Wire.parse("""{"task_id":7,"title":"new"}""")!!, "Rename")
        val row = replica.get(Kind.Task, "7")!!
        assertEquals("new", row.json.s("title"))
        assertEquals("old", row.base!!.s("title"))
        assertTrue(row.pending)
    }

    @Test
    fun `a finished entry an open one still waits on is not pruned`() = runBlocking {
        val create = OutboxEntry("4b1c2d3e-0000-4000-8000-000000000001", outbox.nextSeq(), "task.create", Wire.parse("""{"project_id":1,"title":"x"}""")!!, "New", 1, state = OutboxEntry.State.Done, result = Wire.parse("""{"id":42}"""))
        val old = OutboxEntry("4b1c2d3e-0000-4000-8000-000000000002", outbox.nextSeq(), "task.move", Wire.parse("""{"task_id":7,"column":"done"}""")!!, "Move", 1, state = OutboxEntry.State.Done)
        val label = OutboxEntry("4b1c2d3e-0000-4000-8000-000000000003", outbox.nextSeq(), "task.label.add", Wire.parse("""{"task_id":{"${'$'}ref":"${create.id}","path":"id"},"label":"phone"}""")!!, "Label", 2)
        listOf(create, old, label).forEach { outbox.add(it) }
        outbox.prune(before = 10)
        assertEquals("still named by the label", OutboxEntry.State.Done, outbox.get(create.id)?.state)
        assertNull(outbox.get(old.id))
        outbox.delete(label.id)
        outbox.prune(before = 10)
        assertNull(outbox.get(create.id))
    }

    @Test
    fun `the outbox keeps order, state and the PC's refusal across a reopen`() = runBlocking {
        val first = OutboxEntry("a", outbox.nextSeq(), "task.move", Wire.parse("""{"task_id":1,"column":"done"}""")!!, "Move", 1)
        outbox.add(first)
        val second = OutboxEntry("b", outbox.nextSeq(), "notes.create", Wire.parse("""{"project_id":1,"body":"x"}""")!!, "Note", 2)
        outbox.add(second)
        outbox.update(first.copy(state = OutboxEntry.State.Conflict, error = BusError("conflict", "task.edit_conflict", "changed")))
        val open = outbox.open()
        assertEquals(listOf("a", "b"), open.map { it.id })
        assertEquals("task.edit_conflict", open[0].error!!.code)
        outbox.update(second.copy(state = OutboxEntry.State.Done))
        assertEquals(listOf("a"), outbox.open().map { it.id })
        outbox.delete("a")
        assertNull(outbox.get("a"))
    }
}
