package com.forge.app.core.io

import java.io.File
import java.io.RandomAccessFile

/**
 * Whether an image file is STRUCTURALLY COMPLETE — its container ends where it says it ends.
 *
 * ## Why the decoder cannot answer this
 *
 * The obvious check is "does it decode?", and it does not work. Android's `BitmapFactory` treats
 * incomplete input as success: Skia reports `kIncompleteInput`, the framework accepts that
 * alongside `kSuccess`, and the missing rows are filled in rather than refused. So
 * `decodeFile` returns a perfectly real `Bitmap` for a download that was cut off half way — one
 * with the top of the photo and grey where the rest should be. Bounds-only decoding is weaker
 * still: a header is the first few dozen bytes of a file that may be a hundredth of its length.
 *
 * `ImageDecoder` DOES reject partial images, by throwing unless an `OnPartialImageListener` says
 * otherwise — but it is API 28+, and this app supports 26. That leaves two API levels where the
 * platform offers no answer at all, which is exactly the situation this file exists for.
 *
 * ## What it checks instead
 *
 * Every container this app can receive states its own extent, so completeness is a property of the
 * bytes rather than of the decoder:
 *
 *  - **JPEG** ends with the `FF D9` End-of-Image marker. Byte stuffing inside entropy-coded data
 *    guarantees a literal `FF` is followed by `00` or a restart marker, so `FF D9` cannot occur as
 *    image data — finding it in the tail means the encoder wrote it.
 *  - **PNG** ends with the `IEND` chunk, whose CRC is a constant because its payload is empty.
 *  - **GIF** ends with the `3B` trailer byte.
 *  - **WebP** is RIFF: a length field in the header states the rest of the file's size.
 *  - **HEIF/AVIF** is ISO-BMFF: a chain of length-prefixed boxes that must tile the file exactly.
 *    A truncated one has a final box claiming more bytes than remain — which is precisely the
 *    iPhone-transfer case, since HEIC is what those arrive as.
 *
 * A format not on that list returns true rather than false. This is one check in a chain, not a
 * whitelist: refusing everything unrecognised would reject valid photos in some format nobody
 * thought of, which is a worse failure than the one being prevented.
 *
 * Pure `java.io` on purpose — no Android types — so the logic is unit-testable on the JVM, where
 * `BitmapFactory` is a stub that decodes nothing and `ImageDecoder` does not exist.
 */
internal object ImageIntegrity {

    /** How far back from EOF to look for a JPEG's end marker, allowing for encoder padding. */
    private const val JPEG_TAIL = 256

    /** `IEND` + its CRC, which is fixed: the chunk has no payload, so the CRC has one input. */
    private val PNG_IEND = byteArrayOf(0x49, 0x45, 0x4E, 0x44, 0xAE.toByte(), 0x42, 0x60, 0x82.toByte())

    /**
     * @return false only when the file's own container says it is unfinished. Unreadable files are
     *   false too — a photo that cannot be read now will not render later either.
     */
    fun looksComplete(file: File): Boolean = runCatching {
        val length = file.length()
        if (length < 16) return@runCatching false
        val head = ByteArray(16)
        file.inputStream().use { input ->
            var read = 0
            while (read < head.size) {
                val n = input.read(head, read, head.size - read)
                if (n < 0) break
                read += n
            }
            if (read < head.size) return@runCatching false
        }
        when {
            head.startsWith(0xFF, 0xD8) -> endsWithJpegMarker(file, length)
            head.startsWith(0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A) -> tailEquals(file, length, PNG_IEND)
            head.startsWith(0x47, 0x49, 0x46, 0x38) -> tailEquals(file, length, byteArrayOf(0x3B))
            head.ascii(0, "RIFF") && head.ascii(8, "WEBP") -> length >= head.leU32(4) + 8
            head.ascii(4, "ftyp") -> boxesTileFile(file, length)
            else -> true
        }
    }.getOrDefault(false)

