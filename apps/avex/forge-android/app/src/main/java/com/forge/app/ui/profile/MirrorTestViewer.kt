@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class, androidx.compose.material3.ExperimentalMaterial3Api::class)

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
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberDatePickerState
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
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.photo.PhotoPose
import com.forge.app.domain.photo.PhotoTag
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.parseToLb
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.common.bounceClick
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.text.SimpleDateFormat
import java.time.Instant
import java.time.ZoneId
import java.util.Date
import java.util.Locale

/**
 * Full-screen, swipeable photo viewer + metadata editor. Opens on the tapped photo and pages through
 * the exact list the grid showed. Each page shows the whole photo (Fit) over a dark scrim; the editor
 * beneath it owns every field the gallery filters on — date, title, pose, muscles, tags, bodyweight,
 * note and album — and deletes. Title, note, weight and a typed tag commit on swipe or dismiss;
 * chip taps (pose, muscles, tags, album) and the date reflect at once, because a chip that needs a
 * separate save step is a chip you cannot trust.
 */
@Composable
internal fun GalleryViewerPager(
    photos: List<ProgressPhoto>,
    startIndex: Int,
    albumNames: List<String>,
    knownTags: List<String>,
    weightUnit: WeightUnit,
    fileFor: (ProgressPhoto) -> File,
    onSaveNote: (ProgressPhoto, String) -> Unit,
    onSaveTitle: (ProgressPhoto, String) -> Unit,
    onMove: (ProgressPhoto, String) -> Unit,
    onSetPose: (ProgressPhoto, String) -> Unit,
    onSetMuscles: (ProgressPhoto, List<String>) -> Unit,
    onSetTags: (ProgressPhoto, List<String>) -> Unit,
    onSetWeight: (ProgressPhoto, Double?) -> Unit,
    onSetDate: (ProgressPhoto, Long) -> Unit,
    onDelete: (ProgressPhoto) -> Unit,
    onDismiss: () -> Unit
) {
    if (photos.isEmpty()) { onDismiss(); return }
    val start = startIndex.coerceIn(0, photos.lastIndex)
    val pagerState = rememberPagerState(initialPage = start) { photos.size }
    val current = photos.getOrElse(pagerState.currentPage) { photos[start] }

    // Live buffers keyed by file so edits survive swipes; overrides echo committed values back when you
    // page to a photo again, over the (frozen) `photos` snapshot.
    var editingFile by remember { mutableStateOf(current.fileName) }
    val noteOverride = remember { mutableStateMapOf<String, String>() }
    val titleOverride = remember { mutableStateMapOf<String, String>() }
    val albumOverride = remember { mutableStateMapOf<String, String>() }
    val poseOverride = remember { mutableStateMapOf<String, String>() }
    val muscleOverride = remember { mutableStateMapOf<String, List<String>>() }
    val tagOverride = remember { mutableStateMapOf<String, List<String>>() }
    val weightOverride = remember { mutableStateMapOf<String, Double?>() }
    val dateOverride = remember { mutableStateMapOf<String, Long>() }
    var noteInput by remember { mutableStateOf(noteOverride[current.fileName] ?: current.note) }
    var titleInput by remember { mutableStateOf(titleOverride[current.fileName] ?: current.title) }
    var weightInput by remember {
        mutableStateOf(current.weightLb?.let { weightInputValue(it, weightUnit) } ?: "")
    }
    var showDatePicker by remember { mutableStateOf(false) }
    var tagInput by remember { mutableStateOf("") }
    var editorOpen by remember { mutableStateOf(false) }

    val currentAlbum = albumOverride[current.fileName] ?: current.album
    val currentPose = poseOverride[current.fileName] ?: current.pose
    val currentMuscles = muscleOverride[current.fileName] ?: current.muscles
    val currentTags = tagOverride[current.fileName] ?: current.tags
    val currentDate = dateOverride[current.fileName] ?: current.takenAtMs

    fun commit() {
        val original = photos.firstOrNull { it.fileName == editingFile } ?: return
        // A tag typed but never submitted is still a tag the user meant. Flush it rather than
        // silently dropping it when they swipe to the next shot.
        if (tagInput.isNotBlank()) {
            val existing = tagOverride[editingFile] ?: original.tags
            val next = PhotoTag.added(existing, tagInput)
            tagInput = ""
            if (next != existing) {
                tagOverride[editingFile] = next
                onSetTags(original, next)
            }
        }
        val trimmed = noteInput.trim()
        noteOverride[editingFile] = trimmed
        if (trimmed != original.note) onSaveNote(original, trimmed)
        val trimmedTitle = titleInput.trim()
        titleOverride[editingFile] = trimmedTitle
        if (trimmedTitle != original.title) onSaveTitle(original, trimmedTitle)
        // Compare in DISPLAY units, not lb: weightInput was seeded via weightInputValue (which rounds to
        // the display step), so a 0.1-kg rounding is ~0.11 lb and an untouched field would trip a raw-lb
        // threshold and silently rewrite the snapshot. Same display text as seeded ⇒ untouched ⇒ no write.
        val prevDisplay = original.weightLb?.let { weightInputValue(it, weightUnit) } ?: ""
        val curDisplay = weightInput.trim()
        if (curDisplay != prevDisplay) {
            val parsed = curDisplay.ifBlank { null }?.let { parseToLb(it, weightUnit) }
            weightOverride[editingFile] = parsed
            onSetWeight(original, parsed)
        }
    }
    LaunchedEffect(pagerState.currentPage) {
        if (current.fileName != editingFile) {
            commit()
            editingFile = current.fileName
            noteInput = noteOverride[current.fileName] ?: current.note
            titleInput = titleOverride[current.fileName] ?: current.title
            weightInput = (weightOverride[current.fileName] ?: current.weightLb)?.let { weightInputValue(it, weightUnit) } ?: ""
        }
    }

    val onSurface = MaterialTheme.colorScheme.onSurface
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary

    Dialog(onDismissRequest = { commit(); onDismiss() }, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.94f))) {
            Row(
                Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.SpaceBetween
            ) {
                IconButton(onClick = { commit(); onDismiss() }) { Icon(Icons.Filled.Close, contentDescription = "Close", tint = Color.White) }
                Text(
                    "${pagerState.currentPage + 1} / ${photos.size}",
                    style = MaterialTheme.typography.labelMedium, color = Color.White.copy(alpha = 0.85f)
                )
                Spacer(Modifier.size(48.dp))
            }

            HorizontalPager(state = pagerState, modifier = Modifier.weight(1f).fillMaxWidth()) { page ->
                GalleryFullImage(fileFor(photos[page]), Modifier.fillMaxSize().padding(horizontal = 8.dp))
            }

            // Metadata editor. Collapsed it is one reading of the shot (date, what it is of, its
            // tags); expanded it is every field, in its own scroll so a full muscle rail at 200%
            // font scale cannot push the photo off the screen. The photo stays the largest thing
            // on a photo viewer, which is the whole reason the fields fold.
            Column(
                Modifier.fillMaxWidth()
                    .clip(RoundedCornerShape(topStart = 16.dp, topEnd = 16.dp))
                    .background(MaterialTheme.colorScheme.surface)
                    .heightIn(max = if (editorOpen) 420.dp else Dp.Unspecified)
                    .verticalScroll(rememberScrollState())
                    .padding(16.dp)
            ) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.SpaceBetween) {
                    Text(
                        SimpleDateFormat("EEEE, MMM d, yyyy", Locale.getDefault()).format(Date(currentDate)),
                        style = MaterialTheme.typography.labelMedium, color = accent,
                        modifier = Modifier.bounceClick { showDatePicker = true }.padding(vertical = 4.dp)
                    )
                    IconButton(onClick = { commit(); onDelete(current) }) {
                        Icon(Icons.Filled.Delete, contentDescription = "Delete photo", tint = MaterialTheme.colorScheme.error)
                    }
                }
                // TITLE — the short label the grid and the day header show, above the long-form note
                // it sits over. Same bare-field treatment as the note (§1: the page IS the surface),
                // one type step up so the two read as caption-then-body rather than two equal fields.
                Spacer(Modifier.height(8.dp))
                BasicTextField(
                    value = titleInput,
                    onValueChange = { titleInput = it.take(60) },
                    singleLine = true,
                    textStyle = MaterialTheme.typography.titleMedium.copy(color = onSurface),
                    cursorBrush = SolidColor(accent),
                    decorationBox = { inner ->
                        Box {
                            if (titleInput.isEmpty()) Text(
                                "Add a title…", style = MaterialTheme.typography.titleMedium,
                                color = muted.copy(alpha = 0.6f), fontStyle = FontStyle.Italic
                            )
                            inner()
                        }
                    },
                    modifier = Modifier.fillMaxWidth()
                )
                Spacer(Modifier.height(6.dp))
                BasicTextField(
                    value = noteInput,
                    onValueChange = { noteInput = it.take(140) },
                    textStyle = MaterialTheme.typography.bodyMedium.copy(color = onSurface),
                    cursorBrush = SolidColor(accent),
                    decorationBox = { inner ->
                        Box {
                            if (noteInput.isEmpty()) Text(
                                "Add a note…", style = MaterialTheme.typography.bodyMedium,
                                color = muted.copy(alpha = 0.6f), fontStyle = FontStyle.Italic
                            )
                            inner()
                        }
                    },
                    modifier = Modifier.fillMaxWidth()
                )

                // The reading, and the way into the fields that produce it.
                Spacer(Modifier.height(12.dp))
                Row(
                    Modifier.fillMaxWidth().bounceClick { editorOpen = !editorOpen }.padding(vertical = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(
                        photoTagSummary(currentPose, currentMuscles, currentTags),
                        style = MaterialTheme.typography.labelMedium, color = muted,
                        modifier = Modifier.weight(1f)
                    )
                    Spacer(Modifier.width(12.dp))
                    Text(
                        if (editorOpen) "done" else "edit →",
                        style = MaterialTheme.typography.labelSmall, color = accent
                    )
                }

                if (!editorOpen) return@Column

                // POSE — where the camera stood. One per photo.
                EditorField("Pose", muted) {
                    PhotoPose.entries.forEach { p ->
                        GalleryChip(p.label, selected = currentPose == p.name) {
                            val next = if (currentPose == p.name) "" else p.name
                            poseOverride[current.fileName] = next
                            onSetPose(current, next)
                        }
                    }
                }

                // MUSCLES — what the shot is evidence of. Several per photo, from the program's own
                // vocabulary, so a photo and a training week can be read against each other.
                EditorField("Muscles", muted) {
                    MuscleGroup.entries.forEach { m ->
                        GalleryChip(m.displayName, selected = m.code in currentMuscles) {
                            val next = if (m.code in currentMuscles) currentMuscles - m.code else currentMuscles + m.code
                            muscleOverride[current.fileName] = next
                            onSetMuscles(current, next)
                        }
                    }
                }

                // TAGS — whatever the user invents. Tapping one removes it; the rail underneath
                // offers the tags already in the library, so the vocabulary converges instead of
                // sprouting a new spelling of "cut" every time.
                EditorField("Tags", muted) {
                    currentTags.forEach { t ->
                        GalleryChip(PhotoTag.display(t), selected = true, trailing = "✕") {
                            val next = currentTags - t
                            tagOverride[current.fileName] = next
                            onSetTags(current, next)
                        }
                    }
                    knownTags.filter { it !in currentTags }.take(6).forEach { t ->
                        GalleryChip(PhotoTag.display(t), selected = false) {
                            val next = PhotoTag.added(currentTags, t)
                            tagOverride[current.fileName] = next
                            onSetTags(current, next)
                        }
                    }
                }
                if (currentTags.size < PhotoTag.MAX_PER_PHOTO) {
                    Spacer(Modifier.height(8.dp))
                    BasicTextField(
                        value = tagInput,
                        onValueChange = { tagInput = it.take(PhotoTag.MAX_LENGTH + 1) },
                        singleLine = true,
                        textStyle = MaterialTheme.typography.bodyMedium.copy(color = onSurface),
                        cursorBrush = SolidColor(accent),
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                        keyboardActions = KeyboardActions(onDone = {
                            val next = PhotoTag.added(currentTags, tagInput)
                            tagInput = ""
                            if (next != currentTags) {
                                tagOverride[current.fileName] = next
                                onSetTags(current, next)
                            }
                        }),
                        decorationBox = { inner ->
                            Box {
                                if (tagInput.isEmpty()) Text(
                                    "Add a tag…", style = MaterialTheme.typography.bodyMedium,
                                    color = muted.copy(alpha = 0.6f), fontStyle = FontStyle.Italic
                                )
                                inner()
                            }
                        },
                        modifier = Modifier.fillMaxWidth()
                    )
                }

                // Bodyweight.
                Spacer(Modifier.height(14.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text("WEIGHT", style = MaterialTheme.typography.labelSmall, color = muted)
                    Spacer(Modifier.width(10.dp))
                    BasicTextField(
                        value = weightInput,
                        onValueChange = { weightInput = it.filter { c -> c.isDigit() || c == '.' }.take(6) },
                        textStyle = MaterialTheme.typography.bodyMedium.copy(color = onSurface),
                        cursorBrush = SolidColor(accent),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                        decorationBox = { inner ->
                            Box {
                                if (weightInput.isEmpty()) Text(
                                    "Not set", style = MaterialTheme.typography.bodyMedium,
                                    color = muted.copy(alpha = 0.6f)
                                )
                                inner()
                            }
                        },
                        modifier = Modifier.width(88.dp)
                    )
                    Text(unitLabel(weightUnit), style = MaterialTheme.typography.labelMedium, color = muted)
                }

                // Album.
                EditorField("Album", muted) {
                    GalleryChip("Unsorted", selected = currentAlbum.isBlank()) {
                        albumOverride[current.fileName] = ""; onMove(current, "")
                    }
                    albumNames.forEach { name ->
                        GalleryChip(name, selected = currentAlbum == name) {
                            albumOverride[current.fileName] = name; onMove(current, name)
                        }
                    }
                }
            }
        }
    }

    if (showDatePicker) {
        val dpState = rememberDatePickerState(initialSelectedDateMillis = currentDate)
        // Shared §5 tones — M3's own default lands this dialog on an unthemed, markedly paler slab.
        val pickerColors = forgeDatePickerColors()
        DatePickerDialog(
            onDismissRequest = { showDatePicker = false },
            colors = pickerColors,
            confirmButton = {
                TextButton(onClick = {
                    dpState.selectedDateMillis?.let { picked ->
                        // DatePicker returns UTC midnight — map that calendar day to local start-of-day.
                        val localDate = Instant.ofEpochMilli(picked).atZone(ZoneId.of("UTC")).toLocalDate()
                        val ms = localDate.atStartOfDay(ZoneId.systemDefault()).toInstant().toEpochMilli()
                        dateOverride[current.fileName] = ms
                        onSetDate(current, ms)
                    }
                    showDatePicker = false
                },
                    colors = ButtonDefaults.textButtonColors(
                        contentColor = MaterialTheme.colorScheme.onBackground
                    )
                ) { Text("Set") }
            },
            dismissButton = {
                TextButton(
                    onClick = { showDatePicker = false },
                    colors = ButtonDefaults.textButtonColors(
                        contentColor = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                ) { Text("Cancel") }
            }
        ) { DatePicker(state = dpState, colors = pickerColors) }
    }
}

