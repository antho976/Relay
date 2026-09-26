@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.profile

import android.net.Uri
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Sort
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.outlined.AddPhotoAlternate
import androidx.compose.material.icons.outlined.CheckCircle
import androidx.compose.material.icons.outlined.Compare
import androidx.compose.material.icons.outlined.CreateNewFolder
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.DriveFileRenameOutline
import androidx.compose.material.icons.outlined.Folder
import androidx.compose.material.icons.outlined.FolderOff
import androidx.compose.material.icons.outlined.PhotoAlbum
import androidx.compose.material.icons.outlined.PhotoCamera
import androidx.compose.material.icons.outlined.SelectAll
import androidx.compose.material.icons.outlined.ZoomIn
import androidx.compose.material.icons.outlined.ZoomOut
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.ui.common.window.AlertDialog
import com.forge.app.ui.common.window.DropdownMenu
import com.forge.app.ui.common.window.ModalBottomSheet
import com.forge.app.ui.theme.LocalForgeSettings
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.time.LocalDate
import java.time.ZoneId

/** A frozen snapshot of the list the viewer pages through, plus the index it opened on. */
private data class ViewerTarget(val photos: List<ProgressPhoto>, val index: Int)

/**
 * The photo **Gallery** (reached from the Profile teaser's "view all").
 *
 * Built for getting to a photo, and to a comparison, in as few taps as possible:
 *
 * - The grid starts right under one row of filter chips. Nothing stands between the top bar and
 *   the photos except the compare shortcuts, which are one short row.
 * - Search, import and everything else live in the top bar; the one primary action, taking a
 *   photo, is the floating button.
 * - Long-press starts a multi-selection, the way every Android gallery works: pick two to compare,
 *   or any number to file into an album or delete.
 * - Pinch the grid to change how many photos fit per row.
 *
 * Albums are a filter, not a separate screen: the Album chip narrows this same grid, so search and
 * the other filters keep working inside an album. Private, app-private files only (see
 * [MirrorTestViewModel] / [com.forge.app.data.repo.ProgressPhotoRepository]).
 */
@Composable
fun MirrorTestScreen(
    onBack: () -> Unit,
    onOpenCamera: () -> Unit = {},
    viewModel: MirrorTestViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val actions = remember(viewModel) { GalleryActions.of(viewModel) }
    GalleryScreen(state = state, actions = actions, onBack = onBack, onOpenCamera = onOpenCamera)
}

/** Everything the gallery asks of its ViewModel, so [GalleryScreen] can be rendered from plain data. */
@Stable
internal class GalleryActions(
    val fileFor: (ProgressPhoto) -> File,
    val addPhotos: (uris: List<Uri>, album: String, pose: String, muscles: List<String>) -> Unit,
    val createAlbum: (name: String, moving: List<ProgressPhoto>) -> Unit,
    val renameAlbum: (old: String, new: String) -> Unit,
    val deleteAlbum: (String) -> Unit,
    val moveToAlbum: (List<ProgressPhoto>, String) -> Unit,
    val deletePhotos: (List<ProgressPhoto>) -> Unit,
    val setNote: (ProgressPhoto, String) -> Unit,
    val setTitle: (ProgressPhoto, String) -> Unit,
    val setAlbum: (ProgressPhoto, String) -> Unit,
    val setPose: (ProgressPhoto, String) -> Unit,
    val setMuscles: (ProgressPhoto, List<String>) -> Unit,
    val setTags: (ProgressPhoto, List<String>) -> Unit,
    val setWeight: (ProgressPhoto, Double?) -> Unit,
    val setTakenAt: (ProgressPhoto, Long) -> Unit
) {
    companion object {
        fun of(vm: MirrorTestViewModel) = GalleryActions(
            fileFor = vm::fileFor,
            addPhotos = { u, a, p, m -> vm.addPhotos(u, a, p, m) },
            createAlbum = { n, m -> vm.createAlbum(n, m) },
            renameAlbum = { o, n -> vm.renameAlbum(o, n) },
            deleteAlbum = { vm.deleteAlbum(it) },
            moveToAlbum = { ps, a -> vm.moveToAlbum(ps, a) },
            deletePhotos = { vm.deletePhotos(it) },
            setNote = { p, n -> vm.setNote(p, n) },
            setTitle = { p, t -> vm.setTitle(p, t) },
            setAlbum = { p, a -> vm.setAlbum(p, a) },
            setPose = { p, x -> vm.setPose(p, x) },
            setMuscles = { p, m -> vm.setMuscles(p, m) },
            setTags = { p, t -> vm.setTags(p, t) },
            setWeight = { p, w -> vm.setWeight(p, w) },
            setTakenAt = { p, ms -> vm.setTakenAt(p, ms) }
        )
    }
}

