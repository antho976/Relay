@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package com.forge.app.ui.profile

import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
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
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.ui.common.bounceClick
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * The full Mirror-test gallery. Two levels: a grid of album folders, and (on tapping one) that
 * album's photos. Adding, moving between albums, captioning and deleting all happen here; the
 * Profile screen only shows a 3-photo teaser that opens this. Private, app-private files only.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MirrorTestScreen(
    onBack: () -> Unit,
    viewModel: MirrorTestViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    // null = folder grid; non-null (including "" for Unsorted) = that album's photos.
    var openAlbum by remember { mutableStateOf<String?>(null) }
    var viewing by remember { mutableStateOf<ProgressPhoto?>(null) }
    var newAlbumOpen by remember { mutableStateOf(false) }
    var renaming by remember { mutableStateOf<String?>(null) }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    val picker = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        // Adds into the open album, or Unsorted ("") at the folder level.
        uri?.let { viewModel.addPhoto(it, openAlbum ?: "") }
    }
    fun pickPhoto() = picker.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))

    fun goBack() { if (openAlbum != null) openAlbum = null else onBack() }
    BackHandler(enabled = openAlbum != null) { openAlbum = null }

    val openName = openAlbum
    val title = when {
        openName == null -> "Mirror test."
        openName.isBlank() -> "Unsorted."
        else -> "$openName."
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(title, style = MaterialTheme.typography.headlineMedium) },
                navigationIcon = {
                    IconButton(onClick = { goBack() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") }
                },
                actions = {
                    // Adding is only meaningful inside an album view (so the photo lands somewhere).
                    if (openName != null) IconButton(onClick = { pickPhoto() }) {
                        Icon(Icons.Filled.Add, contentDescription = "Add photo", tint = accent)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        Column(
            Modifier.fillMaxSize().padding(inner).verticalScroll(rememberScrollState()).padding(horizontal = 24.dp)
        ) {
            Text(
                "Private — these never leave your phone.",
                style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic, fontSize = 11.sp
            )
            Spacer(Modifier.height(16.dp))

            if (openName == null) {
                FolderLevel(
                    folders = state.folders,
                    onOpen = { openAlbum = it },
                    onNewAlbum = { newAlbumOpen = true },
                    onAddPhoto = { openAlbum = ""; pickPhoto() },
                    fileFor = viewModel::fileFor,
                    onBg = onBg, muted = muted, accent = accent, outline = outline
                )
            } else {
                AlbumLevel(
                    photos = state.photosIn(openName),
                    isNamed = openName.isNotBlank(),
                    onAdd = { pickPhoto() },
                    onView = { viewing = it },
                    onRename = { renaming = openName },
                    onDelete = { viewModel.deleteAlbum(openName); openAlbum = null },
                    fileFor = viewModel::fileFor,
                    onBg = onBg, muted = muted, accent = accent, outline = outline
                )
            }
            Spacer(Modifier.height(40.dp))
        }
    }

    viewing?.let { photo ->
        PhotoViewerDialog(
            photo = photo,
            file = viewModel.fileFor(photo),
            albumNames = state.albumNames,
            onSaveNote = { viewModel.setNote(photo, it) },
            onMove = { album -> viewModel.setAlbum(photo, album); viewing = null },
            onDelete = { viewModel.deletePhoto(photo); viewing = null },
            onDismiss = { viewing = null }
        )
    }

    if (newAlbumOpen) {
        NameDialog(
            title = "New album",
            initial = "",
            onConfirm = { name ->
                viewModel.createAlbum(name)
                newAlbumOpen = false
                // Open the canonical folder: reuse an existing album that matches case-insensitively
                // (createAlbum no-ops on a dup), else open the new name.
                val typed = name.trim()
                openAlbum = state.albumNames.firstOrNull { it.equals(typed, ignoreCase = true) } ?: typed
            },
            onDismiss = { newAlbumOpen = false }
        )
    }

    renaming?.let { old ->
        NameDialog(
            title = "Rename album",
            initial = old,
            onConfirm = { viewModel.renameAlbum(old, it); openAlbum = it; renaming = null },
            onDismiss = { renaming = null }
        )
    }
}

@Composable
private fun FolderLevel(
    folders: List<MirrorTestViewModel.AlbumFolder>,
    onOpen: (String) -> Unit,
    onNewAlbum: () -> Unit,
    onAddPhoto: () -> Unit,
    fileFor: (ProgressPhoto) -> java.io.File,
    onBg: Color, muted: Color, accent: Color, outline: Color
) {
    Row(horizontalArrangement = Arrangement.spacedBy(20.dp)) {
        Text("+ New album", style = MaterialTheme.typography.labelLarge, color = accent,
            modifier = Modifier.bounceClick { onNewAlbum() }.padding(vertical = 4.dp))
        Text("+ Add photo", style = MaterialTheme.typography.labelLarge, color = accent,
            modifier = Modifier.bounceClick { onAddPhoto() }.padding(vertical = 4.dp))
    }
    Spacer(Modifier.height(16.dp))

    if (folders.isEmpty()) {
        Text(
            "No photos yet. Add one — or make an album to group them (e.g. Front, Back, a cut).",
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

@Composable
private fun AlbumLevel(
    photos: List<ProgressPhoto>,
    isNamed: Boolean,
    onAdd: () -> Unit,
    onView: (ProgressPhoto) -> Unit,
    onRename: () -> Unit,
    onDelete: () -> Unit,
    fileFor: (ProgressPhoto) -> java.io.File,
    onBg: Color, muted: Color, accent: Color, outline: Color
) {
    // Rename / delete only apply to real albums — Unsorted is the implicit bucket.
    if (isNamed) {
        Row(horizontalArrangement = Arrangement.spacedBy(20.dp)) {
            Text("Rename", style = MaterialTheme.typography.labelMedium, color = accent,
                modifier = Modifier.bounceClick { onRename() }.padding(vertical = 4.dp))
            Text("Delete album", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.error,
                modifier = Modifier.bounceClick { onDelete() }.padding(vertical = 4.dp))
        }
        Spacer(Modifier.height(14.dp))
    }

    if (photos.isEmpty()) {
        Text("No photos in this album yet — tap + to add one.", style = MaterialTheme.typography.bodyMedium, color = muted)
        return
    }
    photos.chunked(3).forEach { row ->
        Row(Modifier.fillMaxWidth().padding(bottom = 6.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            row.forEach { photo ->
                Column(Modifier.weight(1f)) {
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
            repeat(3 - row.size) { Box(Modifier.weight(1f)) }
        }
    }
}

@Composable
private fun PhotoViewerDialog(
    photo: ProgressPhoto,
    file: java.io.File,
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

    androidx.compose.ui.window.Dialog(onDismissRequest = { commitNote(); onDismiss() }) {
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
                AlbumChip("Unsorted", selected = photo.album.isBlank()) { commitNote(); onMove("") }
                albumNames.forEach { name ->
                    AlbumChip(name, selected = photo.album == name) { commitNote(); onMove(name) }
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

@Composable
private fun AlbumChip(label: String, selected: Boolean, onClick: () -> Unit) {
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

@Composable
private fun NameDialog(
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
