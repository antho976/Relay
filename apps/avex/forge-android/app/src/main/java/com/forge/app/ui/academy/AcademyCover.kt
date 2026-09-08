package com.forge.app.ui.academy

import android.content.res.Resources
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.painter.BitmapPainter
import androidx.compose.ui.graphics.painter.ColorPainter
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalResources
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

/** Bounded decoded covers. Decode once off Main; lazy gallery rows retain only displayed painters. */
internal object AcademyCoverCache {
    private val mutex = Mutex()
    private val cache = object : LruCache<Pair<Int, Int>, Bitmap>(8 * 1024 * 1024) {
        override fun sizeOf(key: Pair<Int, Int>, value: Bitmap) = value.allocationByteCount
    }

    internal fun sampleSize(width: Int, height: Int, requestedMax: Int): Int {
        var sample = 1
        while (maxOf(width, height) / sample > requestedMax.coerceIn(256, 1024)) sample *= 2
        return sample
    }

    suspend fun load(resources: Resources, resource: Int, maxPixels: Int): Bitmap? = withContext(Dispatchers.IO) {
        val bound = when { maxPixels <= 256 -> 256; maxPixels <= 512 -> 512; else -> 1024 }
        val key = resource to bound
        mutex.withLock {
            cache.get(key) ?: run {
                val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                BitmapFactory.decodeResource(resources, resource, bounds)
                BitmapFactory.decodeResource(resources, resource, BitmapFactory.Options().apply {
                    inSampleSize = sampleSize(bounds.outWidth, bounds.outHeight, bound)
                    inScaled = false
                })?.also { cache.put(key, it) }
            }
        }
    }
}

@Composable
internal fun academyCoverPainter(cover: Int, maxPixels: Int): Painter {
    val resources = LocalResources.current
    val bitmap by produceState<Bitmap?>(null, cover, maxPixels, resources) {
        value = if (maxPixels > 0) AcademyCoverCache.load(resources, cover, maxPixels) else null
    }
    return bitmap?.let { BitmapPainter(it.asImageBitmap()) } ?: ColorPainter(Color.Transparent)
}
