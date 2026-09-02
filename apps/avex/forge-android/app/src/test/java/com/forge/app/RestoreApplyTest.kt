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

    /** What staging does last: publish the manifest that vouches for the pending files as they are. */
    private fun publish() = assertTrue("the manifest must publish", RestoreManifest.publish(filesDir))

    private fun manifest() = RestoreManifest.file(filesDir)

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
        publish()
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
        publish()
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
        assertFalse("the manifest is spent with the set it described", manifest().exists())

        // The pre-restore bytes are still there, because nothing has opened the database yet.
        assertEquals("live-db", File(liveDb.path + ".prerestore").readText())
        assertEquals("live-prefs", File(livePrefs().path + ".prerestore").readText())
        RestoreApply.confirm(filesDir, liveDb)
        assertFalse("confirm releases them", File(liveDb.path + ".prerestore").exists())
        assertFalse(File(livePrefs().path + ".prerestore").exists())
        assertFalse(File(liveAvatar().path + ".prerestore").exists())
        assertFalse(File(livePhotos().path + ".prerestore").exists())
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
        publish()

        assertTrue(RestoreApply.apply(filesDir, liveDb))
        assertFalse("a seeded cover must not outlive a blank avatarDefaultId", liveAvatar().exists())
        // Moved aside, not deleted, so a revert can bring it back with the rest of the set.
        assertEquals("live-avatar", File(liveAvatar().path + ".prerestore").readText())
        RestoreApply.revert(filesDir, liveDb)
        assertEquals("live-avatar", liveAvatar().readText())
    }

    @Test
    fun `stale WAL sidecars do not survive the swapped database`() {
        setUpDirs()
        write(liveDb, "live-db")
        write(File(liveDb.path + "-wal"), "stale-wal")
        write(File(liveDb.path + "-shm"), "stale-shm")
        write(File(filesDir, "pending_restore.db"), "restored-db")
        publish()

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
        // Exactly what a crash mid-staging leaves: staged bytes, no pending file, no journal — and
        // the manifest staging published before the boot began.
        write(File(filesDir, "pending_restore.db"), "restored-db")
        publish()
        assertTrue(File(filesDir, "pending_restore.db").renameTo(File(liveDb.path + ".restoring")))

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

    // ── A journal entry that outlives its staged file ─────────────────────────

    /**
     * The gap the first journal left: a crash between a component's RENAME and the journal being
     * shortened. The entry survives naming a `.restoring` file that no longer exists, because it
     * has become the live file.
     *
     * Treating "nothing staged" as a failure made that permanent. Every later boot found nothing to
     * commit, kept the entry, reported the restore unfinished — and, because a journal still
     * existed, skipped the orphan sweep as well, so any genuinely stranded file stayed stranded too.
     * The component had actually landed; the only thing missing was the record saying so.
     */
    @Test
    fun `a journal entry whose file already landed is retired, not retried forever`() {
        setUpDirs()
        write(liveDb, "restored-db")
        write(livePrefs(), "restored-prefs")   // the rename happened
        write(journal(), "prefs")              // the shortening did not
        // The snapshot the commit took, also left behind by the crash.
        write(File(filesDir, "datastore/forge_settings.preferences_pb.prerestore"), "live-prefs")

        assertTrue("the interrupted restore is complete, and is reported so", RestoreApply.apply(filesDir, liveDb))

        assertFalse("the journal must not survive a boot that resolved it", journal().exists())
        assertEquals("and the committed file is left alone", "restored-prefs", livePrefs().readText())
        // Whole is not yet proven: the snapshot waits for the database to open.
        val snapshot = File(filesDir, "datastore/forge_settings.preferences_pb.prerestore")
        assertTrue("the snapshot is kept until the set is confirmed", snapshot.exists())
        RestoreApply.confirm(filesDir, liveDb)
        assertFalse("and is finished business once it is", snapshot.exists())
    }

    @Test
    fun `an unrecognised journal entry cannot keep the journal alive`() {
        setUpDirs()
        write(liveDb, "live-db")
        write(journal(), "something-a-later-version-wrote")

        assertTrue(RestoreApply.apply(filesDir, liveDb))
        assertFalse(journal().exists())
    }

    // ── The journal itself failing ────────────────────────────────────────────

    /**
     * A journal that cannot be written means the renames about to happen have no crash recovery,
     * which is the entire reason phase 2 has one. The first version swallowed the write failure and
     * committed anyway — restoring precisely the unrecoverable mixed state the journal exists to
     * prevent, and doing it silently.
     *
     * Blocked by making the scratch file the journal is written through a directory: `writeText`
     * cannot open it, on any machine, without depending on permissions.
     */
    @Test
    fun `a restore that cannot be journalled is not committed at all`() {
        setUpDirs()
        stageEverything()
        File(filesDir, "pending_restore_journal.tmp").mkdirs()

        assertFalse("an unjournalled commit is not a restore", RestoreApply.apply(filesDir, liveDb))

        assertEquals("live-db", liveDb.readText())
        assertEquals("live-prefs", livePrefs().readText())
        assertEquals("live-avatar", liveAvatar().readText())
        assertEquals("live-photo", File(livePhotos(), "pp_old.jpg").readText())

        // And the set is queued whole, so clearing the blockage lets the next boot finish it.
        assertEquals("restored-db", File(filesDir, "pending_restore.db").readText())
        assertEquals("restored-prefs", File(filesDir, "pending_restore_prefs.pb").readText())
        assertEquals("restored-avatar", File(filesDir, "pending_restore_avatar.jpg").readText())
        assertTrue(File(filesDir, "pending_restore_photos").isDirectory)
    }

    /**
     * A journal that exists but cannot be READ names components that are mid-commit, and there is no
     * way to tell which. Sweeping would strand them and staging a new set would interleave two
     * restores, so the only safe move is to touch nothing and let a later boot try again.
     */
    @Test
    fun `an unreadable journal stops the boot from touching anything`() {
        setUpDirs()
        stageEverything()
        journal().mkdirs()

        assertFalse(RestoreApply.apply(filesDir, liveDb))

        assertEquals("live-db", liveDb.readText())
        assertEquals("live-prefs", livePrefs().readText())
        assertEquals("the pending set is left exactly where it was", "restored-db", File(filesDir, "pending_restore.db").readText())
    }

    // ── A commit that fails mid-set ───────────────────────────────────────────

    /**
     * The half of atomicity a journal does not provide.
     *
     * Deferring a failed component to the next boot is the ORIGINAL defect in new clothing: the app
     * comes up on a restored database with pre-restore preferences, and the staged ones land later,
     * over whatever the user changed in between. So a commit that fails takes the whole set back
     * out — every component that already landed included — and the boot runs entirely on pre-restore
     * data, exactly as if the restore had never been attempted.
     *
     * The database is made to fail AFTER its rename, at the WAL cleanup, so the test exercises the
     * case where a component is genuinely live by the time the set is abandoned. A non-empty
     * directory where the `-wal` file goes cannot be deleted on any platform.
     */
    @Test
    fun `a commit that fails after another already landed takes the whole set back out`() {
        setUpDirs()
        stageEverything()
        write(File(liveDb.path + "-wal", "occupied"), "cannot be deleted")

        assertFalse("a rolled-back restore is not a success", RestoreApply.apply(filesDir, liveDb))

        assertEquals("the database that DID swap is put back", "live-db", liveDb.readText())
        assertEquals("live-prefs", livePrefs().readText())
        assertEquals("live-avatar", liveAvatar().readText())
        assertEquals("live-photo", File(livePhotos(), "pp_old.jpg").readText())

        // Deferred, not discarded: the user asked for this restore, so the whole set is queued.
        assertEquals("restored-db", File(filesDir, "pending_restore.db").readText())
        assertEquals("restored-prefs", File(filesDir, "pending_restore_prefs.pb").readText())
        assertEquals("restored-avatar", File(filesDir, "pending_restore_avatar.jpg").readText())
        assertTrue(File(filesDir, "pending_restore_photos").isDirectory)

        assertFalse("no journal outlives the rollback", journal().exists())
        assertFalse("and no snapshot", File(liveDb.path + ".prerestore").exists())
        assertFalse("and nothing left staged", File(liveDb.path + ".restoring").exists())
        assertTrue("the manifest still vouches for the requeued set", RestoreManifest.verify(filesDir))
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

    // ── The READY marker: an unfinished set is quarantined, never applied ─────

    /**
     * The process died during staging, after `pending_restore.db` was created and before the
     * manifest was published. This boot used to rename that file live: a truncated database, or a
     * complete one paired with the preferences of another backup, with the snapshot discarded
     * straight after. The restore screen never reported success, so nothing is lost by refusing.
     */
    @Test
    fun `a pending set without a manifest is quarantined, not applied`() {
        setUpDirs()
        write(liveDb, "live-db")
        write(livePrefs(), "live-prefs")
        write(File(filesDir, "pending_restore.db"), "half-a-restored-db")
        write(File(filesDir, "pending_restore_prefs.pb"), "restored-prefs")

        assertFalse("an unfinished set is not a restore", RestoreApply.apply(filesDir, liveDb))

        assertEquals("live-db", liveDb.readText())
        assertEquals("live-prefs", livePrefs().readText())
        assertFalse("the unfinished set is gone", File(filesDir, "pending_restore.db").exists())
        assertFalse(File(filesDir, "pending_restore_prefs.pb").exists())
        assertFalse("and nothing was moved aside", File(liveDb.path + ".prerestore").exists())
    }

    @Test
    fun `a pending database that no longer matches its manifest is quarantined`() {
        setUpDirs()
        stageEverything()
        // The bytes changed after the manifest vouched for them: the copy was cut short.
        write(File(filesDir, "pending_restore.db"), "restored-")

        assertFalse(RestoreApply.apply(filesDir, liveDb))

        assertEquals("live-db", liveDb.readText())
        assertEquals("live-prefs", livePrefs().readText())
        assertEquals("live-photo", File(livePhotos(), "pp_old.jpg").readText())
        assertFalse(File(filesDir, "pending_restore.db").exists())
        assertFalse(File(filesDir, "pending_restore_photos").exists())
        assertFalse("the marker goes with the set", manifest().exists())
    }

    @Test
    fun `a manifest naming a component that is not there is quarantined`() {
        setUpDirs()
        stageEverything()
        File(filesDir, "pending_restore_prefs.pb").delete()

        assertFalse(RestoreApply.apply(filesDir, liveDb))
        assertEquals("live-db", liveDb.readText())
        assertFalse(File(filesDir, "pending_restore.db").exists())
    }

    @Test
    fun `a component the manifest never saw is quarantined with the set`() {
        setUpDirs()
        write(liveDb, "live-db")
        write(liveAvatar(), "live-avatar")
        write(File(filesDir, "pending_restore.db"), "restored-db")
        publish()
        // Staged after the manifest: it belongs to nothing this boot can vouch for.
        write(File(filesDir, "pending_restore_avatar.jpg"), "restored-avatar")

        assertFalse(RestoreApply.apply(filesDir, liveDb))
        assertEquals("live-db", liveDb.readText())
        assertEquals("live-avatar", liveAvatar().readText())
    }

    @Test
    fun `a photo folder is vouched for file by file`() {
        setUpDirs()
        stageEverything()
        write(File(filesDir, "pending_restore_photos/pp_new.jpg"), "restored-photo-but-different")

        assertFalse(RestoreApply.apply(filesDir, liveDb))
        assertEquals("live-photo", File(livePhotos(), "pp_old.jpg").readText())
    }

    @Test
    fun `the manifest outlives a deferred set and is spent by the one that lands`() {
        setUpDirs()
        stageEverythingExceptLivePrefs()
        blockThePreferencesPath()

        assertFalse(RestoreApply.apply(filesDir, liveDb))
        assertTrue("deferred, so still vouched for", RestoreManifest.verify(filesDir))

        File(filesDir, "datastore").delete()
        assertTrue(RestoreApply.apply(filesDir, liveDb))
        assertFalse(manifest().exists())
    }

    // ── The restored database must open before the old one is let go ─────────

    /**
     * The swap is not the end of a restore. The replacement was validated when it was staged, but
     * the proof that THIS boot can open it comes from Room, and until then the pre-restore bytes
     * must be recoverable: a restore that lands and then cannot be opened used to be an app that
     * could not start, with the previous database already deleted.
     */
    @Test
    fun `revert puts every component of a landed set back`() {
        setUpDirs()
        stageEverything()
        write(File(liveDb.path + "-wal"), "stale-wal")
        assertTrue(RestoreApply.apply(filesDir, liveDb))
        assertEquals("restored-db", liveDb.readText())
        write(File(liveDb.path + "-wal"), "frames-from-the-failed-open")

        assertTrue(RestoreApply.revert(filesDir, liveDb))

        assertEquals("live-db", liveDb.readText())
        assertEquals("live-prefs", livePrefs().readText())
        assertEquals("live-avatar", liveAvatar().readText())
        assertEquals("live-photo", File(livePhotos(), "pp_old.jpg").readText())
        assertFalse("the restored photos went with the set", File(livePhotos(), "pp_new.jpg").exists())
        assertFalse("no sidecar can replay over the returned file", File(liveDb.path + "-wal").exists())
        assertFalse("nothing is left aside", File(liveDb.path + ".prerestore").exists())
        assertFalse("and the set is not requeued", File(filesDir, "pending_restore.db").exists())
    }

    @Test
    fun `revert removes what the set restored over nothing`() {
        setUpDirs()
        // A fresh install: no live database, no live preferences, nothing to snapshot.
        write(File(filesDir, "pending_restore.db"), "restored-db")
        write(File(filesDir, "pending_restore_prefs.pb"), "restored-prefs")
        publish()
        assertTrue(RestoreApply.apply(filesDir, liveDb))
        assertEquals("restored-db", liveDb.readText())

        assertTrue(RestoreApply.revert(filesDir, liveDb))

        assertFalse("the database that could not open is gone, as it was before", liveDb.exists())
        assertFalse(livePrefs().exists())
    }

    @Test
    fun `confirm leaves the restored set in place and nothing else`() {
        setUpDirs()
        stageEverything()
        assertTrue(RestoreApply.apply(filesDir, liveDb))

        RestoreApply.confirm(filesDir, liveDb)

        assertEquals("restored-db", liveDb.readText())
        assertEquals("restored-prefs", livePrefs().readText())
        assertFalse(File(liveDb.path + ".prerestore").exists())
        assertFalse(File(filesDir, "pending_restore_applied").exists())
        // A revert after a confirm has nothing to act on.
        assertTrue(RestoreApply.revert(filesDir, liveDb))
        assertEquals("restored-db", liveDb.readText())
    }

    @Test
    fun `a snapshot a previous boot never confirmed is swept on the next one`() {
        setUpDirs()
        stageEverything()
        assertTrue(RestoreApply.apply(filesDir, liveDb))
        // The process died between the swap and the open; the app has since run on this data.
        assertTrue(File(liveDb.path + ".prerestore").exists())

        assertFalse("nothing pending, so nothing to apply", RestoreApply.apply(filesDir, liveDb))
        assertFalse(File(liveDb.path + ".prerestore").exists())
        assertFalse(File(filesDir, "pending_restore_applied").exists())
        assertEquals("restored-db", liveDb.readText())
    }
}
