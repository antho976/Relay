@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.profile

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.photo.PhotoPose
import com.forge.app.domain.photo.PhotoTag
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.common.window.ModalBottomSheet

/** The filters a chip can open. Each one is a sheet; the grid never changes mode to filter. */
internal enum class GalleryFacet { ALBUM, POSE, MUSCLE, TAG, DATE }

/**
 * The one filter row: a chip per facet, each naming its current value once one is picked, so the
 * row itself is the summary of what is hiding photos. Picking opens a sheet, and a single-choice
 * sheet closes on the pick, so narrowing the grid is two taps.
 *
 * Album and Tag only appear once there are albums or tags to pick; the other three are fixed
 * vocabularies and always draw.
 */
@Composable
internal fun GalleryFilterRow(
    filter: GalleryFilter,
    hasAlbums: Boolean,
    hasTags: Boolean,
    onOpen: (GalleryFacet) -> Unit,
    onClearAll: () -> Unit,
    modifier: Modifier = Modifier
) {
    Row(
        modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = GALLERY_GUTTER),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        if (filter.activeFacets > 0) {
            ProfileFilledChip(
                text = "Clear",
                selected = false,
                onClick = onClearAll,
                leading = { Icon(Icons.Filled.Close, contentDescription = null, Modifier.size(16.dp)) },
                role = Role.Button
            )
        }
        if (hasAlbums || filter.album != null) {
            FacetChip(albumChipLabel(filter.album), filter.album != null) { onOpen(GalleryFacet.ALBUM) }
        }
        FacetChip(filter.pose?.label ?: "Pose", filter.pose != null) { onOpen(GalleryFacet.POSE) }
        FacetChip(setChipLabel("Muscle", filter.muscles.map(::muscleName)), filter.muscles.isNotEmpty()) {
            onOpen(GalleryFacet.MUSCLE)
        }
        if (hasTags || filter.tags.isNotEmpty()) {
            FacetChip(setChipLabel("Tag", filter.tags.map(PhotoTag::display)), filter.tags.isNotEmpty()) {
                onOpen(GalleryFacet.TAG)
            }
        }
        FacetChip(
            if (filter.range == GalleryRange.ALL) "Date" else filter.range.label,
            filter.range != GalleryRange.ALL
        ) { onOpen(GalleryFacet.DATE) }
    }
}

@Composable
private fun FacetChip(label: String, active: Boolean, onClick: () -> Unit) {
    // Filled, with the accent ring and wash once the facet is narrowing the grid.
    ProfileFilledChip(
        text = label,
        selected = active,
        onClick = onClick,
        trailing = { Icon(Icons.Filled.ArrowDropDown, contentDescription = null, Modifier.size(18.dp)) },
        role = Role.DropdownList
    )
}

private fun albumChipLabel(album: String?): String = when (album) {
    null -> "Album"
    "" -> "No album"
    else -> album
}

/** "Muscle" · "Chest" · "Chest +2". */
private fun setChipLabel(name: String, values: List<String>): String = when (values.size) {
    0 -> name
    1 -> values.first()
    else -> "${values.first()} +${values.size - 1}"
}

// ── Sheets ───────────────────────────────────────────────────────────────────

/**
 * The sheet behind one filter chip. Every option carries how many photos it would show, so an
 * option that would empty the grid says so before it is picked.
 */
