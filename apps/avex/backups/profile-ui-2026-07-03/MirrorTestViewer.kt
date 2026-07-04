@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package com.forge.app.ui.profile

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Matrix
import android.media.ExifInterface
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.ui.common.bounceClick
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Full-screen, swipeable photo viewer. Opens on the tapped photo and pages left/right through the
 * exact list the grid was showing (search results, an album, or the whole timeline). Each page shows
 * the whole photo (Fit, not cropped) over a dark scrim, with its date, an editable note, move-to-album
 * chips and delete in a bottom sheet. Note edits commit on swipe / dismiss; album moves reflect at once.
 */
@Composable
internal fun GalleryViewerPager(
    photos: List<ProgressPhoto>,
    startIndex: Int,
    albumNames: List<String>,
    fileFor: (ProgressPhoto) -> File,
    onSaveNote: (ProgressPhoto, String) -> Unit,
    onMove: (ProgressPhoto, String) -> Unit,
    onDelete: (ProgressPhoto) -> Unit,
    onDismiss: () -> Unit
) {
    if (photos.isEmpty()) { onDismiss(); return }
    val start = startIndex.coerceIn(0, photos.lastIndex)
    val pagerState = rememberPagerState(initialPage = start) { photos.size }
    val current = photos.getOrElse(pagerState.currentPage) { photos[start] }

    // Note editing: one live buffer tracked by which file it belongs to, so it survives swipes and
    // commits the previous photo's edit before loading the next.
    var editingFile by remember { mutableStateOf(current.fileName) }
    var noteInput by remember { mutableStateOf(current.note) }
    // Local album echo so the chip highlight updates instantly without waiting on a VM reload.
    val albumOverride = remember { mutableStateMapOf<String, String>() }
    val currentAlbum = albumOverride[current.fileName] ?: current.album

    fun commitNote() {
        val original = photos.firstOrNull { it.fileName == editingFile } ?: return
        if (noteInput.trim() != original.note) onSaveNote(original, noteInput.trim())
    }
    LaunchedEffect(pagerState.currentPage) {
        if (current.fileName != editingFile) {
            commitNote()
            editingFile = current.fileName
            noteInput = current.note
        }
    }

    val onSurface = MaterialTheme.colorScheme.onSurface
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary

    Dialog(
        onDismissRequest = { commitNote(); onDismiss() },
        properties = DialogProperties(usePlatformDefaultWidth = false)
    ) {
        Column(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.94f))) {
            // Top bar: close + "3 / 5" counter.
            Row(
                Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                IconButton(onClick = { commitNote(); onDismiss() }) {
                    Icon(Icons.Filled.Close, contentDescription = "Close", tint = Color.White)
                }
                Text(
                    "${pagerState.currentPage + 1} / ${photos.size}",
                    style = MaterialTheme.typography.labelMedium, color = Color.White.copy(alpha = 0.85f)
                )
                Spacer(Modifier.size(48.dp))
            }

            HorizontalPager(state = pagerState, modifier = Modifier.weight(1f).fillMaxWidth()) { page ->
                GalleryFullImage(fileFor(photos[page]), Modifier.fillMaxSize().padding(horizontal = 8.dp))
            }

            // Bottom sheet: date + delete, editable note, move-to-album chips.
            Column(
                Modifier.fillMaxWidth()
                    .clip(RoundedCornerShape(topStart = 16.dp, topEnd = 16.dp))
                    .background(MaterialTheme.colorScheme.surface)
                    .padding(16.dp)
            ) {
                Row(
                    Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(
                        SimpleDateFormat("EEEE, MMM d, yyyy", Locale.getDefault()).format(Date(current.takenAtMs)),
                        style = MaterialTheme.typography.labelMedium, color = muted
                    )
                    IconButton(onClick = { commitNote(); onDelete(current) }) {
                        Icon(Icons.Filled.Delete, contentDescription = "Delete photo", tint = MaterialTheme.colorScheme.error)
                    }
                }
                Spacer(Modifier.height(6.dp))
                BasicTextField(
                    value = noteInput,
                    onValueChange = { noteInput = it.take(140) },
                    textStyle = MaterialTheme.typography.bodyMedium.copy(color = onSurface),
                    cursorBrush = SolidColor(accent),
                    decorationBox = { inner ->
                        Box {
                            if (noteInput.isEmpty()) Text(
                                "Add a note…",
                                style = MaterialTheme.typography.bodyMedium,
                                color = muted.copy(alpha = 0.6f), fontStyle = FontStyle.Italic
                            )
                            inner()
                        }
                    },
                    modifier = Modifier.fillMaxWidth()
                )
                Spacer(Modifier.height(14.dp))
                Text("ALBUM", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                Spacer(Modifier.height(6.dp))
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    GalleryChip("Unsorted", selected = currentAlbum.isBlank()) {
                        commitNote(); albumOverride[current.fileName] = ""; onMove(current, "")
                    }
                    albumNames.forEach { name ->
                        GalleryChip(name, selected = currentAlbum == name) {
                            commitNote(); albumOverride[current.fileName] = name; onMove(current, name)
                        }
                    }
                }
            }
        }
    }
}

/**
 * Loads a progress photo scaled to fit ([ContentScale.Fit]) and rotated per its EXIF orientation —
 * the full-screen counterpart to [ProgressPhotoImage] (which crops to fill for the grid). Kept local
 * so the viewer never cuts off the top/bottom of a physique shot.
 */
@Composable
internal fun GalleryFullImage(file: File, modifier: Modifier = Modifier, reqPx: Int = 1400) {
    val bitmap by produceState<ImageBitmap?>(initialValue = null, file.path, reqPx) {
        value = withContext(Dispatchers.IO) { decodeFittedBitmap(file, reqPx)?.asImageBitmap() }
    }
    val bmp = bitmap
    if (bmp != null) {
        Image(
            bitmap = bmp,
            contentDescription = "Progress photo",
            modifier = modifier,
            contentScale = ContentScale.Fit,
            filterQuality = FilterQuality.High
        )
    } else {
        Box(modifier)
    }
}

private fun decodeFittedBitmap(file: File, reqPx: Int): Bitmap? {
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

    val orientation = runCatching {
        ExifInterface(file.path).getAttributeInt(ExifInterface.TAG_ORIENTATION, ExifInterface.ORIENTATION_NORMAL)
    }.getOrDefault(ExifInterface.ORIENTATION_NORMAL)
    val degrees = when (orientation) {
        ExifInterface.ORIENTATION_ROTATE_90 -> 90f
        ExifInterface.ORIENTATION_ROTATE_180 -> 180f
        ExifInterface.ORIENTATION_ROTATE_270 -> 270f
        else -> return decoded
    }
    return runCatching {
        Bitmap.createBitmap(decoded, 0, 0, decoded.width, decoded.height, Matrix().apply { postRotate(degrees) }, true)
    }.getOrDefault(decoded)
}
