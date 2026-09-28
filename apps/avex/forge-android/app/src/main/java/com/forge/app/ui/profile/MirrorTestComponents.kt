@file:OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)

package com.forge.app.ui.profile

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.outlined.Circle
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.text.input.KeyboardCapitalization
import com.forge.app.ui.common.ForgeFieldRow
import com.forge.app.ui.common.GROUP_OUTER
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.photo.PhotoPose
import com.forge.app.ui.common.window.AlertDialog
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/** The inset the gallery's text content sits in. The grid itself runs edge to edge. */
internal val GALLERY_GUTTER = 16.dp

/** The gap between thumbnails, both ways. Thin, so the photos read as one sheet rather than tiles. */
private val CELL_GAP = 2.dp

/**
 * The gallery's type: the app theme, with the label roles moved to the sans face.
 *
 * The app's label roles are monospace and its headline roles serif, and every Material control
 * (chips, buttons, menus, segmented buttons, the FAB, dialog titles) sets its text in them. Inside
 * the gallery those controls are plain Material, so they get the one sans face they were designed
 * around. Colour and shape stay the app's own, with the one gap in its scheme filled below.
 */
@Composable
internal fun GalleryTheme(content: @Composable () -> Unit) {
    val base = MaterialTheme.typography
    val medium = base.bodySmall.copy(fontWeight = FontWeight.Medium)
    fun TextStyle.sans() = copy(fontFamily = FontFamily.SansSerif)
    val colors = MaterialTheme.colorScheme
    MaterialTheme(
        // The app theme never sets the secondary containers, so Material's selected chips and tonal
        // buttons fell through to its stock purple. Point them at the accent's own container.
        colorScheme = colors.copy(
            secondaryContainer = colors.primaryContainer,
            onSecondaryContainer = colors.onPrimaryContainer
        ),
        shapes = MaterialTheme.shapes,
        typography = base.copy(
            headlineLarge = base.headlineLarge.sans(),
            headlineMedium = base.headlineMedium.sans(),
            headlineSmall = base.headlineSmall.sans(),
            labelLarge = base.titleSmall,
            labelMedium = medium,
            labelSmall = medium
        ),
        content = content
    )
}

// ── Grid ─────────────────────────────────────────────────────────────────────

/** How the grid renders each photo, and what a tap or a hold on one does. */
internal data class GalleryGridSpec(
    val columns: Int,
    val fileFor: (ProgressPhoto) -> File,
    val onPhotoClick: (ProgressPhoto) -> Unit,
    val onPhotoLongClick: (ProgressPhoto) -> Unit,
    /** True while a multi-selection is open: taps toggle instead of opening the viewer. */
    val selecting: Boolean = false,
    val isSelected: (ProgressPhoto) -> Boolean = { false },
    /** Select or clear a whole month at once from its header, while selecting. */
    val onToggleSection: (GallerySection) -> Unit = {},
    /** Print each thumbnail's date on it. Off where the cell is too small for the words to fit. */
    val showDates: Boolean = true
)

/**
 * The month-grouped photo grid, emitted straight into the screen's [LazyListScope] so the rows are
 * the list's own items: nothing composes or decodes until it scrolls near the screen. Each month's
 * header pins while its rows scroll under it.
 */
internal fun LazyListScope.galleryGrid(sections: List<GallerySection>, spec: GalleryGridSpec) {
    sections.forEach { section ->
        stickyHeader(key = "month-${section.date}", contentType = "month") { SectionHeader(section, spec) }
        val rows = section.photos.chunked(spec.columns)
        rows.forEachIndexed { index, row ->
            item(key = "row-${section.date}-${spec.columns}-${row.first().fileName}", contentType = "row") {
                PhotoRow(row, spec, lastInSection = index == rows.lastIndex)
            }
        }
    }
}

