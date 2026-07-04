package com.forge.app.ui.profile

import android.net.Uri
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.BodyweightRepository
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.data.repo.GoalRepository
import com.forge.app.data.repo.ProfileData
import com.forge.app.data.repo.ProfileRepository
import com.forge.app.data.repo.AvatarRepository
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.data.repo.ProgressPhotoRepository
import com.forge.app.ui.profile.state.ProfileUiState
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.io.File
import javax.inject.Inject

/**
 * The "You" hub (profile). Pure local — no account, no server. A thin mapper: [ProfileRepository]
 * does the snapshot fan-out + runs the rank/XP/standing engines; this VM layers on the live name,
 * progress photos and the bodyweight log (moved here from Stats — your body lives on your profile),
 * and owns the photo + weigh-in mutations.
 */
@HiltViewModel
class ProfileViewModel @Inject constructor(
    private val profileRepo: ProfileRepository,
    private val settingsRepo: SettingsRepository,
    private val photoRepo: ProgressPhotoRepository,
    private val avatarRepo: AvatarRepository,
    private val goalRepo: GoalRepository,
    private val extendedGoalRepo: ExtendedGoalRepository,
    private val bodyweightRepo: BodyweightRepository
) : ViewModel() {

    private val _state = MutableStateFlow(ProfileUiState())
    val state: StateFlow<ProfileUiState> = _state.asStateFlow()

    // ── Bodyweight (the BODYWEIGHT section + its quick-log sheet) ─────────────────

    /** Recent weigh-ins, oldest → newest (the DAO emits newest-first) — feeds the trend sparkline. */
    val bodyweight: StateFlow<List<BodyweightEntry>> =
        bodyweightRepo.observeRecent(90)
            .map { entries -> entries.sortedBy { it.recordedAt } }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    /** Whether the quick-log should offer "Import from Health Connect" (read granted). */
    private val _weightConnected = MutableStateFlow(false)
    val weightConnected: StateFlow<Boolean> = _weightConnected.asStateFlow()

    /** Transient confirmation/result line for the quick-log sheet; cleared when the sheet closes. */
    private val _bodyweightMessage = MutableStateFlow<String?>(null)
    val bodyweightMessage: StateFlow<String?> = _bodyweightMessage.asStateFlow()

    /** Save a typed weigh-in (lb); the trend updates reactively via observeRecent. */
    fun logBodyweight(weightLb: Double) = viewModelScope.launch {
        bodyweightRepo.log(weightLb)
        _bodyweightMessage.value = "Saved."
    }

    /** Pull the latest weight from Health Connect into the log (no-op if nothing newer). */
    fun importBodyweight() = viewModelScope.launch {
        val imported = bodyweightRepo.importLatestFromHealthConnect()
        _bodyweightMessage.value =
            if (imported != null) "Imported your latest weight." else "No newer weight in Health Connect."
    }

    fun clearBodyweightMessage() { _bodyweightMessage.value = null }

    /** Re-check HC read permission right before showing the quick-log sheet, so a grant made in
     *  Settings after this screen opened surfaces the import option without recreating the VM. */
    fun refreshWeightConnected() = viewModelScope.launch {
        _weightConnected.value = runCatching { bodyweightRepo.canImportFromHealthConnect() }.getOrDefault(false)
    }

    /** True on this profile open iff the user just crossed into a higher tier since last visit. */
    private val _showRankUpCelebration = MutableStateFlow(false)
    val showRankUpCelebration: StateFlow<Boolean> = _showRankUpCelebration.asStateFlow()

    init { load() }

    private fun load() = viewModelScope.launch {
        val name = settingsRepo.userName.first()
        val photos = photoRepo.photos()
        val hasAvatar = avatarRepo.exists()
        val avatarStamp = if (hasAvatar) avatarRepo.file.lastModified() else 0L
        // All goals (sorted achieved-first / closest-first); the Profile previews the top few. Lift
        // targets and auto-tracked custom goals both feed the profile goal box.
        val goals = runCatching { goalRepo.goalsWithProgress() }.getOrDefault(emptyList())
        val customGoals = runCatching { extendedGoalRepo.goalsWithProgress() }.getOrDefault(emptyList())
        // Instant first paint on re-entry: render the last-assembled data while the fresh fan-out runs (P3).
        val cached = profileRepo.cached()
        _state.value = if (cached != null) buildState(cached, name, photos, hasAvatar, avatarStamp, goals, customGoals)
            else _state.value.copy(name = name, photos = photos, hasAvatar = hasAvatar, avatarStamp = avatarStamp, goals = goals, customGoals = customGoals)
        val data = profileRepo.load()
        // Merge the fresh fan-out but keep the user-editable fields from current state, so a rename /
        // photo-note / avatar change made while the (slow) fan-out ran isn't reverted by the pre-load
        // snapshot — the edit fns persist then update _state, so reading them back here is correct.
        _state.value = buildState(data, _state.value.name, _state.value.photos, _state.value.hasAvatar, _state.value.avatarStamp, goals, customGoals)

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
        avatarStamp: Long,
        goals: List<GoalRepository.GoalProgress>,
        customGoals: List<ExtendedGoalRepository.Progress>
    ) = ProfileUiState(
        loading = false,
        name = name,
        sinceLabel = data.sinceLabel,
        rank = data.rank,
        xp = data.xp,
        totalSessions = data.totalSessions,
        totalVolumeLb = data.totalVolumeLb,
        totalPrs = data.totalPrs,
        totalSets = data.totalSets,
        streakDays = data.streakDays,
        longestStreakDays = data.longestStreakDays,
        workoutsThisWeek = data.workoutsThisWeek,
        workoutsLastWeek = data.workoutsLastWeek,
        setsThisWeek = data.setsThisWeek,
        setsLastWeek = data.setsLastWeek,
        prsThisWeek = data.prsThisWeek,
        prsLastWeek = data.prsLastWeek,
        standings = data.standings,
        topLift = data.topLift,
        mostLoggedDay = data.mostLoggedDay,
        usualHour = data.usualHour,
        goals = goals,
        customGoals = customGoals,
        photos = photos,
        hasAvatar = hasAvatar,
        avatarStamp = avatarStamp,
        trophyUnlocked = data.trophyUnlocked,
        trophyTotal = data.trophyTotal,
        trophyGrid = data.trophyGrid,
        closestTrophy = data.closestTrophy,
        memory = data.memory,
        cardioSessions = data.cardioSessions,
        cardioMinutes = data.cardioMinutes,
        cardioDistanceKm = data.cardioDistanceKm
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
