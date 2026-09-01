package com.forge.app

import java.io.File

/**
 * Applying a staged restore at boot, as one set or not at all.
 *
 * `BackupRepository.restoreFromUri` stages up to four components — the database, the DataStore
 * preferences file, the progress-photo folder and the avatar — and this puts them into place before
 * Room or DataStore is ever opened. Doing it at boot is what avoids replacing files while live flows
 * are reading them.
 *
 * ## Why this is two phases
 *
 * It used to copy and rename each component into place in turn, set a flag if one failed, and CARRY
 * ON. The database is the anchor the others describe, so a user could finish a boot with a database
 * from the backup and preferences, photos and an avatar from before it — a program describing days
 * none of their sessions match. The failed component was then retried on a LATER boot, so any
 * setting they changed in the meantime was silently overwritten when the staged preferences finally
 * landed. The code's own comments called the set atomic; it was not.
 *
 * So everything that can realistically fail happens in phase 1, where nothing is live yet and
 * abandoning costs nothing: the byte copy, a cross-filesystem move, a full disk, a file another
 * process still holds. Phase 2 is renames within a single directory, which are atomic and
 * essentially cannot fail once the bytes are already there. If any component cannot be staged, every
 * staged sibling is returned to its pending name and the boot runs entirely on pre-restore data,
 * with the whole set retried as one unit next time.
 *
 * ## Why phase 2 is journalled AND reversible
 *
 * "Essentially cannot fail" is not the same as cannot, and phase 2 has two ways to end badly. A
 * process death between two renames leaves some components swapped and some not, with nothing on
 * disk saying so — that is what [JOURNAL] is for. A rename that simply FAILS mid-set leaves the same
 * mixture, except the process is still running and about to finish booting into it.
 *
 * A journal alone does not fix the second one: deferring the failed component to the next boot
 * re-creates the exact defect described above, because the app comes up on a restored database with
 * pre-restore preferences and lands the staged ones later, over whatever the user changed in
 * between. So each component's pre-restore bytes are moved aside rather than overwritten, and if the
 * set cannot be completed every one of them goes back. `apply` therefore never returns having left a
 * mixture: either the whole set is live, or none of it is and the whole set is queued to retry.
 *
 * Lives outside `ForgeApp` so the sequencing is reachable from a test with a temp directory; the
 * Application only supplies the two paths and records the confirmation flag.
 */
internal object RestoreApply {

    /** Must match `preferencesDataStore(name = "forge_settings")`. */
    private const val PREFS_PATH = "datastore/forge_settings.preferences_pb"
    /** Must match `AvatarRepository.FILE_NAME`. */
    private const val AVATAR_NAME = "avatar.jpg"
    /** Must match `ProgressPhotoRepository`'s folder. */
    private const val PHOTOS_NAME = "progress_photos"

    /** Component names, as the journal records them. */
    private const val DB = "db"
    private const val PREFS = "prefs"
    private const val PHOTOS = "photos"
    private const val AVATAR = "avatar"

    /** Commit order. The database is first and is the anchor; the rest follow it. */
    private val ORDER = listOf(DB, PREFS, PHOTOS, AVATAR)

    /**
     * The commit journal: which components are staged and still need to be renamed into place.
     *
     * Written before the first commit, shortened after each successful one, deleted when empty. Its
     * presence at boot means a restore was interrupted mid-commit and the components it names still
     * have to land.
     */
    private const val JOURNAL = "pending_restore_journal"

    /** Phase 1 puts a component's incoming bytes here, beside where they are going. */
    private const val STAGED_SUFFIX = ".restoring"

    /** Phase 2 puts a component's OUTGOING bytes here, so the swap can be undone. */
    private const val SNAPSHOT_SUFFIX = ".prerestore"

    /**
     * What committing one component did.
     *
     * [ALREADY] is the distinction that matters, and conflating it with [FAILED] wedged recovery
     * permanently: a crash between a component's rename and the journal being shortened leaves an
     * entry naming a staged file that is no longer there, because it has become the live file. Read
     * as a failure, that entry can never be retired — every subsequent boot finds nothing to commit,
     * reports the restore unfinished, and skips the orphan sweep because a journal still exists.
     * Read as "this one is done", the journal empties on the very next boot.
     */
    private enum class Commit { LANDED, ALREADY, FAILED }

