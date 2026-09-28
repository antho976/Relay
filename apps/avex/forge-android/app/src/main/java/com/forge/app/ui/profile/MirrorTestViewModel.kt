package com.forge.app.ui.profile

import android.net.Uri
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.data.repo.ProgressPhotoRepository
import com.forge.app.ui.common.SnackbarController
import com.forge.app.ui.common.launchDurable
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.io.File
import javax.inject.Inject

/**
 * The full photo Gallery (reached from the Profile teaser's "view all"). Photos carry a date, an
 * optional pose tag, muscle tags, free tags, the bodyweight nearest that date, a note and an album — the metadata that makes
 * two shots comparable over time. Reloads reactively off [ProgressPhotoRepository.revision], so a
 * capture from the in-app camera (a different screen) refreshes the grid. Pure local, app-private
 * files; see [ProgressPhotoRepository].
 */
@HiltViewModel
class MirrorTestViewModel @Inject constructor(
    private val photoRepo: ProgressPhotoRepository,
    private val snackbar: SnackbarController
) : ViewModel() {

    /** One album folder for the gallery's top level — its photo count and newest-photo cover. */
    data class AlbumFolder(
        /** "" for the implicit Unsorted bucket. */
        val name: String,
        val displayName: String,
        val count: Int,
        val cover: ProgressPhoto?
    )

    data class UiState(
        val loading: Boolean = true,
        val photos: List<ProgressPhoto> = emptyList(),
        /** Explicit, user-created album names (excludes Unsorted), in creation order. */
        val albumNames: List<String> = emptyList(),
        /** Every free tag in use across the library, most-used first — the viewer's suggestion
         *  rail and the filter rail read the same list, so neither can offer a tag the other
         *  doesn't know about. Precomputed at load, not derived per recomposition. */
        val knownTags: List<String> = emptyList(),
        /** Top-level folders, precomputed at load — not a per-read getter, so the grid doesn't
         *  re-run the groupBy on every recomposition. */
        val folders: List<AlbumFolder> = emptyList()
    )

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    // Reload whenever the store changes. StateFlow emits its current value on subscribe, so this also
    // does the initial load — no separate init { reload() }.
    init { viewModelScope.launch { photoRepo.revision.collect { reload() } } }

    private suspend fun reload() {
        val photos = photoRepo.photos()
        val albumNames = photoRepo.albums()
        _state.value = UiState(
            loading = false,
            photos = photos,
            albumNames = albumNames,
            knownTags = tagsByUse(photos),
            folders = foldersOf(photos, albumNames)
        )
    }

    /** Every free tag in use, ranked by how many photos carry it, then alphabetically so ties are
     *  stable across reloads rather than following index order. */
    private fun tagsByUse(photos: List<ProgressPhoto>): List<String> =
        photos.flatMap { it.tags }
            .groupingBy { it }.eachCount()
            .entries.sortedWith(compareByDescending<Map.Entry<String, Int>> { it.value }.thenBy { it.key })
            .map { it.key }

    /** Folders shown at the gallery's top level: every named album, then Unsorted if non-empty. The
     *  named list is (explicit album list ∪ any album referenced by a photo) so no photo can hide in
     *  a missing folder even if the two files ever drift. */
    private fun foldersOf(photos: List<ProgressPhoto>, albumNames: List<String>): List<AlbumFolder> {
        val byAlbum = photos.groupBy { it.album }
        val named = (albumNames + byAlbum.keys.filter { it.isNotEmpty() }).distinct()
        val namedFolders = named.map { n ->
            val ps = byAlbum[n].orEmpty()
            AlbumFolder(n, n, ps.size, ps.firstOrNull())
        }
        val unsorted = byAlbum[""].orEmpty()
        return if (unsorted.isEmpty()) namedFolders
        else namedFolders + AlbumFolder("", "Unsorted", unsorted.size, unsorted.firstOrNull())
    }

    /** Create an album and, when [moving] is non-empty, file those photos into it in the same pass. */
    fun createAlbum(name: String, moving: List<ProgressPhoto> = emptyList()) = viewModelScope.launch {
        val created = photoRepo.createAlbum(name)
        if (created.isNotEmpty()) moving.forEach { photoRepo.setAlbum(it, created) }
    }
    fun renameAlbum(old: String, new: String) = viewModelScope.launch { photoRepo.renameAlbum(old, new) }
    fun deleteAlbum(name: String) = viewModelScope.launch { photoRepo.deleteAlbum(name) }

    /**
     * Import a whole selection in one go. Sequential on purpose: each add copies bytes and rewrites
     * the shared JSON index under the repository's write lock, so running them concurrently would
     * only contend for that lock. One `revision` bump per photo keeps the grid filling in as they
     * land rather than appearing all at once at the end.
     *
     * Durable, so leaving the gallery mid-import doesn't cancel the rest of the selection, and a
     * photo that could not be imported (too large, not an image) is counted and reported on the
     * app-root snackbar rather than dropped without a word.
     */
    fun addPhotos(
        uris: List<Uri>,
        album: String,
        pose: String = "",
        muscles: List<String> = emptyList()
    ) = viewModelScope.launchDurable {
        var failed = 0
        uris.forEach { if (photoRepo.add(it, album = album, pose = pose, muscles = muscles) == null) failed++ }
        if (failed > 0) {
            snackbar.show(if (failed == 1) "1 photo couldn't be imported." else "$failed photos couldn't be imported.")
        }
    }

    // Durable: the viewer commits a typed title, note or weight as it is DISMISSED, and Back out of
    // the gallery on the same breath cleared this ViewModel and cancelled the write it had just made.
    fun setAlbum(photo: ProgressPhoto, album: String) = viewModelScope.launchDurable { photoRepo.setAlbum(photo, album) }
    fun setNote(photo: ProgressPhoto, note: String) = viewModelScope.launchDurable { photoRepo.setNote(photo, note) }
    fun setTitle(photo: ProgressPhoto, title: String) = viewModelScope.launchDurable { photoRepo.setTitle(photo, title) }
    fun setPose(photo: ProgressPhoto, pose: String) = viewModelScope.launchDurable { photoRepo.setPose(photo, pose) }
    fun setMuscles(photo: ProgressPhoto, muscles: List<String>) = viewModelScope.launchDurable { photoRepo.setMuscles(photo, muscles) }
    fun setTags(photo: ProgressPhoto, tags: List<String>) = viewModelScope.launchDurable { photoRepo.setTags(photo, tags) }
    fun setWeight(photo: ProgressPhoto, weightLb: Double?) = viewModelScope.launchDurable { photoRepo.setWeight(photo, weightLb) }
    /** Re-date [photo]; [onStored] gets the entry as written (its re-snapshotted weight), on the main thread. */
    fun setTakenAt(photo: ProgressPhoto, takenAtMs: Long, onStored: (ProgressPhoto) -> Unit = {}) =
        viewModelScope.launchDurable { photoRepo.setTakenAt(photo, takenAtMs)?.let(onStored) }
    fun deletePhoto(photo: ProgressPhoto) = viewModelScope.launchDurable { photoRepo.delete(photo) }

    /** Multi-select delete. Sequential for the same reason as [addPhotos]: one index, one lock. */
    fun deletePhotos(photos: List<ProgressPhoto>) = viewModelScope.launchDurable { photos.forEach { photoRepo.delete(it) } }

    /** Multi-select move. "" takes the photos out of every album. */
    fun moveToAlbum(photos: List<ProgressPhoto>, album: String) =
        viewModelScope.launchDurable { photos.forEach { photoRepo.setAlbum(it, album) } }

    fun fileFor(photo: ProgressPhoto): File = photoRepo.fileFor(photo)
}
