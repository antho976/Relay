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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
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
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.ui.common.ConfettiOverlay
import com.forge.app.ui.common.FirstTouchTip
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.gym.stats.components.statsEntrance
import com.forge.app.ui.theme.emphasized
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * The "You" hub: rank ladder + earned XP, an offline standing estimate, signature lifts, the
 * private mirror-test photos, the trophy case and the month/year recaps. All local — no account,
 * no server. See [ProfileViewModel] / `data/repo/ProfileRepository` / `domain/rank`.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ProfileScreen(
    onBack: () -> Unit,
    onOpenTrophies: () -> Unit,
    onOpenRecaps: () -> Unit,
    onOpenGoals: () -> Unit = {},
    onOpenCoachBrief: () -> Unit = {},
    viewModel: ProfileViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val showRankUpCelebration by viewModel.showRankUpCelebration.collectAsStateWithLifecycle()
    var viewing by remember { mutableStateOf<ProgressPhoto?>(null) }
    var showXpInfo by remember { mutableStateOf(false) }
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val haptic = LocalHapticFeedback.current

    // One-shot rank-up haptic — fires as soon as the celebration flag goes true, then clears.
    LaunchedEffect(showRankUpCelebration) {
        if (showRankUpCelebration) {
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
        Scaffold(
            topBar = {
                TopAppBar(
                    title = { Text("Profile.", style = MaterialTheme.typography.headlineMedium) },
                    navigationIcon = {
                        IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") }
                    },
                    actions = {
                        state.rank?.let { r ->
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
                // The header block (label + name + rank line) enters together as the first item.
                Column(Modifier.statsEntrance(0)) {
                    Text(
                        "ATHLETE PROFILE" + if (state.sinceLabel.isNotBlank()) " · SINCE ${state.sinceLabel}" else "",
                        style = MaterialTheme.typography.labelSmall, color = emphasized(muted), fontSize = 9.sp
                    )
                    Spacer(Modifier.height(6.dp))
                    Text(state.name.ifBlank { "Athlete" } + ".", style = MaterialTheme.typography.displaySmall, color = emphasized(onBg))
                    state.rank?.let { r ->
                        Spacer(Modifier.height(2.dp))
                        val streak = if (state.streakDays >= 2) " · ${state.streakDays}-day streak, still alive." else ""
                        Text(
                            "Rank ${r.roman} — ${r.tier.display}$streak",
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
                        "Log a workout and this page fills in — you'll earn XP, climb the rank ladder, and unlock your standing, signature lifts and trophies."
                    )
                }

                // ── Rank track (index 1 — already has its own internal enter animation) ──
                // Wrapped in statsEntrance so the whole block slides in, complementing the
                // internal emblem scale/alpha. The two animations compound tastefully.
                state.rank?.let { r ->
                    Spacer(Modifier.height(20.dp))
                    Box(Modifier.statsEntrance(1)) {
                        RankSection(r, muted, accent, outline, onInfo = { showXpInfo = true })
                    }
                }

                // ── Sections (indices 2 onwards, top-to-bottom) ──────────────────
                // StandingSection has its own bar animations — still wrap it for the block entrance.
                Box(Modifier.statsEntrance(2)) {
                    LedgerSection(state.totalSessions, state.totalVolumeLb, state.totalPrs, state.rank?.xpTotal ?: 0L, muted, accent, outline)
                }
                Box(Modifier.statsEntrance(3)) {
                    StandingSection(state.standings, onBg, muted, accent, outline)
                }
                Box(Modifier.statsEntrance(4)) {
                    SignatureSection(state.topLift, state.mostLoggedDay, state.usualHour, onBg, muted, accent, outline)
                }
                Box(Modifier.statsEntrance(5)) {
                    MirrorTestSection(state.photos, viewModel::fileFor, onAdd = { pickPhoto() }, onView = { viewing = it }, onBg, muted, accent, outline)
                }

                state.memory?.let { m ->
                    Box(Modifier.statsEntrance(6)) {
                        ProfileBlock("ON THIS DAY", muted, accent, outline) {
                            val useKg = LocalForgeSettings.current.useKg
                            val ago = if (m.monthsAgo % 12 == 0) "${m.monthsAgo / 12} year${if (m.monthsAgo == 12) "" else "s"} ago"
                            else "${m.monthsAgo} months ago"
                            Text(
                                "$ago you trained ${m.dayName} — ${formatVolume(m.totalVolumeLb, useKg)} ${unitLabel(useKg)}" +
                                    if (m.prCount > 0) " · ${m.prCount} PR${if (m.prCount == 1) "" else "s"}" else "",
                                style = MaterialTheme.typography.bodyMedium, color = onBg
                            )
                        }
                    }
                }

                Box(Modifier.statsEntrance(7)) {
                    TrophyCaseSection(state.trophyGrid, state.trophyUnlocked, state.trophyTotal, state.closestTrophy, onOpenTrophies, onBg, muted, accent, outline)
                }

                Box(Modifier.statsEntrance(8)) {
                    ProfileBlock("GOALS", muted, accent, outline) {
                        Row(
                            Modifier.fillMaxWidth().bounceClick { onOpenGoals() },
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Text("Set targets, track your lifts", style = MaterialTheme.typography.bodyMedium, color = onBg)
                            Text("→", style = MaterialTheme.typography.bodyMedium, color = accent)
                        }
                    }
                }

                Box(Modifier.statsEntrance(9)) {
                    ProfileBlock("COACH", muted, accent, outline) {
                        Row(
                            Modifier.fillMaxWidth().bounceClick { onOpenCoachBrief() },
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Text("Week brief · what it's tracking", style = MaterialTheme.typography.bodyMedium, color = onBg)
                            Text("→", style = MaterialTheme.typography.bodyMedium, color = accent)
                        }
                    }
                }

                Box(Modifier.statsEntrance(10)) {
                    OnTheRecordSection(state.recaps, onOpenRecaps, onBg, muted, accent, outline)
                }

                Spacer(Modifier.height(40.dp))
            }
        }

        // ── Rank-up celebration overlay (Item 2) ─────────────────────────────
        // One-shot confetti burst on the first profile open after the user crosses into a new tier.
        // Drawn above the Scaffold so it covers the whole screen — touches pass through (Canvas has
        // no pointer-input). Clears itself after the animation completes via onComplete.
        if (showRankUpCelebration) {
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
private fun PhotoViewerDialog(file: File, takenAtMs: Long, onDelete: () -> Unit, onDismiss: () -> Unit) {
    Dialog(onDismissRequest = onDismiss) {
        Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp))) {
            ProgressPhotoImage(file, Modifier.fillMaxWidth().aspectRatio(0.8f).clip(RoundedCornerShape(12.dp)), reqPx = 1200)
            Spacer(Modifier.height(10.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                Text(
                    SimpleDateFormat("MMM d, yyyy", Locale.getDefault()).format(Date(takenAtMs)),
                    style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant
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
