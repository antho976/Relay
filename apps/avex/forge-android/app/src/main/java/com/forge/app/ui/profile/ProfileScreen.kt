package com.forge.app.ui.profile

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.Features
import com.forge.app.ui.common.ConfettiOverlay
import com.forge.app.ui.common.statsEntrance
import com.forge.app.ui.experiment.SectionAnchor
import com.forge.app.ui.experiment.SurfaceCard
import com.forge.app.ui.experiment.surfacePalette
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * # Profile — the open page (2026-08-22)
 *
 * The "You" hub, in this order and no other: **the cover, all time, this month's activity, body,
 * gallery** (Antho, 2026-08-22). Read down the page it answers four questions in the order a lifter
 * actually asks them — who am I, what have I done, am I showing up *now*, what is my body doing —
 * and then shows the photographic record of it.
 *
 * ## Three changes, one direction
 *
 * **The cover is untouched, again.** [ProfileHeaderCard] is byte-for-byte the shipped one and every
 * redesign of this page has left it alone, which by now is evidence rather than caution: it is the
 * one element here that was already right. Nothing below competes with it — no second photo, no
 * gradient, and a full 28dp of air so its dissolve lands on bare page.
 *
 * **The cards are gone.** The 2026-08-15 experiment boxed every section in a `SurfaceCard`; this
 * pass takes the fills away and keeps the icons. Structure is now anchors, hairlines and space —
 * the app's own editorial language (`ui/common/Editorial.kt`), which Home and the live session
 * already speak, so the Profile stopped being the one screen with its own dialect. What that cost
 * and what it bought is written up on [ProfileAllTime].
 *
 * **THIS YEAR became ACTIVITY.** The 12-row year grid is replaced by [ProfileActivityMonth], a
 * GitHub-style contribution grid over the current month. The year is still in the package
 * ([YearConsistencySection]) and no longer called.
 *
 * Sections that stayed: the gallery filmstrip, untouched on instruction. It was already unboxed and
 * full-bleed, so there was nothing to take from it. The "seeded pictures" turned out not to be code
 * at all — fifteen `pp_seed*.jpg` fixtures were sitting in the debug app's storage from an earlier
 * session, and they were cleared off the device.
 *
 * The open-editorial original is at `.design-backups/editorial-2026-08/src/profile/`; one command
 * restores it. Every shipped section (`AllTimeSection`, `BodyMetricsSection`, `LifetimeVolumeGraph`,
 * `SectionHeader`, `ChartCaption`) is still in the package, untouched and simply no longer called.
 *
 * The rank / standing / trophy sections are still boxed. They are behind `Features
 * .SHOW_GAMIFICATION`, which is `false`, so nothing renders — unboxing UI no one can see would be
 * an unverifiable change, and they get the same pass when the flag does.
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
    var showXpInfo by remember { mutableStateOf(false) }
    var showWeightSheet by remember { mutableStateOf(false) }
    var showBodyFatSheet by remember { mutableStateOf(false) }
    var showAvatarSheet by remember { mutableStateOf(false) }

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

                // ── ALL TIME (index 1) ──────────────────────────────────────────
                // 28dp so the cover's dissolve lands on bare page before the first mark starts.
                Spacer(Modifier.height(28.dp))
                SectionAnchor("All time", muted, onBg, modifier = pad)
                Spacer(Modifier.height(12.dp))
                ProfileAllTime(
                    palette = palette,
                    totalVolumeLb = state.lifetimeVolumeSeriesLb.lastOrNull() ?: state.totalVolumeLb,
                    series = state.lifetimeVolumeSeriesLb,
                    totalSets = state.totalSets,
                    totalSessions = state.totalSessions,
                    workoutsThisWeek = state.workoutsThisWeek,
                    workoutsLastWeek = state.workoutsLastWeek,
                    totalPrs = state.totalPrs,
                    prsThisWeek = state.prsThisWeek,
                    prsLastWeek = state.prsLastWeek,
                    onBg = onBg,
                    muted = muted,
                    modifier = pad.statsEntrance(1)
                )

                // ── ACTIVITY — this month as a contribution grid (index 2) ───────
                // Swap this call for [ProfileActivityYear] to get the two-band year instead; both
                // take the same arguments and both live in this package, on purpose. The year read
                // as a texture and the month reads as a calendar, and which one belongs here is a
                // question about how you use the page, not one the code can settle — so the switch
                // stays a one-line change rather than being resolved by deleting the loser.
                //
                // Drawn even at zero. The 12-row dot grid these replaced hid itself until the year
                // had activity, because twelve rows of dead dots is a lot of nothing; an empty
                // month is a legible "nothing yet" that also teaches what fills in (§12).
                Spacer(Modifier.height(34.dp))
                ProfileActivityMonth(
                    activityByDay = state.activityByDay,
                    onBg = onBg,
                    muted = muted,
                    // hues[0], not the accent. A green band would be more literally GitHub, and
                    // ember is budgeted (2026-08-16) for the four places that carry a decision —
                    // this one carries a fact. Taking the palette hue also keeps the band honest
                    // in monochrome mode, where colour-as-data is switched off entirely.
                    hue = palette.hues[0],
                    modifier = pad.statsEntrance(2)
                )

                // ── BODY as open rows (index 3) ─────────────────────────────────
                // No "measurements →" link: the SIZES row already opens Measurements, and the two
                // sat one above the other saying the same thing (Antho, 2026-08-15 — "move
                // measurement as a tile too"). §4.3's one-home rule, and the row is the better half
                // of the pair because it carries a reading as well as a destination.
                Spacer(Modifier.height(34.dp))
                SectionAnchor("Body", muted, onBg, modifier = pad)
                Spacer(Modifier.height(12.dp))
                ProfileBodyRows(
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
                    modifier = pad.statsEntrance(3)
                )

                if (Features.SHOW_GAMIFICATION) {
                    Spacer(Modifier.height(34.dp))
                    SurfaceCard(palette, pad.statsEntrance(4)) {
                        StandingSection(state.standings, onBg, muted, accent, outline)
                    }
                }

                // ── Gallery filmstrip (index 4) ──────────────────────────────────
                // Every cell opens the Gallery now — no header link, no viewer dialog, and no
                // difference between having photos and not (Antho, 2026-08-22). See [GalleryStrip].
                //
                // Its empty state keeps the three-cell ghost strip, which is the only card fill left
                // on this page; that is the one deliberate exception to the de-boxing.
                Spacer(Modifier.height(34.dp))
                Column(Modifier.fillMaxWidth().statsEntrance(4)) {
                    GalleryStrip(
                        state.photos, viewModel::fileFor,
                        onOpenGallery = onOpenPhotoGallery,
                        palette = palette, muted = muted
                    )
                }

                if (Features.SHOW_GAMIFICATION) {
                    Spacer(Modifier.height(28.dp))
                    SurfaceCard(palette, pad.statsEntrance(5)) {
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
