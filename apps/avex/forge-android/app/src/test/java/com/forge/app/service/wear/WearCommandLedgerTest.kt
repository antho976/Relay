package com.forge.app.service.wear

import android.app.Application
import com.forge.app.core.time.Clock
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.db.session
import com.forge.shared.protocol.CmdAckDto
import java.io.File
import kotlinx.coroutines.*
import org.junit.After
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class, sdk = [34])
class WearCommandLedgerTest {
    @get:Rule val temp = TemporaryFolder()
    private val db = inMemoryForgeDb()
    private fun ledger() = WearCommandLedger(db, File(temp.root, "ledger.json"), Clock { 1000L })
    private fun ack(id: String) = CmdAckDto(commandId = id, ok = true, atMs = 1000L)
    @After fun close() = db.close()

    @Test fun `failure after mutation rolls back effect and outcome together`() = runBlocking {
        try {
            ledger().run("a", publish = {}) {
                db.sessionDao().insert(session(id = 12))
                error("injected failure after insert")
            }
            fail("must fail")
        } catch (_: IllegalStateException) { }
        assertNull(db.sessionDao().get(12))
        assertNull(db.wearCommandDao().get("a"))
        ledger().run("a", publish = {}) {
            db.sessionDao().insert(session(id = 12))
            ack("a")
        }
        assertNotNull(db.sessionDao().get(12))
        assertNotNull(db.wearCommandDao().get("a"))
    }

    @Test fun `lost publication replays exact ack in fresh ledger without repeating mutation or timer`() = runBlocking {
        var effects = 0
        var timers = 0
        try {
            ledger().run("a", publish = { error("transport failed") }, afterCommit = { timers++ }) {
                effects++
                db.sessionDao().insert(session(id = 14))
                ack("a")
            }
        } catch (_: IllegalStateException) { }
        var replay: CmdAckDto? = null
        ledger().run("a", publish = { replay = it }, afterCommit = { timers++ }) {
            effects++
            error("must replay")
        }
        assertEquals(ack("a"), replay)
        assertEquals(1, effects)
        assertEquals(1, timers)
    }

    @Test fun `parallel deliveries execute once and all receive ack`() = runBlocking {
        var effects = 0
        val replies = java.util.concurrent.ConcurrentLinkedQueue<CmdAckDto>()
        withContext(Dispatchers.Default) {
            (1..12).map { async {
                ledger().run("same", publish = { replies.add(it) }) {
                    effects++
                    delay(5)
                    ack("same")
                }
            } }.awaitAll()
        }
        assertEquals(1, effects)
        assertEquals(12, replies.size)
    }

    @Test fun `upgrade retains completed legacy UUIDs`() = runBlocking {
        val encoded = com.forge.shared.protocol.WearCodec.encode(ack("old")).decodeToString()
        File(temp.root, "ledger.json").writeText("""{"entries":[{"commandId":"old","ack":$encoded}]}""")
        var result: CmdAckDto? = null
        ledger().run("old", publish = { result = it }) { error("legacy command must not repeat") }
        assertEquals(ack("old"), result)
        assertNotNull(db.wearCommandDao().get("old"))
    }

    @Test fun `postcommit side effect failure preserves result and still publishes`() = runBlocking {
        var published = false
        try {
            ledger().run("a", publish = { published = true }, afterCommit = { error("timer failed") }) { ack("a") }
        } catch (_: IllegalStateException) { }
        assertTrue(published)
        ledger().run("a", publish = {}) { error("must not repeat after side effect failure") }
    }
}
