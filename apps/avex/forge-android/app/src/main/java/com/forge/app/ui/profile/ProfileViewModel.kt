package com.forge.app.ui.profile

import android.net.Uri
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.entities.BodyFatEntry
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.BodyFatRepository
import com.forge.app.data.repo.BodyweightRepository
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.data.repo.ProfileData
import com.forge.app.data.repo.ProfileRepository
import com.forge.app.data.repo.AvatarRepository
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.data.repo.ProgressPhotoRepository
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.goal.parseGoalType
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
import java.time.LocalDate
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
    private val bodyweightRepo: BodyweightRepository,
    private val bodyFatRepo: BodyFatRepository,
    private val extendedGoalRepo: ExtendedGoalRepository
) : ViewModel() {

    private val _state = MutableStateFlow(ProfileUiState())
    val state: StateFlow<ProfileUiState> = _state.asStateFlow()

    // ── Bodyweight (the BODYWEIGHT section + its quick-log sheet) ─────────────────

    /** Recent weigh-ins, oldest → newest (the DAO emits newest-first) — feeds the trend sparkline. */
    val bodyweight: StateFlow<List<BodyweightEntry>> =
        bodyweightRepo.observeRecent(90)
            .map { entries -> entries.sortedBy { it.recordedAt } }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    /** The active bodyweight goal target in lb (canonical), or null when none is set — feeds the
     *  chart's dashed target line. Reads just the goal's target; progress is computed elsewhere. */
    val bodyweightGoalLb: StateFlow<Double?> =
        extendedGoalRepo.observeAll()
            .map { goals ->
                goals.firstNotNullOfOrNull { g ->
                    if (parseGoalType(g.goalType)?.metric == GoalMetric.BODYWEIGHT) g.targetValue else null
                }
            }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)

    /** Whether the quick-log should offer "Import from Health Connect" (read granted). */
    private val _weightConnected = MutableStateFlow(false)
    val weightConnected: StateFlow<Boolean> = _weightConnected.asStateFlow()

    /** Transient confirmation/result line for the quick-log sheet; cleared when the sheet closes. */
    private val _bodyweightMessage = MutableStateFlow<String?>(null)
    val bodyweightMessage: StateFlow<String?> = _bodyweightMessage.asStateFlow()

    /** Save a typed weigh-in (lb) for [date] with an optional [note]; the trend updates reactively. */
    fun logBodyweight(weightLb: Double, date: LocalDate, note: String?) = viewModelScope.launch {
        bodyweightRepo.log(weightLb, date, note)
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

    // ── Body fat % (the BODY FAT section + its quick-log sheet, GYMAP-62) ─────────

    /** Recent body-fat readings, oldest → newest (the DAO emits newest-first) — feeds the sparkline. */
    val bodyFat: StateFlow<List<BodyFatEntry>> =
        bodyFatRepo.observeRecent(90)
            .map { entries -> entries.sortedBy { it.recordedAt } }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    /** Whether the quick-log should offer "Import from Health Connect" (body-fat read granted). */
    private val _bodyFatConnected = MutableStateFlow(false)
    val bodyFatConnected: StateFlow<Boolean> = _bodyFatConnected.asStateFlow()

    /** Transient confirmation/result line for the body-fat quick-log sheet; cleared when it closes. */
    private val _bodyFatMessage = MutableStateFlow<String?>(null)
    val bodyFatMessage: StateFlow<String?> = _bodyFatMessage.asStateFlow()

    /** Save a typed body-fat reading (%) for [date]; the trend updates reactively. */
    fun logBodyFat(percent: Double, date: LocalDate) = viewModelScope.launch {
        bodyFatRepo.log(percent, date)
        _bodyFatMessage.value = "Saved."
    }

    /** Pull the latest body fat from Health Connect into the log (no-op if nothing newer). */
    fun importBodyFat() = viewModelScope.launch {
        val imported = bodyFatRepo.importLatestFromHealthConnect()
        _bodyFatMessage.value =
            if (imported != null) "Imported your latest body fat." else "No newer body fat in Health Connect."
    }

    fun clearBodyFatMessage() { _bodyFatMessage.value = null }

    /** Re-check body-fat HC read permission right before showing the quick-log sheet (see above). */
    fun refreshBodyFatConnected() = viewModelScope.launch {
        _bodyFatConnected.value = runCatching { bodyFatRepo.canImportFromHealthConnect() }.getOrDefault(false)
    }

    /** True on this profile open iff the user just crossed into a higher tier since last visit. */
    private val _showRankUpCelebration = MutableStateFlow(false)
    val showRankUpCelebration: StateFlow<Boolean> = _showRankUpCelebration.asStateFlow()

    init {
        load()
        // Keep the filmstrip fresh when the photo store changes elsewhere — most importantly a shot
        // taken in the in-app camera (a separate screen), but also edits made in the full gallery.
        // Collect from the start (no drop): the replayed initial revision costs one redundant photos()
        // read that matches load()'s, but dropping it risks losing a bump that lands between load()'s
        // read and this collector subscribing (that bump would BE the replayed value we'd skip).
        viewModelScope.launch {
            photoRepo.revision.collect { _state.value = _state.value.copy(photos = photoRepo.photos()) }
        }
    }

    private fun load() = viewModelScope.launch {
        val name = settingsRepo.userName.first()
        val photos = photoRepo.photos()

        // ── Avatar (GYMAP-22) ──────────────────────────────────────────────────
        // AvatarRepository owns the file + its paired pref: ensureSeeded assigns a random default the
        // first time no avatar exists (one-shot, guarded), so the identity cover is never empty and the
        // ringed default stays in sync with the cover on disk.
        val defaultKey = avatarRepo.ensureSeeded(DefaultAvatars.all.map { it.key to it.resId })
        val hasAvatar = avatarRepo.exists()
        val avatarStamp = if (hasAvatar) avatarRepo.file.lastModified() else 0L
        // The one-time "tap to change" hint teaches the tap (the old "tap to add a photo" placeholder is
        // gone now a default is always present); shows only over an un-personalised default, once.
        val showHint = defaultKey != null && !settingsRepo.avatarEditHintShown.first()

        // Instant first paint on re-entry: render the last-assembled data while the fresh fan-out runs (P3).
        val cached = profileRepo.cached()
        _state.value = if (cached != null) buildState(cached, name, photos, hasAvatar, avatarStamp, defaultKey, showHint)
            else _state.value.copy(
                name = name, photos = photos, hasAvatar = hasAvatar, avatarStamp = avatarStamp,
                avatarDefaultKey = defaultKey, showAvatarHint = showHint
            )
        val data = profileRepo.load()
        // Merge the fresh fan-out but keep the user-editable fields from current state, so a rename /
        // photo-note / avatar change made while the (slow) fan-out ran isn't reverted by the pre-load
        // snapshot — the edit fns persist then update _state, so reading them back here is correct.
        _state.value = buildState(
            data, _state.value.name, _state.value.photos, _state.value.hasAvatar, _state.value.avatarStamp,
            _state.value.avatarDefaultKey, _state.value.showAvatarHint
        )

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
        avatarDefaultKey: String?,
        showAvatarHint: Boolean
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
        photos = photos,
        hasAvatar = hasAvatar,
        avatarStamp = avatarStamp,
        avatarDefaultKey = avatarDefaultKey,
        showAvatarHint = showAvatarHint,
        trophyUnlocked = data.trophyUnlocked,
        trophyTotal = data.trophyTotal,
        trophyGrid = data.trophyGrid,
        closestTrophy = data.closestTrophy,
        lifetimeVolumeSeriesLb = data.lifetimeVolumeSeriesLb,
        activityByDay = data.activityByDay
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
        // Dated by EXIF capture time inside the repo; the revision collector refreshes the strip.
        photoRepo.add(uri)
    }

    fun deletePhoto(photo: ProgressPhoto) = viewModelScope.launch {
        photoRepo.delete(photo)
        _state.value = _state.value.copy(photos = photoRepo.photos())
    }

    fun avatarFile(): File = avatarRepo.file

    /** The user's own photo (system Photo Picker) — clears any active default and the edit hint. */
    fun setAvatar(uri: Uri) = viewModelScope.launch {
        if (avatarRepo.adoptOwn(uri)) {
            _state.value = _state.value.copy(
                hasAvatar = true, avatarStamp = System.currentTimeMillis(),
                avatarDefaultKey = null, showAvatarHint = false
            )
        }
    }

    /** Adopt one of the app's bundled default covers (GYMAP-22) — baked into avatar.jpg. */
    fun setAvatarFromDefault(avatar: DefaultAvatars.Item) = viewModelScope.launch {
        if (avatarRepo.adoptDefault(avatar.key, avatar.resId)) {
            _state.value = _state.value.copy(
                hasAvatar = true, avatarStamp = System.currentTimeMillis(),
                avatarDefaultKey = avatar.key, showAvatarHint = false
            )
        }
    }

    /** Persist that the one-time edit hint has been seen — it stays visible this session, gone next. */
    fun markAvatarHintSeen() = viewModelScope.launch { settingsRepo.setAvatarEditHintShown() }

    /** Hide the edit hint now (the user opened the picker). */
    fun dismissAvatarHint() { _state.value = _state.value.copy(showAvatarHint = false) }
}