/** A pinned month header on the page's own ground, so photos are hidden cleanly as they pass under it. */
@Composable
private fun SectionHeader(section: GallerySection, spec: GalleryGridSpec) {
    val allSelected = spec.selecting && section.photos.all(spec.isSelected)
    Row(
        Modifier.fillMaxWidth()
            .background(MaterialTheme.colorScheme.background)
            .heightIn(min = 48.dp)
            .padding(start = GALLERY_GUTTER, end = if (spec.selecting) 4.dp else GALLERY_GUTTER),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            section.label,
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onBackground,
            modifier = Modifier.weight(1f).padding(vertical = 8.dp)
        )
        Text(section.meta, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        if (spec.selecting) {
            IconButton(onClick = { spec.onToggleSection(section) }) {
                SelectionMark(
                    selected = allSelected,
                    contentDescription = if (allSelected) "Clear ${section.label}" else "Select all from ${section.label}",
                    onPhoto = false
                )
            }
        }
    }
}

@Composable
private fun PhotoRow(row: List<ProgressPhoto>, spec: GalleryGridSpec, lastInSection: Boolean) {
    // Decode at roughly the cell's size: a 5-across cell is ~80dp, a 2-across one ~180dp.
    val reqPx = when {
        spec.columns >= 5 -> 200
        spec.columns == 4 -> 260
        spec.columns == 3 -> 360
        else -> 520
    }
    Row(
        Modifier.fillMaxWidth().padding(bottom = if (lastInSection) 12.dp else CELL_GAP),
        horizontalArrangement = Arrangement.spacedBy(CELL_GAP)
    ) {
        row.forEach { photo -> PhotoCell(photo, spec, Modifier.weight(1f), reqPx) }
        repeat(spec.columns - row.size) { Box(Modifier.weight(1f)) }
    }
}

/**
 * One square thumbnail: the photo, and its date small in the corner on a short scrim, so a month of
 * shots can be told apart without opening them. While selecting, a picked photo insets and rounds
 * with a filled check, and an unpicked one shows an empty ring, the way the system photo picker does.
 */
@Composable
private fun PhotoCell(photo: ProgressPhoto, spec: GalleryGridSpec, modifier: Modifier, reqPx: Int) {
    val selected = spec.selecting && spec.isSelected(photo)
    val inset by animateDpAsState(if (selected) 10.dp else 0.dp, label = "cell-inset")
    val corner by animateDpAsState(if (selected) 12.dp else 0.dp, label = "cell-corner")
    val haptics = LocalHapticFeedback.current
    val reading = remember(photo.takenAtMs, photo.title, photo.pose) { photoReading(photo) }
    val date = remember(photo.takenAtMs) { SimpleDateFormat("MMM d", Locale.getDefault()).format(Date(photo.takenAtMs)) }

    Box(
        modifier.aspectRatio(1f)
            .background(if (selected) MaterialTheme.colorScheme.primaryContainer else Color.Transparent)
            .combinedClickable(
                onClickLabel = if (spec.selecting) (if (selected) "Deselect" else "Select") else "Open",
                onLongClickLabel = if (spec.selecting) null else "Select",
                onLongClick = if (spec.selecting) null else {
                    {
                        haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                        spec.onPhotoLongClick(photo)
                    }
                },
                onClick = { spec.onPhotoClick(photo) }
            )
            .semantics {
                contentDescription = reading
                if (spec.selecting) this.selected = selected
            }
    ) {
        ProgressPhotoImage(
            spec.fileFor(photo),
            Modifier.fillMaxSize().padding(inset).clip(RoundedCornerShape(corner)),
            reqPx = reqPx
        )
        if (spec.showDates && !selected) {
            Box(
                Modifier.fillMaxWidth().aspectRatio(2.5f).align(Alignment.BottomStart)
                    .background(Brush.verticalGradient(listOf(Color.Transparent, Color.Black.copy(alpha = 0.6f))))
            )
            Text(
                date,
                style = MaterialTheme.typography.labelMedium,
                color = Color.White,
                modifier = Modifier.align(Alignment.BottomStart).padding(horizontal = 6.dp, vertical = 4.dp)
            )
        }
        if (spec.selecting) {
            if (!selected) {
                // A faint top scrim so the white ring reads on a pale shot.
                Box(
                    Modifier.fillMaxWidth().aspectRatio(3f).align(Alignment.TopStart)
                        .background(Brush.verticalGradient(listOf(Color.Black.copy(alpha = 0.35f), Color.Transparent)))
                )
            }
            Box(Modifier.align(Alignment.TopStart).padding(6.dp)) {
                SelectionMark(selected = selected, contentDescription = null, onPhoto = true)
            }
        }
    }
}

