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
import androidx.compose.material.icons.filled.Settings
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
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.statsEntrance
import com.forge.app.ui.experiment.SectionAnchor
import com.forge.app.ui.experiment.SurfaceCard
import com.forge.app.ui.experiment.surfacePalette
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * # Profile — design/surface-experiment (2026-08-15)
 *
 * The "You" hub in the card-led language, built to sit UNDER the blending cover rather than beside
 * it. The cover ([ProfileHeaderCard]) is byte-for-byte the shipped one: it is the piece of the
 * current design that already has the warmth this branch is chasing, so nothing here competes with
 * it — no second photo, no gradient, no second elevation, and a full 28dp of air before the first
 * card so the dissolve lands on bare page.
 *
 * The open-editorial original is at `.design-backups/editorial-2026-08/src/profile/`; one command
 * restores it. Every shipped section (`AllTimeSection`, `BodyMetricsSection`, `LifetimeVolumeGraph`,
 * `SectionHeader`, `ChartCaption`) is still in the package, untouched and simply no longer called.
 *
 * All local — no account. See [ProfileViewModel] / `data/repo/ProfileRepository` / `domain/rank`.
 */

/** The page gutter (§7). Sections apply it themselves so the strips can break out full-bleed. */
private val GUTTER = 24.dp

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ProfileScreen(
    // Null when shown as a hub pager page (no redundant back arrow); a real callback as a deep route.
    onBack: (() -> Unit)? = null,
    /** Settings moved here from the Home top bar (2026-07-27) — this page's one action (§4.6). */
    onOpenSettings: () -> Unit = {},
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
    val bodyFat by viewModel.bodyFat.collectAsStateWithLifecycle()
    val bodyFatConnected by viewModel.bodyFatConnected.collectAsStateWithLifecycle()
    val bodyFatMessage by viewModel.bodyFatMessage.collectAsStateWithLifecycle()
    var viewing by remember { mutableStateOf<ProgressPhoto?>(null) }
    var showXpInfo by remember { mutableStateOf(false) }
    var showWeightSheet by remember { mutableStateOf(false) }
    var showBodyFatSheet by remember { mutableStateOf(false) }
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
    val palette = surfacePalette()

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
                    // §4.6: never the screen's own name. The cover's bumped name below is the identity.
                    title = {},
                    navigationIcon = {
                        if (onBack != null) IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") }
                    },
                    actions = {
                        IconButton(onClick = onOpenSettings) {
                            Icon(Icons.Filled.Settings, contentDescription = "Settings", tint = muted)
                        }
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
                // ── The blending cover — UNTOUCHED (entrance index 0) ────────────
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
                        onPickAvatar = { viewModel.dismissAvatarHint(); showAvatarSheet = true },
                        onBg = onBg, muted = muted, accent = accent,
                        topInset = inner.calculateTopPadding()
                    )
                }

                // One-time nudge teaching the cover is tappable to change (GYMAP-22). Left OFF a
                // card on purpose: it is transient chrome, and boxing it would promise a tap it
                // does not have.
                if (state.showAvatarHint) {
                    Text(
                        "Tap your photo to change it",
                        style = MaterialTheme.typography.bodySmall,
                        color = muted, fontStyle = FontStyle.Italic,
                        modifier = Modifier.padding(horizontal = GUTTER).padding(top = 8.dp)
                    )
                }

                val pad = Modifier.fillMaxWidth().padding(horizontal = GUTTER)

                // ── Rank track (gamification, index 1) ───────────────────────────
                if (Features.SHOW_GAMIFICATION) state.rank?.let { r ->
                    Spacer(Modifier.height(20.dp))
                    SurfaceCard(palette, pad.statsEntrance(1)) {
                        RankSection(r, muted, accent, outline, onInfo = { showXpInfo = true })
                    }
                }

                // ── Hero card: lifetime volume over its curve (index 1) ──────────
                // 28dp so the cover's dissolve lands on bare page before the first fill starts.
                Spacer(Modifier.height(28.dp))
                ProfileHeroCard(
                    palette = palette,
                    totalVolumeLb = state.lifetimeVolumeSeriesLb.lastOrNull() ?: state.totalVolumeLb,
                    series = state.lifetimeVolumeSeriesLb,
                    totalSets = state.totalSets,
                    sinceLabel = state.sinceLabel,
                    onBg = onBg,
                    muted = muted,
                    modifier = pad.statsEntrance(1)
                )

                // ── Two-up: the tallies whose week-over-week movement means something ──
                Spacer(Modifier.height(10.dp))
                ProfileTwoUp(
                    palette = palette,
                    totalSessions = state.totalSessions,
                    workoutsThisWeek = state.workoutsThisWeek,
                    workoutsLastWeek = state.workoutsLastWeek,
                    totalPrs = state.totalPrs,
                    prsThisWeek = state.prsThisWeek,
                    prsLastWeek = state.prsLastWeek,
                    onBg = onBg,
                    muted = muted,
                    modifier = pad.statsEntrance(2)
                )

                // ── BODY as a horizontal strip (full-bleed, peeks the next card) ──
                Spacer(Modifier.height(28.dp))
                // No "measurements →" link: the strip's own SIZES card already opens Measurements,
                // and the two sat one above the other saying the same thing (Antho, 2026-08-15 —
                // "move measurement as a tile too"). §4.3's one-home rule, and the tile is the
                // better half of the pair because it carries a reading as well as a destination.
                SectionAnchor("Body", muted, onBg, modifier = pad)
                Spacer(Modifier.height(10.dp))
                ProfileBodyStrip(
                    palette = palette,
                    bodyweight = bodyweight,
                    bodyFat = bodyFat,
                    onLogWeight = {
                        // Fresh sheet: drop any prior result line and re-check HC permission so a
                        // grant made in Settings since this screen opened surfaces the import option.
                        viewModel.clearBodyweightMessage()
                        viewModel.refreshWeightConnected()
                        showWeightSheet = true
                    },
                    onLogBodyFat = {
                        viewModel.clearBodyFatMessage()
                        viewModel.refreshBodyFatConnected()
                        showBodyFatSheet = true
                    },
                    onOpenMeasurements = onOpenMeasurements,
                    onBg = onBg,
                    muted = muted,
                    modifier = Modifier.statsEntrance(3)
                )

                // ── This year's consistency — the same grid, boxed ───────────────
                // Hidden until the year has any activity, so a new user never sees a dead grid (§12).
                if (state.activityByDay.isNotEmpty()) {
                    Spacer(Modifier.height(28.dp))
                    ProfileYearCard(
                        palette = palette,
                        activityByDay = state.activityByDay,
                        muted = muted,
                        modifier = pad.statsEntrance(4)
                    )
                }

                if (Features.SHOW_GAMIFICATION) {
                    Spacer(Modifier.height(28.dp))
                    SurfaceCard(palette, pad.statsEntrance(5)) {
                        StandingSection(state.standings, onBg, muted, accent, outline)
                    }
                }

                // ── Gallery filmstrip (index 6) ──────────────────────────────────
                // The photos stay full-bleed and unboxed — they carry their own edges, and a card
                // around a photo is a frame around a frame. What DID change (2026-08-15) is that
                // the cells now share the cards' 18dp radius, and the empty ones take the card fill
                // rather than being hollow outlines, so the section reads as part of the same page.
                Spacer(Modifier.height(28.dp))
                Column(Modifier.fillMaxWidth().statsEntrance(6)) {
                    GalleryStrip(
                        state.photos, viewModel::fileFor,
                        onAdd = { addChooser = true },
                        onView = { viewing = it },
                        onViewAll = onOpenPhotoGallery,
                        palette, onBg, muted, outline
                    )
                }

                if (Features.SHOW_GAMIFICATION) {
                    Spacer(Modifier.height(28.dp))
                    SurfaceCard(palette, pad.statsEntrance(7)) {
                        TrophyCaseSection(
                            state.trophyGrid, state.trophyUnlocked, state.trophyTotal,
                            state.closestTrophy, onOpenTrophies, onBg, muted, accent, outline
                        )
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

    if (showBodyFatSheet) {
        BodyFatLogSheet(
            entries = bodyFat,
            canImport = bodyFatConnected,
            message = bodyFatMessage,
            onSave = { pct, date ->
                viewModel.logBodyFat(pct, date)
                showBodyFatSheet = false
            },
            onImport = { viewModel.importBodyFat() },  // stays open so the result line shows
            onDismiss = {
                showBodyFatSheet = false
                viewModel.clearBodyFatMessage()
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

    // The bodyweight GOAL line is still plumbed but no longer drawn: the shipped WEIGHT row put the
    // target on its sparkline as a dashed reference, and a 26dp card spark has no room for one. That
    // is a real loss this direction causes, not an oversight — see the report.
    @Suppress("UNUSED_EXPRESSION") bodyweightGoalLb
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
