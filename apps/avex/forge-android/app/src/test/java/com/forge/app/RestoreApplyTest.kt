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

    // ── Interruption between commits ──────────────────────────────────────────

    private fun journal() = File(filesDir, "pending_restore_journal")
    private fun stagedPrefs() = File(filesDir, "datastore/forge_settings.preferences_pb.restoring")

    /**
     * Phase 2 is a sequence of renames, and a process death between two of them used to be
     * unrecoverable: the database was live, the preferences were not, and the next boot saw neither
     * a pending file (staging had renamed it away) nor any record that a restore was half-applied.
     * The staged preferences sat under a `.restoring` name nothing reads, and the mixed state was
     * permanent.
     *
     * Simulated exactly as a crash leaves it: the database committed, the journal still naming what
     * has not.
     */
    @Test
    fun `a boot interrupted between commits is finished by the next one`() {
        setUpDirs()
        write(liveDb, "restored-db")                       // the database already landed
        write(livePrefs(), "live-prefs")                   // the preferences did not
        write(stagedPrefs(), "restored-prefs")             // ...but they are staged
        write(journal(), "prefs")                          // and the journal says so

        assertTrue("the resumed restore completes", RestoreApply.apply(filesDir, liveDb))

        assertEquals("restored-prefs", livePrefs().readText())
        assertFalse("the journal is spent", journal().exists())
        assertFalse("and nothing is left staged", stagedPrefs().exists())
    }

    @Test
    fun `a resumed boot with nothing else pending reports the restore complete`() {
        setUpDirs()
        write(liveDb, "restored-db")
        write(stagedPrefs(), "restored-prefs")
        write(journal(), "prefs")

        // No pending_restore_* files at all: this boot's only work is finishing the last one's.
        assertTrue(RestoreApply.apply(filesDir, liveDb))
    }

    /**
     * A crash during STAGING, before any journal existed. Staging renames the pending file away, so
     * without recovery the restore is not delayed — it is gone: the next boot looks for
     * `pending_restore.db` and finds nothing, while the bytes sit under a name nothing reads.
     *
     * Recovery and application happen on the SAME boot, because the sweep runs ahead of the
     * `pending_restore.db` existence check rather than after it. So putting the file back is not a
     * deferral — by the time this pass asks what there is to restore, the answer already includes
     * the recovered file, and the boot proceeds exactly as if the crash had never happened.
     */
    @Test
    fun `a file stranded by a crash during staging is put back and retried`() {
        setUpDirs()
        write(liveDb, "live-db")
        // Exactly what a crash mid-staging leaves: staged bytes, no pending file, no journal.
        write(File(liveDb.path + ".restoring"), "restored-db")

        assertTrue("the stranded restore is recovered, not lost", RestoreApply.apply(filesDir, liveDb))
        assertEquals("restored-db", liveDb.readText())
        // And it leaves the same clean slate a normal restore does — no second `.restoring` orphan
        // for the next boot to sweep, no pending file, no journal.
        assertFalse("nothing left staged", File(liveDb.path + ".restoring").exists())
        assertFalse("no pending file left behind", File(filesDir, "pending_restore.db").exists())
        assertFalse("and no journal", journal().exists())
    }

    /**
     * A journalled staged file belongs to the RESUME, and the resume must reach it first.
     *
     * The two recovery mechanisms both look at `.restoring` files, so their order decides which one
     * claims a given file. Run the sweep first and it returns this file to a pending name; the
     * resume then finds nothing staged, leaves `prefs` in the journal, and reports failure — and the
     * staging pass that follows re-applies it as a NEW restore with no database in it, so the boot
     * ends with the preferences correct and the restore reported as failed. The user is told their
     * restore did not work while looking at the restored data.
     *
     * The return value is what pins the order: the file lands either way, but only the resume can
     * report the interrupted restore as complete.
     */
    @Test
    fun `a journalled file is claimed by the resume, not the orphan sweep`() {
        setUpDirs()
        write(liveDb, "restored-db")
        write(stagedPrefs(), "restored-prefs")
        write(journal(), "prefs")

        assertTrue("the resume owns it, so the restore reports complete", RestoreApply.apply(filesDir, liveDb))

        assertEquals("committed, not unstaged", "restored-prefs", livePrefs().readText())
        assertFalse("and not returned to a pending name", File(filesDir, "pending_restore_prefs.pb").exists())
    }

    @Test
    fun `a complete restore leaves no journal behind`() {
        setUpDirs()
        stageEverything()

        assertTrue(RestoreApply.apply(filesDir, liveDb))
        assertFalse(journal().exists())
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
