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
import com.forge.app.ui.common.ForgeWordmark
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.gym.stats.components.statsEntrance
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * The "You" hub: an identity card (avatar · name · streak), a dashboard of stat tiles, signature
 * lifts, the private gallery and an on-this-day throwback. The rank ladder, offline
 * standing and trophy case stay gated behind [Features.SHOW_GAMIFICATION]. All local — no account.
 * See [ProfileViewModel] / `data/repo/ProfileRepository` / `domain/rank`.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ProfileScreen(
    // Null when shown as a hub pager page (no redundant back arrow); a real callback as a deep route.
    onBack: (() -> Unit)? = null,
    onOpenTrophies: () -> Unit,
    onOpenPhotoGallery: () -> Unit = {},
    onOpenCamera: () -> Unit = {},
    onOpenMeasurements: () -> Unit = {},
    viewModel: ProfileViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val showRankUpCelebration by viewModel.showRankUpCelebration.collectAsStateWithLifecycle()
    val bodyweight by viewModel.bodyweight.collectAsStateWithLifecycle()
    val bodyweightGoalLb by viewModel.bodyweightGoalLb.collectAsStateWithLifecycle()
    val weightConnected by viewModel.weightConnected.collectAsStateWithLifecycle()
    val bodyweightMessage by viewModel.bodyweightMessage.collectAsStateWithLifecycle()
    var viewing by remember { mutableStateOf<ProgressPhoto?>(null) }
    var showXpInfo by remember { mutableStateOf(false) }
    var showWeightSheet by remember { mutableStateOf(false) }
    var showAvatarSheet by remember { mutableStateOf(false) }
    var addChooser by remember { mutableStateOf(false) }

    // Persist the one-time edit hint as soon as it surfaces — it stays visible this session, gone next.
    LaunchedEffect(state.showAvatarHint) { if (state.showAvatarHint) viewModel.markAvatarHintSeen() }
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
                    // §2: wordmark in the chrome — the bumped avatar + name hero below stays the page's identity.
                    title = { ForgeWordmark() },
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
                ProfileSkeleton(
                    Modifier.fillMaxSize().padding(bottom = inner.calculateBottomPadding()),
                    topInset = inner.calculateTopPadding()
                )
            } else Column(
                // Don't apply the TOP inset — the cover banner draws to the very top of the screen
                // (behind the status bar), filling the whole page. Only the bottom bar is cleared.
                Modifier.fillMaxSize().padding(bottom = inner.calculateBottomPadding()).verticalScroll(rememberScrollState())
            ) {
                // ── Full-bleed identity banner (entrance index 0) — the profile photo as a cover ──
                Box(Modifier.statsEntrance(0)) {
                    ProfileHeaderCard(
                        name = state.name,
                        sinceLabel = state.sinceLabel,
                        streakDays = state.streakDays,
                        longestStreakDays = state.longestStreakDays,
                        hasAvatar = state.hasAvatar,
                        avatarFile = viewModel.avatarFile(),
                        avatarStamp = state.avatarStamp,
                        onSetName = viewModel::setUserName,
                        // GYMAP-22: the cover tap now opens the photo picker sheet (own photo + provided
                        // defaults), not the system picker directly; dismiss the one-time hint on tap.
                        onPickAvatar = { viewModel.dismissAvatarHint(); showAvatarSheet = true },
                        onBg = onBg, muted = muted, accent = accent,
                        topInset = inner.calculateTopPadding()
                    )
                }

                // One-time nudge teaching the (now photo-seeded) cover is tappable to change (GYMAP-22).
                if (state.showAvatarHint) {
                    Text(
                        "Tap your photo to change it",
                        style = MaterialTheme.typography.bodySmall,
                        color = muted, fontStyle = FontStyle.Italic,
                        modifier = Modifier.padding(horizontal = 20.dp).padding(top = 8.dp)
                    )
                }

                // Sections sit openly on the page. Each applies the side margins itself so the
                // gallery filmstrip can break out and run edge-to-edge like the cover above.
                val pad = Modifier.fillMaxWidth().padding(horizontal = 20.dp)

                // ── Rank track (gamification, index 1) ───────────────────────────
                if (Features.SHOW_GAMIFICATION) state.rank?.let { r ->
                    Spacer(Modifier.height(20.dp))
                    Column(pad.statsEntrance(1)) {
                        RankSection(r, muted, accent, outline, onInfo = { showXpInfo = true })
                    }
                }

                // ── All-time figures → bodyweight → lifetime volume curve (index 2) ─
                //    Bodyweight sits between the tallies and the curve (Antho 2026-07-03).
                Spacer(Modifier.height(28.dp))
                Column(pad.statsEntrance(2)) {
                    AllTimeSection(
                        sessions = state.totalSessions,
                        volumeLb = state.totalVolumeLb,
                        prs = state.totalPrs,
                        sets = state.totalSets,
                        xp = state.rank?.xpTotal ?: 0L,
                        workoutsDelta = state.workoutsThisWeek - state.workoutsLastWeek,
                        setsDelta = state.setsThisWeek - state.setsLastWeek,
                        prsDelta = state.prsThisWeek - state.prsLastWeek,
                        onBg = onBg, muted = muted, accent = accent
                    )
                    Spacer(Modifier.height(28.dp))
                    BodySection(
                        entries = bodyweight,
                        goalLb = bodyweightGoalLb,
                        onLog = {
                            // Fresh sheet: drop any prior result line and re-check HC permission so a
                            // grant made in Settings since this screen opened surfaces the import option.
                            viewModel.clearBodyweightMessage()
                            viewModel.refreshWeightConnected()
                            showWeightSheet = true
                        },
                        onBg = onBg, muted = muted, accent = accent
                    )
                    if (state.lifetimeVolumeSeriesLb.size >= 2) {
                        Spacer(Modifier.height(28.dp))
                        LifetimeVolumeGraph(state.lifetimeVolumeSeriesLb, muted, accent)
                    }
                }

                // ── This year's consistency (gym + cardio) — a whole-calendar-year glance, a
                //    DIFFERENT mark from the Stats 26-week load heatmap (§4.3). Hidden until the
                //    year has any activity, so a new user never sees a dead grid (§12).
                if (state.activityByDay.isNotEmpty()) {
                    Spacer(Modifier.height(28.dp))
                    Column(pad.statsEntrance(3)) {
                        YearConsistencySection(state.activityByDay, muted, accent)
                    }
                }

                // ── Body measurements (GYMAP-52) — sits in the "your body" cluster, next to
                //    bodyweight; its own hiltViewModel so ProfileViewModel stays untouched.
                Spacer(Modifier.height(28.dp))
                Column(pad.statsEntrance(3)) {
                    MeasurementsHubCard(onOpen = onOpenMeasurements)
                }

                if (Features.SHOW_GAMIFICATION) {
                    Spacer(Modifier.height(28.dp))
                    Column(pad.statsEntrance(3)) {
                        StandingSection(state.standings, onBg, muted, accent, outline)
                    }
                }

                // Signature + Cardio sections removed 2026-07-03 (Antho), Goals too (they live on
                // Home and the Goals screen) — the profile keeps the all-time figures + bodyweight
                // + lifetime-volume graph, gallery and on-this-day.

                // ── Gallery filmstrip (index 4) — full-bleed, pads itself ────────
                Spacer(Modifier.height(28.dp))
                Column(Modifier.fillMaxWidth().statsEntrance(4)) {
                    GalleryStrip(state.photos, viewModel::fileFor, onAdd = { addChooser = true }, onView = { viewing = it }, onViewAll = onOpenPhotoGallery, onBg, muted, outline)
                }

                state.memory?.let { m ->
                    Spacer(Modifier.height(28.dp))
                    Column(pad.statsEntrance(5)) {
                        OnThisDaySection(m, onBg, muted, accent)
                    }
                }

                if (Features.SHOW_GAMIFICATION) {
                    Spacer(Modifier.height(28.dp))
                    Column(pad.statsEntrance(6)) {
                        TrophyCaseSection(state.trophyGrid, state.trophyUnlocked, state.trophyTotal, state.closestTrophy, onOpenTrophies, onBg, muted, accent, outline)
                    }
                }

                Spacer(Modifier.height(44.dp))
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

    if (showWeightSheet) {
        BodyweightLogSheet(
            entries = bodyweight,
            canImport = weightConnected,
            message = bodyweightMessage,
            onSave = { lb, date, note ->
                viewModel.logBodyweight(lb, date, note)
                showWeightSheet = false
            },
            onImport = { viewModel.importBodyweight() },  // stays open so the result line shows
            onDismiss = {
                showWeightSheet = false
                viewModel.clearBodyweightMessage()
            }
        )
    }

    if (addChooser) {
        AddPhotoChooser(
            onCamera = { addChooser = false; onOpenCamera() },
            onImport = { addChooser = false; pickPhoto() },
            onDismiss = { addChooser = false }
        )
    }

    if (showAvatarSheet) {
        AvatarPickerSheet(
            selectedKey = state.avatarDefaultKey,
            // "Select your own" hands off to the system Photo Picker (the pre-GYMAP-22 behaviour).
            onPickOwn = { showAvatarSheet = false; pickAvatar() },
            onSelectDefault = { showAvatarSheet = false; viewModel.setAvatarFromDefault(it) },
            onDismiss = { showAvatarSheet = false }
        )
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
            Spacer(Modifier.height(4.dp))
        }
    }
}
