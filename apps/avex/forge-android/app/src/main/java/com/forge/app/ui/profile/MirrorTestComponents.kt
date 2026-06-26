@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.theme.LocalForgeSettings
import java.io.File
import java.text.SimpleDateFormat
import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.YearMonth
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.TemporalAdjusters
import java.util.Date
import java.util.Locale

/** Time windows for the Gallery photo grid. Calendar-aware (not rolling) so they read predictably. */
internal enum class GalleryRange(val label: String) {
    ALL("All"), WEEK("This week"), MONTH("This month"), LAST_MONTH("Last month"), QUARTER("Last 3 months")
}

private val MONTH_HEADER_FMT: DateTimeFormatter = DateTimeFormatter.ofPattern("MMMM yyyy", Locale.getDefault())

/** True if [takenAtMs] falls inside [range]. [firstDayMonday] only affects the "This week" window. */
internal fun galleryRangeMatches(takenAtMs: Long, range: GalleryRange, zone: ZoneId, firstDayMonday: Boolean): Boolean {
    if (range == GalleryRange.ALL) return true
    val date = Instant.ofEpochMilli(takenAtMs).atZone(zone).toLocalDate()
    val today = LocalDate.now(zone)
    val thisMonth = YearMonth.from(today)
    return when (range) {
        GalleryRange.ALL -> true
        GalleryRange.WEEK -> {
            val firstDow = if (firstDayMonday) DayOfWeek.MONDAY else DayOfWeek.SUNDAY
            val weekStart = today.with(TemporalAdjusters.previousOrSame(firstDow))
            !date.isBefore(weekStart) && !date.isAfter(today)
        }
        GalleryRange.MONTH -> YearMonth.from(date) == thisMonth
        GalleryRange.LAST_MONTH -> YearMonth.from(date) == thisMonth.minusMonths(1)
        GalleryRange.QUARTER -> !date.isBefore(today.minusMonths(3)) && !date.isAfter(today)
    }
}

/**
 * The default Gallery level: a chronological photo grid grouped under month headers, with time-range
 * filter chips and a quiet "Albums →" entry. First open shows photos + dates — never an empty folder
 * wall. Photos arrive newest-first (the VM sorts), so the month groups and cells stay in that order.
 */
@Composable
internal fun PhotosLevel(
    photos: List<ProgressPhoto>,
    range: GalleryRange,
    onRangeChange: (GalleryRange) -> Unit,
    onOpenAlbums: () -> Unit,
    onView: (ProgressPhoto) -> Unit,
    fileFor: (ProgressPhoto) -> File,
    onBg: Color, muted: Color, accent: Color, outline: Color
) {
    // Quiet "Albums" affordance — organizing into folders is optional and tucked up here, never forced.
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text("Newest first.", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic, fontSize = 11.sp)
        Text(
            "Albums →", style = MaterialTheme.typography.labelMedium, color = accent,
            modifier = Modifier.bounceClick { onOpenAlbums() }.padding(vertical = 4.dp)
        )
    }

    if (photos.isEmpty()) {
        Spacer(Modifier.height(12.dp))
        Text("No photos yet — tap + to add your first.", style = MaterialTheme.typography.bodyMedium, color = muted)
        return
    }

    // Time-range chips.
    Spacer(Modifier.height(10.dp))
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        GalleryRange.entries.forEach { r -> GalleryChip(r.label, selected = r == range) { onRangeChange(r) } }
    }
    Spacer(Modifier.height(16.dp))

    val zone = remember { ZoneId.systemDefault() }
    val firstDayMonday = LocalForgeSettings.current.firstDayMonday
    val filtered = remember(photos, range, firstDayMonday) {
        photos.filter { galleryRangeMatches(it.takenAtMs, range, zone, firstDayMonday) }
    }
    if (filtered.isEmpty()) {
        Text("No photos in this range.", style = MaterialTheme.typography.bodyMedium, color = muted)
        return
    }
    val grouped = remember(filtered) {
        filtered.groupBy { YearMonth.from(Instant.ofEpochMilli(it.takenAtMs).atZone(zone)) }
    }
    grouped.forEach { (month, monthPhotos) ->
        Text(
            month.format(MONTH_HEADER_FMT).uppercase(),
            style = MaterialTheme.typography.labelMedium, color = muted
        )
        Spacer(Modifier.height(8.dp))
        monthPhotos.chunked(3).forEach { rowPhotos ->
            Row(Modifier.fillMaxWidth().padding(bottom = 6.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                rowPhotos.forEach { photo -> PhotoCell(photo, fileFor, onView, muted, Modifier.weight(1f)) }
                repeat(3 - rowPhotos.size) { Box(Modifier.weight(1f)) }
            }
        }
        Spacer(Modifier.height(16.dp))
    }
}

