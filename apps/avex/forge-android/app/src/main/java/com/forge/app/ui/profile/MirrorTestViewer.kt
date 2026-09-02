@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class, androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.profile

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
import com.forge.app.core.io.OrientedBitmaps
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.photo.PhotoPose
import com.forge.app.domain.photo.PhotoTag
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.domain.units.weightInputValue
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.onboarding.MAX_BODYWEIGHT_LB
import com.forge.app.ui.onboarding.MIN_BODYWEIGHT_LB
import com.forge.app.ui.onboarding.parseSaneBodyweightLb
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.text.SimpleDateFormat
import java.time.Instant
import java.time.ZoneId
import java.util.Date
import java.util.Locale
import kotlin.math.roundToInt

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

    // The last COMMITTED metadata per file, seeded from the frozen `photos` snapshot and updated
    // after every dispatched mutation, so edits survive swipes and page back in as what was saved.
    // Dirty checks compare against THIS, never against the launch snapshot: with the snapshot as
    // the baseline, editing a note A to B (committed on swipe) and back to A read as "no change"
    // and left B on disk. Title and weight had the same failure.
    val committed = remember { mutableStateMapOf<String, ProgressPhoto>() }
    fun baseline(p: ProgressPhoto): ProgressPhoto = committed[p.fileName] ?: p
    fun record(next: ProgressPhoto) { committed[next.fileName] = next }
    fun weightText(p: ProgressPhoto): String = p.weightLb?.let { weightInputValue(it, weightUnit) } ?: ""

    var editingFile by remember { mutableStateOf(current.fileName) }
    var noteInput by remember { mutableStateOf(baseline(current).note) }
    var titleInput by remember { mutableStateOf(baseline(current).title) }
    var weightInput by remember { mutableStateOf(weightText(baseline(current))) }
    var showDatePicker by remember { mutableStateOf(false) }
    var tagInput by remember { mutableStateOf("") }
    var editorOpen by remember { mutableStateOf(false) }

    val shown = baseline(current)
    val currentAlbum = shown.album
    val currentPose = shown.pose
    val currentMuscles = shown.muscles
    val currentTags = shown.tags
    val currentDate = shown.takenAtMs
    // Invalid nonblank weight text is shown as such while it is typed and never written (see
    // weightCommitDecision); the same range line the weigh-in sheet uses.
    val weightInvalid = weightCommitDecision(weightInput, shown.weightLb, weightUnit) is WeightCommit.Invalid
    val weightRangeText = if (weightUnit == WeightUnit.ST) {
        "Enter ${formatWeight(MIN_BODYWEIGHT_LB, weightUnit)}–${formatWeight(MAX_BODYWEIGHT_LB, weightUnit)}."
    } else {
        val minDisp = toDisplayWeight(MIN_BODYWEIGHT_LB, weightUnit).roundToInt()
        val maxDisp = toDisplayWeight(MAX_BODYWEIGHT_LB, weightUnit).roundToInt()
        "Enter $minDisp–$maxDisp ${unitLabel(weightUnit)}."
    }

    fun commit() {
        val original = photos.firstOrNull { it.fileName == editingFile } ?: return
        var base = baseline(original)
        // A tag typed but never submitted is still a tag the user meant. Flush it rather than
        // silently dropping it when they swipe to the next shot.
        if (tagInput.isNotBlank()) {
            val next = PhotoTag.added(base.tags, tagInput)
            tagInput = ""
            if (next != base.tags) {
                base = base.copy(tags = next); record(base)
                onSetTags(base, next)
            }
        }
        val trimmed = noteInput.trim()
        if (trimmed != base.note) {
            base = base.copy(note = trimmed); record(base)
            onSaveNote(base, trimmed)
        }
        val trimmedTitle = titleInput.trim()
        if (trimmedTitle != base.title) {
            base = base.copy(title = trimmedTitle); record(base)
            onSaveTitle(base, trimmedTitle)
        }
        // Blank is the one way to clear a weight. Text that does not parse keeps the committed
        // value: it used to be written as null over a valid snapshot, because "invalid" and
        // "cleared" both parsed to null.
        val decision = weightCommitDecision(weightInput, base.weightLb, weightUnit)
        if (decision is WeightCommit.Set) {
            base = base.copy(weightLb = decision.lb); record(base)
            onSetWeight(base, decision.lb)
        }
    }
    LaunchedEffect(pagerState.currentPage) {
        if (current.fileName != editingFile) {
            commit()
            editingFile = current.fileName
            val next = baseline(current)
            noteInput = next.note
            titleInput = next.title
            // Seeded from the committed value, so a cleared weight stays cleared when you page
            // back, and invalid text you swiped away from is dropped rather than carried along.
            weightInput = weightText(next)
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
                            record(baseline(current).copy(pose = next))
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
                            record(baseline(current).copy(muscles = next))
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
                            record(baseline(current).copy(tags = next))
                            onSetTags(current, next)
                        }
                    }
                    knownTags.filter { it !in currentTags }.take(6).forEach { t ->
                        GalleryChip(PhotoTag.display(t), selected = false) {
                            val next = PhotoTag.added(currentTags, t)
                            record(baseline(current).copy(tags = next))
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
                                record(baseline(current).copy(tags = next))
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
                if (weightInvalid) {
                    Spacer(Modifier.height(4.dp))
                    Text(weightRangeText, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.error)
                }

                // Album.
                EditorField("Album", muted) {
                    GalleryChip("Unsorted", selected = currentAlbum.isBlank()) {
                        record(baseline(current).copy(album = "")); onMove(current, "")
                    }
                    albumNames.forEach { name ->
                        GalleryChip(name, selected = currentAlbum == name) {
                            record(baseline(current).copy(album = name)); onMove(current, name)
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
                        record(baseline(current).copy(takenAtMs = ms))
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

/** What committing the viewer's weight field should do. See [weightCommitDecision]. */
internal sealed interface WeightCommit {
    /** The text is the committed value as displayed: untouched, so nothing is written. */
    object Keep : WeightCommit
    /** Nonblank text that is not a plausible bodyweight: the committed value stays, the range line shows. */
    object Invalid : WeightCommit
    /** Write [lb]. Null only for a blank field, which is the one way to clear a weight. */
    data class Set(val lb: Double?) : WeightCommit
}

/**
 * The commit decision for the weight field, against the LAST COMMITTED value rather than the launch
 * snapshot (so A to B to A commits A), compared in DISPLAY units: [input] was seeded via
 * [weightInputValue], which rounds to the display step, so a 0.1-kg rounding is ~0.11 lb and an
 * untouched field would trip a raw-lb comparison and silently rewrite the snapshot.
 *
 * Blank is the only clear. Text that parses to nothing, or to an implausible weight, used to be
 * indistinguishable from blank at this point and was written as null over a valid stored value;
 * it is now [WeightCommit.Invalid], which writes nothing. Pure so the three outcomes are testable.
 */
internal fun weightCommitDecision(input: String, committedLb: Double?, unit: WeightUnit): WeightCommit {
    val text = input.trim()
    val committedText = committedLb?.let { weightInputValue(it, unit) } ?: ""
    if (text == committedText) return WeightCommit.Keep
    if (text.isEmpty()) return WeightCommit.Set(null)
    val lb = parseSaneBodyweightLb(text, unit) ?: return WeightCommit.Invalid
    return WeightCommit.Set(lb)
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
        // EXACT fit (P-09): `inSampleSize` only halves, so a source landing just under twice the
        // request keeps close to four times the pixels — about 23 MB of ARGB at 1400 px rather than
        // 5.9 MB, doubled again by the compare view holding two at once.
        value = withContext(Dispatchers.IO) {
            OrientedBitmaps.decode(file, reqPx, exactFit = true)?.asImageBitmap()
        }
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
