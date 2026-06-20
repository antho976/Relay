package com.forge.app.ui.profile

import android.net.Uri
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.data.repo.ProgressPhotoRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.io.File
import javax.inject.Inject

/**
 * The full "Mirror test" gallery (reached from the Profile teaser's "view all"). Photos are grouped
 * into albums — one album per photo, with an implicit "Unsorted" bucket for the rest. Pure local,
 * app-private files; see [ProgressPhotoRepository].
 */
@HiltViewModel
class MirrorTestViewModel @Inject constructor(
    private val photoRepo: ProgressPhotoRepository
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
        /** Top-level folders, precomputed at load — not a per-read getter, so the grid doesn't
         *  re-run the groupBy on every recomposition. */
        val folders: List<AlbumFolder> = emptyList()
    ) {
        // Case-insensitive so a folder whose name differs only in case still shows its photos.
        fun photosIn(album: String): List<ProgressPhoto> = photos.filter { it.album.equals(album, ignoreCase = true) }
    }

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    init { reload() }

    private fun reload() = viewModelScope.launch {
        val photos = photoRepo.photos()
        val albumNames = photoRepo.albums()
        _state.value = UiState(loading = false, photos = photos, albumNames = albumNames, folders = foldersOf(photos, albumNames))
    }

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

    fun createAlbum(name: String) = viewModelScope.launch { photoRepo.createAlbum(name); reload() }
    fun renameAlbum(old: String, new: String) = viewModelScope.launch { photoRepo.renameAlbum(old, new); reload() }
    fun deleteAlbum(name: String) = viewModelScope.launch { photoRepo.deleteAlbum(name); reload() }

    fun addPhoto(uri: Uri, album: String) = viewModelScope.launch {
        photoRepo.add(uri, System.currentTimeMillis(), album = album); reload()
    }

    fun setAlbum(photo: ProgressPhoto, album: String) = viewModelScope.launch { photoRepo.setAlbum(photo, album); reload() }
    fun setNote(photo: ProgressPhoto, note: String) = viewModelScope.launch { photoRepo.setNote(photo, note); reload() }
    fun deletePhoto(photo: ProgressPhoto) = viewModelScope.launch { photoRepo.delete(photo); reload() }

    fun fileFor(photo: ProgressPhoto): File = photoRepo.fileFor(photo)
}
