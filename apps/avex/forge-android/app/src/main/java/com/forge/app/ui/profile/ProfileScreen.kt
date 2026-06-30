package com.forge.app.ui.profile

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Share
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.Features
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.ui.common.ConfettiOverlay
import com.forge.app.ui.common.FirstTouchTip
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.gym.stats.components.statsEntrance
import com.forge.app.ui.theme.LocalForgeSettings
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * The "You" hub: an identity card (avatar · name · streak), a dashboard of stat tiles, signature
 * lifts, goals-as-rings, the private gallery and an on-this-day throwback. The rank ladder, offline
 * standing and trophy case stay gated behind [Features.SHOW_GAMIFICATION]. All local — no account.
 * See [ProfileViewModel] / `data/repo/ProfileRepository` / `domain/rank`.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ProfileScreen(
    // Null when shown as a hub pager page (no redundant back arrow); a real callback as a deep route.
    onBack: (() -> Unit)? = null,
    onOpenTrophies: () -> Unit,
    onOpenGoals: () -> Unit = {},
    onOpenPhotoGallery: () -> Unit = {},
    viewModel: ProfileViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val showRankUpCelebration by viewModel.showRankUpCelebration.collectAsStateWithLifecycle()
    var viewing by remember { mutableStateOf<ProgressPhoto?>(null) }
    var showXpInfo by remember { mutableStateOf(false) }
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val haptic = LocalHapticFeedback.current

    // One-shot rank-up haptic — fires as soon as the celebration flag goes true (gamification only).
    LaunchedEffect(showRankUpCelebration) {
        if (Features.SHOW_GAMIFICATION && showRankUpCelebration) {
            haptic.performHapticFeedback(HapticFeedbackType.LongPress)
        }
    }

    val photoPicker = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        uri?.let { viewModel.addPhoto(it) }
    }
    fun pickPhoto() = photoPicker.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))

    val avatarPicker = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        uri?.let { viewModel.setAvatar(it) }
    }
    fun pickAvatar() = avatarPicker.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    Box(Modifier.fillMaxSize()) {
        // Soft rank-tier wash bleeding down from the top bar (parked with the rest of the rank UI).
        if (Features.SHOW_GAMIFICATION) state.rank?.let { r ->
            Box(
                Modifier.fillMaxWidth().height(140.dp)
                    .background(Brush.verticalGradient(listOf(r.tier.color().copy(alpha = 0.13f), Color.Transparent)))
            )
        }
        Scaffold(
            topBar = {
                TopAppBar(
                    title = { Text("Profile.", style = MaterialTheme.typography.headlineMedium) },
                    navigationIcon = {
                        if (onBack != null) IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") }
                    },
                    actions = {
                        if (Features.SHOW_GAMIFICATION) state.rank?.let { r ->
                            IconButton(onClick = {
                                scope.launch {
                                    val uri = withContext(Dispatchers.Default) {
                                        RankCardRenderer.render(
                                            context, state.name, r.displayName, r.roman, r.xpTotal, r.tier.colorArgb,
                                            standingLine = state.standings.minByOrNull { it.topPercent }
                                                ?.let { s -> "Top ${s.topPercent}% · ${s.label}" }
                                        )
                                    }
                                    uri?.let { RankCardRenderer.share(context, it) }
                                }
                            }) {
                                Icon(Icons.Filled.Share, contentDescription = "Share rank card", tint = muted)
                            }
                        }
                    },
                    colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
                )
            },
            containerColor = Color.Transparent
        ) { inner ->
            if (state.loading) {
                ProfileSkeleton(Modifier.fillMaxSize().padding(inner).padding(horizontal = 20.dp))
            } else Column(
                Modifier.fillMaxSize().padding(inner).verticalScroll(rememberScrollState()).padding(horizontal = 20.dp)
            ) {
                // ── Identity hero (entrance index 0) ─────────────────────────────
                Column(Modifier.statsEntrance(0)) {
                    ProfileHeaderCard(
                        name = state.name,
                        sinceLabel = state.sinceLabel,
                        streakDays = state.streakDays,
                        hasAvatar = state.hasAvatar,
                        avatarFile = viewModel.avatarFile(),
                        avatarStamp = state.avatarStamp,
                        onSetName = viewModel::setUserName,
                        onPickAvatar = { pickAvatar() },
                        onBg = onBg, muted = muted, accent = accent
                    )
                }

                // First-touch (D1): a brand-new profile is all dashes and empty rings — say what fills it.
                if (state.totalSessions == 0 && !LocalForgeSettings.current.firstWorkoutDone) {
                    Spacer(Modifier.height(20.dp))
                    FirstTouchTip(
                        "Your profile starts with your first set.",
                        "Log a workout and this page fills in — your lifetime totals, signature lifts, goals and progress photos."
                    )
                }

                // ── Rank track (gamification, index 1) ───────────────────────────
                if (Features.SHOW_GAMIFICATION) state.rank?.let { r ->
                    Spacer(Modifier.height(20.dp))
                    Column(Modifier.statsEntrance(1)) {
                        RankSection(r, muted, accent, outline, onInfo = { showXpInfo = true })
                    }
                }

                // ── All-time stat tiles (index 2) ────────────────────────────────
                Spacer(Modifier.height(24.dp))
                Column(Modifier.statsEntrance(2)) {
                    AllTimeTiles(
                        sessions = state.totalSessions,
                        volumeLb = state.totalVolumeLb,
                        prs = state.totalPrs,
                        xp = state.rank?.xpTotal ?: 0L,
                        longestStreakDays = state.longestStreakDays,
                        volumeSeriesLb = state.lifetimeVolumeSeriesLb,
                        onBg = onBg, muted = muted, accent = accent, outline = outline
                    )
                }

                if (Features.SHOW_GAMIFICATION) {
                    Spacer(Modifier.height(24.dp))
                    Column(Modifier.statsEntrance(3)) {
                        StandingSection(state.standings, onBg, muted, accent, outline)
                    }
                }

                // ── Signature (index 4) ──────────────────────────────────────────
                Spacer(Modifier.height(24.dp))
                Column(Modifier.statsEntrance(4)) {
                    SignatureCard(state.topLift, state.mostLoggedDay, state.usualHour, onBg, muted, accent, outline)
                }

                if (state.cardioSessions > 0) {
                    Spacer(Modifier.height(24.dp))
                    Column(Modifier.statsEntrance(5)) {
                        CardioCard(state.cardioSessions, state.cardioMinutes, state.cardioDistanceKm, onBg, muted, accent, outline)
                    }
                }

                // ── Goals as rings (index 6) ─────────────────────────────────────
                Spacer(Modifier.height(24.dp))
                Column(Modifier.statsEntrance(6)) {
                    GoalTilesSection(state.goals, onOpenGoals, onBg, muted, accent, outline)
                }

                // ── Gallery (index 7) ────────────────────────────────────────────
                Spacer(Modifier.height(24.dp))
                Column(Modifier.statsEntrance(7)) {
                    GalleryCard(state.photos, viewModel::fileFor, onAdd = { pickPhoto() }, onView = { viewing = it }, onViewAll = onOpenPhotoGallery, onBg, muted, accent, outline)
                }

                state.memory?.let { m ->
                    Spacer(Modifier.height(24.dp))
                    Column(Modifier.statsEntrance(8)) {
                        OnThisDayCard(m, onBg, muted, accent)
                    }
                }

                if (Features.SHOW_GAMIFICATION) {
                    Spacer(Modifier.height(24.dp))
                    Column(Modifier.statsEntrance(9)) {
                        TrophyCaseSection(state.trophyGrid, state.trophyUnlocked, state.trophyTotal, state.closestTrophy, onOpenTrophies, onBg, muted, accent, outline)
                    }
                }

                Spacer(Modifier.height(40.dp))
            }
        }

        // ── Rank-up celebration overlay (gamification) ───────────────────────────
        if (Features.SHOW_GAMIFICATION && showRankUpCelebration) {
            ConfettiOverlay(
                modifier = Modifier.fillMaxSize(),
                onComplete = { viewModel.clearRankUpCelebration() }
            )
        }
    }

    viewing?.let { photo ->
        PhotoViewerDialog(
            file = viewModel.fileFor(photo),
            takenAtMs = photo.takenAtMs,
            note = photo.note,
            onSaveNote = { viewModel.setPhotoNote(photo, it) },
            onDelete = { viewModel.deletePhoto(photo); viewing = null },
            onDismiss = { viewing = null }
        )
    }

    if (showXpInfo) {
        val r = state.rank
        val xp = state.xp
        if (r != null && xp != null) RankInfoSheet(r, xp, onDismiss = { showXpInfo = false })
    }
}

