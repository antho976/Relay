@file:OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)

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
import androidx.compose.foundation.lazy.LazyListScope
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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.photo.musclesFromCodes
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.bounceCombinedClick
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/** The gutter the gallery's own content sits in — the grid runs edge-to-edge inside it. */
internal val GALLERY_GUTTER = 24.dp

/** How the grid renders each photo, and what a tap or a hold on one does. */
internal data class GalleryGridSpec(
    val columns: Int,
    val fileFor: (ProgressPhoto) -> java.io.File,
    val onPhotoClick: (ProgressPhoto) -> Unit,
    val onPhotoLongClick: ((ProgressPhoto) -> Unit)? = null,
    val selectable: Boolean = false,
    val selectionIndexOf: (ProgressPhoto) -> Int? = { null },
    /** The muscle the grid is currently lensed to; its badge is suppressed on every cell, because a
     *  label repeated on every thumbnail in view is noise, not information (§8). */
    val lensMuscle: MuscleGroup? = null
)

/**
 * The chronological photo grid, emitted straight into the screen's [LazyListScope].
 *
 * Lazy, not a scrolling Column of every cell. The old grid composed and decoded a bitmap for every
 * photo in the library the moment the screen opened, which is survivable at twenty shots and is a
 * stall you can feel at three hundred. A gallery is the one screen in this app whose content is
 * genuinely unbounded, so it is the one screen that has to page.
 *
 * Days pin their header while their own rows scroll under it, the way a phone gallery does. The
 * header carries the day's own reading (how many shots, what they are, the tags they carry), so a
 * set of angles reads as one sitting rather than as N anonymous thumbnails (§4.10).
 */
internal fun LazyListScope.galleryGrid(
    days: List<GalleryDay>,
    spec: GalleryGridSpec,
    muted: Color,
    accent: Color,
    background: Color
) {
    days.forEach { day ->
        stickyHeader(key = "day-${day.date}") { DayHeader(day, muted, background) }
        // Rows, not cells, are the lazy unit: a row is one measured line of the grid, so the list
        // never has to reason about how many cells share a line at the current density.
        val rows = day.photos.chunked(spec.columns)
        rows.forEachIndexed { index, row ->
            item(key = "row-${day.date}-${row.first().fileName}") {
                PhotoRow(row, spec, accent, lastInDay = index == rows.lastIndex)
            }
        }
    }
}

/**
 * A pinned day header. It fills with the page's own ground and fades out at its foot rather than
 * sitting in a box: photos have to be occluded as they pass under it, and a gradient is the one
 * fill this language allows on passive content (§1/§5).
 */
@Composable
private fun DayHeader(day: GalleryDay, muted: Color, background: Color) {
    Column(
        Modifier.fillMaxWidth()
            .background(Brush.verticalGradient(0f to background, 0.72f to background, 1f to Color.Transparent))
            .padding(start = GALLERY_GUTTER, end = GALLERY_GUTTER, top = 10.dp, bottom = 12.dp)
    ) {
        Text(day.label, style = MaterialTheme.typography.labelMedium, color = muted)
        day.meta?.let { meta ->
            Spacer(Modifier.height(2.dp))
            Text(meta, style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.7f))
        }
    }
}

