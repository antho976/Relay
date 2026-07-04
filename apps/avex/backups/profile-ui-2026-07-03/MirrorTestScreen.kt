@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package com.forge.app.ui.profile

import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.CompareArrows
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Tune
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.theme.LocalForgeSettings
import java.time.ZoneId

/** A frozen snapshot of the list the viewer pages through, plus the index it opened on. */
private data class ViewerTarget(val photos: List<ProgressPhoto>, val index: Int)

/**
 * The photo "Gallery" (reached from the Profile teaser's "view all"). Photos-first: the default level
 * is a chronological, month-grouped grid with a search bar, time-range filters, sort + density controls
 * and a compare mode. Albums are optional and tucked behind an "Albums →" entry. Tapping a photo opens
 * a swipeable full-screen viewer. Private, app-private files only (see [MirrorTestViewModel] /
 * [com.forge.app.data.repo.ProgressPhotoRepository]).
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
    var query by remember { mutableStateOf("") }
    var sort by remember { mutableStateOf(GallerySort.NEWEST) }
    var columns by remember { mutableStateOf(3) }
    var compareMode by remember { mutableStateOf(false) }
    val compareSel = remember { mutableStateListOf<ProgressPhoto>() }
    var compareSheet by remember { mutableStateOf(false) }
    var viewer by remember { mutableStateOf<ViewerTarget?>(null) }
    var newAlbumOpen by remember { mutableStateOf(false) }
    var renaming by remember { mutableStateOf<String?>(null) }
    // Search + filters are hidden by default and revealed from the top bar, so the resting gallery is
    // just the stat line and the photos. Opening one closes the other to keep it uncluttered.
    var searchOpen by remember { mutableStateOf(false) }
    var filtersOpen by remember { mutableStateOf(false) }
    val searchFocus = remember { FocusRequester() }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    val zone = remember { ZoneId.systemDefault() }
    val firstDayMonday = LocalForgeSettings.current.firstDayMonday
    val searching = query.isNotBlank()

    val picker = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        // Adds into the open album, or Unsorted ("") from the photo grid.
        uri?.let { viewModel.addPhoto(it, if (showAlbums) (openAlbum ?: "") else "") }
    }
    fun pickPhoto() = picker.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))

    // The photos shown on the main gallery level: search OR range filtered, then sorted.
    val galleryPhotos = remember(state.photos, query, range, sort, firstDayMonday) {
        val base = if (query.isNotBlank()) state.photos.filter { photoMatchesQuery(it, query, zone) }
        else state.photos.filter { galleryRangeMatches(it.takenAtMs, range, zone, firstDayMonday) }
        when (sort) {
            GallerySort.NEWEST -> base.sortedByDescending { it.takenAtMs }
            GallerySort.OLDEST -> base.sortedBy { it.takenAtMs }
        }
    }

    fun toggleCompare(photo: ProgressPhoto) {
        val idx = compareSel.indexOfFirst { it.fileName == photo.fileName }
        if (idx >= 0) compareSel.removeAt(idx)
        else {
            if (compareSel.size >= 2) compareSel.removeAt(0)
            compareSel.add(photo)
        }
    }
    fun selectionIndexOf(photo: ProgressPhoto): Int? =
        compareSel.indexOfFirst { it.fileName == photo.fileName }.takeIf { it >= 0 }

    fun exitCompare() { compareMode = false; compareSheet = false; compareSel.clear() }
    fun openViewer(list: List<ProgressPhoto>, photo: ProgressPhoto) {
        viewer = ViewerTarget(list, list.indexOfFirst { it.fileName == photo.fileName }.coerceAtLeast(0))
    }

    // Search/filter reveal helpers. Closing search clears the query so the grid falls back to the
    // range filter — "searching" (query non-blank) can only happen while the field is visible.
    fun openSearch() { searchOpen = true; filtersOpen = false }
    fun closeSearch() { searchOpen = false; query = "" }
    fun toggleFilters() { filtersOpen = !filtersOpen; if (filtersOpen) closeSearch() }
    LaunchedEffect(searchOpen) { if (searchOpen) runCatching { searchFocus.requestFocus() } }

    // Back drills out: compare → grid; album → folder grid → photo grid → leave the screen.
    fun goBack() {
        when {
            compareMode -> exitCompare()
            openAlbum != null -> openAlbum = null
            showAlbums -> showAlbums = false
            else -> onBack()
        }
    }
    BackHandler(enabled = compareMode || showAlbums) { goBack() }

    val openName = openAlbum
    val title = when {
        compareMode -> "Compare."
        !showAlbums -> "Gallery."
        openName == null -> "Albums."
        openName.isBlank() -> "Unsorted."
        else -> "$openName."
    }
    val showCompareToggle = !showAlbums && !compareMode && state.photos.size >= 2
    val showAdd = !compareMode && (!showAlbums || openName != null)
    // The search + filter toggles only make sense on the main photo grid with something to filter.
    val showQueryTools = !showAlbums && !compareMode && state.photos.isNotEmpty()
    val filtersActive = range != GalleryRange.ALL || sort != GallerySort.NEWEST || columns != 3

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(title, style = MaterialTheme.typography.headlineMedium) },
                navigationIcon = {
                    IconButton(onClick = { goBack() }) {
                        Icon(
                            if (compareMode) Icons.Filled.Close else Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back"
                        )
                    }
                },
                actions = {
                    if (showQueryTools) {
                        IconButton(onClick = { if (searchOpen) closeSearch() else openSearch() }) {
                            Icon(Icons.Filled.Search, contentDescription = "Search photos", tint = if (searchOpen) accent else muted)
                        }
                        IconButton(onClick = { toggleFilters() }) {
                            Icon(Icons.Filled.Tune, contentDescription = "Filter & sort", tint = if (filtersOpen || filtersActive) accent else muted)
                        }
                    }
                    if (showCompareToggle) IconButton(onClick = { compareMode = true }) {
                        Icon(Icons.AutoMirrored.Filled.CompareArrows, contentDescription = "Compare photos", tint = accent)
                    }
                    if (showAdd) IconButton(onClick = { pickPhoto() }) {
                        Icon(Icons.Filled.Add, contentDescription = "Add photo", tint = accent)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        bottomBar = {
            if (compareMode) CompareBar(
                selectedCount = compareSel.size,
                onClear = { compareSel.clear() },
                onCompare = { compareSheet = true },
                muted = muted, accent = accent
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
                compareMode -> CompareLevel(
                    photos = state.photos,
                    zone = zone,
                    onToggle = { toggleCompare(it) },
                    selectionIndexOf = { selectionIndexOf(it) },
                    fileFor = viewModel::fileFor,
                    muted = muted, accent = accent
                )
                !showAlbums -> PhotosLevel(
                    allPhotos = state.photos,
                    visiblePhotos = galleryPhotos,
                    searching = searching,
                    query = query,
                    onQueryChange = { query = it },
                    range = range,
                    onRangeChange = { range = it },
                    sort = sort,
                    onToggleSort = { sort = if (sort == GallerySort.NEWEST) GallerySort.OLDEST else GallerySort.NEWEST },
                    columns = columns,
                    onCycleColumns = { columns = GALLERY_DENSITIES[(GALLERY_DENSITIES.indexOf(columns) + 1) % GALLERY_DENSITIES.size] },
                    searchOpen = searchOpen,
                    filtersOpen = filtersOpen,
                    searchFocus = searchFocus,
                    zone = zone,
                    onOpenAlbums = { showAlbums = true },
                    onView = { openViewer(galleryPhotos, it) },
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
                else -> {
                    val albumPhotos = remember(state.photos, openName, sort) {
                        val ps = state.photosIn(openName)
                        if (sort == GallerySort.OLDEST) ps.sortedBy { it.takenAtMs } else ps.sortedByDescending { it.takenAtMs }
                    }
                    AlbumPhotos(
                        photos = albumPhotos,
                        isNamed = openName.isNotBlank(),
                        columns = columns,
                        zone = zone,
                        onView = { openViewer(albumPhotos, it) },
                        onRename = { renaming = openName },
                        onDelete = { viewModel.deleteAlbum(openName); openAlbum = null },
                        fileFor = viewModel::fileFor,
                        onBg = onBg, muted = muted, accent = accent, outline = outline
                    )
                }
            }
            Spacer(Modifier.height(40.dp))
        }
    }

    viewer?.let { target ->
        GalleryViewerPager(
            photos = target.photos,
            startIndex = target.index,
            albumNames = state.albumNames,
            fileFor = viewModel::fileFor,
            onSaveNote = { p, n -> viewModel.setNote(p, n) },
            onMove = { p, a -> viewModel.setAlbum(p, a) },
            onDelete = { p -> viewModel.deletePhoto(p); viewer = null },
            onDismiss = { viewer = null }
        )
    }

    if (compareSheet && compareSel.size == 2) {
        CompareSheet(pair = compareSel.toList(), fileFor = viewModel::fileFor, onDismiss = { compareSheet = false })
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

/**
 * The default Gallery level. At rest it's just the stat line + "Albums →" + the month-grouped grid.
 * The search field ([searchOpen]) and the range/sort/density panel ([filtersOpen]) are only shown
 * when revealed from the top bar, so the gallery stays photos-first and uncluttered.
 */
