package com.forge.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

/**
 * A restore lands as one set, or it does not land.
 *
 * The old path copied and renamed each component into place in turn, set a flag if one failed, and
 * carried on. Since the database is the anchor the other three describe, a boot could end with a
 * database from the backup and preferences, photos and an avatar from before it. Worse, the failed
 * component was retried on a LATER boot, so a setting changed in between was silently overwritten
 * when the staged preferences finally landed. The comments called the set atomic; it was not.
 *
 * These drive real files in a temp directory, because the guarantee is about file operations.
 */
class RestoreApplyTest {

    @get:Rule val tmp = TemporaryFolder()

    private lateinit var filesDir: File
    private lateinit var liveDb: File

    private fun setUpDirs() {
        filesDir = tmp.newFolder("files")
        liveDb = File(tmp.newFolder("databases"), "forge.db")
    }

    private fun write(f: File, text: String) = f.apply { parentFile?.mkdirs() }.writeText(text)

    private fun livePrefs() = File(filesDir, "datastore/forge_settings.preferences_pb")
    private fun liveAvatar() = File(filesDir, "avatar.jpg")
    private fun livePhotos() = File(filesDir, "progress_photos")

    /** Every component staged, plus a full set of pre-restore live files. */
    private fun stageEverything() {
        write(liveDb, "live-db")
        write(livePrefs(), "live-prefs")
        write(liveAvatar(), "live-avatar")
        write(File(livePhotos(), "pp_old.jpg"), "live-photo")

        write(File(filesDir, "pending_restore.db"), "restored-db")
        write(File(filesDir, "pending_restore_prefs.pb"), "restored-prefs")
        write(File(filesDir, "pending_restore_avatar.jpg"), "restored-avatar")
        write(File(filesDir, "pending_restore_photos/pp_new.jpg"), "restored-photo")
    }

    /** As [stageEverything], minus the live preferences file — see [blockThePreferencesPath]. */
    private fun stageEverythingExceptLivePrefs() {
        write(liveDb, "live-db")
        write(liveAvatar(), "live-avatar")
        write(File(livePhotos(), "pp_old.jpg"), "live-photo")

        write(File(filesDir, "pending_restore.db"), "restored-db")
        write(File(filesDir, "pending_restore_prefs.pb"), "restored-prefs")
        write(File(filesDir, "pending_restore_avatar.jpg"), "restored-avatar")
        write(File(filesDir, "pending_restore_photos/pp_new.jpg"), "restored-photo")
    }

    /**
     * Make the preferences destination impossible to write: `datastore` is a regular FILE, so the
     * directory the staged copy needs cannot exist. A stand-in for the real causes — a full disk, a
     * file another process holds open, a cross-filesystem move — chosen because it fails the same
     * way on every machine rather than depending on permissions or the user the tests run as.
     */
    private fun blockThePreferencesPath() {
        write(File(filesDir, "datastore"), "not a directory")
    }

    // ── The whole set lands together ──────────────────────────────────────────

    @Test
    fun `a complete restore swaps every component and reports success`() {
        setUpDirs()
        stageEverything()

        assertTrue(RestoreApply.apply(filesDir, liveDb))

        assertEquals("restored-db", liveDb.readText())
        assertEquals("restored-prefs", livePrefs().readText())
        assertEquals("restored-avatar", liveAvatar().readText())
        assertEquals("restored-photo", File(livePhotos(), "pp_new.jpg").readText())
        assertFalse("the old photo is gone with its folder", File(livePhotos(), "pp_old.jpg").exists())

        // Nothing left staged, so the next boot does no work.
        assertFalse(File(filesDir, "pending_restore.db").exists())
        assertFalse(File(filesDir, "pending_restore_prefs.pb").exists())
        assertFalse(File(filesDir, "pending_restore_avatar.jpg").exists())
        assertFalse(File(filesDir, "pending_restore_photos").exists())
    }

    @Test
    fun `nothing staged is not a restore`() {
        setUpDirs()
        write(liveDb, "live-db")
        assertFalse(RestoreApply.apply(filesDir, liveDb))
        assertEquals("live-db", liveDb.readText())
    }

    // ── A component that cannot be staged aborts the whole set ────────────────

