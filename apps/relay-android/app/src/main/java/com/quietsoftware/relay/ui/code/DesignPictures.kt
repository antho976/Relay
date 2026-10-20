package com.quietsoftware.relay.ui.code

import android.content.Context
import android.graphics.ImageDecoder
import android.util.LruCache
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.snapshots.SnapshotStateMap
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.data.Relay as RelayData
import com.quietsoftware.relay.ui.dev.atMost
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withPermit
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import java.io.File
import java.nio.ByteBuffer
import java.util.Base64

/** Where a picture of a design is on the phone. The canvas draws [Ready] ones; the rest show its placeholder. */
internal sealed interface PictureState {
    data object Loading : PictureState

    data class Ready(val image: ImageBitmap) : PictureState

    /** Out of reach or refused (an older PC, the link down, a picture gone): tried again on the page's next answer. */
    data object Failed : PictureState
}

/** The longer side a design's picture is decoded at: past what the phone's canvas shows it at. */
private const val SIDE = 2048

/** Pictures kept on this phone: about the pictures of a few designs. */
private const val KEEP_BYTES = 64L * 1024 * 1024

/** A picture's name as design.asset.add gives it: 32 hex characters and a raster or SVG extension. */
private val SRC = Regex("^[0-9a-f]{32}\\.(png|jpg|gif|webp|svg)$")

/**
 * The pictures one design's image nodes show (feature-phone §4.7): read from the PC with
 * `design.asset.get`, a piece of at most 3 MiB at a time, kept on disk under `<src>` (pictures are
 * named by their content, so a kept one never goes stale) and decoded, upright, at most [SIDE]
 * on their longer side. [states] is read where the canvas draws, so a picture that arrives redraws
 * it. At most two pictures are read at once, across every design shown. An SVG picture is never
 * read: the phone has no SVG decoder, and its omissions line counts it.
 */
internal class DesignPictures(private val dir: File, private val fetch: suspend (src: String, offset: Long) -> JsonObject) {
    val states: SnapshotStateMap<String, PictureState> = mutableStateMapOf()

    /** Picture [src] if the phone holds it decoded. */
    fun picture(src: String): ImageBitmap? = (states[src] as? PictureState.Ready)?.image

    /** Read the pictures [srcs] names that are not here yet; one that failed before is tried again. */
    suspend fun load(srcs: Collection<String>) = coroutineScope {
        for (src in srcs.distinct()) {
            if (!readable(src) || states[src] is PictureState.Ready || states[src] is PictureState.Loading) continue
            states[src] = PictureState.Loading
            launch { states[src] = gate.withPermit { read(src) } }
        }
    }

    private suspend fun read(src: String): PictureState {
        shown.get("$src@$SIDE")?.let { return PictureState.Ready(it) }
        val bytes = try {
            bytes(src)
        } catch (_: BusException) {
            null
        } ?: return PictureState.Failed
        val image = withContext(Dispatchers.Default) { decode(bytes) } ?: return PictureState.Failed
        shown.put("$src@$SIDE", image)
        return PictureState.Ready(image)
    }

    /** Picture [src]'s bytes: kept here (and so kept longest), else read from the PC and kept; null when they did not all come. */
    private suspend fun bytes(src: String): ByteArray? {
        val kept = File(dir, src)
        withContext(Dispatchers.IO) {
            runCatching { kept.takeIf { it.isFile }?.readBytes()?.also { kept.setLastModified(System.currentTimeMillis()) } }.getOrNull()
        }?.let { return it }
        val part = File(dir, "$src.part")
        withContext(Dispatchers.IO) { dir.mkdirs() }
        if (!readPieces(part) { offset -> fetch(src, offset) }) {
            withContext(Dispatchers.IO) { part.delete() }
            return null
        }
        return withContext(Dispatchers.IO) {
            if (!part.renameTo(kept)) part.delete()
            trim(dir, KEEP_BYTES)
            runCatching { kept.readBytes() }.getOrNull()
        }
    }

    companion object {
        /** Decoded pictures, across designs: 48 MiB of them. */
        private val shown = object : LruCache<String, ImageBitmap>(48 * 1024 * 1024) {
            override fun sizeOf(key: String, value: ImageBitmap) = value.width * value.height * 4
        }

        private val gate = Semaphore(2)

        /** Design [id]'s pictures, read through [relay], kept in the app's files. */
        fun of(context: Context, relay: RelayData, id: Long) = DesignPictures(File(context.filesDir, "design-assets")) { src, offset ->
            relay.call("design.asset.get", buildJsonObject {
                put("design_id", id)
                put("src", src)
                if (offset > 0) put("offset", offset)
            }) as? JsonObject ?: JsonObject(emptyMap())
        }
    }
}

/** Whether [src] names a picture the phone reads: one design.asset.add named, and not an SVG. */
internal fun readable(src: String): Boolean = SRC.matches(src) && !src.endsWith(".svg")

/** The pictures [plan]'s page shows: its image nodes' sources, in paint order. */
internal fun pictureSrcs(plan: PagePlan): List<String> =
    plan.scene.nodes.filter { it.kind == "image" && plan.placed.rect(it.id) != null }.mapNotNull { it.src }.filter(::readable).distinct()

/**
 * Picture pieces from [next] (given each piece's start, answering a `design.asset.get` result)
 * written to [part] in order, until the last: whether all of it came, as long as it said it is.
 */
internal suspend fun readPieces(part: File, next: suspend (Long) -> JsonObject): Boolean {
    var offset = 0L
    var size = -1L
    val out = withContext(Dispatchers.IO) { part.outputStream() }
    try {
        while (true) {
            val piece = next(offset)
            val bytes = piece.s("bytes_b64")?.let { runCatching { Base64.getDecoder().decode(it) }.getOrNull() } ?: return false
            size = piece.l("size") ?: return false
            withContext(Dispatchers.IO) { out.write(bytes) }
            offset += bytes.size
            val more = piece.l("next") ?: break
            // A piece that is not where the next was said to start, or none at all: give up.
            if (more != offset || bytes.isEmpty()) return false
        }
    } finally {
        withContext(Dispatchers.IO) { out.close() }
    }
    return offset == size
}

/** Keep [dir] to [most] bytes of pictures, the least recently drawn going first (never a piece being read). */
internal fun trim(dir: File, most: Long) {
    val kept = dir.listFiles().orEmpty().filter { it.isFile && !it.name.endsWith(".part") }.sortedByDescending { it.lastModified() }
    var total = 0L
    for (f in kept) {
        total += f.length()
        if (total > most) f.delete()
    }
}

/** [bytes] decoded, upright as its EXIF says (ImageDecoder turns it), at most [SIDE] on its longer side; null when the phone cannot. */
internal fun decode(bytes: ByteArray): ImageBitmap? =
    runCatching { ImageDecoder.decodeBitmap(ImageDecoder.createSource(ByteBuffer.wrap(bytes)), atMost(SIDE)).asImageBitmap() }.getOrNull()