@Composable
internal fun GalleryScreen(
    state: MirrorTestViewModel.UiState,
    actions: GalleryActions,
    onBack: () -> Unit,
    onOpenCamera: () -> Unit
) = GalleryTheme {
    var filter by remember { mutableStateOf(GalleryFilter()) }
    var searchOpen by remember { mutableStateOf(false) }
    var selecting by remember { mutableStateOf(false) }
    var selected by remember { mutableStateOf(emptySet<String>()) }
    var openFacet by remember { mutableStateOf<GalleryFacet?>(null) }
    var viewer by remember { mutableStateOf<ViewerTarget?>(null) }
    var comparePair by remember { mutableStateOf<Pair<ProgressPhoto, ProgressPhoto>?>(null) }
    var confirmDelete by remember { mutableStateOf(false) }
    var moveSheetOpen by remember { mutableStateOf(false) }
    // Non-null while the New album dialog is up: the photos it will file into the new album.
    var newAlbumFor by remember { mutableStateOf<List<ProgressPhoto>?>(null) }
    var renamingAlbum by remember { mutableStateOf<String?>(null) }
    var deletingAlbum by remember { mutableStateOf<String?>(null) }
    val listState = rememberLazyListState()
    val fontScale = LocalDensity.current.fontScale

    // Not remembered (M-15): a remembered zone survives a flight and keeps bucketing photos by the
    // old zone's day boundaries. `today` below is keyed on it, so a changed zone re-derives the days.
    val zone = ZoneId.systemDefault()
    val settings = LocalForgeSettings.current
    val weightUnit = settings.weightUnit

    val visible = remember(state.photos, filter, settings.firstDayMonday) {
        applyGalleryFilter(state.photos, filter, zone, settings.firstDayMonday)
    }
    val today = remember(zone) { LocalDate.now(zone) }
    val sections = remember(visible, today) { gallerySections(visible, zone, today) }
    val (first, latest) = remember(visible) { bestComparePair(visible) }
    // Pairing is O(n^2) over weighed photos, so it runs off the main thread; empty until it lands.
    val samePairs by produceState(emptyList<SameWeightPair>(), visible, first, latest) {
        value = withContext(Dispatchers.Default) {
            sameWeightPairs(visible, zone, setOfNotNull(first?.fileName, latest?.fileName))
        }
    }
    val suggestions = remember(first, latest, samePairs) { compareSuggestions(first, latest, samePairs) }
    val selectedPhotos = remember(state.photos, selected) { state.photos.filter { it.fileName in selected } }
    val libraryEmpty = !state.loading && state.photos.isEmpty()
    val namedAlbum = filter.album?.takeIf { it.isNotEmpty() }

    fun exitSelection() { selecting = false; selected = emptySet() }
    fun toggle(photo: ProgressPhoto) {
        selected = if (photo.fileName in selected) selected - photo.fileName else selected + photo.fileName
    }
    fun closeSearch() { searchOpen = false; filter = filter.copy(query = "") }

    // Keep every piece of state honest as the library changes under it.
    LaunchedEffect(state.photos) {
        val live = state.photos.mapTo(HashSet()) { it.fileName }
        if (!selected.all { it in live }) selected = selected.filterTo(HashSet()) { it in live }
    }
    // A tag or album can disappear with its last photo; a filter on it would strand the grid.
    LaunchedEffect(state.knownTags) {
        val kept = filter.tags intersect state.knownTags.toSet()
        if (kept != filter.tags) filter = filter.copy(tags = kept)
    }
    LaunchedEffect(state.folders, state.loading) {
        val album = filter.album ?: return@LaunchedEffect
        if (!state.loading && album.isNotEmpty() && state.folders.none { it.name.equals(album, ignoreCase = true) }) {
            filter = filter.copy(album = null)
        }
    }

    BackHandler(enabled = selecting || searchOpen) {
        if (selecting) exitSelection() else closeSearch()
    }

    // Imports inherit what you were looking at: the album, the pose and the muscles filtered to.
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.PickMultipleVisualMedia()) { uris ->
        if (uris.isNotEmpty()) {
            actions.addPhotos(uris, namedAlbum ?: "", filter.pose?.name ?: "", filter.muscles.toList())
        }
    }
    fun importPhotos() = picker.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))

    val onPinch by rememberUpdatedState<(Boolean) -> Unit> { zoomIn -> filter = filter.stepColumns(denser = !zoomIn) }
    val fabExpanded by remember { derivedStateOf { listState.firstVisibleItemIndex == 0 } }

    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            Column {
                when {
                    selecting -> SelectionTopBar(
                        count = selected.size,
                        onClose = { exitSelection() },
                        onSelectAll = { selected = visible.mapTo(HashSet()) { it.fileName } },
                        onCompare = {
                            val two = selectedPhotos.sortedBy { it.takenAtMs }
                            if (two.size == 2) comparePair = two[0] to two[1]
                        },
                        onMove = { moveSheetOpen = true },
                        onDelete = { confirmDelete = true }
                    )
                    searchOpen -> SearchTopBar(
                        query = filter.query,
                        onQueryChange = { filter = filter.copy(query = it) },
                        onClose = { closeSearch() }
                    )
                    else -> LibraryTopBar(
                        loading = state.loading,
                        total = state.photos.size,
                        shown = visible.size,
                        narrowed = filter.narrowed,
                        filter = filter,
                        namedAlbum = namedAlbum,
                        libraryEmpty = libraryEmpty,
                        onBack = onBack,
                        onSearch = { searchOpen = true },
                        onImport = { importPhotos() },
                        onSelect = { selecting = true },
                        onFilterChange = { filter = it },
                        onNewAlbum = { newAlbumFor = emptyList() },
                        onRenameAlbum = { renamingAlbum = it },
                        onDeleteAlbum = { deletingAlbum = it }
                    )
                }
                if (!selecting && !libraryEmpty && !state.loading) {
                    GalleryFilterRow(
                        filter = filter,
                        hasAlbums = state.folders.any { it.name.isNotEmpty() },
                        hasTags = state.knownTags.isNotEmpty(),
                        onOpen = { openFacet = it },
                        onClearAll = { filter = filter.clearedFacets() },
                        modifier = Modifier.padding(bottom = 4.dp)
                    )
                }
            }
        },
        floatingActionButton = {
            if (!selecting && !libraryEmpty && !state.loading) {
                ExtendedFloatingActionButton(
                    onClick = onOpenCamera,
                    expanded = fabExpanded,
                    icon = { Icon(Icons.Outlined.PhotoCamera, contentDescription = null) },
                    text = { Text("Take photo") }
                )
            }
        }
    ) { inner ->
        LazyColumn(
            state = listState,
            contentPadding = PaddingValues(top = inner.calculateTopPadding(), bottom = inner.calculateBottomPadding() + 96.dp),
            modifier = Modifier.fillMaxSize().pointerInput(Unit) {
                // Two-finger pinch steps the grid density once per gesture. Watched on the Initial
                // pass and consumed only while two fingers are down, so one-finger scrolling and
                // taps reach the list untouched.
                awaitEachGesture {
                    awaitFirstDown(requireUnconsumed = false)
                    var zoom = 1f
                    var fired = false
                    do {
                        val event = awaitPointerEvent(PointerEventPass.Initial)
                        if (event.changes.count { it.pressed } >= 2) {
                            zoom *= event.calculateZoom()
                            if (!fired && (zoom > 1.25f || zoom < 0.8f)) {
                                onPinch(zoom > 1f)
                                fired = true
                            }
                            event.changes.forEach { it.consume() }
                        }
                    } while (event.changes.any { it.pressed })
                }
            }
        ) {
            galleryLibrary(
                loading = state.loading,
                libraryEmpty = libraryEmpty,
                filter = filter,
                sections = sections,
                suggestions = suggestions,
                grid = GalleryGridSpec(
                    columns = filter.columns,
                    fileFor = actions.fileFor,
                    onPhotoClick = { p ->
                        if (selecting) toggle(p)
                        else viewer = ViewerTarget(visible, visible.indexOfFirst { it.fileName == p.fileName }.coerceAtLeast(0))
                    },
                    onPhotoLongClick = { p -> selecting = true; selected = selected + p.fileName },
                    selecting = selecting,
                    isSelected = { it.fileName in selected },
                    onToggleSection = { section ->
                        val names = section.photos.map { it.fileName }
                        selected = if (names.all { it in selected }) selected - names.toSet() else selected + names
                    },
                    showDates = filter.columns < 5 && fontScale <= 1.5f
                ),
                zone = zone,
                weightUnit = weightUnit,
                fileFor = actions.fileFor,
                onCompare = { a, b -> comparePair = a to b },
                onTakePhoto = onOpenCamera,
                onImport = { importPhotos() },
                onClearFilters = { filter = filter.clearedFacets().copy(query = ""); searchOpen = false }
            )
        }
    }

    // A selection that empties (the last pick removed, or deleted elsewhere) stays open on purpose:
    // the bar says "0 selected" and Close is right there, rather than the mode vanishing mid-gesture.

    openFacet?.let { facet ->
        GalleryFacetSheet(
            facet = facet,
            filter = filter,
            photos = state.photos,
            folders = state.folders,
            knownTags = state.knownTags,
            onChange = { filter = it },
            onNewAlbum = { newAlbumFor = emptyList() },
            onDismiss = { openFacet = null }
        )
    }

    viewer?.let { target ->
        GalleryViewerPager(
            photos = target.photos,
            startIndex = target.index,
            albumNames = state.albumNames,
            knownTags = state.knownTags,
            weightUnit = weightUnit,
            fileFor = actions.fileFor,
            onSaveNote = actions.setNote,
            onSaveTitle = actions.setTitle,
            onMove = actions.setAlbum,
            onSetPose = actions.setPose,
            onSetMuscles = actions.setMuscles,
            onSetTags = actions.setTags,
            onSetWeight = actions.setWeight,
            onSetDate = actions.setTakenAt,
            onDelete = { p -> actions.deletePhotos(listOf(p)); viewer = null },
            onDismiss = { viewer = null }
        )
    }

    comparePair?.let { (a, b) ->
        CompareSheet(
            pair = listOf(a, b), zone = zone, weightUnit = weightUnit, fileFor = actions.fileFor,
            onDismiss = { comparePair = null }
        )
    }

    if (confirmDelete) {
        DeletePhotosDialog(
            count = selectedPhotos.size,
            onConfirm = { actions.deletePhotos(selectedPhotos); confirmDelete = false; exitSelection() },
            onDismiss = { confirmDelete = false }
        )
    }

    if (moveSheetOpen) {
        MoveToAlbumSheet(
            folders = state.folders,
            onPick = { album -> actions.moveToAlbum(selectedPhotos, album); moveSheetOpen = false; exitSelection() },
            onNewAlbum = { moveSheetOpen = false; newAlbumFor = selectedPhotos },
            onDismiss = { moveSheetOpen = false }
        )
    }

    newAlbumFor?.let { moving ->
        NameDialog(
            title = "New album",
            initial = "",
            confirmLabel = "Create",
            onConfirm = { name ->
                actions.createAlbum(name, moving)
                newAlbumFor = null
                if (moving.isNotEmpty()) exitSelection()
                // Land in the album just made, so the next import or move has somewhere to go.
                filter = filter.clearedFacets().copy(album = name.trim())
            },
            onDismiss = { newAlbumFor = null }
        )
    }

    renamingAlbum?.let { old ->
        NameDialog(
            title = "Rename album",
            initial = old,
            confirmLabel = "Rename",
            onConfirm = { new ->
                actions.renameAlbum(old, new)
                filter = filter.copy(album = new.trim())
                renamingAlbum = null
            },
            onDismiss = { renamingAlbum = null }
        )
    }

    deletingAlbum?.let { name ->
        AlertDialog(
            onDismissRequest = { deletingAlbum = null },
            title = { DialogTitle("Delete “$name”?") },
            text = {
                Text("Its photos stay in the gallery, just without an album.", style = MaterialTheme.typography.bodyMedium)
            },
            confirmButton = {
                TextButton(onClick = {
                    actions.deleteAlbum(name)
                    filter = filter.copy(album = null)
                    deletingAlbum = null
                }) { Text("Delete album") }
            },
            dismissButton = { TextButton(onClick = { deletingAlbum = null }) { Text("Cancel") } }
        )
    }
}

