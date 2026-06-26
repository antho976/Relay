package com.forge.app.ui.profile

import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.repo.ProgressPhoto

/**
 * The photo "Gallery" (reached from the Profile teaser's "view all"). Photos-first: the default level
 * is a chronological, month-grouped grid with time-range filters — first open shows photos + dates,
 * never folders. Albums are optional and tucked behind an "Albums →" entry, where they can be created,
 * renamed and browsed. Adding, captioning, moving between albums and deleting all happen here.
 * Private, app-private files only (see [MirrorTestViewModel] / [com.forge.app.data.repo.ProgressPhotoRepository]).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MirrorTestScreen(
    onBack: () -> Unit,
    viewModel: MirrorTestViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    // Top-level mode: false = the chronological photo grid (default); true = the optional Albums view.
    var showAlbums by remember { mutableStateOf(false) }
    // Within Albums: null = folder grid; non-null (incl "" for Unsorted) = that album's photos.
    var openAlbum by remember { mutableStateOf<String?>(null) }
    var range by remember { mutableStateOf(GalleryRange.ALL) }
    var viewing by remember { mutableStateOf<ProgressPhoto?>(null) }
    var newAlbumOpen by remember { mutableStateOf(false) }
    var renaming by remember { mutableStateOf<String?>(null) }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    val picker = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        // Adds into the open album, or Unsorted ("") from the photo grid.
        uri?.let { viewModel.addPhoto(it, if (showAlbums) (openAlbum ?: "") else "") }
    }
    fun pickPhoto() = picker.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))

    // Back drills out: album → folder grid → photo grid → leave the screen.
    fun goBack() {
        when {
            openAlbum != null -> openAlbum = null
            showAlbums -> showAlbums = false
            else -> onBack()
        }
    }
    // Intercept system back only while inside the Albums view (the photo grid falls through to nav).
    BackHandler(enabled = showAlbums) { if (openAlbum != null) openAlbum = null else showAlbums = false }

    val openName = openAlbum
    val title = when {
        !showAlbums -> "Gallery."
        openName == null -> "Albums."
        openName.isBlank() -> "Unsorted."
        else -> "$openName."
    }
    // The + has a target in the photo grid (→ Unsorted) and inside an album (→ that album); the folder
    // grid itself has no target, so it's hidden there.
    val showAdd = !showAlbums || openName != null

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(title, style = MaterialTheme.typography.headlineMedium) },
                navigationIcon = {
                    IconButton(onClick = { goBack() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") }
                },
                actions = {
                    if (showAdd) IconButton(onClick = { pickPhoto() }) {
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

            when {
                !showAlbums -> PhotosLevel(
                    photos = state.photos,
                    range = range,
                    onRangeChange = { range = it },
                    onOpenAlbums = { showAlbums = true },
                    onView = { viewing = it },
                    fileFor = viewModel::fileFor,
                    onBg = onBg, muted = muted, accent = accent, outline = outline
                )
                openName == null -> FolderGrid(
                    folders = state.folders,
                    onOpen = { openAlbum = it },
                    onNewAlbum = { newAlbumOpen = true },
                    fileFor = viewModel::fileFor,
                    onBg = onBg, muted = muted, accent = accent, outline = outline
                )
                else -> AlbumPhotos(
                    photos = state.photosIn(openName),
                    isNamed = openName.isNotBlank(),
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
        GalleryPhotoViewerDialog(
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
