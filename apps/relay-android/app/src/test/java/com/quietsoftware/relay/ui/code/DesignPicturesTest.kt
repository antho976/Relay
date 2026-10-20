package com.quietsoftware.relay.ui.code

import android.graphics.Bitmap
import android.media.ExifInterface
import com.quietsoftware.relay.core.wire.BusError
import com.quietsoftware.relay.core.wire.BusException
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.GraphicsMode
import java.io.ByteArrayOutputStream
import java.io.File
import java.util.Base64

/** A design's pictures on the phone: read in pieces, kept to a budget, decoded upright, and never an SVG. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class DesignPicturesTest {
    @get:Rule val tmp = TemporaryFolder()

    private val png = "0123456789abcdef0123456789abcdef.png"

    /** `design.asset.get`'s answers for [bytes], [piece] bytes at a time. */
    private fun pieces(bytes: ByteArray, piece: Int, size: Long = bytes.size.toLong()): suspend (Long) -> JsonObject = { offset ->
        val end = minOf(bytes.size, offset.toInt() + piece)
        buildJsonObject {
            put("media_type", "image/png")
            put("size", size)
            put("bytes_b64", Base64.getEncoder().encodeToString(bytes.copyOfRange(offset.toInt(), end)))
            if (end < bytes.size) put("next", end.toLong())
        }
    }

    private fun photo(w: Int, h: Int, format: Bitmap.CompressFormat = Bitmap.CompressFormat.PNG): ByteArray {
        val bitmap = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
        bitmap.eraseColor(0xFF336699.toInt())
        return ByteArrayOutputStream().also { bitmap.compress(format, 90, it) }.toByteArray()
    }

    @Test
    fun `a picture comes in pieces and is kept only whole`() = runBlocking {
        val bytes = ByteArray(10_000) { it.toByte() }
        val part = tmp.newFile("a.part")
        assertTrue(readPieces(part, pieces(bytes, 3_000)))
        assertArrayEquals(bytes, part.readBytes())
        // Shorter than it said it is, or a piece not where the next was to start: not kept.
        assertFalse(readPieces(tmp.newFile("b.part"), pieces(bytes, 3_000, size = 12_000)))
        val skipping: suspend (Long) -> JsonObject = { offset -> pieces(bytes, 3_000)(offset).let { JsonObject(it + ("next" to kotlinx.serialization.json.JsonPrimitive(offset + 5_000))) } }
        assertFalse(readPieces(tmp.newFile("c.part"), skipping))
    }

    @Test
    fun `pictures are kept to a budget, the least recently drawn going first`() {
        val dir = tmp.newFolder("design-assets")
        for ((i, name) in listOf("old.png", "mid.png", "new.png").withIndex()) {
            File(dir, name).apply { writeBytes(ByteArray(100)) }.setLastModified(1_000_000L * (i + 1))
        }
        File(dir, "reading.png.part").writeBytes(ByteArray(500))
        trim(dir, 250)
        assertEquals(listOf("mid.png", "new.png", "reading.png.part"), dir.list()!!.sorted())
    }

    @Test
    fun `only the pictures design asset add names are read, and never an SVG`() {
        assertTrue(readable(png))
        assertFalse(readable("0123456789abcdef0123456789abcdef.svg"))
        assertFalse(readable("../etc/passwd"))
        assertFalse(readable("photo.png"))
    }

    /** TM-24: a photo whose EXIF says it was taken turned is drawn upright, as the PC draws it. */
    @Test
    fun `a photo is decoded upright as its exif says`() {
        val file = tmp.newFile("turned.jpg").apply { writeBytes(photo(200, 100, Bitmap.CompressFormat.JPEG)) }
        ExifInterface(file.absolutePath).apply {
            setAttribute(ExifInterface.TAG_ORIENTATION, ExifInterface.ORIENTATION_ROTATE_90.toString())
            saveAttributes()
        }
        val image = decode(file.readBytes())!!
        assertEquals(100 to 200, image.width to image.height)
    }

    @Test
    fun `a design's pictures load, a refused one fails, and an SVG is never asked for`() = runBlocking {
        val bytes = photo(30, 20)
        val asked = mutableListOf<String>()
        val refused = "fedcba9876543210fedcba9876543210.png"
        val pictures = DesignPictures(tmp.newFolder("assets")) { src, offset ->
            asked += src
            if (src == refused) throw BusException(BusError.link("link.down", "the PC is out of reach"))
            pieces(bytes, 1_000)(offset)
        }
        pictures.load(listOf(png, refused, "0123456789abcdef0123456789abcdef.svg"))
        assertEquals(30, pictures.picture(png)!!.width)
        assertEquals(PictureState.Failed, pictures.states[refused])
        assertTrue(asked.none { it.endsWith(".svg") })
        // Kept: the next design that shows it reads it from the phone, not the PC.
        asked.clear()
        DesignPictures(File(tmp.root, "assets")) { src, _ -> asked += src; throw BusException(BusError.link("link.down", "")) }.load(listOf(png))
        assertTrue(asked.isEmpty())
    }
}