/** A single square photo cell with its short date underneath. */
@Composable
internal fun PhotoCell(
    photo: ProgressPhoto,
    fileFor: (ProgressPhoto) -> File,
    onView: (ProgressPhoto) -> Unit,
    muted: Color,
    modifier: Modifier = Modifier
) {
    Column(modifier) {
        Box(Modifier.fillMaxWidth().aspectRatio(1f).clip(RoundedCornerShape(8.dp)).bounceClick { onView(photo) }) {
            ProgressPhotoImage(fileFor(photo), Modifier.fillMaxSize(), reqPx = 300)
        }
        Spacer(Modifier.height(4.dp))
        Text(
            SimpleDateFormat("MMM d", Locale.getDefault()).format(Date(photo.takenAtMs)).uppercase(),
            style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp, modifier = Modifier.padding(start = 2.dp)
        )
    }
}

/** The optional Albums level: a grid of folders. Reached via "Albums →"; creating albums lives here. */
@Composable
internal fun FolderGrid(
    folders: List<MirrorTestViewModel.AlbumFolder>,
    onOpen: (String) -> Unit,
    onNewAlbum: () -> Unit,
    fileFor: (ProgressPhoto) -> File,
    onBg: Color, muted: Color, accent: Color, outline: Color
) {
    Text(
        "+ New album", style = MaterialTheme.typography.labelLarge, color = accent,
        modifier = Modifier.bounceClick { onNewAlbum() }.padding(vertical = 4.dp)
    )
    Spacer(Modifier.height(16.dp))

    if (folders.isEmpty()) {
        Text(
            "No albums yet. Make one to group photos (e.g. Front, Back, a cut) — your photos stay in the main grid either way.",
            style = MaterialTheme.typography.bodyMedium, color = muted
        )
        return
    }
    folders.chunked(2).forEach { row ->
        Row(Modifier.fillMaxWidth().padding(bottom = 14.dp), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            row.forEach { folder ->
                Column(Modifier.weight(1f).bounceClick { onOpen(folder.name) }) {
                    Box(
                        Modifier.fillMaxWidth().aspectRatio(1f).clip(RoundedCornerShape(10.dp))
                            .border(1.dp, outline.copy(alpha = 0.25f), RoundedCornerShape(10.dp))
                    ) {
                        folder.cover?.let { ProgressPhotoImage(fileFor(it), Modifier.fillMaxSize()) }
                    }
                    Spacer(Modifier.height(6.dp))
                    Text(folder.displayName, style = MaterialTheme.typography.bodyMedium, color = onBg, maxLines = 1)
                    Text(
                        "${folder.count} photo${if (folder.count == 1) "" else "s"}",
                        style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp
                    )
                }
            }
            if (row.size == 1) Box(Modifier.weight(1f))
        }
    }
}