/** One row of the grid, padded so the last row of a day carries the gap to the next day header. */
@Composable
private fun PhotoRow(
    row: List<ProgressPhoto>,
    spec: GalleryGridSpec,
    accent: Color,
    lastInDay: Boolean
) {
    val reqPx = if (spec.columns >= 4) 240 else 320
    Row(
        Modifier.fillMaxWidth()
            .padding(start = GALLERY_GUTTER, end = GALLERY_GUTTER, bottom = if (lastInDay) 18.dp else 6.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        row.forEach { photo ->
            PhotoCell(photo, spec, accent, Modifier.weight(1f), reqPx = reqPx)
        }
        repeat(spec.columns - row.size) { Box(Modifier.weight(1f)) }
    }
}

/**
 * A single square photo cell: the shot, and on one bottom scrim the muscle it documents over its
 * short date. The muscle line is drawn only when the photo carries tags the grid is not already
 * filtered to.
 *
 * The per-cell POSE chip was removed in an earlier pass as uniform noise, and that finding still
 * holds: nearly every photo had a pose, so the chip said the same word on every thumbnail. A muscle
 * badge is the opposite case. It is optional, it varies shot to shot, and it is the tag you filter
 * by, so seeing it is how you learn which of your shots are actually tagged. It hides itself the
 * moment it would become uniform: under a muscle lens, every visible cell would carry that same
 * word, so none of them do.
 */
@Composable
internal fun PhotoCell(
    photo: ProgressPhoto,
    spec: GalleryGridSpec,
    accent: Color,
    modifier: Modifier = Modifier,
    reqPx: Int = 320,
    showDate: Boolean = true
) {
    val selectionIndex = spec.selectionIndexOf(photo)
    val selected = selectionIndex != null
    val date = remember(photo.takenAtMs) {
        SimpleDateFormat("MMM d", Locale.getDefault()).format(Date(photo.takenAtMs)).uppercase()
    }
    val badge = remember(photo.muscles, spec.lensMuscle) { cellMuscleBadge(photo, spec.lensMuscle) }
    val reading = listOfNotNull(
        photo.title.trim().ifBlank { null },
        date,
        badge,
        if (selected) "selected" else null
    ).joinToString(", ")

    Box(
        modifier.aspectRatio(1f).clip(RoundedCornerShape(12.dp))
            .bounceCombinedClick(
                onClickLabel = if (spec.selectable) "Select photo" else "Open photo",
                onLongClickLabel = if (spec.onPhotoLongClick != null) "Select photos" else null,
                onLongClick = spec.onPhotoLongClick?.let { { it(photo) } },
                onClick = { spec.onPhotoClick(photo) }
            )
            .semantics { contentDescription = reading }
    ) {
        ProgressPhotoImage(spec.fileFor(photo), Modifier.fillMaxSize(), reqPx = reqPx)
        if (showDate) {
            Box(
                // Deeper and taller than the one-line version it replaced: the caption is two lines
                // now, and a pale physique under a light photo left the muscle line reading grey.
                Modifier.matchParentSize().background(
                    Brush.verticalGradient(0.45f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.72f))
                )
            )
            // Muscle over date, both on the scrim the cell already draws. The badge used to wear its
            // own black plate in the top corner, which read as a sticker applied to the photo rather
            // than as the photo's caption, and put two separate dark shapes on every thumbnail.
            Column(Modifier.align(Alignment.BottomStart).padding(horizontal = 7.dp, vertical = 6.dp)) {
                if (badge != null && !spec.selectable) {
                    Text(badge, style = MaterialTheme.typography.labelSmall, color = Color.White.copy(alpha = 0.7f))
                }
                Text(date, style = MaterialTheme.typography.labelSmall, color = Color.White)
            }
        }
        if (spec.selectable) {
            if (selected) {
                Box(Modifier.matchParentSize().border(2.dp, accent, RoundedCornerShape(12.dp)))
                Box(
                    Modifier.align(Alignment.TopEnd).padding(4.dp).size(22.dp).clip(CircleShape).background(accent),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        "${selectionIndex + 1}", style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onPrimary
                    )
                }
            } else {
                // Dim the unpicked while choosing, with an empty ring to read as "selectable".
                Box(Modifier.matchParentSize().background(Color.Black.copy(alpha = 0.25f)))
                Box(
                    Modifier.align(Alignment.TopEnd).padding(4.dp).size(22.dp).clip(CircleShape)
                        .border(1.5.dp, Color.White.copy(alpha = 0.7f), CircleShape)
                )
            }
        }
    }
}

/** "BACK" · "BACK +2" · null when the photo is untagged or the grid is already lensed to it. */
private fun cellMuscleBadge(photo: ProgressPhoto, lens: MuscleGroup?): String? {
    val muscles = musclesFromCodes(photo.muscles)
    if (muscles.isEmpty()) return null
    if (lens != null && muscles.size == 1 && muscles.first() == lens) return null
    val head = muscles.first().displayName.uppercase()
    return if (muscles.size == 1) head else "$head +${muscles.size - 1}"
}

// ── Albums ───────────────────────────────────────────────────────────────────

/** The optional Albums level: a grid of folders. Reached via "Albums →"; creating albums lives here. */
@Composable
internal fun FolderGrid(
    folders: List<MirrorTestViewModel.AlbumFolder>,
    onOpen: (String) -> Unit,
    onNewAlbum: () -> Unit,
    fileFor: (ProgressPhoto) -> java.io.File,
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
                    Text(folder.displayName, style = MaterialTheme.typography.bodyMedium, color = onBg)
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

/** The rename / delete row above a named album's photos. */
@Composable
internal fun AlbumActions(onRename: () -> Unit, onDelete: () -> Unit, accent: Color) {
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
}

// ── Chips + dialogs ──────────────────────────────────────────────────────────

/**
 * The one chip vocabulary the gallery uses, for filters, tag rails and the viewer's tag editors.
 *
 * Selection is carried by the accent BORDER and its 0.15 wash, never by accent text: accent-as-text
 * measures below AA under four of the five accent choices (§14), so the label stays on `onBackground`
 * and the colour does the flagging. [trailing] carries a count or a `✕` without a second tap target.
 */
@Composable
internal fun GalleryChip(
    label: String,
    selected: Boolean,
    trailing: String? = null,
    onClick: () -> Unit
) {
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        Modifier
            .clip(RoundedCornerShape(50))
            .background(if (selected) accent.copy(alpha = 0.15f) else Color.Transparent)
            .border(1.dp, if (selected) accent else outline.copy(alpha = 0.35f), RoundedCornerShape(50))
            .bounceClick { onClick() }
            .padding(horizontal = 12.dp, vertical = 7.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        Text(
            label,
            style = MaterialTheme.typography.labelMedium,
            color = if (selected) onBg else muted
        )
        if (trailing != null) {
            Text(trailing, style = MaterialTheme.typography.labelSmall, color = if (selected) onBg else muted)
        }
    }
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
        containerColor = MaterialTheme.colorScheme.surface,
        title = { Text(title) },
        text = {
            OutlinedTextField(
                value = text,
                onValueChange = { text = it.take(30) },
                label = { Text("Album name") },
                colors = bodyLogFieldColors(),
                singleLine = true
            )
        },
        confirmButton = {
            TextButton(enabled = text.isNotBlank(), onClick = { onConfirm(text.trim()) }) { Text("Save") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } }
    )
}
