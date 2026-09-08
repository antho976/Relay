package com.forge.app

import java.io.File
import org.junit.Assert.*
import org.junit.Assume.assumeFalse
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class RestoreRecoveryTest {
    @get:Rule val temp = TemporaryFolder()
    private fun put(file: File, text: String) {
        file.parentFile?.mkdirs()
        file.writeText(text)
    }
    private fun fixture(): Pair<File, File> = temp.newFolder("files") to File(temp.newFolder("db"), "forge.db")

    @Test fun `rename replay completes WAL cleanup and preserves original frames for revert`() {
        val (files, db) = fixture()
        put(db, "incoming")
        put(File(db.path + ".prerestore"), "original")
        put(File(db.path + "-wal"), "original-committed-frames")
        put(File(db.path + "-shm"), "original-index")
        put(File(files, "pending_restore_set"), "db")
        put(File(files, "pending_restore_journal"), "db")
        assertTrue(RestoreApply.apply(files, db))
        assertFalse(File(db.path + "-wal").exists())
        assertFalse(File(db.path + "-shm").exists())
        assertTrue(RestoreApply.revert(files, db))
        assertEquals("original", db.readText())
        assertEquals("original-committed-frames", File(db.path + "-wal").readText())
    }

    @Test fun `boot resumes rollback direction without confirming a mixed installation`() {
        val (files, db) = fixture()
        val prefs = File(files, "datastore/forge_settings.preferences_pb")
        // Rollback already restored the database; its immutable snapshot remains until settled.
        put(db, "original")
        put(File(db.path + ".prerestore"), "original")
        put(prefs, "incoming-prefs")
        put(File(prefs.path + ".prerestore"), "original-prefs")
        put(File(files, "pending_restore_set"), "db\nprefs")
        put(File(files, "pending_restore_recovery"), "revert\ndb\nprefs")
        assertFalse(RestoreApply.apply(files, db))
        assertEquals("original", db.readText())
        assertEquals("original-prefs", prefs.readText())
        assertFalse(File(files, "pending_restore_set").exists())
        assertFalse(File(files, "pending_restore_recovery").exists())
    }

    @Test fun `failed rollback retains snapshots and restarts safely`() {
        val (files, db) = fixture()
        val prefs = File(files, "datastore/forge_settings.preferences_pb")
        put(db, "original")
        put(prefs, "original-prefs")
        put(File(files, "pending_restore.db"), "incoming")
        put(File(files, "pending_restore_prefs.pb"), "incoming-prefs")
        assertTrue(RestoreManifest.publish(files))
        assertTrue(RestoreApply.apply(files, db))
        val dir = prefs.parentFile!!
        dir.setWritable(false)
        try {
            val bypass = runCatching { File(dir, "probe").createNewFile() }.getOrDefault(false)
            assumeFalse("requires enforced filesystem permissions", bypass)
            assertFalse(RestoreApply.revert(files, db))
            assertEquals("original", db.readText())
            assertTrue(File(db.path + ".prerestore").exists())
            assertTrue(File(prefs.path + ".prerestore").exists())
            assertTrue(File(files, "pending_restore_recovery").exists())
        } finally { dir.setWritable(true) }
        assertFalse(RestoreApply.apply(files, db))
        assertEquals("original", db.readText())
        assertEquals("original-prefs", prefs.readText())
        assertFalse(File(files, "pending_restore_recovery").exists())
    }

    @Test fun `interrupted confirmation only resumes cleanup`() {
        val (files, db) = fixture()
        put(db, "validated-incoming")
        put(File(files, "pending_restore_set"), "db\nprefs")
        put(File(files, "pending_restore_recovery"), "settled\ndb\nprefs")
        put(File(files, "datastore/forge_settings.preferences_pb.prerestore"), "old-prefs")
        assertFalse(RestoreApply.apply(files, db))
        assertEquals("validated-incoming", db.readText())
        assertFalse(File(files, "pending_restore_set").exists())
        assertFalse(File(files, "datastore/forge_settings.preferences_pb.prerestore").exists())
    }
}
