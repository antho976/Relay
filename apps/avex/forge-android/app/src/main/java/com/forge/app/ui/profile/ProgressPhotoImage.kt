package com.forge.app.ui.profile

import android.util.LruCache
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import com.forge.app.core.io.OrientedBitmaps
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

/**
 * Decoded progress photos, kept across compositions so a thumbnail scrolled back into view, the
 * Profile cover on its next open, or a viewer page swiped back to shows at once instead of
 * re-reading and re-decoding a 12 MP JPEG from disk behind a placeholder.
 *
 * Keyed by path and requested size; each entry remembers the file's modification time and length,
 * and a hit is re-checked against the file off the main thread, so an edited or replaced photo (the
 * avatar keeps one path) is re-decoded rather than served stale. Bounded to an eighth of the heap.
 */
internal object PhotoBitmapCache {
    private class Entry(val bitmap: ImageBitmap, val modifiedMs: Long, val length: Long)

    private val entries = object : LruCache<String, Entry>(
        (Runtime.getRuntime().maxMemory() / 8).coerceAtMost(Int.MAX_VALUE.toLong()).toInt()
    ) {
        override fun sizeOf(key: String, value: Entry): Int = value.bitmap.width * value.bitmap.height * 4
    }

    /**
     * Decodes run at most three at a time: `Dispatchers.IO` allows 64, and a fast fling through the
     * gallery started dozens of full-size decodes at once, all contending with the UI for the CPU.
     */
    @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
    val decodeDispatcher = Dispatchers.IO.limitedParallelism(3)

    private fun key(file: File, reqPx: Int) = "${file.path}|$reqPx"

    /** The last decode for [file] at [reqPx], unverified — for the first frame only. */
    fun peek(file: File, reqPx: Int): ImageBitmap? = entries.get(key(file, reqPx))?.bitmap

    /** A current decode of [file] at [reqPx]: the cached one when the file is unchanged, else fresh. */
    suspend fun load(file: File, reqPx: Int): ImageBitmap? = withContext(decodeDispatcher) {
        val key = key(file, reqPx)
        val modified = file.lastModified()
        val length = file.length()
        entries.get(key)?.takeIf { it.modifiedMs == modified && it.length == length }?.let {
            return@withContext it.bitmap
        }
        // Exact fit: a page of thumbnails at up to 2x the target size is the allocation that matters.
        val decoded = OrientedBitmaps.decode(file, reqPx, exactFit = true)?.asImageBitmap()
        if (decoded == null) entries.remove(key) else entries.put(key, Entry(decoded, modified, length))
        decoded
    }
}

/** [file] decoded at [reqPx], from [PhotoBitmapCache] when it has it; null while decoding. */
@Composable
internal fun rememberPhotoBitmap(file: File, reqPx: Int): ImageBitmap? {
    val bitmap by produceState(PhotoBitmapCache.peek(file, reqPx), file.path, reqPx) {
        value = PhotoBitmapCache.load(file, reqPx)
    }
    return bitmap
}

/**
 * Loads a progress photo from app storage, downsampled to exactly [reqPx] and brought upright per
 * its EXIF orientation via [OrientedBitmaps] (phone portrait shots otherwise display sideways). No
 * image library in the project, so this decodes off the main thread through [PhotoBitmapCache] and
 * hands Compose an [ImageBitmap].
 */
@Composable
fun ProgressPhotoImage(file: File, modifier: Modifier = Modifier, reqPx: Int = 600) {
    val bmp = rememberPhotoBitmap(file, reqPx)
    if (bmp != null) {
        Image(
            bitmap = bmp,
            contentDescription = "Progress photo",
            modifier = modifier,
            contentScale = ContentScale.Crop,
            // Bilinear/mipmapped sampling instead of the default Low — noticeably crisper when the
            // bitmap is scaled to fill (esp. the full-width profile banner).
            filterQuality = FilterQuality.High
        )
    } else {
        Box(modifier.background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.4f)))
    }
}
