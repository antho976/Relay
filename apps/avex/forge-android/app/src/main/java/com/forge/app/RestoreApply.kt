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

    /**
     * @return true only when a database was swapped AND every other staged component landed — the
     *   one case that may be reported to the user as a successful restore.
     */
    fun apply(filesDir: File, liveDb: File): Boolean {
        val pendingDb = File(filesDir, "pending_restore.db")
        val pendingPrefs = File(filesDir, "pending_restore_prefs.pb")
        val pendingPhotos = File(filesDir, "pending_restore_photos")
        val pendingAvatar = File(filesDir, "pending_restore_avatar.jpg")
        // Captured BEFORE staging, which renames these away.
        val hadDb = pendingDb.exists()
        val hadPrefs = pendingPrefs.exists()
        val hadPhotos = pendingPhotos.isDirectory
        val hadAvatar = pendingAvatar.exists()
        if (!hadDb && !hadPrefs && !hadPhotos && !hadAvatar) return false

        val livePrefs = File(filesDir, PREFS_PATH)
        val liveAvatar = File(filesDir, AVATAR_NAME)

        // ── Phase 1: stage beside each destination. Nothing is live yet. ──
        val stagedDb = if (hadDb) stageBeside(pendingDb, liveDb) else null
        val stagedPrefs = if (hadPrefs) stageBeside(pendingPrefs, livePrefs) else null
        val stagedAvatar = if (hadAvatar) stageBeside(pendingAvatar, liveAvatar) else null

        if ((hadDb && stagedDb == null) || (hadPrefs && stagedPrefs == null) || (hadAvatar && stagedAvatar == null)) {
            stagedDb?.let { unstage(it, pendingDb) }
            stagedPrefs?.let { unstage(it, pendingPrefs) }
            stagedAvatar?.let { unstage(it, pendingAvatar) }
            return false
        }

        // ── Phase 2: commit. Renames inside one directory, back to back. ──
        var applied = false
        var anyFailed = false

        if (stagedDb != null) {
            // Drop stale WAL/-shm sidecars so SQLite can't replay old frames over the restored file.
            // A surviving sidecar makes the swap a failure.
            val ok = commitStaged(stagedDb, liveDb, afterSwap = {
                deleteOrThrow(File(liveDb.path + "-wal"))
                deleteOrThrow(File(liveDb.path + "-shm"))
            })
            if (ok) {
                pendingDb.delete(); applied = true
            } else {
                // The anchor did not land, so nothing else may: the staged preferences describe the
                // staged dataset. Return everything to its pending name and retry next boot.
                unstage(stagedDb, pendingDb)
                stagedPrefs?.let { unstage(it, pendingPrefs) }
                stagedAvatar?.let { unstage(it, pendingAvatar) }
                return false
            }
        }
        if (stagedPrefs != null) {
            if (commitStaged(stagedPrefs, livePrefs)) pendingPrefs.delete() else anyFailed = true
        }
        if (hadPhotos) {
            // Swap via rename: move the current folder aside, slot the restored one in, then drop the
            // old copy — and if the slot-in fails, move the original back. renameTo within filesDir is
            // atomic, so there's no window where the user is left with neither folder.
            val swapped = runCatching {
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
            }.isSuccess
            // Only discard the staged folder once it's actually in place; otherwise keep it for retry.
            if (swapped) pendingPhotos.deleteRecursively() else anyFailed = true
        }
        if (stagedAvatar != null) {
            if (commitStaged(stagedAvatar, liveAvatar)) pendingAvatar.delete() else anyFailed = true
        } else if (hadPrefs && !anyFailed) {
            // The restore replaced the prefs but carried no avatar → the restored state has none. Clear
            // any live avatar so a previously-seeded default cover can't outlive the (now blank)
            // avatarDefaultId — otherwise the cover shows but the picker rings nothing. The one-time
            // seed re-runs cleanly on next Profile open. Gated on a clean prefs swap (!anyFailed).
            runCatching { liveAvatar.delete() }
        }
        return applied && !anyFailed
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
