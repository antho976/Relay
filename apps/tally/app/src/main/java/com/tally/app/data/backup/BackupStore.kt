package com.tally.app.data.backup

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import com.tally.app.data.Clock
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.DataRepository
import com.tally.app.data.repo.DataResult
import com.tally.app.work.BackupWorker
import com.tally.core.BackupCodec
import com.tally.core.BackupReadResult
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import java.io.File
import java.time.Instant
import java.time.LocalDateTime
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import javax.inject.Inject
import javax.inject.Singleton

/** One backup kept on the phone: its file name, when it was written, and its size. */
data class BackupCopy(val name: String, val savedAt: Long, val bytes: Long)

/**
 * The automatic backup, Avex's model: a restorable JSON copy of everything, written weekly (and on
 * demand) into the app's own storage, the newest [KEEP] kept, and a second copy into a folder the
 * owner picked, which outlives an uninstall. Nothing leaves the phone: the folder is on it (or on
 * whatever the owner's file picker put there by their own choice).
 */
@Singleton
class BackupStore @Inject constructor(
    @ApplicationContext private val context: Context,
    private val data: DataRepository,
    private val settings: SettingsRepository,
    private val clock: Clock,
) {
    private val dir: File get() = File(context.filesDir, DIR)
    private val lock = Mutex()

    private val copiesFlow = MutableStateFlow<List<BackupCopy>>(emptyList())

    /** The copies kept on the phone, newest first. Read again with [refresh]. */
    val copies: StateFlow<List<BackupCopy>> = copiesFlow.asStateFlow()

    suspend fun refresh() {
        copiesFlow.value = withContext(Dispatchers.IO) { listCopies() }
    }

    private fun listCopies(): List<BackupCopy> =
        (dir.listFiles { f -> f.isFile && f.name.startsWith(PREFIX) && f.name.endsWith(".json") } ?: emptyArray())
            .map { BackupCopy(it.name, it.lastModified(), it.length()) }
            .sortedByDescending { it.name }

    /**
     * Writes a backup now: the copy on the phone first (the one that must not fail), then the
     * folder copy when a folder is set. Records how it went, so Settings can say so.
     */
    suspend fun backupNow(): DataResult = lock.withLock {
        withContext(Dispatchers.IO) {
            val now = clock.nowMillis()
            val text = runCatching { BackupCodec.encode(data.snapshot()) }.getOrElse {
                settings.recordBackup(now, failed = true, folderFailed = false)
                return@withContext DataResult.Failed("The backup could not be written. Your data is unchanged.")
            }
            val name = fileName(now)
            val local = runCatching {
                dir.mkdirs()
                val tmp = File(dir, "$name.tmp")
                tmp.writeText(text)
                check(tmp.renameTo(File(dir, name)))
                prune()
            }
            if (local.isFailure) {
                settings.recordBackup(now, failed = true, folderFailed = false)
                return@withContext DataResult.Failed("The backup could not be written. Free some space, then try again.")
            }
            val folder = settings.currentBackup().folderUri
            val folderOk = folder == null || runCatching { writeToFolder(Uri.parse(folder), name, text) }.isSuccess
            settings.recordBackup(now, failed = false, folderFailed = !folderOk)
            copiesFlow.value = listCopies()
            if (folderOk) DataResult.Done("Backup saved") else DataResult.Failed("Saved on this phone. The folder copy failed; choose the folder again.")
        }
    }

    /** Turns the weekly run on or off, and keeps the switch. */
    suspend fun setAuto(on: Boolean) {
        settings.setBackupAuto(on)
        BackupWorker.sync(context, on)
    }

    /**
     * Takes lasting access to the folder the owner picked and makes it the backup folder, letting
     * the old one go. False when the picker's grant cannot be kept (a provider that refuses it).
     */
    suspend fun adoptFolder(uri: Uri): Boolean = withContext(Dispatchers.IO) {
        val resolver = context.contentResolver
        runCatching {
            resolver.takePersistableUriPermission(uri, FOLDER_FLAGS)
            val old = settings.currentBackup().folderUri
            if (old != null && old != uri.toString()) {
                runCatching { resolver.releasePersistableUriPermission(Uri.parse(old), FOLDER_FLAGS) }
            }
            settings.setBackupFolder(uri.toString())
        }.isSuccess
    }

    /** Stops copying into the folder; what is already there stays. */
    suspend fun forgetFolder() = withContext(Dispatchers.IO) {
        val old = settings.currentBackup().folderUri
        if (old != null) runCatching { context.contentResolver.releasePersistableUriPermission(Uri.parse(old), FOLDER_FLAGS) }
        settings.setBackupFolder(null)
    }

    /** Reads a kept copy back, checked like any backup file. */
    suspend fun read(copy: BackupCopy): BackupReadResult = withContext(Dispatchers.IO) {
        val file = File(dir, copy.name)
        val text = runCatching { file.readText() }.getOrNull()
            ?: return@withContext BackupReadResult.Invalid("That copy could not be opened.")
        BackupCodec.decode(text)
    }

    /** Keeps the newest [KEEP] copies on the phone. */
    private fun prune() {
        listCopies().drop(KEEP).forEach { File(dir, it.name).delete() }
    }

    /** Writes [text] as [name] into the picked folder, then keeps the newest [KEEP] of Tally's copies there. */
    private fun writeToFolder(tree: Uri, name: String, text: String) {
        val resolver = context.contentResolver
        val parent = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
        val doc = DocumentsContract.createDocument(resolver, parent, "application/json", name) ?: error("no document")
        resolver.openOutputStream(doc, "wt")?.use { it.write(text.toByteArray()) } ?: error("no stream")
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
        val ours = ArrayList<Pair<String, String>>()
        resolver.query(
            children,
            arrayOf(DocumentsContract.Document.COLUMN_DOCUMENT_ID, DocumentsContract.Document.COLUMN_DISPLAY_NAME),
            null, null, null,
        )?.use { c ->
            while (c.moveToNext()) {
                val id = c.getString(0) ?: continue
                val display = c.getString(1) ?: continue
                if (display.startsWith(PREFIX) && display.endsWith(".json")) ours += id to display
            }
        }
        ours.sortedByDescending { it.second }.drop(KEEP).forEach { (id, _) ->
            runCatching { DocumentsContract.deleteDocument(resolver, DocumentsContract.buildDocumentUriUsingTree(tree, id)) }
        }
    }

    private fun fileName(millis: Long): String {
        val time = LocalDateTime.ofInstant(Instant.ofEpochMilli(millis), ZoneId.systemDefault())
        return PREFIX + time.format(STAMP) + ".json"
    }

    companion object {
        const val DIR = "backups"
        const val PREFIX = "tally-auto-"
        /** Five weekly copies: a month and a bit to notice something went wrong. */
        const val KEEP = 5
        private val STAMP: DateTimeFormatter = DateTimeFormatter.ofPattern("yyyy-MM-dd-HHmmss")
        private const val FOLDER_FLAGS = Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
    }
}
