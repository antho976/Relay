package com.forge.app.data.repo

import android.content.Context
import android.content.Intent
import android.net.Uri
import com.forge.app.data.prefs.SettingsRepository
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.first
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Who owns a persisted document-tree grant, and when Avex gives one up (M-18).
 *
 * A persisted URI permission lives until it is released or revoked — nothing expires it — so the
 * visible "connected folder" state has to be the whole truth about what the app can open. Two
 * settings can name the same tree (Downloads, most obviously): the backup folder holds it
 * read+write, the import folder read-only. That made ownership a question neither repository could
 * answer alone, and each got it wrong in its own direction:
 *
 *  - Backup released a tree it was replacing without asking whether import still pointed at it —
 *    fixed by an ad-hoc check inside `BackupRepository`, which import had no equivalent of.
 *  - Import overwrote its own preference and released NOTHING, so pointing import at a second
 *    folder left the first held with no setting naming it: invisible retained access, forever.
 *
 * One place to ask "does anything else still reference this tree", so the answer cannot differ
 * depending on which side is asking.
 */
@Singleton
open class PersistedTreeGrants @Inject constructor(
    @ApplicationContext private val context: Context,
    private val settingsRepo: SettingsRepository
) {
    /** Which setting is asking. Its own reference to a tree never counts as "someone else". */
    enum class Owner { BACKUP, IMPORT }

    /**
     * Take a persistable grant on [treeUri].
     *
     * @return false when the grant could not be taken — which callers must treat as "the folder was
     *   NOT connected". Swallowing this is how a failed replacement could move the preference to a
     *   folder the app cannot open and release the one it could: the picker's own transient grant
     *   makes the next few operations succeed, and access is gone by the time it matters.
     */
    open suspend fun take(treeUri: Uri, write: Boolean): Boolean = runCatching {
        val flags = Intent.FLAG_GRANT_READ_URI_PERMISSION or
            if (write) Intent.FLAG_GRANT_WRITE_URI_PERMISSION else 0
        context.contentResolver.takePersistableUriPermission(treeUri, flags)
        holds(treeUri)
    }.getOrDefault(false)

    /**
     * Give up the grant on [uriString], unless a setting other than [owner] still names that tree.
     *
     * Released with exactly the flags this app is actually holding: releasing a flag that was never
     * taken throws, so a tree taken read-only by an older build would otherwise fail the whole
     * release. Unparseable, already-released and never-held uris are all no-ops and report success —
     * there is nothing left to give up.
     *
     * @return false only when a release was attempted and Avex still holds the tree afterwards.
     */
    suspend fun releaseUnlessSharedWith(uriString: String, owner: Owner): Boolean {
        if (referencedByOthers(uriString, owner)) return true
        val uri = runCatching { Uri.parse(uriString) }.getOrNull() ?: return true
        val held = context.contentResolver.persistedUriPermissions.firstOrNull { it.uri == uri }
            ?: return true
        var flags = 0
        if (held.isReadPermission) flags = flags or Intent.FLAG_GRANT_READ_URI_PERMISSION
        if (held.isWritePermission) flags = flags or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
        if (flags == 0) return true
        runCatching { context.contentResolver.releasePersistableUriPermission(uri, flags) }
        return !holds(uri)
    }

    /** Whether any setting OTHER than [owner]'s still points at [uriString]. */
    private suspend fun referencedByOthers(uriString: String, owner: Owner): Boolean {
        val backup = settingsRepo.backupFolderUri.first()
        val import = settingsRepo.importFolderUri.first()
        return when (owner) {
            Owner.BACKUP -> import == uriString
            Owner.IMPORT -> backup == uriString
        }
    }

    private fun holds(uri: Uri): Boolean =
        context.contentResolver.persistedUriPermissions.any { it.uri == uri }
}
