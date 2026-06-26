package com.forge.app.ui.profile

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.foundation.background
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
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
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.material.icons.filled.Share
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.platform.LocalContext
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import com.forge.app.Features
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.ui.common.ConfettiOverlay
import com.forge.app.ui.common.FirstTouchTip
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.gym.stats.components.statsEntrance
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * The "You" hub: rank ladder + earned XP, an offline standing estimate, signature lifts, goals,
 * the private mirror-test photos and the trophy case. All local — no account, no server.
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
    var editingName by remember { mutableStateOf(false) }
    var nameInput by remember { mutableStateOf("") }
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val haptic = LocalHapticFeedback.current

    // One-shot rank-up haptic — fires as soon as the celebration flag goes true, then clears.
    // Suppressed while the gamification layer is parked (Features.SHOW_GAMIFICATION).
    LaunchedEffect(showRankUpCelebration) {
        if (Features.SHOW_GAMIFICATION && showRankUpCelebration) {
            haptic.performHapticFeedback(HapticFeedbackType.LongPress)
        }
    }

    val photoPicker = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        uri?.let { viewModel.addPhoto(it) }
    }
    fun pickPhoto() = photoPicker.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    Box(Modifier.fillMaxSize()) {
        // Soft rank-tier wash bleeding down from the top bar — ties the screen to the current tier.
        // Parked with the rest of the rank UI behind Features.SHOW_GAMIFICATION.
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
                // Shimmer placeholder instead of the empty-default flash while load() runs (#8).
                ProfileSkeleton(Modifier.fillMaxSize().padding(inner).padding(horizontal = 24.dp))
            } else
            Column(
                Modifier.fillMaxSize().padding(inner).verticalScroll(rememberScrollState()).padding(horizontal = 24.dp)
            ) {
                // ── Header (entrance index 0) ────────────────────────────────────
                // The header block (name + "since" line + rank line) enters together as the first item.
                Column(Modifier.statsEntrance(0)) {
                    if (editingName) {
                        val nameFocus = remember { FocusRequester() }
                        LaunchedEffect(Unit) { nameFocus.requestFocus() }
                        // Commit on Done OR on focus loss (tap-away / keyboard dismiss / leaving the
                        // screen) so a typed name is never silently lost. Blank input is ignored — it
                        // would otherwise wipe the name to the "Athlete" placeholder.
                        fun commitName() {
                            val trimmed = nameInput.trim()
                            if (trimmed.isNotEmpty() && trimmed != state.name) viewModel.setUserName(trimmed)
                            editingName = false
                        }
                        BasicTextField(
                            value = nameInput,
                            onValueChange = { nameInput = it.take(30) },
                            singleLine = true,
                            textStyle = MaterialTheme.typography.displaySmall.copy(color = onBg),
                            cursorBrush = SolidColor(accent),
                            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                            keyboardActions = KeyboardActions(onDone = { commitName() }),
                            modifier = Modifier.fillMaxWidth().focusRequester(nameFocus)
                                .onFocusChanged { if (!it.isFocused && editingName) commitName() }
                        )
                    } else {
                        Text(
                            state.name.ifBlank { "Athlete" },
                            style = MaterialTheme.typography.displaySmall, color = onBg,
                            modifier = Modifier.bounceClick { nameInput = state.name; editingName = true }
                        )
                    }
                    // Member-since line, now directly under the name.
                    if (state.sinceLabel.isNotBlank()) {
                        Spacer(Modifier.height(4.dp))
                        Text(
                            "SINCE ${state.sinceLabel}",
                            style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp
                        )
                    }
                    // Rank line parked behind Features.SHOW_GAMIFICATION; a streak line stands in for it.
                    if (Features.SHOW_GAMIFICATION) state.rank?.let { r ->
                        Spacer(Modifier.height(2.dp))
                        val streak = if (state.streakDays >= 2) " · ${state.streakDays}-day streak, still alive." else ""
                        Text(
                            "Rank ${r.roman} — ${r.tier.display}$streak",
                            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
                        )
                    } else if (state.streakDays >= 2) {
                        Spacer(Modifier.height(2.dp))
                        Text(
                            "${state.streakDays}-day streak, still alive.",
                            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
                        )
                    }
                }

                // First-touch (D1): a brand-new profile is all dashes and empty bars — say what fills it in.
                // Persistent flag too, so a returning user (e.g. after a data wipe) isn't told they're new.
                if (state.totalSessions == 0 && !LocalForgeSettings.current.firstWorkoutDone) {
                    Spacer(Modifier.height(20.dp))
                    FirstTouchTip(
                        "Your profile starts with your first set.",
                        "Log a workout and this page fills in — your lifetime totals, signature lifts, goals and progress photos."
                    )
                }

                // ── Rank track (index 1 — already has its own internal enter animation) ──
                // Wrapped in statsEntrance so the whole block slides in, complementing the
                // internal emblem scale/alpha. The two animations compound tastefully.
                if (Features.SHOW_GAMIFICATION) state.rank?.let { r ->
                    Spacer(Modifier.height(20.dp))
                    Column(Modifier.statsEntrance(1)) {
                        RankSection(r, muted, accent, outline, onInfo = { showXpInfo = true })
                    }
                }

                // ── Sections (indices 2 onwards, top-to-bottom) ──────────────────
                // These sections (via ProfileBlock) emit several stacked siblings — divider, label,
                // body — so each entrance wrapper MUST be a Column, not a Box. A Box would pile the
                // siblings at the same top-start corner and they'd overlap.
                Column(Modifier.statsEntrance(2)) {
                    LedgerSection(state.totalSessions, state.totalVolumeLb, state.totalPrs, state.rank?.xpTotal ?: 0L, muted, accent, outline, state.longestStreakDays, state.lifetimeVolumeSeriesLb)
                }
                if (Features.SHOW_GAMIFICATION) {
                    Column(Modifier.statsEntrance(3)) {
                        StandingSection(state.standings, onBg, muted, accent, outline)
                    }
                }
                Column(Modifier.statsEntrance(4)) {
                    SignatureSection(state.topLift, state.mostLoggedDay, state.usualHour, onBg, muted, accent, outline)
                }
                if (state.cardioSessions > 0) {
                    Column(Modifier.statsEntrance(5)) {
                        CardioTotalsSection(state.cardioSessions, state.cardioMinutes, state.cardioDistanceKm, muted, accent, outline)
                    }
                }
                // Goals sit above the mirror test, previewing the top few with progress.
                Column(Modifier.statsEntrance(6)) {
                    GoalsPreviewSection(state.goals, onOpenGoals, onBg, muted, accent, outline)
                }

                Column(Modifier.statsEntrance(7)) {
                    MirrorTestSection(state.photos, viewModel::fileFor, onAdd = { pickPhoto() }, onView = { viewing = it }, onViewAll = onOpenPhotoGallery, onBg, muted, accent, outline)
                }

                state.memory?.let { m ->
                    Column(Modifier.statsEntrance(8)) {
                        ProfileBlock("ON THIS DAY", muted, accent, outline) {
                            val useKg = LocalForgeSettings.current.useKg
                            val ago = com.forge.app.ui.common.monthsAgoPhrase(m.monthsAgo)
                            Text(
                                "$ago you trained ${m.dayName} — ${formatVolume(m.totalVolumeLb, useKg)} ${unitLabel(useKg)}" +
                                    if (m.prCount > 0) " · ${m.prCount} PR${if (m.prCount == 1) "" else "s"}" else "",
                                style = MaterialTheme.typography.bodyMedium, color = onBg
                            )
                        }
                    }
                }

                if (Features.SHOW_GAMIFICATION) {
                    Column(Modifier.statsEntrance(9)) {
                        TrophyCaseSection(state.trophyGrid, state.trophyUnlocked, state.trophyTotal, state.closestTrophy, onOpenTrophies, onBg, muted, accent, outline)
                    }
                }

                Spacer(Modifier.height(40.dp))
            }
        }

        // ── Rank-up celebration overlay (Item 2) ─────────────────────────────
        // One-shot confetti burst on the first profile open after the user crosses into a new tier.
        // Drawn above the Scaffold so it covers the whole screen — touches pass through (Canvas has
        // no pointer-input). Clears itself after the animation completes via onComplete.
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
    Dialog(onDismissRequest = { commit() }) {
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