// ── Top bars ─────────────────────────────────────────────────────────────────

@Composable
private fun galleryBarColors() = TopAppBarDefaults.topAppBarColors(
    containerColor = MaterialTheme.colorScheme.background,
    scrolledContainerColor = MaterialTheme.colorScheme.background
)

/** The resting bar: back, the screen's name over its count, then search, import and the menu. */
@Composable
private fun LibraryTopBar(
    loading: Boolean,
    total: Int,
    shown: Int,
    narrowed: Boolean,
    filter: GalleryFilter,
    namedAlbum: String?,
    libraryEmpty: Boolean,
    onBack: () -> Unit,
    onSearch: () -> Unit,
    onImport: () -> Unit,
    onSelect: () -> Unit,
    onFilterChange: (GalleryFilter) -> Unit,
    onNewAlbum: () -> Unit,
    onRenameAlbum: (String) -> Unit,
    onDeleteAlbum: (String) -> Unit
) {
    var menuOpen by remember { mutableStateOf(false) }
    TopAppBar(
        title = {
            GalleryBarTitle(
                title = "Gallery",
                subtitle = when {
                    loading -> null
                    narrowed -> "$shown of ${photoCountLabel(total)}"
                    else -> photoCountLabel(total)
                }
            )
        },
        navigationIcon = {
            IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back") }
        },
        actions = {
            if (!libraryEmpty && !loading) {
                IconButton(onClick = onSearch) { Icon(Icons.Filled.Search, contentDescription = "Search photos") }
            }
            IconButton(onClick = onImport) {
                Icon(Icons.Outlined.AddPhotoAlternate, contentDescription = "Import from phone")
            }
            Box {
                IconButton(onClick = { menuOpen = true }) { Icon(Icons.Filled.MoreVert, contentDescription = "More options") }
                DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                    fun pick(action: () -> Unit) { menuOpen = false; action() }
                    if (!libraryEmpty) {
                        MenuItem("Select photos", Icons.Outlined.CheckCircle) { pick(onSelect) }
                        MenuItem(
                            if (filter.sort == GallerySort.NEWEST) "Show oldest first" else "Show newest first",
                            Icons.AutoMirrored.Filled.Sort
                        ) {
                            pick {
                                onFilterChange(
                                    filter.copy(sort = if (filter.sort == GallerySort.NEWEST) GallerySort.OLDEST else GallerySort.NEWEST)
                                )
                            }
                        }
                        MenuItem("Larger thumbnails", Icons.Outlined.ZoomIn, enabled = filter.columns > GALLERY_DENSITIES.first()) {
                            pick { onFilterChange(filter.stepColumns(denser = false)) }
                        }
                        MenuItem("Smaller thumbnails", Icons.Outlined.ZoomOut, enabled = filter.columns < GALLERY_DENSITIES.last()) {
                            pick { onFilterChange(filter.stepColumns(denser = true)) }
                        }
                    }
                    MenuItem("New album", Icons.Outlined.CreateNewFolder) { pick(onNewAlbum) }
                    if (namedAlbum != null) {
                        MenuItem("Rename album", Icons.Outlined.DriveFileRenameOutline) { pick { onRenameAlbum(namedAlbum) } }
                        MenuItem("Delete album", Icons.Outlined.FolderOff) { pick { onDeleteAlbum(namedAlbum) } }
                    }
                }
            }
        },
        colors = galleryBarColors()
    )
}