/** A single album's photos (drilled into from [FolderGrid]). Rename / delete apply to named albums only. */
@Composable
internal fun AlbumPhotos(
    photos: List<ProgressPhoto>,
    isNamed: Boolean,
    onView: (ProgressPhoto) -> Unit,
    onRename: () -> Unit,
    onDelete: () -> Unit,
    fileFor: (ProgressPhoto) -> File,
    onBg: Color, muted: Color, accent: Color, outline: Color
) {
    if (isNamed) {
        Row(horizontalArrangement = Arrangement.spacedBy(20.dp)) {
            Text(
                "Rename", style = MaterialTheme.typography.labelMedium, color = accent,
                modifier = Modifier.bounceClick { onRename() }.padding(vertical = 4.dp)
            )
            Text(
                "Delete album", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.error,
                modifier = Modifier.bounceClick { onDelete() }.padding(vertical = 4.dp)
            )
        }
        Spacer(Modifier.height(14.dp))
    }

    if (photos.isEmpty()) {
        Text("No photos in this album yet — tap + to add one.", style = MaterialTheme.typography.bodyMedium, color = muted)
        return
    }
    photos.chunked(3).forEach { row ->
        Row(Modifier.fillMaxWidth().padding(bottom = 6.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            row.forEach { photo -> PhotoCell(photo, fileFor, onView, muted, Modifier.weight(1f)) }
            repeat(3 - row.size) { Box(Modifier.weight(1f)) }
        }
    }
}

/** Full-screen photo viewer: caption + move-to-album chips + delete. */
@Composable
internal fun GalleryPhotoViewerDialog(
    photo: ProgressPhoto,
    file: File,
    albumNames: List<String>,
    onSaveNote: (String) -> Unit,
    onMove: (String) -> Unit,
    onDelete: () -> Unit,
    onDismiss: () -> Unit
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val accent = MaterialTheme.colorScheme.primary
    var noteInput by remember(photo.fileName) { mutableStateOf(photo.note) }
    fun commitNote() { if (noteInput.trim() != photo.note) onSaveNote(noteInput.trim()) }

    Dialog(onDismissRequest = { commitNote(); onDismiss() }) {
        Column(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp))
                .background(MaterialTheme.colorScheme.surface).padding(12.dp)
        ) {
            ProgressPhotoImage(file, Modifier.fillMaxWidth().aspectRatio(0.8f).clip(RoundedCornerShape(12.dp)), reqPx = 1200)
            Spacer(Modifier.height(10.dp))
            BasicTextField(
                value = noteInput,
                onValueChange = { noteInput = it.take(140) },
                textStyle = MaterialTheme.typography.bodySmall.copy(color = onBg),
                cursorBrush = SolidColor(accent),
                decorationBox = { inner ->
                    Box {
                        if (noteInput.isEmpty()) Text(
                            "Add a note…",
                            style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.5f), fontStyle = FontStyle.Italic
                        )
                        inner()
                    }
                },
                modifier = Modifier.fillMaxWidth()
            )
            Spacer(Modifier.height(12.dp))

            // Move-to-album chips — Unsorted plus every named album, current one highlighted.
            Text("ALBUM", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
            Spacer(Modifier.height(6.dp))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                GalleryChip("Unsorted", selected = photo.album.isBlank()) { commitNote(); onMove("") }
                albumNames.forEach { name ->
                    GalleryChip(name, selected = photo.album == name) { commitNote(); onMove(name) }
                }
            }

            Spacer(Modifier.height(12.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                Text(
                    SimpleDateFormat("MMM d, yyyy", Locale.getDefault()).format(Date(photo.takenAtMs)),
                    style = MaterialTheme.typography.labelSmall, color = muted
                )
                Text(
                    "delete", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.bounceClick { onDelete() }.padding(8.dp)
                )
            }
        }
    }
}

/** A pill chip used for both the time-range filters and the move-to-album picker. */
@Composable
internal fun GalleryChip(label: String, selected: Boolean, onClick: () -> Unit) {
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline
    val bg = if (selected) accent.copy(alpha = 0.15f) else Color.Transparent
    val border = if (selected) accent else outline.copy(alpha = 0.4f)
    Text(
        label,
        style = MaterialTheme.typography.labelMedium,
        color = if (selected) accent else MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier
            .clip(RoundedCornerShape(50))
            .background(bg)
            .border(1.dp, border, RoundedCornerShape(50))
            .bounceClick { onClick() }
            .padding(horizontal = 12.dp, vertical = 6.dp)
    )
}

/** Album name entry — used for both "New album" and "Rename album". */
@Composable
internal fun NameDialog(
    title: String,
    initial: String,
    onConfirm: (String) -> Unit,
    onDismiss: () -> Unit
) {
    var text by remember { mutableStateOf(initial) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            OutlinedTextField(
                value = text,
                onValueChange = { text = it.take(30) },
                label = { Text("Album name") },
                singleLine = true
            )
        },
        confirmButton = {
            TextButton(enabled = text.isNotBlank(), onClick = { onConfirm(text.trim()) }) { Text("Save") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } }
    )
}
