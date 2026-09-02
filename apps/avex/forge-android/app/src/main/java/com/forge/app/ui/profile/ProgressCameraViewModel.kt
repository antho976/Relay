package com.forge.app.ui.profile

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.data.repo.ProgressPhotoRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.CancellationException
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

    /**
     * A shot the camera wrote to cache that the repository has not yet accepted. Non-null after a
     * failed save: the file is the only copy of that photo, so it is kept and offered for retry
     * rather than deleted, and the screen must not report it as saved.
     */
    data class PendingCapture(val file: File, val poseKey: String)

    private val _pending = MutableStateFlow<PendingCapture?>(null)
    val pending: StateFlow<PendingCapture?> = _pending.asStateFlow()

    /**
     * Persist a fresh capture, then invoke [onSaved] (the screen's back-navigation). [onSaved] runs
     * INSIDE the coroutine after the write completes, so popping the screen — which clears this VM and
     * cancels [viewModelScope] — can't race the save and drop the photo.
     *
     * [onSaved] fires ONLY on a non-null repository result. The nullable contract of
     * [ProgressPhotoRepository.addCaptured] is a real failure (copy, decode or index), and treating
     * it as success navigated back from a photo that did not exist. On failure the capture is kept
     * in [pending] and [onFailed] runs, so the screen can show the consequence and retry against
     * the same file.
     */
    fun addCaptured(temp: File, poseKey: String, onSaved: () -> Unit, onFailed: () -> Unit) =
        viewModelScope.launch {
            val saved = try {
                photoRepo.addCaptured(temp, poseKey)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                null
            }
            if (saved != null) {
                _pending.value = null
                onSaved()
            } else {
                _pending.value = PendingCapture(temp, poseKey)
                onFailed()
            }
        }

    /** Retry persisting the capture a failed save left in [pending]. No-op when there is none. */
    fun retryPending(onSaved: () -> Unit, onFailed: () -> Unit) {
        val p = _pending.value ?: return
        addCaptured(p.file, p.poseKey, onSaved, onFailed)
    }

    fun fileFor(photo: ProgressPhoto): File = photoRepo.fileFor(photo)
}