    /**
     * JPEG's `FF D9`, searched backwards through the tail rather than required as the final two
     * bytes: cameras and editors append their own trailing data after the marker often enough that
     * demanding it sit exactly at EOF would reject real photos.
     */
    private fun endsWithJpegMarker(file: File, length: Long): Boolean {
        val window = minOf(JPEG_TAIL.toLong(), length).toInt()
        val tail = readTail(file, length, window) ?: return false
        for (i in tail.size - 2 downTo 0) {
            if (tail[i] == 0xFF.toByte() && tail[i + 1] == 0xD9.toByte()) return true
        }
        return false
    }

    private fun tailEquals(file: File, length: Long, expected: ByteArray): Boolean {
        if (length < expected.size) return false
        val tail = readTail(file, length, expected.size) ?: return false
        return tail.contentEquals(expected)
    }

    private fun readTail(file: File, length: Long, count: Int): ByteArray? = runCatching {
        RandomAccessFile(file, "r").use { raf ->
            raf.seek(length - count)
            ByteArray(count).also { raf.readFully(it) }
        }
    }.getOrNull()

    /**
     * Walk the ISO-BMFF box chain from byte zero and require it to land exactly on EOF.
     *
     * Each box is a 32-bit big-endian size covering the header itself, then a four-character type.
     * Size 1 means the real size is a 64-bit field that follows; size 0 means "to end of file",
     * which is legal only for the last box and is by definition complete. A truncated file fails
     * because its final box — nearly always the `mdat` holding the pixels — claims more bytes than
     * the file has left.
     */
    private fun boxesTileFile(file: File, length: Long): Boolean = runCatching {
        RandomAccessFile(file, "r").use { raf ->
            var offset = 0L
            var boxes = 0
            while (offset < length) {
                if (length - offset < 8) return@runCatching false // A partial box header is a cut file.
                raf.seek(offset)
                val header = ByteArray(8).also { raf.readFully(it) }
                var size = header.beU32(0)
                if (size == 1L) {
                    if (length - offset < 16) return@runCatching false
                    val large = ByteArray(8).also { raf.readFully(it) }
                    size = large.beU64()
                    if (size < 16) return@runCatching false
                } else if (size == 0L) {
                    // Extends to EOF by definition, so nothing is missing.
                    return@runCatching true
                } else if (size < 8) {
                    return@runCatching false // Not a length this format can express; refuse to guess.
                }
                if (size > length - offset) return@runCatching false // Overruns EOF: truncated.
                offset += size
                boxes++
            }
            offset == length && boxes > 0
        }
    }.getOrDefault(false)

    // ── Byte helpers ────────────────────────────────────────────────────────────────────────────

    private fun ByteArray.startsWith(vararg bytes: Int): Boolean {
        if (size < bytes.size) return false
        return bytes.withIndex().all { (i, b) -> this[i] == b.toByte() }
    }

    private fun ByteArray.ascii(at: Int, text: String): Boolean {
        if (size < at + text.length) return false
        return text.indices.all { this[at + it] == text[it].code.toByte() }
    }

    private fun ByteArray.leU32(at: Int): Long =
        (this[at].toLong() and 0xFF) or
            ((this[at + 1].toLong() and 0xFF) shl 8) or
            ((this[at + 2].toLong() and 0xFF) shl 16) or
            ((this[at + 3].toLong() and 0xFF) shl 24)

    private fun ByteArray.beU32(at: Int): Long =
        ((this[at].toLong() and 0xFF) shl 24) or
            ((this[at + 1].toLong() and 0xFF) shl 16) or
            ((this[at + 2].toLong() and 0xFF) shl 8) or
            (this[at + 3].toLong() and 0xFF)

    /** Clamped to [Long.MAX_VALUE] semantics by refusing negatives: a box cannot be that large. */
    private fun ByteArray.beU64(): Long {
        var value = 0L
        for (i in 0 until 8) {
            if (i == 0 && (this[0].toInt() and 0x80) != 0) return -1 // Would overflow a signed Long.
            value = (value shl 8) or (this[i].toLong() and 0xFF)
        }
        return value
    }
}
