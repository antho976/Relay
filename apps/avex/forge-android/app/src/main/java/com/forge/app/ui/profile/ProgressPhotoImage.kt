package com.forge.app.ui.profile

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Matrix
import android.media.ExifInterface
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
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

/**
 * Loads a progress photo from app storage, downsampled to ~[reqPx] and rotated per its EXIF
 * orientation (phone portrait shots otherwise display sideways). No image library in the project,
 * so this decodes on the IO dispatcher via [produceState] and hands Compose an [ImageBitmap].
 */
@Composable
fun ProgressPhotoImage(file: File, modifier: Modifier = Modifier, reqPx: Int = 600) {
    val bitmap by produceState<ImageBitmap?>(initialValue = null, file.path, reqPx) {
        value = withContext(Dispatchers.IO) { decodeOriented(file, reqPx)?.asImageBitmap() }
    }
    val bmp = bitmap
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

private fun decodeOriented(file: File, reqPx: Int): Bitmap? {
    if (!file.exists()) return null
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeFile(file.path, bounds)
    var sample = 1
    val maxDim = maxOf(bounds.outWidth, bounds.outHeight)
    while (maxDim / (sample * 2) >= reqPx) sample *= 2
    val opts = BitmapFactory.Options().apply {
        inSampleSize = sample
        inPreferredConfig = Bitmap.Config.ARGB_8888
    }
    val decoded = BitmapFactory.decodeFile(file.path, opts) ?: return null

    // inSampleSize only halves, so `decoded` can be up to ~2× the target. Scale it precisely to reqPx
    // with a bilinear filter: caps memory (no oversized bitmap held for the view) and reads sharper
    // than letting the GPU rescale a ragged power-of-two sample at draw time.
    val decodedMax = maxOf(decoded.width, decoded.height)
    val bmp = if (decodedMax > reqPx && decodedMax > 0) {
        val ratio = reqPx.toFloat() / decodedMax
        // Guard the scale allocation like the rotation step below — a very large source can OOM here,
        // and an uncaught throw would crash produceState's coroutine. Fall back to the valid `decoded`.
        runCatching {
            Bitmap.createScaledBitmap(
                decoded,
                (decoded.width * ratio).toInt().coerceAtLeast(1),
                (decoded.height * ratio).toInt().coerceAtLeast(1),
                true
            )
        }.getOrDefault(decoded).also { if (it !== decoded) decoded.recycle() }
    } else decoded

    val orientation = runCatching {
        ExifInterface(file.path).getAttributeInt(ExifInterface.TAG_ORIENTATION, ExifInterface.ORIENTATION_NORMAL)
    }.getOrDefault(ExifInterface.ORIENTATION_NORMAL)
    val degrees = when (orientation) {
        ExifInterface.ORIENTATION_ROTATE_90 -> 90f
        ExifInterface.ORIENTATION_ROTATE_180 -> 180f
        ExifInterface.ORIENTATION_ROTATE_270 -> 270f
        else -> return bmp
    }
    return runCatching {
        Bitmap.createBitmap(bmp, 0, 0, bmp.width, bmp.height, Matrix().apply { postRotate(degrees) }, true)
    }.getOrDefault(bmp).also { if (it !== bmp) bmp.recycle() }
}