@Composable
private fun PhotosLevel(
    allPhotos: List<ProgressPhoto>,
    visiblePhotos: List<ProgressPhoto>,
    searching: Boolean,
    query: String,
    onQueryChange: (String) -> Unit,
    range: GalleryRange,
    onRangeChange: (GalleryRange) -> Unit,
    sort: GallerySort,
    onToggleSort: () -> Unit,
    columns: Int,
    onCycleColumns: () -> Unit,
    searchOpen: Boolean,
    filtersOpen: Boolean,
    searchFocus: FocusRequester,
    zone: ZoneId,
    onOpenAlbums: () -> Unit,
    onView: (ProgressPhoto) -> Unit,
    fileFor: (ProgressPhoto) -> java.io.File,
    onBg: Color, muted: Color, accent: Color, outline: Color
) {
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        if (allPhotos.isEmpty()) {
            Text("Newest first.", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic, fontSize = 11.sp)
        } else {
            GalleryStatsHeader(allPhotos, muted)
        }
        Text(
            "Albums →", style = MaterialTheme.typography.labelMedium, color = accent,
            modifier = Modifier.bounceClick { onOpenAlbums() }.padding(vertical = 4.dp)
        )
    }

    if (allPhotos.isEmpty()) {
        Spacer(Modifier.height(12.dp))
        Text("No photos yet — tap + to add your first.", style = MaterialTheme.typography.bodyMedium, color = muted)
        return
    }

    // Revealed controls: the search field, OR the range/sort/density panel — never both at once.
    when {
        searchOpen -> {
            Spacer(Modifier.height(12.dp))
            GallerySearchBar(query, onQueryChange, focusRequester = searchFocus)
            Spacer(Modifier.height(14.dp))
        }
        filtersOpen -> {
            Spacer(Modifier.height(12.dp))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                GalleryRange.entries.forEach { r -> GalleryChip(r.label, selected = r == range) { onRangeChange(r) } }
            }
            Spacer(Modifier.height(12.dp))
            GalleryControlsRow(sort, onToggleSort, columns, onCycleColumns, muted, outline)
            Spacer(Modifier.height(16.dp))
        }
        else -> Spacer(Modifier.height(16.dp))
    }

    if (visiblePhotos.isEmpty()) {
        Text(
            if (searching) "No photos match “$query”." else "No photos in this range.",
            style = MaterialTheme.typography.bodyMedium, color = muted
        )
        return
    }
    if (searching) {
        Text(
            "${visiblePhotos.size} result${if (visiblePhotos.size == 1) "" else "s"}",
            style = MaterialTheme.typography.labelMedium, color = muted
        )
        Spacer(Modifier.height(12.dp))
    }
    MonthGroupedGrid(visiblePhotos, columns, zone, fileFor, muted, accent, onView)
}

/** Compare mode: an instruction line then a selectable grid of every photo (newest first). */
@Composable
private fun CompareLevel(
    photos: List<ProgressPhoto>,
    zone: ZoneId,
    onToggle: (ProgressPhoto) -> Unit,
    selectionIndexOf: (ProgressPhoto) -> Int?,
    fileFor: (ProgressPhoto) -> java.io.File,
    muted: Color, accent: Color
) {
    Text(
        "Pick two photos to see them side by side.",
        style = MaterialTheme.typography.bodyMedium, color = muted
    )
    Spacer(Modifier.height(16.dp))
    if (photos.isEmpty()) {
        Text("Add a few photos first.", style = MaterialTheme.typography.bodyMedium, color = muted)
        return
    }
    MonthGroupedGrid(
        photos = photos, columns = 3, zone = zone, fileFor = fileFor, muted = muted, accent = accent,
        onPhotoClick = onToggle, selectable = true, selectionIndexOf = selectionIndexOf
    )
}