    /**
     * The case the old code got wrong. The preferences cannot be placed, so under the old path the
     * database swapped anyway and the user ran a whole process on a restored database with their
     * pre-restore preferences — then had any settings they touched overwritten when the retry
     * finally landed.
     *
     * A directory where the staged file needs to be is a stand-in for the real causes (a full disk,
     * a file another process holds, a cross-filesystem move): whatever the reason, the bytes cannot
     * be put in place.
     */
    @Test
    fun `a component that cannot be staged leaves every other component untouched`() {
        setUpDirs()
        stageEverythingExceptLivePrefs()
        blockThePreferencesPath()

        assertFalse("a partial restore is not a success", RestoreApply.apply(filesDir, liveDb))

        assertEquals("the database must not have swapped", "live-db", liveDb.readText())
        assertEquals("nor the avatar", "live-avatar", liveAvatar().readText())
        assertEquals("nor the photos", "live-photo", File(livePhotos(), "pp_old.jpg").readText())
    }

    /**
     * And the abandoned set has to be RETRYABLE. Staging renames the pending files away, so leaving
     * them under their `.restoring` names would lose the restore entirely — the next boot looks for
     * `pending_restore.db` and would find nothing.
     */
    @Test
    fun `an aborted restore is put back where the next boot looks for it`() {
        setUpDirs()
        stageEverythingExceptLivePrefs()
        blockThePreferencesPath()

        RestoreApply.apply(filesDir, liveDb)

        assertEquals("restored-db", File(filesDir, "pending_restore.db").readText())
        assertEquals("restored-avatar", File(filesDir, "pending_restore_avatar.jpg").readText())
        assertTrue(File(filesDir, "pending_restore_photos").isDirectory)

        // Clear the blockage; the retry now completes the whole set.
        File(filesDir, "datastore").delete()
        assertTrue("the retry converges", RestoreApply.apply(filesDir, liveDb))
        assertEquals("restored-db", liveDb.readText())
        assertEquals("restored-prefs", livePrefs().readText())
        assertEquals("restored-avatar", liveAvatar().readText())
        assertEquals("restored-photo", File(livePhotos(), "pp_new.jpg").readText())
    }

    // ── Partial sets are legitimate; mixed ones are not ───────────────────────

    @Test
    fun `a restore with no avatar clears a live one so it cannot outlive the restored prefs`() {
        setUpDirs()
        write(liveDb, "live-db")
        write(liveAvatar(), "live-avatar")
        write(File(filesDir, "pending_restore.db"), "restored-db")
        write(File(filesDir, "pending_restore_prefs.pb"), "restored-prefs")

        assertTrue(RestoreApply.apply(filesDir, liveDb))
        assertFalse("a seeded cover must not outlive a blank avatarDefaultId", liveAvatar().exists())
    }

    @Test
    fun `stale WAL sidecars do not survive the swapped database`() {
        setUpDirs()
        write(liveDb, "live-db")
        write(File(liveDb.path + "-wal"), "stale-wal")
        write(File(liveDb.path + "-shm"), "stale-shm")
        write(File(filesDir, "pending_restore.db"), "restored-db")

        assertTrue(RestoreApply.apply(filesDir, liveDb))
        assertEquals("restored-db", liveDb.readText())
        assertFalse("SQLite must not replay old frames over the restored file", File(liveDb.path + "-wal").exists())
        assertFalse(File(liveDb.path + "-shm").exists())
    }

    // ── The primitives that carry the guarantee ───────────────────────────────

    @Test
    fun `staging leaves the live file exactly as it was`() {
        setUpDirs()
        write(liveDb, "live-db")
        val pending = File(filesDir, "pending_restore.db").also { write(it, "restored-db") }

        val staged = RestoreApply.stageBeside(pending, liveDb)

        assertTrue(staged!!.exists())
        assertEquals("live-db", liveDb.readText())
        assertEquals("restored-db", staged.readText())
    }

    @Test
    fun `unstaging returns the bytes to the pending name`() {
        setUpDirs()
        val pending = File(filesDir, "pending_restore.db").also { write(it, "restored-db") }
        val staged = RestoreApply.stageBeside(pending, liveDb)!!
        assertFalse("staging moved it away", pending.exists())

        RestoreApply.unstage(staged, pending)

        assertEquals("restored-db", pending.readText())
        assertFalse(staged.exists())
    }
}
