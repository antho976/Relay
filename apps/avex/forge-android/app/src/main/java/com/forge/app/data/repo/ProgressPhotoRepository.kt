package com.forge.app.data.repo

import android.content.Context
import android.net.Uri
import androidx.annotation.RequiresApi
import androidx.exifinterface.media.ExifInterface
import com.forge.app.core.io.ImageIntegrity
import com.forge.app.core.io.existsAtomically
import com.forge.app.core.io.readTextAtomically
import com.forge.app.core.io.writeTextAtomically
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
    val title: String = "",
    /**
     * The muscle groups this shot documents, as [com.forge.app.program.MuscleGroup] codes. A
     * SEPARATE axis from [pose]: the pose is where the camera stood, these are what the shot is
     * evidence of, and one back shot honestly covers both Back and Rear Delts. Empty = untagged.
     */
    val muscles: List<String> = emptyList(),
    /**
     * Free tags the user invented ("fasted", "week-12", "cut"), normalized by
     * [com.forge.app.domain.photo.PhotoTag]. Empty = untagged.
     */
    val tags: List<String> = emptyList()
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

    // Staging files of imports currently being copied in. [sweepOrphans] skips these, so a sweep
    // that lands mid-import (the first read of a cold process, say) cannot delete a copy in flight.
    private val inFlight: MutableSet<String> = java.util.concurrent.ConcurrentHashMap.newKeySet<String>()

    // The orphan sweep runs once per process, on the first read: that is "repository init" for a
    // lazily-constructed singleton, and it happens before any surface can show the library.
    @Volatile private var swept = false

    /** All photos, newest first. */
    suspend fun photos(): List<ProgressPhoto> = withContext(Dispatchers.IO) {
        if (!swept) { swept = true; sweepOrphans() }
        runCatching { readIndex().sortedByDescending { it.takenAtMs } }.getOrDefault(emptyList())
    }

    /**
     * Delete photo-shaped files in [dir] that the index does not name.
     *
     * An import used to copy straight to its final `pp_*.jpg` name and index it afterwards, so a
     * process death between the two left an image on disk that no gallery entry pointed at: hidden,
     * undeletable from the UI, and swept into every backup, because the archive zipped the whole
     * folder. Imports now stage under [STAGING_SUFFIX] and publish under the lock (see [publish]),
     * which narrows the window to the rename-then-index step; this sweep closes it, and clears any
     * staging file an older or interrupted process left behind.
     *
     * Skips in-flight staging files, and does nothing at all when the index cannot be read: an
     * unreadable index is exactly the state in which "not indexed" proves nothing.
     */
    suspend fun sweepOrphans() = withContext(Dispatchers.IO) {
        runCatching {
            writeMutex.withLock {
                val indexed = readIndex().mapTo(HashSet()) { it.fileName }
                orphanFileNames(dir.list().orEmpty().toList(), indexed, inFlight.toSet())
                    .forEach { File(dir, it).delete() }
            }
        }
    }

    /** What a backup carries from this library. See [backupSnapshot]. */
    class BackupSnapshot(
        /** Metadata files by basename, as bytes read through the same atomic path the app reads. */
        val metadata: Map<String, ByteArray>,
        /** The image files the index names, resolved through [safeFile]. */
        val photos: List<File>
    )

    /**
     * The files a backup archive should carry: the index and album list, plus the photo files the
     * index names. Never the folder's whole listing, which is what used to propagate an orphaned
     * image (and any stray file) into every archive.
     *
     * If the index cannot be parsed there is no way to tell an orphan from a photo, so every file
     * named the way this app names photos is carried along with the index bytes as they are: a
     * corrupt index should cost the user a repair, not their images.
     */
    fun backupSnapshot(): BackupSnapshot {
        val metadata = LinkedHashMap<String, ByteArray>()
        listOf(indexFile, albumsFile).forEach { f ->
            if (f.existsAtomically()) {
                runCatching { f.readTextAtomically().toByteArray(Charsets.UTF_8) }
                    .onSuccess { metadata[f.name] = it }
            }
        }
        val photos = runCatching { readIndex().mapNotNull { safeFile(it.fileName) } }
            .getOrElse { dir.list().orEmpty().filter { PHOTO_FILE_NAME.matches(it) }.mapNotNull { safeFile(it) } }
            .filter { it.isFile }
        return BackupSnapshot(metadata, photos)
    }

    /**
     * The file backing [photo], or null when its recorded name is not one this app could have
     * written. See [safeFile] — every filesystem operation on an index-supplied name goes through it.
     */
    fun fileForOrNull(photo: ProgressPhoto): File? = safeFile(photo.fileName)

    /** [fileForOrNull] for callers that only render — a non-existent file reads as a missing image. */
    fun fileFor(photo: ProgressPhoto): File = safeFile(photo.fileName) ?: File(dir, UNSAFE_PLACEHOLDER)

    /**
     * Resolve an index-supplied file name to a real file inside [dir], or null if it is not one.
     *
     * The ZIP restore validates ENTRY names — flat basenames only, no separators — and then trusts
     * `progress_photos/index.json`, which is one of those entries and is metadata the app reads back
     * as instructions. An index whose `file` reads `../../databases/forge.db` produced a
     * "photo" whose backing file was the live database: it appeared in the gallery, and the Delete
     * action deleted it. Any app-private file the attacker could name was reachable that way.
     *
     * Two independent checks, because either alone is a single point of failure:
     *
     *  1. The NAME must be one this app writes — `pp_<id>.jpg`, from [add] / [addCaptured]. That
     *     excludes every separator and traversal sequence by construction, and `index.json` and
     *     `albums.json` with them.
     *  2. The RESOLVED path's canonical parent must be [dir] itself. Cheap, and it holds even if the
     *     name rule is ever loosened — a symlink or an encoding trick that survives (1) does not
     *     survive being resolved and compared.
     */
    private fun safeFile(fileName: String): File? {
        if (!PHOTO_FILE_NAME.matches(fileName)) return null
        return runCatching {
            val candidate = File(dir, fileName).canonicalFile
            candidate.takeIf { it.parentFile == dir.canonicalFile }
        }.getOrNull()
    }

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
        muscles: List<String> = emptyList(),
        takenAtMsOverride: Long? = null
    ): ProgressPhoto? = withContext(Dispatchers.IO) {
        val fileName = newFileName()
        val staging = stagingFile(fileName)
        inFlight += staging.name
        try {
            // BOUNDED. `copyTo` reads until the stream ends, and the stream belongs to a content
            // provider chosen by the user from the system picker — a huge file, or a provider that never
            // terminates, wrote until internal storage was full, taking the database's room with it. A
            // photo over the cap is refused with its partial file removed rather than truncated into
            // something that looks like a valid image.
            val ok = runCatching {
                context.contentResolver.openInputStream(source)?.use { input ->
                    staging.outputStream().use { output -> copyAtMost(input, output, MAX_PHOTO_BYTES) }
                } ?: false
            }.getOrDefault(false)
            if (!ok || staging.length() == 0L) { staging.delete(); return@withContext null }
            if (!isDecodableImage(staging)) { staging.delete(); return@withContext null }
            val takenAt = exifTakenAtMs(staging) ?: takenAtMsOverride ?: System.currentTimeMillis()
            runCatching { publish(staging, fileName, takenAt, note, album, pose, muscles) }
                .getOrElse { staging.delete(); null }
        } finally {
            inFlight -= staging.name
        }
    }

    /**
     * Save a photo just captured by the in-app camera. The file already sits in cache; its bytes are
     * copied in, and [temp] is removed ONLY once the photo is published. Capture time is "now" — a
     * fresh shot has no meaningful EXIF date.
     *
     * A null result leaves [temp] exactly where it was. It is the only copy of a shot the user has
     * just taken, and deleting it on a failed copy, a refused decode or an unwritable index turned
     * a transient failure into a lost photo; kept, the caller can offer a retry against the same
     * file (see `ProgressCameraViewModel`).
     */
    suspend fun addCaptured(temp: File, pose: String = "", album: String = ""): ProgressPhoto? =
        withContext(Dispatchers.IO) {
            if (!temp.exists() || temp.length() == 0L) return@withContext null
            val fileName = newFileName()
            val staging = stagingFile(fileName)
            inFlight += staging.name
            try {
                // Bounded like [add], even though this source is the app's own camera temp file rather
                // than a provider the user picked: the asymmetry was the only thing making one of these
                // two paths safe and the other trusting.
                val ok = runCatching {
                    temp.inputStream().use { input ->
                        staging.outputStream().use { output -> copyAtMost(input, output, MAX_PHOTO_BYTES) }
                    }
                }.getOrDefault(false)
                if (!ok || staging.length() == 0L) { staging.delete(); return@withContext null }
                if (!isDecodableImage(staging)) { staging.delete(); return@withContext null }
                val photo = runCatching { publish(staging, fileName, System.currentTimeMillis(), "", album, pose, emptyList()) }
                    .getOrElse { staging.delete(); null }
                if (photo != null) temp.delete()
                photo
            } finally {
                inFlight -= staging.name
            }
        }

    private fun newFileName(): String = "pp_${UUID.randomUUID().toString().take(12)}.jpg"

    /** Where an import's bytes land until they are published: beside the final name, never AS it. */
    private fun stagingFile(fileName: String): File = File(dir, "$fileName$STAGING_SUFFIX")

    /**
     * Publish a validated [staging] file as [fileName] and append it to the index, snapshotting the
     * nearest bodyweight for its date. Null when the file could not be moved into place.
     *
     * Both steps happen under the write lock, in this order: read the index (a corrupt index throws
     * HERE, before any file is touched), rename the staging file to its final name, then commit the
     * index naming it. A process death before the rename leaves a staging file; one between the
     * rename and the commit leaves an unindexed `pp_*.jpg`. Both are exactly what [sweepOrphans]
     * removes, so neither can become a hidden image. A commit that throws unwinds the rename.
     */
    private suspend fun publish(
        staging: File,
        fileName: String,
        takenAtMs: Long,
        note: String,
        album: String,
        pose: String,
        muscles: List<String>
    ): ProgressPhoto? {
        // Snapshot outside the lock (canonicalAlbum + the bodyweight read don't touch the index).
        val photo = ProgressPhoto(
            fileName, takenAtMs, note, canonicalAlbum(album), pose, nearestBodyweightLb(takenAtMs),
            muscles = muscles
        )
        return writeMutex.withLock {
            val current = readIndex()
            val dest = File(dir, fileName)
            if (!staging.renameTo(dest)) {
                staging.delete()
                return@withLock null
            }
            try {
                writeIndex(current + photo)
            } catch (failure: Throwable) {
                dest.delete()
                throw failure
            }
            bump()
            photo
        }
    }

    /**
     * Delete a photo: its BYTES first, then its metadata.
     *
     * The old order removed the index entry, then called `File.delete()` and ignored the result. A
     * failed delete — or a process death between the two — left the image on disk with nothing
     * pointing at it: gone from the gallery, still in app-private storage, and still swept into
     * every future backup archive, because [BackupRepository] zips the whole folder rather than the
     * indexed subset. "Deleted" has to mean the bytes are gone, so the bytes go first and the index
     * entry is only dropped once they are verified absent.
     */
    suspend fun delete(photo: ProgressPhoto) = withContext(Dispatchers.IO) {
        runCatching {
            writeMutex.withLock {
                val file = safeFile(photo.fileName)
                // An unsafe or already-absent file leaves nothing to delete — drop the entry so a
                // hostile or stale index row cannot become permanently undeletable.
                if (file != null && file.exists() && !file.delete() && file.exists()) {
                    return@withLock
                }
                writeIndex(readIndex().filterNot { it.fileName == photo.fileName })
                bump()
            }
        }
    }

    suspend fun setNote(photo: ProgressPhoto, note: String) = updatePhoto(photo) { it.copy(note = note) }
    suspend fun setTitle(photo: ProgressPhoto, title: String) = updatePhoto(photo) { it.copy(title = title.trim()) }
    suspend fun setPose(photo: ProgressPhoto, pose: String) = updatePhoto(photo) { it.copy(pose = pose) }
    /** Replace a photo's muscle tags wholesale (the viewer toggles chips and hands back the new set). */
    suspend fun setMuscles(photo: ProgressPhoto, muscles: List<String>) =
        updatePhoto(photo) { it.copy(muscles = muscles.distinct()) }
    /** Replace a photo's free tags wholesale; values are expected already normalized by `PhotoTag`. */
    suspend fun setTags(photo: ProgressPhoto, tags: List<String>) =
        updatePhoto(photo) { it.copy(tags = tags.distinct()) }
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
            runCatching {
                writeMutex.withLock {
                    writeIndex(readIndex().map { if (it.fileName == photo.fileName) transform(it) else it })
                    bump()
                }
            }
        }

    // ── Albums ───────────────────────────────────────────────────────────────
    /** The user's album names, in creation order (does not include the implicit "Unsorted"). */
    suspend fun albums(): List<String> = withContext(Dispatchers.IO) {
        runCatching { readAlbums() }.getOrDefault(emptyList())
    }

    /** Create a named album (no-op for a blank name or a case-insensitive duplicate). Returns the
     *  CANONICAL name — the existing album's casing on a duplicate, else the new trimmed name — so
     *  callers open the real folder instead of spawning a second one that differs only in case. */
    suspend fun createAlbum(name: String): String = withContext(Dispatchers.IO) {
        val n = name.trim()
        if (n.isEmpty()) return@withContext ""
        runCatching {
            writeMutex.withLock {
                val current = readAlbums()
                val existing = current.firstOrNull { it.equals(n, ignoreCase = true) }
                if (existing != null) return@withLock existing
                writeAlbums(current + n)
                bump()
                n
            }
        }.getOrDefault("")
    }

    /** Rename an album, carrying its photos over to the new name. Matches the old name case-insensitively. */
    suspend fun renameAlbum(old: String, new: String) = withContext(Dispatchers.IO) {
        val n = new.trim()
        if (n.isEmpty() || old.isBlank()) return@withContext
        runCatching {
            writeMutex.withLock {
                val albums = readAlbums()
                val photos = readIndex()
                writeAlbums(albums.map { if (it.equals(old, ignoreCase = true)) n else it }.distinct())
                writeIndex(photos.map { if (it.album.equals(old, ignoreCase = true)) it.copy(album = n) else it })
                bump()
            }
        }
    }

    /** Delete an album — its photos fall back to Unsorted (the images themselves are kept). Case-insensitive. */
    suspend fun deleteAlbum(name: String) = withContext(Dispatchers.IO) {
        if (name.isBlank()) return@withContext
        runCatching {
            writeMutex.withLock {
                val albums = readAlbums()
                val photos = readIndex()
                writeAlbums(albums.filterNot { it.equals(name, ignoreCase = true) })
                writeIndex(photos.map { if (it.album.equals(name, ignoreCase = true)) it.copy(album = "") else it })
                bump()
            }
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
    /**
     * Does this file actually contain a decodable image?
     *
     * The copy was bounded and the partial output cleaned up, but ANY non-empty byte stream was
     * then indexed as `pp_*.jpg`: a text file, an archive, a truncated download, a provider that
     * returned an error page. The gallery got a permanent entry that renders as a grey box, the
     * backup carried it, and nothing anywhere said what had gone wrong.
     *
     * Two checks, and NEITHER of them is a pixel decode, because a pixel decode does not answer the
     * question.
     *
     * The previous attempt at truncation assumed that decoding the pixels would fail where reading
     * the header had not. It does not: Android treats Skia's `kIncompleteInput` as success
     * alongside `kSuccess`, and Skia fills the rows that never arrived. `decodeFile` hands back a
     * real `Bitmap` for a file cut off half way — the photo down to the cut, then grey — so the
     * check passed and the grey-box entry was indexed anyway. The decode cost a downsampled bitmap
     * per import and bought nothing.
     *
     * What is left is the header and the container:
     *
     *  - `inJustDecodeBounds` proves the file BEGINS like an image and has real dimensions, which
     *    is what rejects a text file, an archive, or a provider's error page.
     *  - [ImageIntegrity.looksComplete] proves it ENDS where its own container says it should —
     *    JPEG's EOI marker, PNG's IEND, the RIFF length, the ISO-BMFF box chain. That is the check
     *    a truncated file fails, and it reads a few hundred bytes rather than decoding anything.
     *
     * On API 28+ `ImageDecoder` is asked as well, because it is the one platform decoder that
     * refuses incomplete input rather than papering over it — a file whose container is intact but
     * whose compressed data is damaged inside gets caught there. It cannot be the only check: this
     * app supports API 26, where `ImageDecoder` does not exist.
     */
    private fun isDecodableImage(file: File): Boolean = runCatching {
        val bounds = android.graphics.BitmapFactory.Options().apply { inJustDecodeBounds = true }
        android.graphics.BitmapFactory.decodeFile(file.path, bounds)
        if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return@runCatching false
        if (!ImageIntegrity.looksComplete(file)) return@runCatching false
        decodesWholeOrUnknown(file)
    }.getOrDefault(false)

    /**
     * `ImageDecoder`'s verdict on API 28+, or true below it.
     *
     * Left at its default `OnPartialImageListener` — absent — which is what makes it throw
     * `DecodeException` on incomplete input instead of returning the partial image. Setting one to
     * return true would restore exactly the behaviour this is here to avoid.
     *
     * True on API 26–27 is not a hole: [ImageIntegrity.looksComplete] has already run there and is
     * what catches truncation. This adds the interior-corruption case on the versions that can
     * answer it, rather than gating imports on an API level.
     */
    private fun decodesWholeOrUnknown(file: File): Boolean =
        if (android.os.Build.VERSION.SDK_INT >= 28) decodesWhole(file) else true

    @RequiresApi(28)
    private fun decodesWhole(file: File): Boolean = runCatching {
        val source = android.graphics.ImageDecoder.createSource(file)
        android.graphics.ImageDecoder.decodeBitmap(source) { decoder, _, _ ->
            // Sampled down purely to keep the allocation small; the decode still reads every byte,
            // which is the part that matters here.
            decoder.setTargetSampleSize(8)
        }.recycle()
        true
    }.getOrDefault(false)

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

    /**
     * Copy [input] into [output], stopping and reporting false once [maxBytes] is exceeded.
     *
     * Mirrors [BackupRepository]'s `copyAtMost`, deliberately rather than sharing it: this one
     * writes to a stream the caller owns, and the two callers' cleanup differs.
     */
    private fun copyAtMost(input: java.io.InputStream, output: java.io.OutputStream, maxBytes: Long): Boolean {
        val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
        var total = 0L
        while (true) {
            val read = input.read(buffer)
            if (read < 0) return true
            total += read
            if (total > maxBytes) return false
            output.write(buffer, 0, read)
        }
    }

    private fun readIndex(): List<ProgressPhoto> {
        if (!indexFile.existsAtomically()) return emptyList()
        val arr = JSONArray(indexFile.readTextAtomically())
        return (0 until arr.length()).mapNotNull { i ->
            val o = arr.getJSONObject(i)
            val name = o.getString("file").trim().ifBlank {
                throw IllegalStateException("Progress photo index contains a blank file name")
            }
            // A name this app could not have written is not a photo, whatever it points at. Dropped
            // rather than thrown: an index carrying one entry of hostile metadata should cost the
            // user that entry, not their whole library.
            val file = safeFile(name) ?: return@mapNotNull null
            // Drop dangling index entries whose file was removed out-of-band.
            if (!file.exists()) return@mapNotNull null
            // Fields absent in older indexes read as their defaults, so pre-revamp data loads cleanly.
            ProgressPhoto(
                name,
                o.getLong("takenAtMs"),
                o.optString("note"),
                o.optString("album"),
                o.optString("pose"),
                if (o.has("weightLb") && !o.isNull("weightLb")) o.optDouble("weightLb") else null,
                o.optString("title"),
                stringList(o.optJSONArray("muscles")),
                stringList(o.optJSONArray("tags"))
            )
        }
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
                // Omitted entirely when empty, so an untagged library's index stays byte-identical
                // to what the pre-tag build wrote and a downgrade reads it back cleanly.
                if (p.muscles.isNotEmpty()) put("muscles", JSONArray(p.muscles))
                if (p.tags.isNotEmpty()) put("tags", JSONArray(p.tags))
            })
        }
        indexFile.writeTextAtomically(arr.toString())
    }

    /** Read a JSON string array as a clean list, dropping blanks. Absent array reads as empty. */
    private fun stringList(arr: JSONArray?): List<String> {
        if (arr == null) return emptyList()
        return (0 until arr.length()).mapNotNull { i -> arr.optString(i).trim().ifBlank { null } }
    }

    private fun readAlbums(): List<String> {
        if (!albumsFile.existsAtomically()) return emptyList()
        val arr = JSONArray(albumsFile.readTextAtomically())
        return (0 until arr.length()).mapNotNull { i -> arr.getString(i).trim().ifBlank { null } }
    }

    private fun writeAlbums(names: List<String>) {
        val arr = JSONArray()
        names.forEach { arr.put(it) }
        albumsFile.writeTextAtomically(arr.toString())
    }

    internal companion object {
        // A weigh-in this close to the shot is treated as "the weight at that time".
        const val NEAR_WINDOW_MS = 14L * 24 * 60 * 60 * 1000

        /**
         * Suffix of an import's staging file (`pp_<id>.jpg.tmp`). It cannot match [PHOTO_FILE_NAME],
         * so a staging file is never a photo, however far the import got.
         */
        const val STAGING_SUFFIX = ".tmp"

        /**
         * Which of the names [present] in the photo folder are orphans: staging files, and
         * photo-named files the index does not list. Everything else (the index, the album list, a
         * name this app never writes) is left alone, as is anything in [inFlight]. Pure so the
         * decision is testable apart from the filesystem.
         */
        fun orphanFileNames(present: Collection<String>, indexed: Set<String>, inFlight: Set<String>): List<String> =
            present.filter { name ->
                if (name in inFlight) return@filter false
                val isStaging = name.endsWith(STAGING_SUFFIX) &&
                    PHOTO_FILE_NAME.matches(name.removeSuffix(STAGING_SUFFIX))
                isStaging || (PHOTO_FILE_NAME.matches(name) && name !in indexed)
            }
        // Plausible capture dates: 2000-01-01 up to now + a day of camera-clock slack. Outside this,
        // the EXIF value is treated as corrupt and the import falls back to override/now.
        const val EXIF_MIN_MS = 946_684_800_000L
        const val EXIF_CLOCK_SLACK_MS = 24L * 60 * 60 * 1000

        /**
         * The only shape a progress-photo file name may take: exactly what [add] and [addCaptured]
         * write, `pp_` + a UUID fragment + `.jpg`.
         *
         * Anchored, and the character class admits no separator, no dot beyond the extension and no
         * traversal sequence — so "is this a name we wrote" and "is this inside our folder" are the
         * same question, and it is answered before the string ever reaches the filesystem.
         */
        val PHOTO_FILE_NAME = Regex("""^pp_[0-9a-fA-F-]{1,64}\.jpg$""")

        /**
         * A name that matches nothing on disk, handed to render-only callers in place of an unsafe
         * one. They already treat a missing file as a missing image.
         */
        const val UNSAFE_PLACEHOLDER = "pp_unsafe_entry.jpg"

        /**
         * Ceiling for one imported photo. Comfortably past any phone camera's full-resolution
         * output — a 200 MP shot is a few tens of MB — and far below what it takes to fill a device.
         */
        const val MAX_PHOTO_BYTES = 64L * 1024 * 1024
    }
}
