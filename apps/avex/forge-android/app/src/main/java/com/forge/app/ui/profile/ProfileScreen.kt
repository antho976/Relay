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
import androidx.compose.ui.draw.drawBehind
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
import com.forge.app.security.LocalAppLock
import com.forge.app.ui.common.ConfettiOverlay
import com.forge.app.ui.common.DayLogSheet
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
 * ## The colour rule (2026-08-24)
 *
 * **Colour is for marks; type stays type.** Every drawn reading on this page — the lifetime curve,
 * the THIS WK bars, the activity ramp, the body-row trends — is the accent. Every figure, label,
 * caption and zero-state line stays on `onBg` / `muted`.
 *
 * The page used to take its data colour from `SurfacePalette.hues`, which is
 * `[onBg, muted, muted * 0.7]` — neutral by design, under the 2026-08-16 budget reserving ember for
 * the four places that carry a decision. The effect on a page that is nothing *but* readings was
 * that Profile drew its curve, its bars and its calendar in the same white as its own type, so the
 * one screen made entirely of data was the one screen with no data colour in it — while Stats next
 * door lights every chart line, heatmap and week bar off `colorScheme.primary`. Antho, 2026-08-24:
 * *"use the accent color for the page, look at other pages to see what should be accented."*
 *
 * The split is what keeps that from becoming a rash. Marks are shapes, so the accent on them is
 * exempt from the text contrast floor; accent TEXT is not, and §14 measures it at 2.35:1 on this
 * ground — which is why the section anchors, the `CardLink` "view all →" and the zero-state lines
 * are all deliberately still `onBg`. Nothing here reads by colour alone either: every bar carries
 * its count, the grid carries its LESS/MORE key and its spoken reading, and with the accent
 * switched off in Appearance `colorScheme.primary` resolves to the near-white neutral, so the whole
 * page goes monochrome without a single branch in this file.
 *
 * Sections that stayed: the gallery filmstrip, untouched on instruction. It was already unboxed and
 * full-bleed, so there was nothing to take from it. The "seeded pictures" turned out not to be code
 * at all — fifteen `pp_seed*.jpg` fixtures were sitting in the debug app's storage from an earlier
 * session, and they were cleared off the device.
 *
 * The open-editorial original is in git history, not in a snapshot directory beside the live file
 * (`git log -- app/src/main/java/com/forge/app/ui/profile/` finds the commit that replaced it).
 * Every shipped section (`AllTimeSection`, `BodyMetricsSection`, `LifetimeVolumeGraph`,
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

/**
 * What keeps a white control legible over an arbitrary cover photo (Antho, 2026-08-24: the back
 * arrow "is hard to see when you have a white background").
 *
 * ## Why a halo and not a colour test
 *
 * The obvious reading of "make it background-aware" is to measure the photo and flip the glyph to
 * black over a bright one. That fails in two ways this page will actually hit. A photograph is not
 * one colour: this cover is a bright misty sky in its top third and near-black forest below, so a
 * single luminance sample decides the whole control on whichever half it happened to land in — and
 * a flipped-to-black arrow would then sit directly above a white "Athlete" that never flips, on the
 * same image, which reads as two different apps.
 *
 * So the answer is the one the cover already uses for every other mark on it: keep the glyph white
 * and put a dark halo behind it. [NameShadow] and [MetaShadow] in [ProfileHeaderCard] are the same
 * decision for text, made 2026-07-24 for the same reason. This is genuinely background-aware where
 * it counts — it darkens what is behind the glyph whatever that is, so it cannot pick wrong — and
 * over a dark photo it costs nothing visible.
 *
 * ## It goes on the BUTTON, not on the glyph
 *
 * Hung on the `Icon`, this drew a flat grey disc with a hard rim — the exact chip the paragraph
 * above says not to make, and widening the gradient only made the rim crisper. The rim was never
 * the gradient: Material's `IconButton` clips its content to a 48dp circle for its ripple, so a
 * halo drawn inside it is cut off at 24dp from centre while it is still around 0.15 alpha, and that
 * cut IS the edge you see. The falloff was working; it was being amputated.
 *
 * Applied to the button instead, the halo sits ahead of that clip in the modifier chain and fades
 * out on its own terms. A draw modifier is only clipped by a `clip` that comes AFTER it.
 *
 * The rest is tail. It runs to 0.85× the 48dp target and gives its outer third to the last rung, so
 * it reaches the background with room to spare — over a bright sky even a sixth of black still
 * reads, and a fade that only arrives at zero at its own edge hands the eye an edge to find.
 *
 * The stops are §5 ladder rungs (0.6 / 0.35 / 0.15), not values picked for the curve. A gradient is
 * the one place a one-off alpha could hide — nobody reads a stop as a tone — which is exactly why
 * it should not: the ladder costs nothing here, and an exception granted where it would not have
 * been noticed is how a system stops being one.
 */
private fun Modifier.coverHalo(): Modifier = drawBehind {
    val r = size.maxDimension * 0.85f
    drawCircle(
        brush = Brush.radialGradient(
            0f to Color.Black.copy(alpha = 0.6f),
            0.30f to Color.Black.copy(alpha = 0.35f),
            0.62f to Color.Black.copy(alpha = 0.15f),
            1f to Color.Transparent,
            center = center,
            radius = r
        ),
        radius = r
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ProfileScreen(
    // Null when shown as a hub pager page (no redundant back arrow); a real callback as a deep route.
    onBack: (() -> Unit)? = null,
    onOpenTrophies: () -> Unit,
    onOpenPhotoGallery: () -> Unit = {},
    onOpenMeasurements: () -> Unit = {},
    /** Drill-downs out of the ACTIVITY day sheet — the same destinations History opens. */
    onOpenSession: (Long) -> Unit = {},
    onOpenCardio: (Long) -> Unit = {},
    viewModel: ProfileViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val dayLog by viewModel.dayLog.collectAsStateWithLifecycle()
    val galleryLocked by LocalAppLock.current.galleryLocked.collectAsStateWithLifecycle()
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
                        if (onBack != null) IconButton(onClick = onBack, modifier = Modifier.coverHalo()) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back", tint = Color.White)
                        }
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
                            }, modifier = Modifier.coverHalo()) {
                                Icon(Icons.Filled.Share, contentDescription = "Share rank card", tint = Color.White)
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
                    accent = accent,
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
                    // Attendance, so it lives with attendance. Both streak figures came off the
                    // cover photo on 2026-08-24 — see [ProfileHeaderCard]'s header for why the
                    // accent chip could not stay up there.
                    streakDays = state.streakDays,
                    longestStreakDays = state.longestStreakDays,
                    onBg = onBg,
                    muted = muted,
                    // The accent, like every other heatmap in the app (Stats' adherence calendar
                    // and its body map both light off `c.accent`). This grid used to take a neutral
                    // palette hue under the 2026-08-16 ember budget, which left the page's one
                    // texture the same colour as its type — see the colour rule in [ProfileScreen]'s
                    // header. Monochrome mode is safe by construction: with the accent switched off
                    // `colorScheme.primary` IS the near-white neutral, so the ramp goes grey with
                    // the rest of the app rather than needing a branch here.
                    hue = accent,
                    onDayTap = viewModel::openDay,
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
                    accent = accent,
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
                        photos = if (galleryLocked) emptyList() else state.photos,
                        fileFor = viewModel::fileFor,
                        onOpenGallery = onOpenPhotoGallery,
                        muted = muted,
                        locked = galleryLocked,
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

    // "What did I do that day?" — opened by tapping a lit day on the ACTIVITY calendar. The same
    // sheet Stats' heatmap opens, so a day looks the same wherever you ask about it, and the rows
    // drill into the same detail screens History uses.
    dayLog?.let { log ->
        DayLogSheet(
            log = log,
            onOpenSession = { viewModel.closeDay(); onOpenSession(it) },
            onOpenCardio = { viewModel.closeDay(); onOpenCardio(it) },
            onDismiss = viewModel::closeDay
        )
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
