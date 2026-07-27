@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.photo.PhotoPose
import com.forge.app.ui.common.bounceClick
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

internal val MONTH_HEADER_FMT: DateTimeFormatter = DateTimeFormatter.ofPattern("MMMM yyyy", Locale.getDefault())
/** Day-group headers — "MON 21 JUL", widened to "MON 21 JUL 2025" outside the current year. */
private val DAY_HEADER_FMT: DateTimeFormatter = DateTimeFormatter.ofPattern("EEE d MMM", Locale.getDefault())
private val DAY_HEADER_YEAR_FMT: DateTimeFormatter = DateTimeFormatter.ofPattern("EEE d MMM yyyy", Locale.getDefault())

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
 * A chronological photo grid grouped under DAY headers — the phone-gallery shape (2026-07-25, Antho:
 * "a gallery of photos like phones, with metadata, classed per day, in order").
 *
 * It grouped by MONTH until then, which is the wrong grain for this library: you shoot a set of
 * angles in one session, so the meaningful unit is the day you took them, not the month they fell in.
 * A month header also silently hid the thing that makes the gallery useful — that several shots
 * belong to one sitting.
 *
 * The header names the day the way you'd say it ("TODAY", "YESTERDAY", "MON 21 JUL", plus the year
 * once it isn't this one) and carries the day's own metadata as meta: how many shots, and the titles
 * or poses they carry. [photos] arrives already filtered and sorted, so the day groups follow that
 * order — newest-first or oldest-first, whichever the sort chip says. When [selectable] the cells
 * carry a selection ring + numeric badge (compare mode).
 */
@Composable
internal fun DayGroupedGrid(
    photos: List<ProgressPhoto>,
    columns: Int,
    zone: ZoneId,
    fileFor: (ProgressPhoto) -> File,
    muted: Color,
    accent: Color,
    onPhotoClick: (ProgressPhoto) -> Unit,
    selectable: Boolean = false,
    selectionIndexOf: (ProgressPhoto) -> Int? = { null }
) {
    // groupBy keeps first-seen order, so days follow the list's sort.
    val grouped = remember(photos, zone) {
        photos.groupBy { Instant.ofEpochMilli(it.takenAtMs).atZone(zone).toLocalDate() }
    }
    val today = remember(zone) { LocalDate.now(zone) }
    grouped.forEach { (day, dayPhotos) ->
        Text(
            dayHeaderLabel(day, today),
            style = MaterialTheme.typography.labelMedium, color = muted, letterSpacing = 1.sp
        )
        dayMetaLine(dayPhotos)?.let { meta ->
            Spacer(Modifier.height(2.dp))
            // What the day's shots ARE, at the day's own level — so a set of angles reads as one
            // sitting rather than as N anonymous thumbnails (§4.10: the group carries the reading).
            Text(
                meta,
                style = MaterialTheme.typography.labelSmall,
                color = muted.copy(alpha = 0.7f), fontSize = 9.sp, letterSpacing = 0.5.sp
            )
        }
        Spacer(Modifier.height(8.dp))
        PhotoRows(dayPhotos, columns, fileFor, muted, accent, onPhotoClick, selectable, selectionIndexOf)
        Spacer(Modifier.height(16.dp))
    }
}

/** "TODAY" · "YESTERDAY" · "MON 21 JUL" · "MON 21 JUL 2025" once the year isn't the current one. */
private fun dayHeaderLabel(day: LocalDate, today: LocalDate): String = when (day) {
    today -> "TODAY"
    today.minusDays(1) -> "YESTERDAY"
    else -> day.format(if (day.year == today.year) DAY_HEADER_FMT else DAY_HEADER_YEAR_FMT).uppercase()
}

/**
 * The day's metadata line: the shot count once there's more than one, then what those shots are —
 * the titles the user gave them, or their poses when they're untitled. Null when a lone untitled
 * photo would leave nothing worth saying (the cell already carries its own date).
 */
private fun dayMetaLine(dayPhotos: List<ProgressPhoto>): String? {
    val labels = dayPhotos.mapNotNull { p ->
        p.title.trim().ifBlank { PhotoPose.fromKey(p.pose)?.label }
    }.distinct()
    val count = if (dayPhotos.size > 1) "${dayPhotos.size} SHOTS" else null
    val what = labels.takeIf { it.isNotEmpty() }?.joinToString(" · ") { it.uppercase() }
    return listOfNotNull(count, what).takeIf { it.isNotEmpty() }?.joinToString(" · ")
}

