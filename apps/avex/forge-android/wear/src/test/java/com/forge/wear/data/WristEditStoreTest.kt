package com.forge.wear.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

/**
 * M-10 / H-08: the pending wrist edit has to outlive the process holding it.
 *
 * `_failedSend`, `_lastLog`, the command id and the payload were all heap state on a singleton, and
 * Wear reclaims background processes hardest during the exact window the retry exists for: the
 * watch out of Bluetooth range, waiting for the user to walk back into it. The edit and its only
 * affordance disappeared together, on the one screen with nowhere to look it up.
 *
 * Real files, because the guarantee is about a file surviving a process that does not.
 */
class WristEditStoreTest {

    @get:Rule val tmp = TemporaryFolder()

    private fun store(dir: File = tmp.root) = WristEditStore(File(dir, WristEditStore.FILE_NAME))

    private val edit = WristEdit(WristEdit.Kind.RPE, "cmd-1", sessionId = 0L, setId = 42L, rpe = 8.5)

    @Test
    fun `an edit written by one process is read back by the next`() {
        store().save(edit)
        // A DIFFERENT instance over the same file: the process that wrote it is gone.
        assertEquals(edit, store().load())
    }

    @Test
    fun `nothing pending reads as nothing, not as a failure`() {
        assertNull(store().load())
    }

    @Test
    fun `only a positive acknowledgement retires the record`() {
        store().save(edit)
        store().clear()
        assertNull(store().load())
        // Clearing what is already clear is not an error — an ack can arrive after a dismissal.
        store().clear()
        assertNull(store().load())
    }

    @Test
    fun `a newer edit supersedes the one before it`() {
        store().save(edit)
        val newer = WristEdit(WristEdit.Kind.UNDO, "cmd-2", sessionId = 7L, setId = 99L, rpe = null)
        store().save(newer)
        assertEquals("the wrist offers one row at a time, so the record holds one edit", newer, store().load())
    }

    @Test
    fun `a record that cannot be parsed loses no more than having none`() {
        File(tmp.root, WristEditStore.FILE_NAME).writeText("half a line, from a write that did not finish")
        assertNull(store().load())
    }

    @Test
    fun `a store that cannot be written leaves the caller working`() {
        // The scratch path is a directory, so the write throws where a full disk would fail.
        val dir = tmp.newFolder("blocked")
        File(dir, WristEditStore.FILE_NAME + ".tmp").mkdirs()
        store(dir).save(edit)
        assertNull("nothing durable, and nothing thrown at the send path", store(dir).load())
    }
}