    /**
     * @return true when a restore reached a fully-applied state on this boot — either one staged
     *   here, or one a previous boot left part-way through and this boot finished. The only case
     *   that may be reported to the user as a successful restore.
     */
    fun apply(filesDir: File, liveDb: File): Boolean {
        // A journal that EXISTS but cannot be read is the one state in which doing nothing is the
        // only safe move. Its components are mid-commit, so sweeping would strand them and staging a
        // new set would interleave two restores — and we cannot tell which components they are.
        val interrupted = readJournal(filesDir) ?: return false

        // ── Resume: finish what a previous boot started. ──
        //
        // Before anything else, because these components are already committed-or-not against a
        // database that may already have been swapped.
        val resumed = interrupted.isNotEmpty() && finishSet(filesDir, liveDb, interrupted)

        // [finishSet] leaves no journal either way, so anything still here means the clear itself
        // failed. Staging a new set on top of a record we could not retire would mix two restores.
        val settled = readJournal(filesDir)
        if (settled == null || settled.isNotEmpty()) return resumed

        // ── Orphans: a crash during staging, before any journal existed. ──
        //
        // Staging renames the pending file away, so a `.restoring` file with no journal naming it is
        // a restore that would otherwise be lost outright. Put it back under the name the next boot
        // looks for. With no journal nothing is in flight, so this also clears any snapshot left by
        // a crash between the last commit and its cleanup.
        sweepOrphans(filesDir, liveDb)

        val pendingDb = File(filesDir, "pending_restore.db")
        val pendingPrefs = File(filesDir, "pending_restore_prefs.pb")
        val pendingPhotos = File(filesDir, "pending_restore_photos")
        val pendingAvatar = File(filesDir, "pending_restore_avatar.jpg")
        // Captured BEFORE staging, which renames these away.
        val hadDb = pendingDb.exists()
        val hadPrefs = pendingPrefs.exists()
        val hadPhotos = pendingPhotos.isDirectory
        val hadAvatar = pendingAvatar.exists()
        if (!hadDb && !hadPrefs && !hadPhotos && !hadAvatar) return resumed

        val livePrefs = File(filesDir, PREFS_PATH)
        val liveAvatar = File(filesDir, AVATAR_NAME)

        // ── Phase 1: stage beside each destination. Nothing is live yet. ──
        val stagedDb = if (hadDb) stageBeside(pendingDb, liveDb) else null
        val stagedPrefs = if (hadPrefs) stageBeside(pendingPrefs, livePrefs) else null
        val stagedAvatar = if (hadAvatar) stageBeside(pendingAvatar, liveAvatar) else null

        val unstageAll: () -> Unit = {
            stagedDb?.let { unstage(it, pendingDb) }
            stagedPrefs?.let { unstage(it, pendingPrefs) }
            stagedAvatar?.let { unstage(it, pendingAvatar) }
        }

        if ((hadDb && stagedDb == null) || (hadPrefs && stagedPrefs == null) || (hadAvatar && stagedAvatar == null)) {
            // Put back whatever did stage, so the next cold start retries the set as one unit, and
            // leave this boot running entirely on pre-restore data.
            unstageAll()
            return resumed
        }

        // ── Phase 2: commit, journalled and reversible. ──
        val toCommit = ORDER.filter {
            when (it) {
                DB -> stagedDb != null
                PREFS -> stagedPrefs != null
                PHOTOS -> hadPhotos
                else -> stagedAvatar != null
            }
        }
        if (!writeJournal(filesDir, toCommit)) {
            // No journal means no crash recovery for the renames about to happen, which is the whole
            // reason phase 2 has one. Swallowing this failure and committing anyway would leave the
            // unrecoverable mixed state the journal exists to prevent — so abandon the set instead
            // and retry it whole, which costs a boot and loses nothing.
            unstageAll()
            return resumed
        }

        if (!finishSet(filesDir, liveDb, toCommit)) return resumed

        if (hadDb && !hadAvatar && hadPrefs) {
            // The restore replaced the prefs but carried no avatar → the restored state has none.
            // Clear any live avatar so a previously-seeded default cover can't outlive the (now
            // blank) avatarDefaultId — otherwise the cover shows but the picker rings nothing.
            runCatching { liveAvatar.delete() }
        }
        return hadDb
    }

    /**
     * Commit [entries] in full, or leave the filesystem exactly as it was before phase 2 began.
     *
     * There is no third outcome, and that is the point: the caller returns straight into
     * `Application.onCreate` finishing, so anything left half-applied here is what the user's next
     * session runs on.
     *
     * @return true when every component landed.
     */
    private fun finishSet(filesDir: File, liveDb: File, entries: List<String>): Boolean {
        if (runJournal(filesDir, liveDb, entries)) {
            entries.forEach { c -> liveFor(c, filesDir, liveDb)?.let { discard(snapshotOf(it)) } }
            return true
        }
        rollBack(filesDir, liveDb, entries)
        return false
    }

