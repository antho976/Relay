package com.forge.app.ui.profile

import android.net.Uri
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.repo.BodyweightRepository
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.data.repo.ProgressPhotoRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
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
    bodyweightRepo: BodyweightRepository
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
    ) {
        // Case-insensitive so a folder whose name differs only in case still shows its photos.
        fun photosIn(album: String): List<ProgressPhoto> = photos.filter { it.album.equals(album, ignoreCase = true) }
    }

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    /** Weigh-ins over the past year, oldest → newest — the bodyweight-through-time sparkline series. */
    val bodyweight: StateFlow<List<BodyweightEntry>> =
        bodyweightRepo.observeRecent(365)
            .map { entries -> entries.sortedBy { it.recordedAt } }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

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

    fun createAlbum(name: String) = viewModelScope.launch { photoRepo.createAlbum(name) }
    fun renameAlbum(old: String, new: String) = viewModelScope.launch { photoRepo.renameAlbum(old, new) }
    fun deleteAlbum(name: String) = viewModelScope.launch { photoRepo.deleteAlbum(name) }

    fun addPhoto(uri: Uri, album: String, pose: String = "") = viewModelScope.launch {
        photoRepo.add(uri, album = album, pose = pose)
    }

    /**
     * Import a whole selection in one go. Sequential on purpose: each add copies bytes and rewrites
     * the shared JSON index under the repository's write lock, so running them concurrently would
     * only contend for that lock. One `revision` bump per photo keeps the grid filling in as they
     * land rather than appearing all at once at the end.
     */
    fun addPhotos(
        uris: List<Uri>,
        album: String,
        pose: String = "",
        muscles: List<String> = emptyList()
    ) = viewModelScope.launch {
        uris.forEach { photoRepo.add(it, album = album, pose = pose, muscles = muscles) }
    }

    fun setAlbum(photo: ProgressPhoto, album: String) = viewModelScope.launch { photoRepo.setAlbum(photo, album) }
    fun setNote(photo: ProgressPhoto, note: String) = viewModelScope.launch { photoRepo.setNote(photo, note) }
    fun setTitle(photo: ProgressPhoto, title: String) = viewModelScope.launch { photoRepo.setTitle(photo, title) }
    fun setPose(photo: ProgressPhoto, pose: String) = viewModelScope.launch { photoRepo.setPose(photo, pose) }
    fun setMuscles(photo: ProgressPhoto, muscles: List<String>) = viewModelScope.launch { photoRepo.setMuscles(photo, muscles) }
    fun setTags(photo: ProgressPhoto, tags: List<String>) = viewModelScope.launch { photoRepo.setTags(photo, tags) }
    fun setWeight(photo: ProgressPhoto, weightLb: Double?) = viewModelScope.launch { photoRepo.setWeight(photo, weightLb) }
    fun setTakenAt(photo: ProgressPhoto, takenAtMs: Long) = viewModelScope.launch { photoRepo.setTakenAt(photo, takenAtMs) }
    fun deletePhoto(photo: ProgressPhoto) = viewModelScope.launch { photoRepo.delete(photo) }

    fun fileFor(photo: ProgressPhoto): File = photoRepo.fileFor(photo)
}
