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
        profileRepo.cached()?.let { _state.value = buildState(it, name, photos, hasAvatar, avatarStamp) }
        val data = profileRepo.load()
        _state.value = buildState(data, name, photos, hasAvatar, avatarStamp)

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