@Composable
private fun MenuItem(label: String, icon: androidx.compose.ui.graphics.vector.ImageVector, enabled: Boolean = true, onClick: () -> Unit) {
    DropdownMenuItem(
        text = { Text(label) },
        leadingIcon = { Icon(icon, contentDescription = null) },
        enabled = enabled,
        onClick = onClick
    )
}

/**
 * The bar while photos are being picked: the count, and what the selection can do. Compare asks for
 * exactly two, and the bar says so while one is picked instead of leaving a dead button to decode.
 */
@Composable
private fun SelectionTopBar(
    count: Int,
    onClose: () -> Unit,
    onSelectAll: () -> Unit,
    onCompare: () -> Unit,
    onMove: () -> Unit,
    onDelete: () -> Unit
) {
    TopAppBar(
        title = {
            GalleryBarTitle(
                title = "$count selected",
                subtitle = when (count) {
                    0 -> "Tap photos to select them"
                    1 -> "Pick one more to compare"
                    else -> null
                }
            )
        },
        navigationIcon = {
            IconButton(onClick = onClose) { Icon(Icons.Filled.Close, contentDescription = "Cancel selection") }
        },
        actions = {
            IconButton(onClick = onSelectAll) { Icon(Icons.Outlined.SelectAll, contentDescription = "Select all shown") }
            IconButton(onClick = onCompare, enabled = count == 2) {
                Icon(Icons.Outlined.Compare, contentDescription = "Compare the two selected photos")
            }
            IconButton(onClick = onMove, enabled = count > 0) {
                Icon(Icons.Outlined.PhotoAlbum, contentDescription = "Move to album")
            }
            IconButton(onClick = onDelete, enabled = count > 0) {
                Icon(Icons.Outlined.Delete, contentDescription = "Delete selected photos")
            }
        },
        colors = TopAppBarDefaults.topAppBarColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh)
    )
}

