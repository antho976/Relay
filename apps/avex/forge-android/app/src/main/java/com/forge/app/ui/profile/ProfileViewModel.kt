package com.forge.app.ui.profile

import android.net.Uri
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.prefs.SettingsRepository
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

    init { load() }

    private fun load() = viewModelScope.launch {
        val data = profileRepo.load()
        _state.value = ProfileUiState(
            loading = false,
            name = settingsRepo.userName.first(),
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
            photos = photoRepo.photos(),
            hasAvatar = avatarRepo.exists(),
            avatarStamp = if (avatarRepo.exists()) avatarRepo.file.lastModified() else 0L,
            trophyUnlocked = data.trophyUnlocked,
            trophyTotal = data.trophyTotal,
            trophyGrid = data.trophyGrid,
            closestTrophy = data.closestTrophy,
            memory = data.memory,
            recaps = data.recaps
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
