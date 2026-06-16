package com.forge.app.data.repo

import android.content.Context
import android.net.Uri
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.UUID
import javax.inject.Inject
import javax.inject.Singleton

/** One saved progress photo: an app-private image file + when it was taken + an optional note. */
data class ProgressPhoto(
    val fileName: String,
    val takenAtMs: Long,
    val note: String = ""
)

/**
 * Progress-photo store (auto-coach "You" hub). Physique photos the user imports live as files in
 * **app-private** storage (`filesDir/progress_photos/`, never the camera roll), with a small JSON
 * index for dates/notes. Deliberately NOT a Room table — keeps photos off the schema (no migration)
 * and avoids coupling sensitive images to the DB. Offline, single-user, no network.
 *
 * Known follow-up: the whole-DB backup (#138) is the .db file only, so these image files aren't in
 * it yet — a later pass can fold the folder into the backup archive.
 */
@Singleton
class ProgressPhotoRepository @Inject constructor(
    @ApplicationContext private val context: Context
) {
    /** Public so the backup archive ([BackupRepository]) + factory reset can read/clear the folder. */
    val dir: File by lazy { File(context.filesDir, "progress_photos").apply { mkdirs() } }
    private val indexFile: File by lazy { File(dir, "index.json") }

    /** All photos, newest first. */
    suspend fun photos(): List<ProgressPhoto> = withContext(Dispatchers.IO) {
        readIndex().sortedByDescending { it.takenAtMs }
    }

    fun fileFor(photo: ProgressPhoto): File = File(dir, photo.fileName)

    /** Import a picked image into app storage, copying its bytes as-is (EXIF orientation preserved). */
    suspend fun add(source: Uri, takenAtMs: Long, note: String = ""): ProgressPhoto? =
        withContext(Dispatchers.IO) {
            val fileName = "pp_${UUID.randomUUID().toString().take(12)}.jpg"
            val dest = File(dir, fileName)
            val ok = runCatching {
                context.contentResolver.openInputStream(source)?.use { input ->
                    dest.outputStream().use { input.copyTo(it) }
                } != null
            }.getOrDefault(false)
            if (!ok || dest.length() == 0L) { dest.delete(); return@withContext null }
            val photo = ProgressPhoto(fileName, takenAtMs, note)
            writeIndex(readIndex() + photo)
            photo
        }

    suspend fun delete(photo: ProgressPhoto) = withContext(Dispatchers.IO) {
        File(dir, photo.fileName).delete()
        writeIndex(readIndex().filterNot { it.fileName == photo.fileName })
    }

    suspend fun setNote(photo: ProgressPhoto, note: String) = withContext(Dispatchers.IO) {
        writeIndex(readIndex().map { if (it.fileName == photo.fileName) it.copy(note = note) else it })
    }

    /** Wipe every progress photo + the index — called by factory reset (these files live outside the DB). */
    suspend fun deleteAll() = withContext(Dispatchers.IO) {
        dir.deleteRecursively()
        dir.mkdirs()
    }

    private fun readIndex(): List<ProgressPhoto> {
        if (!indexFile.exists()) return emptyList()
        return runCatching {
            val arr = JSONArray(indexFile.readText())
            (0 until arr.length()).mapNotNull { i ->
                val o = arr.optJSONObject(i) ?: return@mapNotNull null
                val name = o.optString("file").ifBlank { return@mapNotNull null }
                // Drop dangling index entries whose file was removed out-of-band.
                if (!File(dir, name).exists()) return@mapNotNull null
                ProgressPhoto(name, o.optLong("takenAtMs"), o.optString("note"))
            }
        }.getOrDefault(emptyList())
    }

    private fun writeIndex(photos: List<ProgressPhoto>) {
        val arr = JSONArray()
        photos.forEach { p ->
            arr.put(JSONObject().apply {
                put("file", p.fileName)
                put("takenAtMs", p.takenAtMs)
                put("note", p.note)
            })
        }
        indexFile.writeText(arr.toString())
    }
}
