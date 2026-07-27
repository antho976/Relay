package com.forge.app.data.repo

import android.content.Context
import android.net.Uri
import androidx.exifinterface.media.ExifInterface
import com.forge.app.data.db.dao.BodyweightDao
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.text.SimpleDateFormat
import java.util.Locale
import java.util.UUID
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.math.abs

/**
 * One saved progress photo: an app-private image file, when the shot was actually taken, an optional
 * note, the album it belongs to ([album] = "" means Unsorted), an optional [pose] tag (angle) and the
 * bodyweight recorded nearest that date ([weightLb], null when there was no weigh-in in range). A photo
 * lives in exactly one album and carries at most one pose.
 */
data class ProgressPhoto(
    val fileName: String,
    val takenAtMs: Long,
    val note: String = "",
    val album: String = "",
    val pose: String = "",
    val weightLb: Double? = null,
    /**
     * A short user label for the shot ("morning, fasted", "week 12"). Distinct from [note]: the title
     * is the caption the grid and the day header can show at a glance, the note is the long-form text
     * behind it. Optional and blank by default — added 2026-07-25 so the gallery search can match a
     * name you chose rather than only what the app recorded.
     */
    val title: String = ""
)

/**
 * Progress-photo store — the physique library behind the Profile gallery. Photos live as files in
 * **app-private** storage (`filesDir/progress_photos/`, never the camera roll), with a small JSON index
 * for their metadata. Deliberately NOT a Room table — keeps photos off the schema (no migration) and
 * avoids coupling sensitive images to the DB. Offline, single-user, no network.
 *
 * Imports read the real capture date from EXIF and snapshot the bodyweight nearest that date (from
 * [bodyweightDao]) so a photo carries the numbers that make it comparable over time. Every mutation
 * bumps [revision] so the several surfaces that show photos (Profile teaser, gallery, camera) refresh.
 */