/** The select ring / filled check shared by cells and day headers. */
@Composable
private fun SelectionMark(selected: Boolean, contentDescription: String?, onPhoto: Boolean) {
    if (selected) {
        Box(Modifier.size(24.dp).clip(CircleShape).background(MaterialTheme.colorScheme.background)) {
            Icon(
                Icons.Filled.CheckCircle, contentDescription = contentDescription,
                tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(24.dp)
            )
        }
    } else {
        Icon(
            Icons.Outlined.Circle, contentDescription = contentDescription,
            tint = if (onPhoto) Color.White else MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(24.dp)
        )
    }
}

/** What TalkBack reads for a cell: its title if it has one, its pose, and its date. */
private fun photoReading(photo: ProgressPhoto): String {
    val date = SimpleDateFormat("EEEE, MMMM d, yyyy", Locale.getDefault()).format(Date(photo.takenAtMs))
    return listOfNotNull(
        photo.title.trim().ifBlank { null },
        PhotoPose.fromKey(photo.pose)?.let { "${it.label} pose" },
        date
    ).joinToString(", ")
}

// ── Dialogs ──────────────────────────────────────────────────────────────────

/** Album name entry, for both "New album" and "Rename album". Opens with the keyboard up. */
@Composable
internal fun NameDialog(
    title: String,
    initial: String,
    confirmLabel: String,
    onConfirm: (String) -> Unit,
    onDismiss: () -> Unit
) {
    var text by remember { mutableStateOf(initial) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { DialogTitle(title) },
        text = {
            // An inline field on one filled row, one step above the dialog's own surface.
            Box(
                Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(GROUP_OUTER))
                    .background(MaterialTheme.colorScheme.surfaceContainerHighest)
            ) {
                ForgeFieldRow(
                    label = "Album name",
                    value = text,
                    onValueChange = { text = it.take(30) },
                    placeholder = "Name",
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Done),
                    keyboardActions = KeyboardActions(onDone = { if (text.isNotBlank()) onConfirm(text.trim()) }),
                    focusRequester = focus
                )
            }
        },
        confirmButton = {
            TextButton(enabled = text.isNotBlank(), onClick = { onConfirm(text.trim()) }) { Text(confirmLabel) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } }
    )
}

/**
 * The one confirmation the gallery asks for: deleting photos, which removes the only copy there is.
 * The confirm button is the error colour so the destructive choice is never the neutral-looking one.
 */
@Composable
internal fun DeletePhotosDialog(count: Int, onConfirm: () -> Unit, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { DialogTitle(if (count == 1) "Delete this photo?" else "Delete $count photos?") },
        text = {
            Text(
                if (count == 1) "It's removed from this phone and can't be undone."
                else "They're removed from this phone and can't be undone.",
                style = MaterialTheme.typography.bodyMedium
            )
        },
        confirmButton = {
            TextButton(onClick = onConfirm) { Text("Delete", color = MaterialTheme.colorScheme.error) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } }
    )
}

/** A dialog heading in the Material dialog-title role. */
@Composable
internal fun DialogTitle(text: String) {
    Text(text, style = MaterialTheme.typography.headlineSmall)
}