@Composable
private fun PhotoViewerDialog(
    file: File,
    takenAtMs: Long,
    note: String,
    onSaveNote: (String) -> Unit,
    onDelete: () -> Unit,
    onDismiss: () -> Unit
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val accent = MaterialTheme.colorScheme.primary
    var noteInput by remember(note) { mutableStateOf(note) }
    // Persist the caption when the viewer closes, only if it actually changed.
    fun commit() { if (noteInput.trim() != note) onSaveNote(noteInput.trim()); onDismiss() }
    androidx.compose.ui.window.Dialog(onDismissRequest = { commit() }) {
        Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp))) {
            ProgressPhotoImage(file, Modifier.fillMaxWidth().aspectRatio(0.8f).clip(RoundedCornerShape(12.dp)), reqPx = 1200)
            Spacer(Modifier.height(10.dp))
            BasicTextField(
                value = noteInput,
                onValueChange = { noteInput = it.take(140) },
                textStyle = MaterialTheme.typography.bodySmall.copy(color = onBg),
                cursorBrush = SolidColor(accent),
                decorationBox = { inner ->
                    Box {
                        if (noteInput.isEmpty()) Text(
                            "Add a note…",
                            style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.5f), fontStyle = FontStyle.Italic
                        )
                        inner()
                    }
                },
                modifier = Modifier.fillMaxWidth()
            )
            Spacer(Modifier.height(10.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                Text(
                    SimpleDateFormat("MMM d, yyyy", Locale.getDefault()).format(Date(takenAtMs)),
                    style = MaterialTheme.typography.labelSmall, color = muted
                )
                Text(
                    "delete", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.bounceClick { onDelete() }.padding(8.dp)
                )
            }
            Spacer(Modifier.height(2.dp))
            HorizontalDivider(color = Color.Transparent)
        }
    }
}
