package com.forge.app.core.io

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

/**
 * The truncation check that the decoder cannot make.
 *
 * `BitmapFactory` accepts a file cut off half way — Skia reports incomplete input, Android counts
 * that as success, and the missing rows come back grey. So a photo import that only asked "does it
 * decode?" indexed the grey box, put it in the gallery permanently, and carried it into every
 * backup. These pin the container-level answer instead, on byte patterns rather than a decoder,
 * which is also why they can run on the JVM at all.
 */
class ImageIntegrityTest {

    @get:Rule val tmp = TemporaryFolder()

    private fun bytes(vararg values: Int) = ByteArray(values.size) { values[it].toByte() }

    private fun file(name: String, vararg parts: ByteArray): File =
        File(tmp.root, name).apply { writeBytes(parts.reduce { a, b -> a + b }) }

    /** Filler that is not a marker, so it can never be mistaken for a terminator. */
    private fun padding(n: Int) = ByteArray(n) { 0x42 }

    // ── JPEG ────────────────────────────────────────────────────────────────────────────────────

    private val jpegHead = bytes(0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01)
    private val eoi = bytes(0xFF, 0xD9)

    @Test
    fun `a jpeg that ends with its end-of-image marker is complete`() {
        assertTrue(ImageIntegrity.looksComplete(file("a.jpg", jpegHead, padding(400), eoi)))
    }

    @Test
    fun `a jpeg cut off before its end marker is not`() {
        // The exact shape of a download that stopped: a valid header, real dimensions, no ending.
        assertFalse(ImageIntegrity.looksComplete(file("b.jpg", jpegHead, padding(400))))
    }

    @Test
    fun `trailing data after the end marker does not make a jpeg incomplete`() {
        // Cameras and editors append their own blocks past EOI. Requiring the marker to sit exactly
        // at EOF would reject real photos, so it is searched for in the tail.
        assertTrue(ImageIntegrity.looksComplete(file("c.jpg", jpegHead, padding(200), eoi, padding(40))))
    }

    @Test
    fun `a jpeg whose end marker fell outside the search window is refused`() {
        // Deliberately pinning the bound: past the tail window the answer is "cannot confirm", and
        // this check resolves that as incomplete rather than guessing.
        assertFalse(ImageIntegrity.looksComplete(file("d.jpg", jpegHead, eoi, padding(4096))))
    }

    // ── PNG ─────────────────────────────────────────────────────────────────────────────────────

    private val pngHead = bytes(0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A)
    private val iend = bytes(0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82)

    @Test
    fun `a png that ends with IEND is complete`() {
        assertTrue(ImageIntegrity.looksComplete(file("a.png", pngHead, padding(300), iend)))
    }

    @Test
    fun `a png without its IEND chunk is not`() {
        assertFalse(ImageIntegrity.looksComplete(file("b.png", pngHead, padding(300))))
    }

    // ── GIF ─────────────────────────────────────────────────────────────────────────────────────

    @Test
    fun `a gif is judged by its trailer byte`() {
        val head = bytes(0x47, 0x49, 0x46, 0x38, 0x39, 0x61)
        assertTrue(ImageIntegrity.looksComplete(file("a.gif", head, padding(60), bytes(0x3B))))
        assertFalse(ImageIntegrity.looksComplete(file("b.gif", head, padding(60))))
    }

    // ── WebP ────────────────────────────────────────────────────────────────────────────────────

    /** RIFF header declaring [payload] bytes to follow the length field. */
    private fun riff(payload: Int) = bytes(0x52, 0x49, 0x46, 0x46) +
        bytes(payload and 0xFF, (payload shr 8) and 0xFF, (payload shr 16) and 0xFF, (payload shr 24) and 0xFF) +
        bytes(0x57, 0x45, 0x42, 0x50)

    @Test
    fun `a webp is judged by the size its RIFF header declares`() {
        // 4 bytes of "WEBP" plus 200 of payload is what the header promises; delivering it is
        // complete, and delivering less is a file that stopped early.
        assertTrue(ImageIntegrity.looksComplete(file("a.webp", riff(204), padding(200))))
        assertFalse(ImageIntegrity.looksComplete(file("b.webp", riff(204), padding(100))))
    }

    // ── HEIF / AVIF ─────────────────────────────────────────────────────────────────────────────

    /** One ISO-BMFF box: a big-endian size covering the 8-byte header, then a four-character type. */
    private fun box(type: String, payload: ByteArray): ByteArray {
        val size = payload.size + 8
        return bytes((size shr 24) and 0xFF, (size shr 16) and 0xFF, (size shr 8) and 0xFF, size and 0xFF) +
            type.toByteArray(Charsets.US_ASCII) + payload
    }

    @Test
    fun `a heic whose boxes tile the file exactly is complete`() {
        val f = file("a.heic", box("ftyp", padding(16)), box("mdat", padding(500)))
        assertTrue(ImageIntegrity.looksComplete(f))
    }

    @Test
    fun `a heic whose last box overruns the file is truncated`() {
        // The iPhone-transfer case: HEIC is what those arrive as, and a cut one leaves an mdat
        // claiming more pixel bytes than the file has left.
        val ftyp = box("ftyp", padding(16))
        val mdat = box("mdat", padding(500))
        val f = File(tmp.root, "b.heic").apply { writeBytes(ftyp + mdat.copyOf(mdat.size - 200)) }
        assertFalse(ImageIntegrity.looksComplete(f))
    }

    @Test
    fun `a heic ending in a partial box header is truncated`() {
        // Cut inside the four size bytes themselves, so there is not even a length to check.
        val f = File(tmp.root, "c.heic").apply { writeBytes(box("ftyp", padding(16)) + bytes(0x00, 0x00, 0x01)) }
        assertFalse(ImageIntegrity.looksComplete(f))
    }

    // ── Everything else ─────────────────────────────────────────────────────────────────────────

    @Test
    fun `a format this cannot verify is accepted rather than refused`() {
        // One check in a chain, not a whitelist. The bounds pass still has to accept it, and
        // rejecting every unrecognised container would lose valid photos to prevent one bad one.
        assertTrue(ImageIntegrity.looksComplete(file("x.bin", padding(64))))
    }

    @Test
    fun `a file too short to hold a header is refused`() {
        assertFalse(ImageIntegrity.looksComplete(file("tiny.jpg", bytes(0xFF, 0xD8, 0xFF, 0xD9))))
    }

    @Test
    fun `a missing file is refused rather than thrown`() {
        assertFalse(ImageIntegrity.looksComplete(File(tmp.root, "absent.jpg")))
    }
}
