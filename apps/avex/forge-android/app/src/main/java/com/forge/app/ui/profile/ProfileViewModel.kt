package com.forge.app.ui.profile

import android.net.Uri
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.ProfileData
import com.forge.app.data.repo.ProfileRepository
import com.forge.app.data.repo.AvatarRepository
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.data.repo.ProgressPhotoRepository
import com.forge.app.ui.profile.state.ProfileUiState
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import java.io.File
import javax.inject.Inject

/**
 * The "You" hub (profile). Pure local — no account, no server. A thin mapper: [ProfileRepository]
 * does the snapshot fan-out + runs the rank/XP/standing engines; this VM layers on the live name
 * and progress photos, and owns the photo mutations.
 */
@HiltViewModel
class ProfileViewModel @Inject constructor(
    private val profileRepo: ProfileRepository,
    private val settingsRepo: SettingsRepository,
    private val photoRepo: ProgressPhotoRepository,
    private val avatarRepo: AvatarRepository
) : ViewModel() {

    private val _state = MutableStateFlow(ProfileUiState())
    val state: StateFlow<ProfileUiState> = _state.asStateFlow()

    /** True on this profile open iff the user just crossed into a higher tier since last visit. */
    private val _showRankUpCelebration = MutableStateFlow(false)
    val showRankUpCelebration: StateFlow<Boolean> = _showRankUpCelebration.asStateFlow()

    init { load() }

    private fun load() = viewModelScope.launch {
        val name = settingsRepo.userName.first()
        val photos = photoRepo.photos()
        val hasAvatar = avatarRepo.exists()
        val avatarStamp = if (hasAvatar) avatarRepo.file.lastModified() else 0L
        // Instant first paint on re-entry: render the last-assembled data while the fresh fan-out runs (P3).
        val cached = profileRepo.cached()
        _state.value = if (cached != null) buildState(cached, name, photos, hasAvatar, avatarStamp)
            else _state.value.copy(name = name, photos = photos, hasAvatar = hasAvatar, avatarStamp = avatarStamp)
        val data = profileRepo.load()
        // Merge the fresh fan-out but keep the user-editable fields from current state, so a rename /
        // photo-note / avatar change made while the (slow) fan-out ran isn't reverted by the pre-load
        // snapshot — the edit fns persist then update _state, so reading them back here is correct.
        _state.value = buildState(data, _state.value.name, _state.value.photos, _state.value.hasAvatar, _state.value.avatarStamp)

        // ── Rank-up celebration detection ─────────────────────────────────────
        // Compare the current tier to the last-seen tier ordinal. A higher ordinal = tier upgrade.
        // -1 = first ever profile open → write current tier as baseline with no celebration (avoid
        // surprising brand-new users who haven't earned anything yet).
        data.rank?.let { rank ->
            val currentOrdinal = rank.tier.ordinal
            val lastOrdinal = settingsRepo.lastSeenRankTierOrdinal.first()
            if (lastOrdinal == -1) {
                // First ever open — seed the baseline, no celebration.
                settingsRepo.setLastSeenRankTierOrdinal(currentOrdinal)
            } else if (currentOrdinal > lastOrdinal) {
                // Tier crossed upward — celebrate, then advance the stored baseline.
                _showRankUpCelebration.value = true
                settingsRepo.setLastSeenRankTierOrdinal(currentOrdinal)
            }
        }
    }

    private fun buildState(
        data: ProfileData,
        name: String,
        photos: List<ProgressPhoto>,
        hasAvatar: Boolean,
        avatarStamp: Long
    ) = ProfileUiState(
        loading = false,
        name = name,
        sinceLabel = data.sinceLabel,
        rank = data.rank,
        xp = data.xp,
        totalSessions = data.totalSessions,
        totalVolumeLb = data.totalVolumeLb,
        totalPrs = data.totalPrs,
        streakDays = data.streakDays,
        longestStreakDays = data.longestStreakDays,
        standings = data.standings,
        topLift = data.topLift,
        mostLoggedDay = data.mostLoggedDay,
        usualHour = data.usualHour,
        photos = photos,
        hasAvatar = hasAvatar,
        avatarStamp = avatarStamp,
        trophyUnlocked = data.trophyUnlocked,
        trophyTotal = data.trophyTotal,
        trophyGrid = data.trophyGrid,
        closestTrophy = data.closestTrophy,
        memory = data.memory,
        recaps = data.recaps
    )

    /** Called by the UI after the one-shot celebration has played so it never replays on recompose. */
    fun clearRankUpCelebration() { _showRankUpCelebration.value = false }

    /** Inline rename from the profile header — persists to prefs and reflects immediately. */
    fun setUserName(name: String) = viewModelScope.launch {
        val trimmed = name.trim()
        settingsRepo.setUserName(trimmed)
        _state.value = _state.value.copy(name = trimmed)
    }

    /** Save a caption for a progress photo (edited in the viewer dialog). */
    fun setPhotoNote(photo: ProgressPhoto, note: String) = viewModelScope.launch {
        val trimmed = note.trim()
        photoRepo.setNote(photo, trimmed)
        // Patch the one edited caption in place rather than re-reading + re-decoding the whole index.
        _state.value = _state.value.copy(
            photos = _state.value.photos.map { if (it.fileName == photo.fileName) it.copy(note = trimmed) else it }
        )
    }

    fun fileFor(photo: ProgressPhoto) = photoRepo.fileFor(photo)

    fun addPhoto(uri: Uri) = viewModelScope.launch {
        photoRepo.add(uri, System.currentTimeMillis())
        _state.value = _state.value.copy(photos = photoRepo.photos())
    }

    fun deletePhoto(photo: ProgressPhoto) = viewModelScope.launch {
        photoRepo.delete(photo)
        _state.value = _state.value.copy(photos = photoRepo.photos())
    }

    fun avatarFile(): File = avatarRepo.file

    fun setAvatar(uri: Uri) = viewModelScope.launch {
        if (avatarRepo.set(uri)) {
            _state.value = _state.value.copy(hasAvatar = true, avatarStamp = System.currentTimeMillis())
        }
    }
}