    /**
     * Commit each component the journal names, shortening it as each lands.
     *
     * A failed shortening is survivable and deliberately not treated as fatal: the component is
     * live, the journal still names it, and the next boot resolves that entry as [Commit.ALREADY]
     * and drops it. The write that MUST succeed is the first one, and its caller checks it.
     *
     * @return true when every entry was retired.
     */
    private fun runJournal(filesDir: File, liveDb: File, entries: List<String>): Boolean {
        var remaining = entries
        for (component in entries.sortedBy { ORDER.indexOf(it) }) {
            if (commitComponent(component, filesDir, liveDb) == Commit.FAILED) return false
            remaining = remaining - component
            writeJournal(filesDir, remaining)
            // Only discard the source once its content is actually in place.
            pendingFor(component, filesDir)?.let { discard(it) }
        }
        return remaining.isEmpty()
    }

    private fun commitComponent(component: String, filesDir: File, liveDb: File): Commit = when (component) {
        DB -> commitFile(liveDb) {
            // Drop stale WAL/-shm sidecars so SQLite can't replay old frames over the restored file.
            deleteOrThrow(File(liveDb.path + "-wal"))
            deleteOrThrow(File(liveDb.path + "-shm"))
        }
        PREFS -> commitFile(File(filesDir, PREFS_PATH))
        AVATAR -> commitFile(File(filesDir, AVATAR_NAME))
        PHOTOS -> commitPhotos(filesDir)
        // An entry we do not recognise has no staged file and never will. Retire it rather than let
        // one unknown word keep the journal alive forever.
        else -> Commit.ALREADY
    }

    /**
     * Phase 2 for one file: move the current one aside, rename the staged one in, run any post-swap
     * cleanup.
     *
     * The snapshot goes first because it is what makes the rename undoable. If it cannot be taken,
     * the swap is refused rather than done irreversibly — a restore delayed by a boot is a far
     * smaller thing than one that cannot be backed out of half way.
     */
    private fun commitFile(live: File, afterSwap: () -> Unit = {}): Commit {
        val staged = stagedBeside(live) ?: return Commit.ALREADY
        if (!snapshot(live)) return Commit.FAILED
        val ok = runCatching {
            if (!staged.renameTo(live)) error("Could not move ${live.name} into place")
            afterSwap()
        }.isSuccess
        return if (ok) Commit.LANDED else Commit.FAILED
    }

    /** Photos stage AS the pending folder rather than beside the live one, so they swap directly. */
    private fun commitPhotos(filesDir: File): Commit {
        val pending = File(filesDir, "pending_restore_photos")
        if (!pending.isDirectory) return Commit.ALREADY
        val live = File(filesDir, PHOTOS_NAME)
        if (!snapshot(live)) return Commit.FAILED
        return if (pending.renameTo(live)) Commit.LANDED else Commit.FAILED
    }

    /**
     * Undo phase 2 for every component in the set, committed or not.
     *
     * A component that never committed still has its bytes under the staging name; one that did has
     * them live. Both go back to the pending name, because the set is being deferred rather than
     * thrown away — the user asked for this restore and it should be retried whole. The snapshot
     * then goes back over the top, returning the app to exactly the state it booted into. Where
     * there is no snapshot the pre-restore state was "this file does not exist", and moving the
     * restored bytes out to the pending name reproduces that.
     */
    private fun rollBack(filesDir: File, liveDb: File, entries: List<String>) {
        for (component in entries) {
            val live = liveFor(component, filesDir, liveDb) ?: continue
            val pending = pendingFor(component, filesDir) ?: continue
            val staged = if (component == PHOTOS) null else stagedBeside(live)
            when {
                staged != null -> move(staged, pending)
                // An uncommitted photo folder is already sitting at the pending name.
                component == PHOTOS && pending.isDirectory -> Unit
                live.exists() -> move(live, pending)
            }
            snapshotOf(live).takeIf { it.exists() }?.let { move(it, live) }
        }
        writeJournal(filesDir, emptyList())
    }

    /**
     * Move [live] aside so its replacement can be undone.
     *
     * A rename within one directory, so the cost is a directory entry rather than a copy of a
     * multi-megabyte database. Nothing to move is success: the component simply had no pre-restore
     * state to preserve.
     */
    private fun snapshot(live: File): Boolean {
        if (!live.exists()) return true
        val snapshot = snapshotOf(live)
        if (snapshot.exists()) discard(snapshot)
        return move(live, snapshot)
    }

    /** The staged file beside [live], if staging left one there. */
    private fun stagedBeside(live: File): File? =
        File(live.parentFile, "${live.name}$STAGED_SUFFIX").takeIf { it.exists() }

    private fun snapshotOf(live: File) = File(live.parentFile, "${live.name}$SNAPSHOT_SUFFIX")

    private fun liveFor(component: String, filesDir: File, liveDb: File): File? = when (component) {
        DB -> liveDb
        PREFS -> File(filesDir, PREFS_PATH)
        PHOTOS -> File(filesDir, PHOTOS_NAME)
        AVATAR -> File(filesDir, AVATAR_NAME)
        else -> null
    }