/**
 * What the shot is, in one line: its pose, the muscles it documents, and its tags. This is the
 * collapsed editor's whole content, so a photo always states its own metadata without being opened
 * for editing. Untagged says so plainly rather than rendering an empty row of nothing.
 */
private fun photoTagSummary(pose: String, muscles: List<String>, tags: List<String>): String {
    val parts = buildList {
        PhotoPose.fromKey(pose)?.let { add(it.label.uppercase()) }
        MuscleGroup.entries.filter { it.code in muscles }.forEach { add(it.displayName.uppercase()) }
        tags.forEach { add(PhotoTag.display(it)) }
    }
    return if (parts.isEmpty()) "No pose, muscles or tags yet" else parts.joinToString(" · ")
}

/**
 * One labeled field of the metadata editor: a mono label over a wrapping chip rail. Every chip axis
 * shares this shell, so pose, muscles, tags and album read as one form rather than four.
 */
@Composable
private fun EditorField(label: String, muted: Color, chips: @Composable () -> Unit) {
    Spacer(Modifier.height(14.dp))
    Text(label.uppercase(), style = MaterialTheme.typography.labelSmall, color = muted)
    Spacer(Modifier.height(8.dp))
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        chips()
    }
}

/**
 * Loads a progress photo scaled to fit ([ContentScale.Fit]) and rotated per its EXIF orientation —
 * the full-screen counterpart to [ProgressPhotoImage] (which crops to fill for the grid). Kept local
 * so the viewer never cuts off the top/bottom of a physique shot.
 */
@Composable
internal fun GalleryFullImage(
    file: File,
    modifier: Modifier = Modifier,
    reqPx: Int = 1400,
    alpha: Float = 1f,
    contentScale: ContentScale = ContentScale.Fit
) {
    val bitmap by produceState<ImageBitmap?>(initialValue = null, file.path, reqPx) {
        value = withContext(Dispatchers.IO) { decodeFittedBitmap(file, reqPx)?.asImageBitmap() }
    }
    val bmp = bitmap
    if (bmp != null) {
        Image(
            bitmap = bmp,
            contentDescription = "Progress photo",
            modifier = modifier,
            contentScale = contentScale,
            alpha = alpha,
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
    }.getOrDefault(decoded).also { if (it !== decoded) decoded.recycle() }
}
