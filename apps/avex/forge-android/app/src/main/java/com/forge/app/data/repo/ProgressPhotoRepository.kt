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

/**
 * One saved progress photo: an app-private image file + when it was taken + an optional note, and
 * the album it belongs to ([album] = "" means Unsorted). A photo lives in exactly one album.
 */
data class ProgressPhoto(
    val fileName: String,
    val takenAtMs: Long,
    val note: String = "",
    val album: String = ""
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
    // The ordered list of album names lives in its own file so an album can exist before it has any
    // photos. Both files sit inside dir/, so the backup archive + factory reset already cover them.
    private val albumsFile: File by lazy { File(dir, "albums.json") }

    /** All photos, newest first. */
    suspend fun photos(): List<ProgressPhoto> = withContext(Dispatchers.IO) {
        readIndex().sortedByDescending { it.takenAtMs }
    }

    fun fileFor(photo: ProgressPhoto): File = File(dir, photo.fileName)

    /** Import a picked image into app storage, copying its bytes as-is (EXIF orientation preserved). */
    suspend fun add(source: Uri, takenAtMs: Long, note: String = "", album: String = ""): ProgressPhoto? =
        withContext(Dispatchers.IO) {
            val fileName = "pp_${UUID.randomUUID().toString().take(12)}.jpg"
            val dest = File(dir, fileName)
            val ok = runCatching {
                context.contentResolver.openInputStream(source)?.use { input ->
                    dest.outputStream().use { input.copyTo(it) }
                } != null
            }.getOrDefault(false)
            if (!ok || dest.length() == 0L) { dest.delete(); return@withContext null }
            val photo = ProgressPhoto(fileName, takenAtMs, note, canonicalAlbum(album))
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

    /** Move a photo into [album] ("" = Unsorted). The target album need not pre-exist. */
    suspend fun setAlbum(photo: ProgressPhoto, album: String) = withContext(Dispatchers.IO) {
        val a = canonicalAlbum(album)
        writeIndex(readIndex().map { if (it.fileName == photo.fileName) it.copy(album = a) else it })
    }

    // ── Albums ───────────────────────────────────────────────────────────────
    /** The user's album names, in creation order (does not include the implicit "Unsorted"). */
    suspend fun albums(): List<String> = withContext(Dispatchers.IO) { readAlbums() }

    /** Create a named album (no-op for a blank name or a case-insensitive duplicate). Returns the
     *  CANONICAL name — the existing album's casing on a duplicate, else the new trimmed name — so
     *  callers open the real folder instead of spawning a second one that differs only in case. */
    suspend fun createAlbum(name: String): String = withContext(Dispatchers.IO) {
        val n = name.trim()
        if (n.isEmpty()) return@withContext ""
        val current = readAlbums()
        val existing = current.firstOrNull { it.equals(n, ignoreCase = true) }
        if (existing != null) return@withContext existing
        writeAlbums(current + n)
        n
    }

    /** Rename an album, carrying its photos over to the new name. Matches the old name case-insensitively. */
    suspend fun renameAlbum(old: String, new: String) = withContext(Dispatchers.IO) {
        val n = new.trim()
        if (n.isEmpty() || old.isBlank()) return@withContext
        writeAlbums(readAlbums().map { if (it.equals(old, ignoreCase = true)) n else it }.distinct())
        writeIndex(readIndex().map { if (it.album.equals(old, ignoreCase = true)) it.copy(album = n) else it })
    }

    /** Delete an album — its photos fall back to Unsorted (the images themselves are kept). Case-insensitive. */
    suspend fun deleteAlbum(name: String) = withContext(Dispatchers.IO) {
        if (name.isBlank()) return@withContext
        writeAlbums(readAlbums().filterNot { it.equals(name, ignoreCase = true) })
        writeIndex(readIndex().map { if (it.album.equals(name, ignoreCase = true)) it.copy(album = "") else it })
    }

    /** Resolve [name] to the canonical album casing — the existing album that matches case-insensitively,
     *  else the trimmed input ("" stays Unsorted) — so a photo's album field never drifts from the
     *  albums list and two casings of one name can't split into separate folders. */
    private fun canonicalAlbum(name: String): String {
        val n = name.trim()
        if (n.isEmpty()) return ""
        return readAlbums().firstOrNull { it.equals(n, ignoreCase = true) } ?: n
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
                // "album" is absent in pre-albums indexes → "" (Unsorted), so old data reads cleanly.
                ProgressPhoto(name, o.optLong("takenAtMs"), o.optString("note"), o.optString("album"))
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
                put("album", p.album)
            })
        }
        indexFile.writeText(arr.toString())
    }

    private fun readAlbums(): List<String> {
        if (!albumsFile.exists()) return emptyList()
        return runCatching {
            val arr = JSONArray(albumsFile.readText())
            (0 until arr.length()).mapNotNull { i -> arr.optString(i).trim().ifBlank { null } }
        }.getOrDefault(emptyList())
    }

    private fun writeAlbums(names: List<String>) {
        val arr = JSONArray()
        names.forEach { arr.put(it) }
        albumsFile.writeText(arr.toString())
    }
}