/** The bar while searching: the field takes the title's place and opens with the keyboard up. */
@Composable
private fun SearchTopBar(query: String, onQueryChange: (String) -> Unit, onClose: () -> Unit) {
    val focus = remember { FocusRequester() }
    val keyboard = LocalSoftwareKeyboardController.current
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    TopAppBar(
        title = {
            BasicTextField(
                value = query,
                onValueChange = onQueryChange,
                singleLine = true,
                textStyle = MaterialTheme.typography.bodyLarge.copy(color = MaterialTheme.colorScheme.onBackground),
                cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                keyboardActions = KeyboardActions(onSearch = { keyboard?.hide() }),
                decorationBox = { field ->
                    Box(contentAlignment = Alignment.CenterStart) {
                        if (query.isEmpty()) {
                            Text(
                                "Search titles, notes, tags, dates",
                                style = MaterialTheme.typography.bodyLarge,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                        field()
                    }
                },
                modifier = Modifier.fillMaxWidth().focusRequester(focus)
            )
        },
        navigationIcon = {
            IconButton(onClick = onClose) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Close search") }
        },
        actions = {
            if (query.isNotEmpty()) {
                IconButton(onClick = { onQueryChange("") }) { Icon(Icons.Filled.Close, contentDescription = "Clear search") }
            }
        },
        colors = galleryBarColors()
    )
}

/**
 * A bar title with an optional quieter second line. The second line drops at large font scales,
 * where two lines no longer fit the bar's fixed height and the title itself would be clipped.
 */
@Composable
private fun GalleryBarTitle(title: String, subtitle: String?) {
    val roomForTwo = LocalDensity.current.fontScale <= 1.3f
    Column {
        Text(title, style = MaterialTheme.typography.titleLarge)
        if (subtitle != null && roomForTwo) {
            Text(subtitle, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

// ── Move to album ────────────────────────────────────────────────────────────

@Composable
private fun MoveToAlbumSheet(
    folders: List<MirrorTestViewModel.AlbumFolder>,
    onPick: (String) -> Unit,
    onNewAlbum: () -> Unit,
    onDismiss: () -> Unit
) {
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = MaterialTheme.colorScheme.surfaceContainerLow
    ) {
        Text(
            "Move to album",
            style = MaterialTheme.typography.titleLarge,
            modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp).padding(horizontal = 24.dp, vertical = 8.dp)
        )
        LazyColumn(Modifier.fillMaxWidth(), contentPadding = PaddingValues(bottom = 16.dp)) {
            items(folders.filter { it.name.isNotEmpty() }, key = { it.name }) { f ->
                AlbumTargetRow(f.displayName, photoCountLabel(f.count), Icons.Outlined.Folder) { onPick(f.name) }
            }
            item { AlbumTargetRow("No album", "Take them out of their album", Icons.Outlined.FolderOff) { onPick("") } }
            item { NewAlbumRow(onNewAlbum) }
        }
    }
}

@Composable
private fun AlbumTargetRow(label: String, supporting: String, icon: androidx.compose.ui.graphics.vector.ImageVector, onClick: () -> Unit) {
    ListItem(
        headlineContent = { Text(label) },
        supportingContent = { Text(supporting) },
        leadingContent = { Icon(icon, contentDescription = null, Modifier.padding(horizontal = 12.dp)) },
        colors = ListItemDefaults.colors(containerColor = Color.Transparent),
        modifier = Modifier.selectable(selected = false, role = Role.Button, onClick = onClick).padding(horizontal = 8.dp)
    )
}