@Singleton
class ProgressPhotoRepository @Inject constructor(
    @ApplicationContext private val context: Context,
    private val bodyweightDao: BodyweightDao
) {
    /** Public so the backup archive ([BackupRepository]) + factory reset can read/clear the folder. */
    val dir: File by lazy { File(context.filesDir, "progress_photos").apply { mkdirs() } }
    private val indexFile: File by lazy { File(dir, "index.json") }
    // The ordered list of album names lives in its own file so an album can exist before it has any
    // photos. Both files sit inside dir/, so the backup archive + factory reset already cover them.
    private val albumsFile: File by lazy { File(dir, "albums.json") }

    // Bumped after every write so collectors (the gallery + Profile ViewModels) reload. A StateFlow so
    // a fresh subscriber gets the current value at once and does its initial load off the same signal.
    private val _revision = MutableStateFlow(0L)
    val revision: StateFlow<Long> = _revision.asStateFlow()
    private fun bump() { _revision.value += 1 }

    // Serializes every read-modify-write on index.json / albums.json. Without it, two concurrent edits
    // (e.g. a pose tap + the deferred note/weight commit firing together from the viewer) each read the
    // index, mutate their copy, and the last writer clobbers the other's change. Non-reentrant, so only
    // the leaf mutators lock — callers (add/addCaptured) never lock and then call a locking helper.
    private val writeMutex = Mutex()

    /** All photos, newest first. */
    suspend fun photos(): List<ProgressPhoto> = withContext(Dispatchers.IO) {
        readIndex().sortedByDescending { it.takenAtMs }
    }

    fun fileFor(photo: ProgressPhoto): File = File(dir, photo.fileName)

    /**
     * Import a picked image into app storage, copying its bytes as-is (EXIF orientation preserved). The
     * photo is dated by its EXIF capture time (falling back to [takenAtMsOverride] then now), and tagged
     * with the bodyweight nearest that date.
     */
    suspend fun add(
        source: Uri,
        note: String = "",
        album: String = "",
        pose: String = "",
        takenAtMsOverride: Long? = null
    ): ProgressPhoto? = withContext(Dispatchers.IO) {
        val fileName = "pp_${UUID.randomUUID().toString().take(12)}.jpg"
        val dest = File(dir, fileName)
        val ok = runCatching {
            context.contentResolver.openInputStream(source)?.use { input ->
                dest.outputStream().use { input.copyTo(it) }
            } != null
        }.getOrDefault(false)
        if (!ok || dest.length() == 0L) { dest.delete(); return@withContext null }
        val takenAt = exifTakenAtMs(dest) ?: takenAtMsOverride ?: System.currentTimeMillis()
        index(fileName, takenAt, note, album, pose)
    }

    /**
     * Save a photo just captured by the in-app camera. The file already sits in cache; its bytes are
     * copied in and the temp removed. Capture time is "now" — a fresh shot has no meaningful EXIF date.
     */
    suspend fun addCaptured(temp: File, pose: String = "", album: String = ""): ProgressPhoto? =
        withContext(Dispatchers.IO) {
            if (!temp.exists() || temp.length() == 0L) return@withContext null
            val fileName = "pp_${UUID.randomUUID().toString().take(12)}.jpg"
            val dest = File(dir, fileName)
            val ok = runCatching { temp.copyTo(dest, overwrite = true); true }.getOrDefault(false)
            temp.delete()
            if (!ok || dest.length() == 0L) { dest.delete(); return@withContext null }
            index(fileName, System.currentTimeMillis(), "", album, pose)
        }

    /** Append a copied-in file to the index, snapshotting the nearest bodyweight for its date. */
    private suspend fun index(fileName: String, takenAtMs: Long, note: String, album: String, pose: String): ProgressPhoto {
        // Snapshot outside the lock (canonicalAlbum + the bodyweight read don't touch the index).
        val photo = ProgressPhoto(fileName, takenAtMs, note, canonicalAlbum(album), pose, nearestBodyweightLb(takenAtMs))
        writeMutex.withLock {
            writeIndex(readIndex() + photo)
            bump()
        }
        return photo
    }

    suspend fun delete(photo: ProgressPhoto) = withContext(Dispatchers.IO) {
        File(dir, photo.fileName).delete()
        writeMutex.withLock {
            writeIndex(readIndex().filterNot { it.fileName == photo.fileName })
            bump()
        }
    }

    suspend fun setNote(photo: ProgressPhoto, note: String) = updatePhoto(photo) { it.copy(note = note) }
    suspend fun setTitle(photo: ProgressPhoto, title: String) = updatePhoto(photo) { it.copy(title = title.trim()) }
    suspend fun setPose(photo: ProgressPhoto, pose: String) = updatePhoto(photo) { it.copy(pose = pose) }
    suspend fun setWeight(photo: ProgressPhoto, weightLb: Double?) = updatePhoto(photo) { it.copy(weightLb = weightLb) }
    /** Re-date a photo (its EXIF date was wrong/absent); re-snapshots the bodyweight for the new date. */
    suspend fun setTakenAt(photo: ProgressPhoto, takenAtMs: Long) {
        val weight = nearestBodyweightLb(takenAtMs)
        updatePhoto(photo) { it.copy(takenAtMs = takenAtMs, weightLb = weight) }
    }

    /** Move a photo into [album] ("" = Unsorted). The target album need not pre-exist. */
    suspend fun setAlbum(photo: ProgressPhoto, album: String) {
        val a = withContext(Dispatchers.IO) { canonicalAlbum(album) }
        updatePhoto(photo) { it.copy(album = a) }
    }

    private suspend fun updatePhoto(photo: ProgressPhoto, transform: (ProgressPhoto) -> ProgressPhoto) =
        withContext(Dispatchers.IO) {
            writeMutex.withLock {
                writeIndex(readIndex().map { if (it.fileName == photo.fileName) transform(it) else it })
                bump()
            }
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
        writeMutex.withLock {
            val current = readAlbums()
            val existing = current.firstOrNull { it.equals(n, ignoreCase = true) }
            if (existing != null) return@withContext existing
            writeAlbums(current + n)
            bump()
        }
        n
    }

    /** Rename an album, carrying its photos over to the new name. Matches the old name case-insensitively. */
    suspend fun renameAlbum(old: String, new: String) = withContext(Dispatchers.IO) {
        val n = new.trim()
        if (n.isEmpty() || old.isBlank()) return@withContext
        writeMutex.withLock {
            writeAlbums(readAlbums().map { if (it.equals(old, ignoreCase = true)) n else it }.distinct())
            writeIndex(readIndex().map { if (it.album.equals(old, ignoreCase = true)) it.copy(album = n) else it })
            bump()
        }
    }

    /** Delete an album — its photos fall back to Unsorted (the images themselves are kept). Case-insensitive. */
    suspend fun deleteAlbum(name: String) = withContext(Dispatchers.IO) {
        if (name.isBlank()) return@withContext
        writeMutex.withLock {
            writeAlbums(readAlbums().filterNot { it.equals(name, ignoreCase = true) })
            writeIndex(readIndex().map { if (it.album.equals(name, ignoreCase = true)) it.copy(album = "") else it })
            bump()
        }
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
        writeMutex.withLock {
            dir.deleteRecursively()
            dir.mkdirs()
            bump()
        }
    }

    // ── Metadata helpers ───────────────────────────────────────────────────────
    /** Bodyweight (lb) recorded nearest [takenAtMs], within [NEAR_WINDOW_MS]; null if none in range. */
    private suspend fun nearestBodyweightLb(takenAtMs: Long): Double? {
        val entries = runCatching { bodyweightDao.all() }.getOrDefault(emptyList())
        return entries
            .minByOrNull { abs(it.recordedAt - takenAtMs) }
            ?.takeIf { abs(it.recordedAt - takenAtMs) <= NEAR_WINDOW_MS }
            ?.weightLb
    }

    /**
     * The image's EXIF capture date in epoch millis, or null if absent/implausible. AndroidX
     * ExifInterface so HEIC (iPhone transfers, most modern phones), PNG and WebP dates are read on
     * every API level, not just JPEG. The timestamp is interpreted in the device's current zone —
     * EXIF times are camera-local and near-always shot on this same phone; being wrong by a zone
     * beats every photo landing on import day. A fresh non-lenient format per call ("0000:00:00…"
     * placeholders must fail to parse; SimpleDateFormat isn't thread-safe), then a plausibility
     * window so corrupt EXIF can't file a photo in 1970 or the future.
     */
    private fun exifTakenAtMs(file: File): Long? = runCatching {
        val exif = ExifInterface(file)
        val raw = exif.getAttribute(ExifInterface.TAG_DATETIME_ORIGINAL)
            ?: exif.getAttribute(ExifInterface.TAG_DATETIME_DIGITIZED)
            ?: exif.getAttribute(ExifInterface.TAG_DATETIME)
            ?: return null
        SimpleDateFormat("yyyy:MM:dd HH:mm:ss", Locale.US)
            .apply { isLenient = false }
            .parse(raw)?.time
            ?.takeIf { it in EXIF_MIN_MS..(System.currentTimeMillis() + EXIF_CLOCK_SLACK_MS) }
    }.getOrNull()

    private fun readIndex(): List<ProgressPhoto> {
        if (!indexFile.exists()) return emptyList()
        return runCatching {
            val arr = JSONArray(indexFile.readText())
            (0 until arr.length()).mapNotNull { i ->
                val o = arr.optJSONObject(i) ?: return@mapNotNull null
                val name = o.optString("file").ifBlank { return@mapNotNull null }
                // Drop dangling index entries whose file was removed out-of-band.
                if (!File(dir, name).exists()) return@mapNotNull null
                // Fields absent in older indexes read as their defaults, so pre-revamp data loads cleanly.
                ProgressPhoto(
                    name,
                    o.optLong("takenAtMs"),
                    o.optString("note"),
                    o.optString("album"),
                    o.optString("pose"),
                    if (o.has("weightLb") && !o.isNull("weightLb")) o.optDouble("weightLb") else null,
                    o.optString("title")
                )
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
                put("pose", p.pose)
                if (p.title.isNotBlank()) put("title", p.title)
                if (p.weightLb != null) put("weightLb", p.weightLb)
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

    private companion object {
        // A weigh-in this close to the shot is treated as "the weight at that time".
        const val NEAR_WINDOW_MS = 14L * 24 * 60 * 60 * 1000
        // Plausible capture dates: 2000-01-01 up to now + a day of camera-clock slack. Outside this,
        // the EXIF value is treated as corrupt and the import falls back to override/now.
        const val EXIF_MIN_MS = 946_684_800_000L
        const val EXIF_CLOCK_SLACK_MS = 24L * 60 * 60 * 1000
    }
}
