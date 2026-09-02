package com.forge.app.ui.profile

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
 * Loads a progress photo from app storage, downsampled to exactly [reqPx] and brought upright per
 * its EXIF orientation via [OrientedBitmaps] (phone portrait shots otherwise display sideways). No
 * image library in the project, so this decodes on the IO dispatcher via [produceState] and hands
 * Compose an [ImageBitmap].
 */
@Composable
fun ProgressPhotoImage(file: File, modifier: Modifier = Modifier, reqPx: Int = 600) {
    val bitmap by produceState<ImageBitmap?>(initialValue = null, file.path, reqPx) {
        value = withContext(Dispatchers.IO) {
            // Exact fit: this is the grid's thumbnail decoder, and a page of them at up to 2x the
            // target size is the allocation that matters.
            OrientedBitmaps.decode(file, reqPx, exactFit = true)?.asImageBitmap()
        }
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
