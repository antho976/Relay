package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SheetValue
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.mood.Mood
import com.forge.app.domain.notify.PrMilestone
import com.forge.app.domain.units.formatVolume
import com.forge.app.service.ForgeNotifications
import com.forge.app.ui.common.ConfettiOverlay
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.gym.stats.components.BodyHeatmap
import com.forge.app.ui.gym.stats.components.statsEntrance
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.gym.train.state.SessionSummary

/**
 * The end-of-session summary. Stripped to five reads: any new PRs (hidden when none), the general
 * stats + time, a recap of what you worked (muscle map + exercise list), and the coach's corner —
 * what he saw and the effort data he'd like more of. [onDismiss] keeps its tags/mood params for the
 * shared event signature, but both are now always empty; the mid-session journal flows straight
 * through so it's still persisted on COMPLETE.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SessionSummarySheet(
    summary: SessionSummary,
    onDismiss: (mood: Mood?, tags: List<String>, journal: String) -> Unit
) {
    // Block swipe-to-dismiss: the only way out is the COMPLETE button, otherwise the sheet can be
    // swiped away while the summary is still "open", leaving FINISH dead.
    val sheetState = rememberModalBottomSheetState(
        skipPartiallyExpanded = true,
        confirmValueChange = { it != SheetValue.Hidden }
    )

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val bg = MaterialTheme.colorScheme.background
    val accent = MaterialTheme.colorScheme.primary
    val weightUnit = LocalForgeSettings.current.weightUnit
    val context = LocalContext.current
    val hapticStrength = LocalForgeSettings.current.hapticStrength

    // A PR is the peak of a session — celebrate it once with confetti + a single haptic. Both are
    // rememberSaveable so a rotation while the sheet is open doesn't replay them.
    val hasPr = summary.prCount > 0
    var showConfetti by rememberSaveable { mutableStateOf(hasPr) }
    val haptic = LocalHapticFeedback.current
    var prHapticFired by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        if (!prHapticFired && hasPr && hapticStrength != "off") {
            haptic.performHapticFeedback(HapticFeedbackType.LongPress)
            prHapticFired = true
        }
    }
    // PR-milestone push (#13): a finished session crossing a lifetime-PR round number drops a
    // celebratory notification in the shade. Own channel; fired once, only on a genuine milestone.
    var prMilestoneFired by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        if (!prMilestoneFired) {
            PrMilestone.check(summary.lifetimePrCount, summary.prCount)?.let { nudge ->
                ForgeNotifications.postPrMilestone(context, nudge.title, nudge.body)
            }
            prMilestoneFired = true
        }
    }

    ModalBottomSheet(
        onDismissRequest = {},
        sheetState = sheetState,
        containerColor = bg
    ) {
        Box {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 24.dp, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(16.dp)
            ) {
                // Header
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(
                        summary.dayWord.uppercase(),
                        style = MaterialTheme.typography.labelSmall,
                        letterSpacing = 1.5.sp,
                        color = muted,
                        fontSize = 9.sp
                    )
                    Text(
                        summary.displayName,
                        style = MaterialTheme.typography.headlineSmall,
                        color = onBg
                    )
                    Text(
                        "workout complete",
                        style = MaterialTheme.typography.bodySmall,
                        color = muted,
                        fontSize = 11.sp,
                        fontStyle = FontStyle.Italic
                    )
                }

                // New PRs — the celebration headline. Hidden entirely when there are none.
                if (hasPr) {
                    Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                        CountUpStat(
                            value = summary.prCount.toDouble(),
                            label = if (summary.prCount == 1) "NEW PR" else "NEW PRs",
                            onBg = onBg,
                            muted = muted
                        ) { it.toInt().toString() }
                    }
                }

                // General stats + time taken.
                Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                    CountUpStat(value = summary.totalVolumeLb, label = "VOLUME", onBg = onBg, muted = muted) { formatVolume(it, weightUnit) }
                    FlatStat(value = "${summary.durationMinutes} min", label = "TIME", onBg = onBg, muted = muted)
                }
                Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                    FlatStat(value = "${summary.setCount}", label = "SETS", onBg = onBg, muted = muted)
                    FlatStat(
                        value = "${summary.exercisesLogged}" + (if (summary.exercisesSkipped > 0) " · ${summary.exercisesSkipped} skipped" else ""),
                        label = "EXERCISES",
                        onBg = onBg,
                        muted = muted
                    )
                }

                // Recap — what you worked on: the muscle map + the exercise list.
                if (summary.setsByMuscle.isNotEmpty() || summary.highlights.isNotEmpty()) {
                    HorizontalDivider(color = outline.copy(alpha = 0.2f))
                    Text("WHAT YOU WORKED", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
                    if (summary.setsByMuscle.isNotEmpty()) {
                        BodyHeatmap(
                            setsByMuscle = summary.setsByMuscle,
                            accent = accent,
                            faint = outline.copy(alpha = 0.34f),
                            silhouette = outline.copy(alpha = 0.26f),
                            labelColor = muted,
                            modifier = Modifier.fillMaxWidth().statsEntrance(0)
                        )
                    }
                    summary.highlights.forEach { h -> HighlightRow(h, onBg = onBg, muted = muted) }
                }

                // Coach — what he read from this session, and the effort data he'd like more of.
                CoachReadSection(
                    coachOpinion = summary.coachOpinion,
                    setsWithRpe = summary.setsWithRpe,
                    totalSets = summary.setCount,
                    exercisesRated = summary.exercisesRated,
                    exercisesLogged = summary.exercisesLogged,
                    onBg = onBg,
                    muted = muted,
                    outline = outline
                )

                // Complete button — only way to dismiss. Tags/mood are gone; the mid-session journal
                // flows straight through so it's still persisted on finish.
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(top = 4.dp, bottom = 16.dp)
                        .clip(RoundedCornerShape(4.dp))
                        .border(0.5.dp, outline.copy(alpha = 0.4f), RoundedCornerShape(4.dp))
                        .bounceClick { onDismiss(null, emptyList(), summary.initialJournal) }
                        .padding(vertical = 16.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        "COMPLETE",
                        style = MaterialTheme.typography.labelSmall,
                        color = onBg,
                        letterSpacing = 1.5.sp
                    )
                }
            }

            // PR confetti: fires once when the sheet opens on a session with a new PR.
            if (showConfetti) {
                ConfettiOverlay(
                    modifier = Modifier.matchParentSize(),
                    onComplete = { showConfetti = false }
                )
            }
        }
    }
}
