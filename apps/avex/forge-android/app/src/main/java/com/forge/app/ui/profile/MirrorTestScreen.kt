@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.profile

import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Scaffold
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.theme.LocalForgeSettings
import java.time.LocalDate
import java.time.ZoneId

/** A frozen snapshot of the list the viewer pages through, plus the index it opened on. */
private data class ViewerTarget(val photos: List<ProgressPhoto>, val index: Int)

/**
 * The photo **Gallery** (reached from the Profile teaser's "view all").
 *
 * Gallery-first, as of the tag revamp: the library leads. A compact masthead (mono eyebrow, serif
 * hero, the first↔now progress band) sits at the top of one lazy list, then the browse bar — search,
 * the pose lens, and the WHEN / MUSCLE / TAGS rails behind Filters — then the day-grouped grid with
 * its headers pinned as you scroll. The bodyweight trend and the auto-paired same-weight shots close
 * the roll. Everything above the grid scrolls away, because in a gallery the photos are the content
 * and the instruments are not.
 *
 * A photo carries four tag axes now: its **date**, its **pose** (where the camera stood), its
 * **muscle tags** (what the shot documents, from the app's own `MuscleGroup` vocabulary) and its
 * free **tags**. Filtering ANDs across those axes and ORs within them. Narrowing to a single muscle
 * turns it into a lens: the progress band re-pairs inside that muscle, so the mark at the top of the
 * screen answers "how has my back changed" and not only "how have I changed".
 *
 * Tapping a photo opens a swipeable full-screen viewer and metadata editor; holding one starts a
 * compare selection. Private, app-private files only (see [MirrorTestViewModel] /
 * [com.forge.app.data.repo.ProgressPhotoRepository]).
 *
 * The screen is written for an EMPTY library first: it opens here, not on a grid. Photos gate the
 * sections that would be dishonest without them, never the page itself, and each browse control is
 * gated on having something to narrow (see [GalleryFilterBar]).
 */
