package com.forge.app.ui.profile

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
 * Backs the in-app guided camera. Exposes the latest shot per pose (plus the newest overall) so the
 * preview can ghost your last same-pose photo for alignment, and saves a capture straight into
 * app-private storage via [ProgressPhotoRepository.addCaptured] (which snapshots the bodyweight).
 */
@HiltViewModel
class ProgressCameraViewModel @Inject constructor(
    private val photoRepo: ProgressPhotoRepository
) : ViewModel() {

    /** Newest photo per pose key ("" = untagged) — the ghost source when you've shot that pose before. */
    private val _latestByPose = MutableStateFlow<Map<String, ProgressPhoto>>(emptyMap())
    val latestByPose: StateFlow<Map<String, ProgressPhoto>> = _latestByPose.asStateFlow()

    /** Your newest photo overall — the fallback ghost for a pose you've never shot. */
    private val _newest = MutableStateFlow<ProgressPhoto?>(null)
    val newest: StateFlow<ProgressPhoto?> = _newest.asStateFlow()

    init { viewModelScope.launch { photoRepo.revision.collect { reload() } } }

    private suspend fun reload() {
        val photos = photoRepo.photos() // newest first
        val map = HashMap<String, ProgressPhoto>()
        photos.forEach { p -> map.putIfAbsent(p.pose, p) }
        _latestByPose.value = map
        _newest.value = photos.firstOrNull()
    }

    fun addCaptured(temp: File, poseKey: String) = viewModelScope.launch { photoRepo.addCaptured(temp, poseKey) }

    fun fileFor(photo: ProgressPhoto): File = photoRepo.fileFor(photo)
}
