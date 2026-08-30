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

    /** Component names, as the journal records them. Order is the commit order. */
    private const val DB = "db"
    private const val PREFS = "prefs"
    private const val PHOTOS = "photos"
    private const val AVATAR = "avatar"

    /** Commit order. The database is first and is the anchor; the rest follow it. */
    private val ORDER = listOf(DB, PREFS, PHOTOS, AVATAR)

    /**
     * The commit journal: which components are staged and still need to be renamed into place.
     *
     * Phase 2 is a sequence of renames, and a process death between two of them used to be
     * unrecoverable. The database would be live, the preferences would not, and the next boot saw
     * neither a pending file (staging had renamed it away) nor any record that a restore was
     * half-applied — so the staged preferences sat under a `.restoring` name nothing reads, and the
     * mixed state became permanent.
     *
     * Written before the first commit, shortened after each successful one, deleted when empty. Its
     * presence at boot means a restore was interrupted mid-commit and the components it names still
     * have to land.
     */
    private const val JOURNAL = "pending_restore_journal"

    /**
     * @return true when a restore reached a fully-applied state on this boot — either one staged
     *   here, or one a previous boot left part-way through and this boot finished. The only case
     *   that may be reported to the user as a successful restore.
     */
    fun apply(filesDir: File, liveDb: File): Boolean {
        // ── Resume: finish what a previous boot started. ──
        //
        // Before anything else, because these components are already committed-or-not against a
        // database that may already have been swapped; staging a NEW restore on top of a half-applied
        // one would interleave two sets.
        val interrupted = readJournal(filesDir)
        val resumed = interrupted.isNotEmpty() && runJournal(filesDir, liveDb, interrupted)

        // ── Orphans: a crash during staging, before any journal existed. ──
        //
        // Staging renames the pending file away, so a `.restoring` file with no journal naming it is
        // a restore that would otherwise be lost outright. Put it back under the name the next boot
        // looks for. Skipped while a journal survives, because then those files are mid-commit and
        // belong to it.
        if (readJournal(filesDir).isEmpty()) unstageOrphans(filesDir, liveDb)

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
        //
        // Everything that can realistically fail — the byte copy, a cross-filesystem move, a full
        // disk, a file another process still holds — happens here, where the live app is untouched
        // and abandoning costs nothing. Phase 2 is renames within a single directory.
        val stagedDb = if (hadDb) stageBeside(pendingDb, liveDb) else null
        val stagedPrefs = if (hadPrefs) stageBeside(pendingPrefs, livePrefs) else null
        val stagedAvatar = if (hadAvatar) stageBeside(pendingAvatar, liveAvatar) else null

        if ((hadDb && stagedDb == null) || (hadPrefs && stagedPrefs == null) || (hadAvatar && stagedAvatar == null)) {
            // Put back whatever did stage, so the next cold start retries the set as one unit, and
            // leave this boot running entirely on pre-restore data.
            stagedDb?.let { unstage(it, pendingDb) }
            stagedPrefs?.let { unstage(it, pendingPrefs) }
            stagedAvatar?.let { unstage(it, pendingAvatar) }
            return resumed
        }

        // ── Phase 2: commit, journalled. ──
        val toCommit = buildList {
            if (stagedDb != null) add(DB)
            if (stagedPrefs != null) add(PREFS)
            if (hadPhotos) add(PHOTOS)
            if (stagedAvatar != null) add(AVATAR)
        }
        writeJournal(filesDir, toCommit)
        val applied = runJournal(filesDir, liveDb, toCommit)

        if (!applied && DB in readJournal(filesDir)) {
            // The anchor itself did not land, so nothing else may: the staged preferences describe
            // the staged dataset. Abandon the set and retry it whole next boot.
            writeJournal(filesDir, emptyList())
            stagedDb?.let { unstage(it, pendingDb) }
            stagedPrefs?.let { unstage(it, pendingPrefs) }
            stagedAvatar?.let { unstage(it, pendingAvatar) }
            return resumed
        }

        if (applied && hadDb && !hadAvatar && hadPrefs) {
            // The restore replaced the prefs but carried no avatar → the restored state has none.
            // Clear any live avatar so a previously-seeded default cover can't outlive the (now
            // blank) avatarDefaultId — otherwise the cover shows but the picker rings nothing.
            runCatching { liveAvatar.delete() }
        }
        return applied && hadDb
    }

    /**
     * Commit each component the journal still names, shortening it as each lands.
     *
     * The database goes first and is the anchor: if it does not land, the caller abandons the set
     * rather than leaving preferences that describe a dataset which is not there. Anything that
     * fails after it stays in the journal, so the next boot finishes it instead of the mixed state
     * becoming permanent.
     *
     * @return true when the journal emptied.
     */
    private fun runJournal(filesDir: File, liveDb: File, entries: List<String>): Boolean {
        var remaining = entries
        for (component in entries.sortedBy { ORDER.indexOf(it) }) {
            if (!commitComponent(component, filesDir, liveDb)) {
                if (component == DB) break // Anchor failed; the caller unwinds the rest.
                continue                   // Leave it journalled for the next boot.
            }
            remaining = remaining - component
            writeJournal(filesDir, remaining)
            // Only discard the source once its content is actually in place.
            pendingFor(component, filesDir)?.let {
                runCatching { if (it.isDirectory) it.deleteRecursively() else it.delete() }
            }
        }
        return remaining.isEmpty()
    }

    private fun commitComponent(component: String, filesDir: File, liveDb: File): Boolean = when (component) {
        DB -> stagedBeside(liveDb)?.let {
            // Drop stale WAL/-shm sidecars so SQLite can't replay old frames over the restored file.
            commitStaged(it, liveDb, afterSwap = {
                deleteOrThrow(File(liveDb.path + "-wal"))
                deleteOrThrow(File(liveDb.path + "-shm"))
            })
        } ?: false
        PREFS -> File(filesDir, PREFS_PATH).let { live -> stagedBeside(live)?.let { commitStaged(it, live) } ?: false }
        AVATAR -> File(filesDir, AVATAR_NAME).let { live -> stagedBeside(live)?.let { commitStaged(it, live) } ?: false }
        PHOTOS -> commitPhotos(filesDir)
        else -> true // An entry we do not recognise cannot be committed; drop it rather than loop.
    }

    /** Swap the restored photo folder in via rename, keeping the originals recoverable throughout. */
    private fun commitPhotos(filesDir: File): Boolean = runCatching {
        val pendingPhotos = File(filesDir, "pending_restore_photos")
        if (!pendingPhotos.isDirectory) return@runCatching false
        val livePhotos = File(filesDir, PHOTOS_NAME)
        val oldPhotos = File(filesDir, "$PHOTOS_NAME.old")
        if (oldPhotos.exists()) oldPhotos.deleteRecursively()
        val hadLive = livePhotos.exists()
        if (hadLive && !livePhotos.renameTo(oldPhotos)) error("Could not move current photos aside")
        if (!pendingPhotos.renameTo(livePhotos)) {
            if (hadLive) oldPhotos.renameTo(livePhotos) // roll back to the originals
            error("Could not move restored photos into place")
        }
        oldPhotos.deleteRecursively()
        true
    }.getOrDefault(false)

    /** The staged file beside [live], if staging left one there. */
    private fun stagedBeside(live: File): File? =
        File(live.parentFile, "${live.name}.restoring").takeIf { it.exists() }

    private fun pendingFor(component: String, filesDir: File): File? = when (component) {
        DB -> File(filesDir, "pending_restore.db")
        PREFS -> File(filesDir, "pending_restore_prefs.pb")
        PHOTOS -> File(filesDir, "pending_restore_photos")
        AVATAR -> File(filesDir, "pending_restore_avatar.jpg")
        else -> null
    }

    /**
     * Return every `.restoring` file to the name the next boot looks for.
     *
     * Only reached with no journal, which means the crash happened during STAGING — the pending file
     * had been renamed away and nothing recorded that it existed. Without this the restore is not
     * merely delayed, it is gone.
     */
    private fun unstageOrphans(filesDir: File, liveDb: File) {
        listOf(
            liveDb to File(filesDir, "pending_restore.db"),
            File(filesDir, PREFS_PATH) to File(filesDir, "pending_restore_prefs.pb"),
            File(filesDir, AVATAR_NAME) to File(filesDir, "pending_restore_avatar.jpg")
        ).forEach { (live, pending) ->
            stagedBeside(live)?.let { if (!pending.exists()) unstage(it, pending) else it.delete() }
        }
    }

    private fun journalFile(filesDir: File) = File(filesDir, JOURNAL)

    private fun readJournal(filesDir: File): List<String> = runCatching {
        journalFile(filesDir).takeIf { it.isFile }
            ?.readLines()?.map { it.trim() }?.filter { it.isNotEmpty() }
            .orEmpty()
    }.getOrDefault(emptyList())

    private fun writeJournal(filesDir: File, remaining: List<String>) {
        runCatching {
            val f = journalFile(filesDir)
            if (remaining.isEmpty()) f.delete() else f.writeText(remaining.joinToString("\n"))
        }
    }

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
        val staged = File(live.parentFile, "${live.name}.restoring")
        if (staged.exists()) staged.delete()
        if (!pending.renameTo(staged)) pending.copyTo(staged, overwrite = true)
        staged
    }.getOrNull()

    /**
     * Phase 2 for one component: the atomic rename into place, plus any post-swap cleanup.
     *
     * Rename within one directory is atomic: a failure leaves the intact original untouched. On a
     * failed [afterSwap] the restored file IS live but its cleanup did not finish, so the swap counts
     * as failed and a copy goes back to [staged] for the next boot to retry.
     */
    fun commitStaged(staged: File, live: File, afterSwap: () -> Unit = {}): Boolean = runCatching {
        if (!staged.renameTo(live)) error("Could not move ${live.name} into place")
        runCatching { afterSwap() }.onFailure { e ->
            runCatching { live.copyTo(staged, overwrite = true) }
            throw e
        }
    }.isSuccess

    /**
     * Abandon a staged component: put its bytes back under the name the next boot looks for.
     *
     * Called when a sibling could not be staged, or when the database — the anchor the others
     * describe — failed to commit. Without it the staged file would sit under a `.restoring` name
     * that nothing reads again, and the restore would be silently lost.
     */
    fun unstage(staged: File, pending: File) {
        runCatching {
            if (!staged.renameTo(pending)) {
                staged.copyTo(pending, overwrite = true)
                staged.delete()
            }
        }
    }

    /** Delete [f]; throw if it survives so the enclosing `runCatching` treats the swap as failed. */
    private fun deleteOrThrow(f: File) {
        if (f.exists() && !f.delete() && f.exists()) error("Could not delete ${f.name}")
    }
}
