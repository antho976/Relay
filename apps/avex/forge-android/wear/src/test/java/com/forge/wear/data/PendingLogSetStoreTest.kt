package com.forge.wear.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

/**
 * W01: a Log set's identity has to outlive the screen and the process that sent it.
 *
 * It lived in SetView's `remember`, so a recreated screen or a reclaimed process minted a fresh id
 * on the re-tap, and a set that had in fact landed was logged a second time.
 */
class PendingLogSetStoreTest {

    @get:Rule val tmp = TemporaryFolder()

    private fun store() = PendingLogSetStore(File(tmp.root, PendingLogSetStore.FILE_NAME))

    private val pending = PendingLogSet(
        commandId = "cmd-1", sessionId = 12L, setKey = "bench:2:100", payload = "102.5|8|false"
    )

    @Test
    fun `a pending log written by one process is read back by the next`() {
        store().save(pending)
        assertEquals(pending, store().load())
    }

    @Test
    fun `clearing retires it`() {
        store().save(pending)
        store().clear()
        assertNull(store().load())
    }

    @Test
    fun `a newer log supersedes the one before it`() {
        store().save(pending)
        val next = pending.copy(commandId = "cmd-2", setKey = "bench:3:100")
        store().save(next)
        assertEquals(next, store().load())
    }

    @Test
    fun `a record that cannot be parsed reads as none`() {
        File(tmp.root, PendingLogSetStore.FILE_NAME).writeText("v1\ncmd-1")
        assertNull(store().load())
    }

    @Test
    fun `a key with a line break is never written, so it can never be misread`() {
        store().save(pending.copy(setKey = "bench\n2"))
        assertNull(store().load())
    }
}