    private fun pendingFor(component: String, filesDir: File): File? = when (component) {
        DB -> File(filesDir, "pending_restore.db")
        PREFS -> File(filesDir, "pending_restore_prefs.pb")
        PHOTOS -> File(filesDir, "pending_restore_photos")
        AVATAR -> File(filesDir, "pending_restore_avatar.jpg")
        else -> null
    }

    /**
     * Return every `.restoring` file to the name the next boot looks for, and drop stale snapshots.
     *
     * Only reached with no journal, which means no restore is in flight. A `.restoring` file here is
     * a crash during STAGING — the pending file had been renamed away and nothing recorded that it
     * existed, so without this the restore is not merely delayed, it is gone. A `.prerestore` file
     * here is the opposite: finished business whose cleanup did not run, and keeping it would leave
     * a whole spare database on a device that may be short of room.
     */
    private fun sweepOrphans(filesDir: File, liveDb: File) {
        ORDER.forEach { component ->
            val live = liveFor(component, filesDir, liveDb) ?: return@forEach
            val pending = pendingFor(component, filesDir) ?: return@forEach
            if (component != PHOTOS) {
                stagedBeside(live)?.let { if (pending.exists()) discard(it) else move(it, pending) }
            }
            snapshotOf(live).takeIf { it.exists() }?.let { discard(it) }
        }
    }

    private fun journalFile(filesDir: File) = File(filesDir, JOURNAL)

    /**
     * @return the journalled components; an empty list when there is no journal; **null** when one
     *   exists but could not be read, which the caller must treat as "a restore is in flight and I
     *   do not know which components", not as "there is nothing to do".
     */
    private fun readJournal(filesDir: File): List<String>? {
        val file = journalFile(filesDir)
        if (!file.exists()) return emptyList()
        return runCatching {
            file.readLines().map { it.trim() }.filter { it.isNotEmpty() }
        }.getOrNull()
    }

    /**
     * @return whether the journal now says exactly [remaining].
     *
     * Written to a sibling and renamed in, because a journal half-written when the process dies is
     * worse than no journal at all: the next boot would act on a truncated list and quietly skip
     * whatever was cut off. The rename is the only step that publishes it, and it is atomic.
     */
    private fun writeJournal(filesDir: File, remaining: List<String>): Boolean = runCatching {
        val file = journalFile(filesDir)
        if (remaining.isEmpty()) return@runCatching !file.exists() || file.delete()
        val scratch = File(filesDir, "$JOURNAL.tmp")
        scratch.writeText(remaining.joinToString("\n"))
        if (!scratch.renameTo(file)) {
            file.delete()
            if (!scratch.renameTo(file)) {
                scratch.delete()
                return@runCatching false
            }
        }
        true
    }.getOrDefault(false)

    /**
     * Phase 1 for one component: get [pending]'s bytes next to [live] without touching [live].
     *
     * The MOVE is attempted first and the copy is the fallback. This runs in `Application.onCreate`,
     * on the main thread, before any UI exists, and copying a multi-megabyte `forge.db`
     * byte-for-byte is seconds of frozen screen on a mid-range device with a long history — a
     * plausible ANR at the one moment a user is least willing to force-stop the app. `filesDir` and
     * `databases/` are the same filesystem, so the move is O(1); a device where it isn't falls back
     * to the copy.
     *
     * @return the staged file, or null if the bytes could not be placed — in which case [live] is
     *   exactly as it was.
     */
    fun stageBeside(pending: File, live: File): File? = runCatching {
        live.parentFile?.mkdirs()
        val staged = File(live.parentFile, "${live.name}$STAGED_SUFFIX")
        if (staged.exists()) staged.delete()
        if (!pending.renameTo(staged)) pending.copyTo(staged, overwrite = true)
        staged
    }.getOrNull()

    /**
     * Abandon a staged component: put its bytes back under the name the next boot looks for.
     *
     * Called when a sibling could not be staged. Without it the staged file would sit under a
     * `.restoring` name that nothing reads again, and the restore would be silently lost.
     */
    fun unstage(staged: File, pending: File) {
        move(staged, pending)
    }

    /** Rename [from] onto [to], falling back to a copy across filesystems. Handles folders. */
    private fun move(from: File, to: File): Boolean = runCatching {
        if (to.exists()) discard(to)
        if (from.renameTo(to)) return@runCatching true
        if (from.isDirectory) from.copyRecursively(to, overwrite = true)
        else from.copyTo(to, overwrite = true)
        discard(from)
        true
    }.getOrDefault(false)

    private fun discard(target: File) {
        runCatching { if (target.isDirectory) target.deleteRecursively() else target.delete() }
    }

    /** Delete [f]; throw if it survives so the enclosing `runCatching` treats the swap as failed. */
    private fun deleteOrThrow(f: File) {
        if (f.exists() && !f.delete() && f.exists()) error("Could not delete ${f.name}")
    }
}