@Composable
fun MirrorTestScreen(
    onBack: () -> Unit,
    onOpenCamera: () -> Unit = {},
    viewModel: MirrorTestViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val bodyweight by viewModel.bodyweight.collectAsStateWithLifecycle()

    // Top-level mode: false = the library (default); true = the optional Albums view.
    var showAlbums by remember { mutableStateOf(false) }
    // Within Albums: null = folder grid; non-null (incl "" for Unsorted) = that album's photos.
    var openAlbum by remember { mutableStateOf<String?>(null) }
    var filter by remember { mutableStateOf(GalleryFilter()) }
    var filtersOpen by remember { mutableStateOf(false) }
    var compareMode by remember { mutableStateOf(false) }
    val compareSel = remember { mutableStateListOf<ProgressPhoto>() }
    // The pair shown in the compare sheet — set from the band, a same-weight card, or a selection.
    var comparePair by remember { mutableStateOf<List<ProgressPhoto>?>(null) }
    var viewer by remember { mutableStateOf<ViewerTarget?>(null) }
    var newAlbumOpen by remember { mutableStateOf(false) }
    var renaming by remember { mutableStateOf<String?>(null) }
    var addChooser by remember { mutableStateOf(false) }
    val searchFocus = remember { FocusRequester() }
    val listState = rememberLazyListState()

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline
    val background = MaterialTheme.colorScheme.background

    // Not remembered (M-15). A remembered zone survives recomposition by definition, so after a
    // flight the gallery kept bucketing new-zone photo timestamps by the old zone's day boundaries.
    // Re-read here, like CardioScreen: `today` below is keyed on it, so a changed zone re-derives
    // the days rather than quietly mis-slicing them.
    val zone = ZoneId.systemDefault()
    val settings = LocalForgeSettings.current
    val firstDayMonday = settings.firstDayMonday
    val weightUnit = settings.weightUnit

    // MULTI-select. It was `PickVisualMedia`, one photo per trip through the system picker — so
    // documenting a session from several angles, which is the whole point of a physique gallery,
    // meant repeating the entire add flow per shot. Imports inherit the lens you are browsing under:
    // the active pose and the muscles you have narrowed to, because that is what you were looking at
    // when you decided to add more of it.
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.PickMultipleVisualMedia()) { uris ->
        if (uris.isNotEmpty()) {
            viewModel.addPhotos(
                uris,
                album = if (showAlbums) (openAlbum ?: "") else "",
                pose = filter.pose?.name ?: "",
                muscles = filter.muscles.toList()
            )
        }
    }
    fun importPhoto() = picker.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))

    // The visible library: every axis of the filter applied, then sorted.
    val galleryPhotos = remember(state.photos, filter, firstDayMonday) {
        applyGalleryFilter(state.photos, filter, zone, firstDayMonday)
    }
    val today = remember(zone) { LocalDate.now(zone) }
    val days = remember(galleryPhotos, today) { galleryDays(galleryPhotos, zone, today) }

    // The band reads the muscle lens when there is exactly one, so it answers the question the grid
    // is currently asking. With none or several it reads the whole library, as it always did.
    val bandSource = remember(state.photos, filter.muscles) {
        val sole = filter.soleMuscle
        if (sole == null) state.photos else state.photos.filter { sole.code in it.muscles }
    }
    val (bandBefore, bandAfter) = remember(bandSource) { bestComparePair(bandSource) }

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
    fun startCompareWith(photo: ProgressPhoto?) {
        compareMode = true
        compareSel.clear()
        photo?.let { compareSel.add(it) }
    }
    fun openViewer(list: List<ProgressPhoto>, photo: ProgressPhoto) {
        viewer = ViewerTarget(list, list.indexOfFirst { it.fileName == photo.fileName }.coerceAtLeast(0))
    }

    // A free tag can outlive its last photo: untag the final #cut shot and the chip leaves the rail,
    // stranding the grid on a filter the user can no longer see or undo. Pose and muscle are fixed
    // vocabularies whose rails always draw, so only tags can go stale this way.
    LaunchedEffect(state.knownTags) {
        val live = state.knownTags.toSet()
        val kept = filter.tags intersect live
        if (kept != filter.tags) filter = filter.copy(tags = kept)
    }
    LaunchedEffect(state.photos.size) { if (state.photos.size < 2 && compareMode) exitCompare() }

    val tools = GalleryTools(
        filter = filter,
        onChange = { filter = it },
        filtersOpen = filtersOpen,
        onToggleFilters = { filtersOpen = !filtersOpen },
        searchFocus = searchFocus
    )

    // Back drills out: compare → grid; album → folder grid → library → leave the screen.
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

    val albumPhotos = remember(state.photos, openName, filter.sort) {
        if (openName == null) emptyList()
        else {
            val ps = state.photosIn(openName)
            if (filter.sort == GallerySort.OLDEST) ps.sortedBy { it.takenAtMs } else ps.sortedByDescending { it.takenAtMs }
        }
    }
    val albumDays = remember(albumPhotos, today) { galleryDays(albumPhotos, zone, today) }

    Scaffold(
        topBar = {
            TopAppBar(
                // §4.6: the serif "Gallery" hero below names the screen, so the bar carries no title.
                title = {},
                navigationIcon = {
                    IconButton(onClick = { goBack() }) {
                        Icon(
                            if (compareMode) Icons.Filled.Close else Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = if (compareMode) "Leave compare" else "Back"
                        )
                    }
                },
                actions = {
                    // ≤1 action (§2): add. Search, filters and compare all live in the content.
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
        LazyColumn(Modifier.fillMaxSize().padding(inner), state = listState) {
            when {
                // ── Compare: the whole library, selectable ──────────────────────────────────
                compareMode -> {
                    if (state.photos.isEmpty()) {
                        item { Gutter { InlineEmptyHint("Add two shots before comparing.", muted) } }
                    } else {
                        val allDays = galleryDays(state.photos, zone, today)
                        galleryGrid(
                            allDays,
                            GalleryGridSpec(
                                columns = 3,
                                fileFor = viewModel::fileFor,
                                onPhotoClick = { toggleCompare(it) },
                                selectable = true,
                                selectionIndexOf = { selectionIndexOf(it) }
                            ),
                            muted, accent, background
                        )
                    }
                }

                // ── Albums: the folder grid, then one album's photos ────────────────────────
                showAlbums && openName == null -> item {
                    Gutter {
                        Spacer(Modifier.height(4.dp))
                        FolderGrid(
                            folders = state.folders,
                            onOpen = { openAlbum = it },
                            onNewAlbum = { newAlbumOpen = true },
                            fileFor = viewModel::fileFor,
                            onBg = onBg, muted = muted, accent = accent, outline = outline
                        )
                    }
                }
                showAlbums -> {
                    val album = openName.orEmpty()
                    item {
                        Gutter {
                            Spacer(Modifier.height(4.dp))
                            if (album.isNotBlank()) {
                                AlbumActions(
                                    onRename = { renaming = album },
                                    onDelete = { viewModel.deleteAlbum(album); openAlbum = null },
                                    accent = accent
                                )
                                Spacer(Modifier.height(14.dp))
                            }
                            if (albumPhotos.isEmpty()) {
                                InlineEmptyHint("Nothing in this album yet. Add a photo with the + above.", muted)
                            }
                        }
                    }
                    galleryGrid(
                        albumDays,
                        GalleryGridSpec(
                            columns = filter.columns,
                            fileFor = viewModel::fileFor,
                            onPhotoClick = { openViewer(albumPhotos, it) }
                        ),
                        muted, accent, background
                    )
                }

                // ── The library ────────────────────────────────────────────────────────────
                else -> galleryLibrary(
                    loading = state.loading,
                    photos = state.photos,
                    visiblePhotos = galleryPhotos,
                    days = days,
                    knownTags = state.knownTags,
                    tools = tools,
                    bodyweight = bodyweight,
                    bandBefore = bandBefore,
                    bandAfter = bandAfter,
                    zone = zone,
                    weightUnit = weightUnit,
                    fileFor = viewModel::fileFor,
                    onOpenAlbums = { showAlbums = true },
                    onStartCompare = { startCompareWith(null) },
                    onLongPressPhoto = { p -> startCompareWith(p) },
                    onCompare = { a, b -> comparePair = listOf(a, b) },
                    onAdd = { addChooser = true },
                    onView = { openViewer(galleryPhotos, it) },
                    onBg = onBg, muted = muted, accent = accent, outline = outline, background = background
                )
            }
            item { Spacer(Modifier.height(40.dp)) }
        }
    }

    viewer?.let { target ->
        GalleryViewerPager(
            photos = target.photos,
            startIndex = target.index,
            albumNames = state.albumNames,
            knownTags = state.knownTags,
            weightUnit = weightUnit,
            fileFor = viewModel::fileFor,
            onSaveNote = { p, n -> viewModel.setNote(p, n) },
            onSaveTitle = { p, t -> viewModel.setTitle(p, t) },
            onMove = { p, a -> viewModel.setAlbum(p, a) },
            onSetPose = { p, pose -> viewModel.setPose(p, pose) },
            onSetMuscles = { p, m -> viewModel.setMuscles(p, m) },
            onSetTags = { p, t -> viewModel.setTags(p, t) },
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
 * The add-a-photo chooser sheet: the guided camera (do-it-now) or a gallery import (sidekick). Two
 * capsules and their anchor, nothing else — the "shots stay on your phone" reassurance line was
 * retired with the before/after share card (§2) and is not re-added here.
 */
@Composable
internal fun AddPhotoChooser(onCamera: () -> Unit, onImport: () -> Unit, onDismiss: () -> Unit) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(),
        // §5: a modal is a `surface` fill — M3 defaults to the unthemed `surfaceContainerLow`.
        containerColor = MaterialTheme.colorScheme.surface
    ) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp).padding(bottom = 32.dp)) {
            EditorialHeader("Add photo", muted, accent)
            Spacer(Modifier.height(18.dp))
            ForgePrimaryCapsule("Take a photo", onClick = onCamera, modifier = Modifier.fillMaxWidth())
            Spacer(Modifier.height(10.dp))
            ForgeOutlineCapsule("Import from gallery", onClick = onImport, modifier = Modifier.fillMaxWidth())
        }
    }
}