@Composable
internal fun GalleryFacetSheet(
    facet: GalleryFacet,
    filter: GalleryFilter,
    photos: List<ProgressPhoto>,
    folders: List<MirrorTestViewModel.AlbumFolder>,
    knownTags: List<String>,
    onChange: (GalleryFilter) -> Unit,
    onNewAlbum: () -> Unit,
    onDismiss: () -> Unit
) {
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = MaterialTheme.colorScheme.surfaceContainerLow
    ) {
        when (facet) {
            GalleryFacet.POSE -> {
                val counts = remember(photos) { photos.groupingBy { it.pose }.eachCount() }
                SheetHeader("Pose", showClear = filter.pose != null) { onChange(filter.copy(pose = null)); onDismiss() }
                OptionList {
                    item { ChoiceRow("Any pose", photos.size, filter.pose == null) { onChange(filter.copy(pose = null)); onDismiss() } }
                    items(PhotoPose.entries) { p ->
                        ChoiceRow(p.label, counts[p.name] ?: 0, filter.pose == p) { onChange(filter.copy(pose = p)); onDismiss() }
                    }
                }
            }
            GalleryFacet.DATE -> {
                SheetHeader("Date", showClear = filter.range != GalleryRange.ALL) {
                    onChange(filter.copy(range = GalleryRange.ALL)); onDismiss()
                }
                OptionList {
                    items(GalleryRange.entries) { r ->
                        ChoiceRow(r.label, null, filter.range == r) { onChange(filter.copy(range = r)); onDismiss() }
                    }
                }
            }
            GalleryFacet.ALBUM -> {
                SheetHeader("Album", showClear = filter.album != null) { onChange(filter.copy(album = null)); onDismiss() }
                OptionList {
                    item { ChoiceRow("All photos", photos.size, filter.album == null) { onChange(filter.copy(album = null)); onDismiss() } }
                    items(folders, key = { "album-${it.name}" }) { f ->
                        val label = if (f.name.isEmpty()) "No album" else f.displayName
                        val picked = filter.album != null && filter.album.equals(f.name, ignoreCase = true)
                        ChoiceRow(label, f.count, picked) { onChange(filter.copy(album = f.name)); onDismiss() }
                    }
                    item { NewAlbumRow { onDismiss(); onNewAlbum() } }
                }
            }
            GalleryFacet.MUSCLE -> {
                val counts = remember(photos) { photos.flatMap { it.muscles }.groupingBy { it }.eachCount() }
                SheetHeader("Muscle", showClear = filter.muscles.isNotEmpty()) { onChange(filter.copy(muscles = emptySet())) }
                OptionList(Modifier.weight(1f, fill = false)) {
                    items(MuscleGroup.entries) { m ->
                        CheckRow(m.displayName, counts[m.code] ?: 0, m.code in filter.muscles) {
                            onChange(filter.withMuscleToggled(m.code))
                        }
                    }
                }
                DoneButton(onDismiss)
            }
            GalleryFacet.TAG -> {
                val counts = remember(photos) { photos.flatMap { it.tags }.groupingBy { it }.eachCount() }
                SheetHeader("Tag", showClear = filter.tags.isNotEmpty()) { onChange(filter.copy(tags = emptySet())) }
                if (knownTags.isEmpty()) {
                    Text(
                        "Add a tag to a photo from its details and it shows up here.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 24.dp, vertical = 12.dp)
                    )
                } else {
                    OptionList(Modifier.weight(1f, fill = false)) {
                        items(knownTags) { t ->
                            CheckRow(PhotoTag.display(t), counts[t] ?: 0, t in filter.tags) { onChange(filter.withTagToggled(t)) }
                        }
                    }
                }
                DoneButton(onDismiss)
            }
        }
    }
}

/** Title on the left, a Clear on the right when the facet is narrowing the grid. */
@Composable
private fun SheetHeader(title: String, showClear: Boolean, onClear: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 48.dp).padding(start = 24.dp, end = 12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(title, style = MaterialTheme.typography.titleLarge, modifier = Modifier.weight(1f))
        if (showClear) TextButton(onClick = onClear) { Text("Clear") }
    }
}

@Composable
private fun OptionList(
    modifier: Modifier = Modifier,
    content: androidx.compose.foundation.lazy.LazyListScope.() -> Unit
) {
    LazyColumn(modifier.fillMaxWidth(), contentPadding = PaddingValues(bottom = 8.dp), content = content)
}

/** A single-choice row: radio, label, and the photo count it would show. */
@Composable
internal fun ChoiceRow(label: String, count: Int?, selected: Boolean, onClick: () -> Unit) {
    ListItem(
        headlineContent = { Text(label) },
        leadingContent = { RadioButton(selected = selected, onClick = null) },
        trailingContent = count?.let { { CountText(it) } },
        colors = ListItemDefaults.colors(containerColor = Color.Transparent),
        modifier = Modifier.selectable(selected = selected, role = Role.RadioButton, onClick = onClick)
            .padding(horizontal = 8.dp)
    )
}

/** A multi-choice row: checkbox, label, count. */
@Composable
private fun CheckRow(label: String, count: Int, checked: Boolean, onToggle: () -> Unit) {
    ListItem(
        headlineContent = { Text(label) },
        leadingContent = { Checkbox(checked = checked, onCheckedChange = null) },
        trailingContent = { CountText(count) },
        colors = ListItemDefaults.colors(containerColor = Color.Transparent),
        modifier = Modifier.toggleable(value = checked, role = Role.Checkbox, onValueChange = { onToggle() })
            .padding(horizontal = 8.dp)
    )
}

@Composable
private fun CountText(count: Int) {
    Text("$count", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

/** The row that starts a new album from inside an album list. */
@Composable
internal fun NewAlbumRow(onClick: () -> Unit) {
    ListItem(
        headlineContent = { Text("New album") },
        leadingContent = { Icon(Icons.Filled.Add, contentDescription = null, Modifier.padding(horizontal = 12.dp)) },
        colors = ListItemDefaults.colors(containerColor = Color.Transparent),
        modifier = Modifier.selectable(selected = false, role = Role.Button, onClick = onClick).padding(horizontal = 8.dp)
    )
}

@Composable
private fun DoneButton(onClick: () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 12.dp)) {
        Button(onClick = onClick, modifier = Modifier.fillMaxWidth()) { Text("Done") }
    }
}
