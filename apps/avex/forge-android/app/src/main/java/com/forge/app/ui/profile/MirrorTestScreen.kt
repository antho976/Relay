@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class, androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.profile

import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.horizontalScroll
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
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.photo.PhotoPose
import com.forge.app.domain.units.WeightUnit
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeWordmark
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.statsEntrance
import com.forge.app.ui.theme.LocalForgeSettings
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.time.ZoneId

/** A frozen snapshot of the list the viewer pages through, plus the index it opened on. */
private data class ViewerTarget(val photos: List<ProgressPhoto>, val index: Int)

/**
 * The photo "Gallery" (reached from the Profile teaser's "view all"). Overview-first: a serif hero,
 * a first↔latest progress band that opens the slider compare, a bodyweight-through-time line, then a
 * TIMELINE section (pose lens pills + search/filters/compare text pills) over a chronological
 * month-grouped grid; "Albums →" rides the TIMELINE header. Sections cascade in via [statsEntrance].
 * Tapping a photo opens a swipeable full-screen viewer + metadata editor. Private, app-private files
 * only (see [MirrorTestViewModel] / [com.forge.app.data.repo.ProgressPhotoRepository]).
 */
@Composable
fun MirrorTestScreen(
    onBack: () -> Unit,
    onOpenCamera: () -> Unit = {},
    viewModel: MirrorTestViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val bodyweight by viewModel.bodyweight.collectAsStateWithLifecycle()

    // Top-level mode: false = the overview + chronological grid (default); true = the optional Albums view.
    var showAlbums by remember { mutableStateOf(false) }
    // Within Albums: null = folder grid; non-null (incl "" for Unsorted) = that album's photos.
    var openAlbum by remember { mutableStateOf<String?>(null) }
    var range by remember { mutableStateOf(GalleryRange.ALL) }
    var poseFilter by remember { mutableStateOf<PhotoPose?>(null) }
    var query by remember { mutableStateOf("") }
    var sort by remember { mutableStateOf(GallerySort.NEWEST) }
    var columns by remember { mutableStateOf(3) }
    var compareMode by remember { mutableStateOf(false) }
    val compareSel = remember { mutableStateListOf<ProgressPhoto>() }
    // The pair shown in the compare sheet — set from the band (first↔latest) or from a 2-photo selection.
    var comparePair by remember { mutableStateOf<List<ProgressPhoto>?>(null) }
    var viewer by remember { mutableStateOf<ViewerTarget?>(null) }
    var newAlbumOpen by remember { mutableStateOf(false) }
    var renaming by remember { mutableStateOf<String?>(null) }
    var addChooser by remember { mutableStateOf(false) }
    var searchOpen by remember { mutableStateOf(false) }
    var filtersOpen by remember { mutableStateOf(false) }
    val searchFocus = remember { FocusRequester() }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    val zone = remember { ZoneId.systemDefault() }
    val settings = LocalForgeSettings.current
    val firstDayMonday = settings.firstDayMonday
    val weightUnit = settings.weightUnit
    val searching = query.isNotBlank()

    val picker = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        // Imports into the open album (or Unsorted), auto-tagged with the active pose lens on the grid.
        uri?.let { viewModel.addPhoto(it, if (showAlbums) (openAlbum ?: "") else "", poseFilter?.name ?: "") }
    }
    fun importPhoto() = picker.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))

    // The overview grid: search OR (range ∧ pose) filtered, then sorted.
    val galleryPhotos = remember(state.photos, query, range, poseFilter, sort, firstDayMonday) {
        val base = if (query.isNotBlank()) state.photos.filter { photoMatchesQuery(it, query, zone) }
        else state.photos.filter {
            galleryRangeMatches(it.takenAtMs, range, zone, firstDayMonday) && (poseFilter == null || it.pose == poseFilter!!.name)
        }
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

    fun exitCompare() { compareMode = false; compareSel.clear() }
    fun openViewer(list: List<ProgressPhoto>, photo: ProgressPhoto) {
        viewer = ViewerTarget(list, list.indexOfFirst { it.fileName == photo.fileName }.coerceAtLeast(0))
    }

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
    val showAdd = !compareMode && (!showAlbums || openName != null)

    Scaffold(
        topBar = {
            TopAppBar(
                // §2: wordmark in the chrome — the serif "Gallery" hero below names the screen.
                title = { ForgeWordmark() },
                navigationIcon = {
                    IconButton(onClick = { goBack() }) {
                        Icon(
                            if (compareMode) Icons.Filled.Close else Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back"
                        )
                    }
                },
                actions = {
                    // ≤1 action (§2): add. All the search / filter / compare controls live in the content.
                    if (showAdd) IconButton(onClick = { addChooser = true }) {
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
                onCompare = { comparePair = compareSel.toList() },
                muted = muted, accent = accent
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        Column(
            Modifier.fillMaxSize().padding(inner).verticalScroll(rememberScrollState()).padding(horizontal = 24.dp)
        ) {
            when {
                compareMode -> {
                    Spacer(Modifier.height(4.dp))
                    CompareLevel(
                        photos = state.photos,
                        zone = zone,
                        onToggle = { toggleCompare(it) },
                        selectionIndexOf = { selectionIndexOf(it) },
                        fileFor = viewModel::fileFor,
                        muted = muted, accent = accent
                    )
                }
                !showAlbums -> OverviewLevel(
                    photos = state.photos,
                    visiblePhotos = galleryPhotos,
                    bodyweight = bodyweight,
                    searching = searching,
                    query = query,
                    onQueryChange = { query = it },
                    range = range,
                    onRangeChange = { range = it },
                    poseFilter = poseFilter,
                    onPoseChange = { poseFilter = it },
                    sort = sort,
                    onToggleSort = { sort = if (sort == GallerySort.NEWEST) GallerySort.OLDEST else GallerySort.NEWEST },
                    columns = columns,
                    onCycleColumns = { columns = GALLERY_DENSITIES[(GALLERY_DENSITIES.indexOf(columns) + 1) % GALLERY_DENSITIES.size] },
                    searchOpen = searchOpen,
                    filtersOpen = filtersOpen,
                    onToggleSearch = { if (searchOpen) closeSearch() else openSearch() },
                    onToggleFilters = { toggleFilters() },
                    searchFocus = searchFocus,
                    zone = zone,
                    weightUnit = weightUnit,
                    onOpenAlbums = { showAlbums = true },
                    onStartCompare = { compareMode = true },
                    onBandCompare = { a, b -> comparePair = listOf(a, b) },
                    onAdd = { addChooser = true },
                    onView = { openViewer(galleryPhotos, it) },
                    fileFor = viewModel::fileFor,
                    onBg = onBg, muted = muted, accent = accent, outline = outline
                )
                openName == null -> {
                    Spacer(Modifier.height(4.dp))
                    FolderGrid(
                        folders = state.folders,
                        onOpen = { openAlbum = it },
                        onNewAlbum = { newAlbumOpen = true },
                        fileFor = viewModel::fileFor,
                        onBg = onBg, muted = muted, accent = accent, outline = outline
                    )
                }
                else -> {
                    Spacer(Modifier.height(4.dp))
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
            weightUnit = weightUnit,
            fileFor = viewModel::fileFor,
            onSaveNote = { p, n -> viewModel.setNote(p, n) },
            onMove = { p, a -> viewModel.setAlbum(p, a) },
            onSetPose = { p, pose -> viewModel.setPose(p, pose) },
            onSetWeight = { p, w -> viewModel.setWeight(p, w) },
            onSetDate = { p, ms -> viewModel.setTakenAt(p, ms) },
            onDelete = { p -> viewModel.deletePhoto(p); viewer = null },
            onDismiss = { viewer = null }
        )
    }

    comparePair?.let { pair ->
        if (pair.size == 2) CompareSheet(
            pair = pair, zone = zone, weightUnit = weightUnit, fileFor = viewModel::fileFor,
            onDismiss = { comparePair = null }
        ) else comparePair = null
    }

    if (addChooser) {
        AddPhotoChooser(
            onCamera = { addChooser = false; onOpenCamera() },
            onImport = { addChooser = false; importPhoto() },
            onDismiss = { addChooser = false }
        )
    }

    if (newAlbumOpen) {
        NameDialog(
            title = "New album",
            initial = "",
            onConfirm = { name ->
                viewModel.createAlbum(name)
                newAlbumOpen = false
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
 * The default Gallery level: the hero, the progress band, the bodyweight line, the pose lens + tool
 * row, then the month-grouped grid. Empty (no photos) is carried entirely by the band's ghost frames.
 */
@Composable
private fun OverviewLevel(
    photos: List<ProgressPhoto>,
    visiblePhotos: List<ProgressPhoto>,
    bodyweight: List<com.forge.app.data.db.entities.BodyweightEntry>,
    searching: Boolean,
    query: String,
    onQueryChange: (String) -> Unit,
    range: GalleryRange,
    onRangeChange: (GalleryRange) -> Unit,
    poseFilter: PhotoPose?,
    onPoseChange: (PhotoPose?) -> Unit,
    sort: GallerySort,
    onToggleSort: () -> Unit,
    columns: Int,
    onCycleColumns: () -> Unit,
    searchOpen: Boolean,
    filtersOpen: Boolean,
    onToggleSearch: () -> Unit,
    onToggleFilters: () -> Unit,
    searchFocus: FocusRequester,
    zone: ZoneId,
    weightUnit: WeightUnit,
    onOpenAlbums: () -> Unit,
    onStartCompare: () -> Unit,
    onBandCompare: (ProgressPhoto, ProgressPhoto) -> Unit,
    onAdd: () -> Unit,
    onView: (ProgressPhoto) -> Unit,
    fileFor: (ProgressPhoto) -> java.io.File,
    onBg: Color, muted: Color, accent: Color, outline: Color
) {
    val (before, after) = remember(photos) { bestComparePair(photos) }

    Column(Modifier.fillMaxWidth().statsEntrance(0)) {
        GalleryHero(photos, zone, onBg, muted)
        Spacer(Modifier.height(20.dp))
        // Signature mark — works at zero (ghost frames + add prompt), so no separate empty text row (§12).
        ProgressBand(
            before = before, after = after, zone = zone, weightUnit = weightUnit, fileFor = fileFor,
            onCompare = onBandCompare, onAdd = onAdd,
            onBg = onBg, muted = muted, accent = accent, outline = outline
        )
    }

    if (photos.isEmpty()) return

    if (bodyweight.size >= 2) {
        Spacer(Modifier.height(28.dp))
        Column(Modifier.fillMaxWidth().statsEntrance(1)) {
            BodyweightSparkline(bodyweight, photos, weightUnit, onBg, muted, accent)
        }
    }

    // Auto-paired "scale held, body changed" shots — excludes the band pair so it never echoes it.
    // Pairing is O(n²) over every weighed photo, so it runs off the main thread (like the figure/decode
    // paths) to keep the overview from janking on large galleries; empty until the first pass lands.
    val samePairs by produceState(emptyList<SameWeightPair>(), photos, before, after) {
        value = withContext(Dispatchers.Default) {
            sameWeightPairs(photos, zone, setOfNotNull(before?.fileName, after?.fileName))
        }
    }
    if (samePairs.isNotEmpty()) {
        Spacer(Modifier.height(28.dp))
        Column(Modifier.fillMaxWidth().statsEntrance(2)) {
            SameWeightSection(samePairs, zone, weightUnit, fileFor, onBandCompare, muted, accent)
        }
    }

    // ── Timeline: header + pose lens + tool pills over the month-grouped grid ──
    Spacer(Modifier.height(28.dp))
    Column(Modifier.fillMaxWidth().statsEntrance(3)) {
        EditorialHeader("Timeline", muted, accent, action = "Albums →", onAction = onOpenAlbums)
        Spacer(Modifier.height(10.dp))

        val poses = remember(photos) { posesPresent(photos) }
        if (poses.isNotEmpty()) {
            Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                SegmentPill("All", selected = poseFilter == null, onClick = { onPoseChange(null) }, accent, onBg, muted, outline)
                poses.forEach { p ->
                    SegmentPill(p.label, selected = poseFilter == p, onClick = { onPoseChange(p) }, accent, onBg, muted, outline)
                }
            }
            Spacer(Modifier.height(10.dp))
        }

        // Tool pills — one pill vocabulary with the range chips below, no stock icons in content (§8).
        val filtersActive = range != GalleryRange.ALL || sort != GallerySort.NEWEST || columns != 3
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            GalleryChip("Search", selected = searchOpen) { onToggleSearch() }
            GalleryChip("Filters", selected = filtersOpen || filtersActive) { onToggleFilters() }
            if (photos.size >= 2) GalleryChip("Compare", selected = false) { onStartCompare() }
        }

        // Revealed controls: the search field OR the range/sort/density panel — never both at once.
        when {
            searchOpen -> {
                Spacer(Modifier.height(10.dp))
                GallerySearchBar(query, onQueryChange, focusRequester = searchFocus)
            }
            filtersOpen -> {
                Spacer(Modifier.height(10.dp))
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    GalleryRange.entries.forEach { r -> GalleryChip(r.label, selected = r == range) { onRangeChange(r) } }
                }
                Spacer(Modifier.height(10.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    GalleryChip(if (sort == GallerySort.NEWEST) "Newest first" else "Oldest first", selected = false) { onToggleSort() }
                    GalleryChip("$columns across", selected = false) { onCycleColumns() }
                }
            }
        }
    }

    Spacer(Modifier.height(16.dp))
    Column(Modifier.fillMaxWidth().statsEntrance(4)) {
        when {
            visiblePhotos.isEmpty() -> Text(
                if (searching) "No photos match “$query”." else "No photos in this view.",
                style = MaterialTheme.typography.bodyMedium, color = muted
            )
            else -> {
                if (searching) {
                    Text(
                        "${visiblePhotos.size} result${if (visiblePhotos.size == 1) "" else "s"}",
                        style = MaterialTheme.typography.labelMedium, color = muted
                    )
                    Spacer(Modifier.height(12.dp))
                }
                MonthGroupedGrid(visiblePhotos, columns, zone, fileFor, muted, accent, onView)
            }
        }
    }
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
    Text("Pick two photos to see them side by side.", style = MaterialTheme.typography.bodyMedium, color = muted)
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

/** The add-a-photo chooser sheet: the guided camera (do-it-now) or a gallery import (sidekick). */
@Composable
internal fun AddPhotoChooser(onCamera: () -> Unit, onImport: () -> Unit, onDismiss: () -> Unit) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState()) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp).padding(bottom = 32.dp)) {
            Text("ADD PHOTO", style = MaterialTheme.typography.labelLarge, color = muted, letterSpacing = 1.5.sp)
            Spacer(Modifier.height(4.dp))
            Text(
                "Shots stay on your phone.",
                style = MaterialTheme.typography.bodySmall, color = muted,
                fontStyle = androidx.compose.ui.text.font.FontStyle.Italic
            )
            Spacer(Modifier.height(18.dp))
            ForgePrimaryCapsule("Take a photo", onClick = onCamera, modifier = Modifier.fillMaxWidth())
            Spacer(Modifier.height(10.dp))
            ForgeOutlineCapsule("Import from gallery", onClick = onImport, modifier = Modifier.fillMaxWidth())
        }
    }
}