/** Lays [photos] out in rows of [columns], padding the last row so cells keep an even width. */
@Composable
internal fun PhotoRows(
    photos: List<ProgressPhoto>,
    columns: Int,
    fileFor: (ProgressPhoto) -> File,
    muted: Color,
    accent: Color,
    onPhotoClick: (ProgressPhoto) -> Unit,
    selectable: Boolean = false,
    selectionIndexOf: (ProgressPhoto) -> Int? = { null }
) {
    val reqPx = if (columns >= 4) 240 else 300
    photos.chunked(columns).forEach { row ->
        Row(Modifier.fillMaxWidth().padding(bottom = 6.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            row.forEach { photo ->
                PhotoCell(
                    photo, fileFor, onPhotoClick, muted, accent, Modifier.weight(1f),
                    reqPx = reqPx, selectable = selectable, selectionIndex = selectionIndexOf(photo)
                )
            }
            repeat(columns - row.size) { Box(Modifier.weight(1f)) }
        }
    }
}

/**
 * A single square photo cell with its short date laid over a soft bottom scrim (matching the
 * Profile filmstrip), plus optional compare-selection chrome. Deliberately date-only — the pose
 * lens pills group by pose, so a pose chip on every cell would be uniform noise (§8).
 */
@Composable
internal fun PhotoCell(
    photo: ProgressPhoto,
    fileFor: (ProgressPhoto) -> File,
    onClick: (ProgressPhoto) -> Unit,
    @Suppress("UNUSED_PARAMETER") muted: Color,
    accent: Color,
    modifier: Modifier = Modifier,
    reqPx: Int = 300,
    showDate: Boolean = true,
    selectable: Boolean = false,
    selectionIndex: Int? = null
) {
    val selected = selectionIndex != null
    Box(modifier.aspectRatio(1f).clip(RoundedCornerShape(12.dp)).bounceClick { onClick(photo) }) {
        ProgressPhotoImage(fileFor(photo), Modifier.fillMaxSize(), reqPx = reqPx)
        if (showDate) {
            Box(
                Modifier.matchParentSize().background(
                    Brush.verticalGradient(0.62f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.55f))
                )
            )
            Text(
                SimpleDateFormat("MMM d", Locale.getDefault()).format(Date(photo.takenAtMs)).uppercase(),
                style = MaterialTheme.typography.labelSmall, color = Color.White.copy(alpha = 0.92f), fontSize = 8.sp,
                modifier = Modifier.align(Alignment.BottomStart).padding(horizontal = 7.dp, vertical = 6.dp)
            )
        }
        if (selectable) {
            if (selected) {
                Box(Modifier.matchParentSize().border(2.dp, accent, RoundedCornerShape(12.dp)))
                Box(
                    Modifier.align(Alignment.TopEnd).padding(4.dp).size(20.dp).clip(CircleShape).background(accent),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        "${selectionIndex!! + 1}", style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onPrimary, fontSize = 11.sp
                    )
                }
            } else {
                // Dim the unpicked while choosing, with an empty ring to read as "selectable".
                Box(Modifier.matchParentSize().background(Color.Black.copy(alpha = 0.28f)))
                Box(
                    Modifier.align(Alignment.TopEnd).padding(4.dp).size(20.dp).clip(CircleShape)
                        .border(1.5.dp, Color.White.copy(alpha = 0.85f), CircleShape)
                )
            }
        }
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
            "No albums yet. Albums group photos without moving them from the main grid.",
            style = MaterialTheme.typography.bodyMedium, color = muted
        )
        return
    }
    folders.chunked(2).forEach { row ->
        Row(Modifier.fillMaxWidth().padding(bottom = 14.dp), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            row.forEach { folder ->
                Column(Modifier.weight(1f).bounceClick { onOpen(folder.name) }) {
                    Box(
                        Modifier.fillMaxWidth().aspectRatio(1f).clip(RoundedCornerShape(12.dp))
                            .border(1.dp, outline.copy(alpha = 0.25f), RoundedCornerShape(12.dp))
                    ) {
                        folder.cover?.let { ProgressPhotoImage(fileFor(it), Modifier.fillMaxSize()) }
                    }
                    Spacer(Modifier.height(6.dp))
                    Text(folder.displayName, style = MaterialTheme.typography.bodyMedium, color = onBg, maxLines = 1)
                    Text(
                        "${folder.count} photo${if (folder.count == 1) "" else "s"}",
                        style = MaterialTheme.typography.labelSmall, color = muted
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
    columns: Int,
    zone: ZoneId,
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
        Text("No photos in this album yet. Tap + to add one.", style = MaterialTheme.typography.bodyMedium, color = muted)
        return
    }
    DayGroupedGrid(photos, columns, zone, fileFor, muted, accent, onView)
}

/** A pill chip used for both the time-range filters and the move-to-album picker. */
@Composable
internal fun GalleryChip(label: String, selected: Boolean, onClick: () -> Unit) {
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline
    val bg = if (selected) accent.copy(alpha = 0.15f) else Color.Transparent
    val border = if (selected) accent else outline.copy(alpha = 0.35f)
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
