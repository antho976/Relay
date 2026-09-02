package com.forge.app.service.wear

import com.forge.app.core.time.Clock
import com.forge.shared.protocol.CmdAckDto
import java.io.File
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/**
 * The durable half of wrist-command idempotency (audit H-08). The old deduper's tests said "an id
 * runs once"; these say what that promise is worth across an exception, a process death and a
 * lost ack — the three ways the wrist ended up with a duplicate set or a permanent "Not logged".
 */
class WearCommandLedgerTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private val clock = Clock { 1_000L }

    private fun ledgerFile() = File(temporaryFolder.root, "ledger.json")

    /** A fresh instance over the same file stands in for a new phone process. */
    private fun ledger(file: File = ledgerFile()) = WearCommandLedger(file, clock)

    private fun ack(id: String, ok: Boolean = true) = CmdAckDto(
        commandId = id, ok = ok, setId = 42L, atMs = 1_000L, kind = CmdAckDto.KIND_LOG_SET
    )

    /** Records what a run published and how often its effect actually executed. */
    private class Probe {
        var executions = 0
        val published = mutableListOf<CmdAckDto>()
        val publish: suspend (CmdAckDto) -> Unit = { published += it }
    }

    @Test
    fun `a command runs once and a duplicate replays its ack without running again`() = runTest {
        val ledger = ledger()
        val probe = Probe()

        ledger.run("a", probe.publish) { probe.executions++; ack("a") }
        ledger.run("a", probe.publish) { probe.executions++; ack("a", ok = false) }

        assertEquals(1, probe.executions)
        // Both deliveries were answered, and with the SAME ack — the second is a replay.
        assertEquals(listOf(ack("a"), ack("a")), probe.published)
        assertEquals(WearCommandLedger.Status.DONE, ledger.status("a"))
    }

    @Test
    fun `distinct ids are independent`() = runTest {
        val ledger = ledger()
        val probe = Probe()

        ledger.run("a", probe.publish) { probe.executions++; ack("a") }
        ledger.run("b", probe.publish) { probe.executions++; ack("b") }

        assertEquals(2, probe.executions)
        assertEquals(listOf(ack("a"), ack("b")), probe.published)
    }

    @Test
    fun `a retry after process death replays the recorded ack instead of logging again`() = runTest {
        val first = Probe()
        ledger().run("a", first.publish) { first.executions++; ack("a") }

        // Reproduce A from the audit: the process died after the write, the watch resends.
        val retry = Probe()
        ledger().run("a", retry.publish) { retry.executions++; ack("a", ok = false) }

        assertEquals(0, retry.executions)
        assertEquals(listOf(ack("a")), retry.published)
    }

    @Test
    fun `the ack is recorded before it is published`() = runTest {
        val ledger = ledger()
        var statusInsideEffect: WearCommandLedger.Status? = null
        var statusAtPublish: WearCommandLedger.Status? = null
        var ackOnDiskAtPublish: CmdAckDto? = null

        ledger.run("a", publish = {
            statusAtPublish = ledger.status("a")
            // A brand-new instance reads the FILE, not this process's memory.
            ackOnDiskAtPublish = ledger().recordedAck("a")
        }) {
            statusInsideEffect = ledger.status("a")
            ack("a")
        }

        assertEquals(WearCommandLedger.Status.IN_FLIGHT, statusInsideEffect)
        assertEquals(WearCommandLedger.Status.DONE, statusAtPublish)
        // Reproduce B: a lost ack put after the mutation is recoverable, because the record
        // already exists when publishing starts.
        assertEquals(ack("a"), ackOnDiskAtPublish)
    }

    @Test
    fun `an exception during execution does not suppress the retry`() = runTest {
        val ledger = ledger()
        val probe = Probe()

        try {
            ledger.run("a", probe.publish) { probe.executions++; error("room write failed") }
            fail("expected the effect's exception to propagate")
        } catch (e: IllegalStateException) {
            assertEquals("room write failed", e.message)
        }
        assertTrue(probe.published.isEmpty())
        assertEquals(WearCommandLedger.Status.IN_FLIGHT, ledger.status("a"))

        // Same process.
        ledger.run("a", probe.publish) { probe.executions++; ack("a") }
        assertEquals(2, probe.executions)
        assertEquals(listOf(ack("a")), probe.published)
    }

    @Test
    fun `an in-flight entry left by a previous process does not block the retry`() = runTest {
        val crashed = Probe()
        try {
            ledger().run("a", crashed.publish) { crashed.executions++; error("process died here") }
        } catch (e: IllegalStateException) {
            // The file now holds an in_flight row for "a" with no ack.
        }

        val retry = Probe()
        ledger().run("a", retry.publish) { retry.executions++; ack("a") }

        assertEquals(1, retry.executions)
        assertEquals(listOf(ack("a")), retry.published)
    }

    @Test
    fun `a duplicate delivered while the command is still running is dropped`() = runTest {
        val ledger = ledger()
        val probe = Probe()
        var nestedRan = false

        ledger.run("a", probe.publish) {
            // The same id arriving on another binder thread mid-execution.
            ledger.run("a", probe.publish) { nestedRan = true; ack("a", ok = false) }
            ack("a")
        }

        assertFalse(nestedRan)
        // The original delivery's ack covers both; nothing was published for the duplicate.
        assertEquals(listOf(ack("a")), probe.published)
    }

    @Test
    fun `the ledger keeps only the newest entries`() = runTest {
        val ledger = ledger()
        val probe = Probe()
        val total = WearCommandLedger.MAX_ENTRIES + 50
        repeat(total) { ledger.run("cmd-$it", probe.publish) { ack("cmd-$it") } }

        // Read back from disk, so the bound is what was PERSISTED, not just what was held.
        val reloaded = ledger()
        assertNull(reloaded.status("cmd-0"))
        assertNull(reloaded.status("cmd-49"))
        assertEquals(WearCommandLedger.Status.DONE, reloaded.status("cmd-50"))
        assertEquals(WearCommandLedger.Status.DONE, reloaded.status("cmd-${total - 1}"))
    }

    @Test
    fun `a corrupt ledger file starts empty and is replaced by a good one`() = runTest {
        ledgerFile().writeText("{broken")
        val probe = Probe()

        ledger().run("a", probe.publish) { probe.executions++; ack("a") }

        assertEquals(1, probe.executions)
        assertEquals(ack("a"), ledger().recordedAck("a"))
    }

    @Test
    fun `writes go through a temp file that is not left behind`() = runTest {
        val probe = Probe()
        ledger().run("a", probe.publish) { ack("a") }

        val names = temporaryFolder.root.list()?.toList().orEmpty()
        assertEquals(listOf("ledger.json"), names)
    }

    @Test
    fun `a ledger in a directory that does not exist yet is created`() = runTest {
        val nested = File(temporaryFolder.root, "deeper/still/ledger.json")
        val probe = Probe()

        ledger(nested).run("a", probe.publish) { ack("a") }

        assertTrue(nested.exists())
        assertEquals(ack("a"), ledger(nested).recordedAck("a"))
    }
}
