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
import com.forge.app.ui.theme.emphasized
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SheetValue
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import com.forge.app.ui.gym.stats.components.statsEntrance
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.mood.Mood
import com.forge.app.domain.units.formatVolume
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.common.ConfettiOverlay
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.theme.ForgeLastGreen
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.gym.train.state.SessionSummary
import com.forge.app.ui.gym.train.state.UnlockedTrophyHighlight
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SessionSummarySheet(
    summary: SessionSummary,
    onDismiss: (mood: Mood?, tags: List<String>, journal: String) -> Unit
) {
    // Block swipe-to-dismiss: the only way out is the COMPLETE button, otherwise the
    // sheet can be swiped away while the summary is still "open", leaving FINISH dead.
    val sheetState = rememberModalBottomSheetState(
        skipPartiallyExpanded = true,
        confirmValueChange = { it != SheetValue.Hidden }
    )
    // Mood/tags are one-tap re-selections, so plain remember (reset on rotation) is fine; the journal
    // is free text the user can't trivially recreate, so it's rememberSaveable to survive a rotation.
    var selectedMood by remember { mutableStateOf<Mood?>(null) }
    var selectedTags by remember { mutableStateOf<Set<String>>(emptySet()) }
    var journal by rememberSaveable { mutableStateOf(summary.initialJournal) }
    // Finishing is an event: confetti for a trophy, a best-ever session, OR a clean sweep of the
    // duel (every set beat last time). rememberSaveable so the burst doesn't replay on every rotation
    // (and stays consistent with the trophy haptic, which also fires only once).
    val cleanSweep = summary.ghostComparable > 0 && summary.ghostBeats == summary.ghostComparable
    var showTrophyConfetti by rememberSaveable {
        mutableStateOf(summary.unlockedTrophies.isNotEmpty() || summary.isBestSession || cleanSweep)
    }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val bg = MaterialTheme.colorScheme.background
    val useKg = LocalForgeSettings.current.useKg
    val context = LocalContext.current
    val accentArgb = MaterialTheme.colorScheme.primary.toArgb().toLong()
    val scope = rememberCoroutineScope()
    // Guards against a second tap kicking off a concurrent render while the first is still in flight.
    var sharing by remember { mutableStateOf(false) }
    // A trophy unlock is the peak moment — confirm it in the hand, exactly once. rememberSaveable so a
    // config change (rotation) while the sheet is open doesn't re-fire the celebratory haptic.
    val haptic = LocalHapticFeedback.current
    var trophyHapticFired by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        if (!trophyHapticFired && summary.unlockedTrophies.isNotEmpty()) {
            haptic.performHapticFeedback(HapticFeedbackType.LongPress)
            trophyHapticFired = true
        }
    }
    // A clean sweep (every comparable set beat last time) earns its own signature in the hand: a
    // three-tap burst, distinct from the single long-press of a PR/finish. Skipped when a trophy
    // already fired its haptic (don't stack two celebrations) and when the user turned haptics off.
    // rememberSaveable so it fires once, not on every rotation.
    val hapticStrength = LocalForgeSettings.current.hapticStrength
    var cleanSweepHapticFired by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        if (!cleanSweepHapticFired && cleanSweep && summary.unlockedTrophies.isEmpty() && hapticStrength != "off") {
            repeat(3) {
                haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                delay(90)
            }
            cleanSweepHapticFired = true
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
                        color = emphasized(onBg)
                    )
                    Text(
                        "workout complete",
                        style = MaterialTheme.typography.bodySmall,
                        color = muted,
                        fontSize = 11.sp,
                        fontStyle = FontStyle.Italic
                    )
                }

                // Primary stats strip — VOLUME + PRs roll up from 0 on open (the celebration numbers).
                Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                    CountUpStat(value = summary.totalVolumeLb, label = "VOLUME", onBg = onBg, muted = muted) { formatVolume(it, useKg) }
                    CountUpStat(value = summary.prCount.toDouble(), label = "PRs", onBg = onBg, muted = muted) { it.toInt().toString() }
                    FlatStat(value = "${summary.durationMinutes} min", label = "TIME", onBg = onBg, muted = muted)
                }

                // Secondary meta stats
                Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                    FlatStat(value = "${summary.setCount}", label = "SETS", onBg = onBg, muted = muted)
                    FlatStat(
                        value = "${summary.exercisesLogged}" + (if (summary.exercisesSkipped > 0) " · ${summary.exercisesSkipped} skipped" else ""),
                        label = "EXERCISES",
                        onBg = onBg,
                        muted = muted
                    )
                }

                // Session efficiency metrics (#83, #127, #82, #133)
                val hasEfficiency = summary.densityScore != null || summary.avgRestSeconds != null || summary.honestyPct != null
                if (hasEfficiency) {
                    HorizontalDivider(color = outline.copy(alpha = 0.2f))
                    Text("SESSION METRICS", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
                    Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                        summary.densityScore?.let { density ->
                            FlatStat(value = "${toDisplayWeight(density, useKg).toInt()} ${unitLabel(useKg)}/min", label = "DENSITY", onBg = onBg, muted = muted)
                        }
                        summary.avgRestSeconds?.let { rest ->
                            val restStr = if (rest >= 60) "${rest / 60}m ${rest % 60}s" else "${rest}s"
                            FlatStat(value = restStr, label = "AVG REST", onBg = onBg, muted = muted)
                        }
                        summary.honestyPct?.let { pct ->
                            FlatStat(value = "$pct%", label = "COMPLETION", onBg = onBg, muted = muted)
                        }
                    }
                }

                // Session comparison vs last same-day session (#52)
                val hasComparison = summary.vsLastVolumeDelta != null || summary.vsLastSetsDelta != null
                if (hasComparison) {
                    HorizontalDivider(color = outline.copy(alpha = 0.2f))
                    Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                        summary.vsLastVolumeDelta?.let { delta ->
                            val sign = if (delta >= 0) "+" else ""
                            FlatStat(value = "$sign${toDisplayWeight(delta, useKg).toInt()} ${unitLabel(useKg)}", label = "vs LAST", onBg = onBg, muted = muted)
                        }
                        summary.vsLastSetsDelta?.let { delta ->
                            val sign = if (delta >= 0) "+" else ""
                            FlatStat(value = "$sign$delta sets", label = "vs LAST", onBg = onBg, muted = muted)
                        }
                    }
                }

                // "Beat the ghost" duel result — the session-long contest vs last time.
                if (summary.ghostComparable > 0) {
                    val beats = summary.ghostBeats
                    val total = summary.ghostComparable
                    val won = beats * 2 >= total
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .statsEntrance(0)
                            .border(0.5.dp, (if (won) ForgeLastGreen else outline).copy(alpha = if (won) 0.6f else 0.3f), RoundedCornerShape(4.dp))
                            .padding(horizontal = 14.dp, vertical = 10.dp)
                    ) {
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                            Text(if (cleanSweep) "⚡" else if (won) "↑" else "·", color = if (won) ForgeLastGreen else muted, style = MaterialTheme.typography.titleSmall)
                            Text(
                                when {
                                    cleanSweep -> "Clean sweep — you beat last session on every set"
                                    won -> "Beat last session on $beats of $total sets"
                                    else -> "$beats of $total sets beat last session"
                                },
                                style = MaterialTheme.typography.bodySmall,
                                color = if (won) onBg else muted
                            )
                        }
                    }
                }

                // Best session callout (#53)
                if (summary.isBestSession) {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .border(0.5.dp, outline.copy(alpha = 0.3f), RoundedCornerShape(4.dp))
                            .padding(horizontal = 14.dp, vertical = 10.dp)
                    ) {
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                            Text("★", color = onBg, fontWeight = FontWeight.Black, style = MaterialTheme.typography.titleSmall)
                            Text("Your best ${summary.dayWord} ever", style = MaterialTheme.typography.bodySmall, color = onBg)
                        }
                    }
                }

                // Coach's read — one session-specific line, derived from this session's own result.
                summary.coachOpinion?.let { opinion ->
                    HorizontalDivider(color = outline.copy(alpha = 0.2f))
                    Column(
                        modifier = Modifier.fillMaxWidth().statsEntrance(1),
                        verticalArrangement = Arrangement.spacedBy(4.dp)
                    ) {
                        Text("COACH'S READ", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
                        Text(opinion, style = MaterialTheme.typography.bodySmall, color = onBg)
                    }
                }

                if (summary.highlights.isNotEmpty()) {
                    HorizontalDivider(color = outline.copy(alpha = 0.2f))
                    Text("EXERCISES", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
                    summary.highlights.forEach { h -> HighlightRow(h, onBg = onBg, muted = muted) }
                }

                if (summary.unlockedTrophies.isNotEmpty()) {
                    HorizontalDivider(color = outline.copy(alpha = 0.2f))
                    Text("TROPHIES UNLOCKED", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
                    // Each trophy row settles in on its own beat (staggered) rather than snapping in mid-confetti.
                    summary.unlockedTrophies.forEachIndexed { i, t ->
                        Box(Modifier.statsEntrance(i)) { TrophyUnlockRow(t, onBg = onBg, muted = muted, outline = outline) }
                    }
                }

                HorizontalDivider(color = outline.copy(alpha = 0.2f))
                MoodPrompt(
                    selected = selectedMood,
                    onSelect = { mood -> selectedMood = if (selectedMood == mood) null else mood },
                    onBg = onBg,
                    muted = muted,
                    outline = outline
                )

                HorizontalDivider(color = outline.copy(alpha = 0.2f))
                JournalField(value = journal, onValueChange = { journal = it }, muted = muted)

                HorizontalDivider(color = outline.copy(alpha = 0.2f))
                TagPicker(
                    selected = selectedTags,
                    onToggle = { tag ->
                        selectedTags = if (tag in selectedTags) selectedTags - tag else selectedTags + tag
                    },
                    onBg = onBg,
                    muted = muted,
                    outline = outline
                )

                // What's next — a first-time user doesn't know where the session goes or where records live (Cat 6).
                HorizontalDivider(color = outline.copy(alpha = 0.2f))
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text("WHAT'S NEXT", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
                    Text(
                        "Your session's saved to history. See your records in Gym → Stats → PRs, and the coach refines your plan as you log more.",
                        style = MaterialTheme.typography.bodySmall, color = muted
                    )
                }

                // Share a recap card (offline bitmap → system share sheet) — the organic "look what I did" moment.
                HorizontalDivider(color = outline.copy(alpha = 0.2f))
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .bounceClick(enabled = !sharing) {
                            // Render the ~4.7 MB bitmap off the main thread (it allocates, draws, PNG-
                            // compresses and writes a file); only the share intent touches the UI thread.
                            sharing = true
                            scope.launch {
                                val uri = withContext(Dispatchers.Default) {
                                    SessionCardRenderer.render(
                                        context = context,
                                        title = summary.displayName,
                                        dateLine = java.time.LocalDate.now()
                                            .format(java.time.format.DateTimeFormatter.ofPattern("MMM d, yyyy")).uppercase(),
                                        // formatVolume already includes the unit — don't append it again.
                                        heroValue = formatVolume(summary.totalVolumeLb, useKg),
                                        heroLabel = "VOLUME",
                                        stats = buildList {
                                            if (summary.prCount > 0) add("${summary.prCount}" to if (summary.prCount == 1) "PR" else "PRs")
                                            add("${summary.setCount}" to "SETS")
                                            if (summary.durationMinutes > 0) add("${summary.durationMinutes}" to "MIN")
                                        },
                                        footnote = when {
                                            summary.isBestSession -> "Best ${summary.dayWord} ever"
                                            summary.prCount > 0 -> "${summary.prCount} new ${if (summary.prCount == 1) "PR" else "PRs"}"
                                            else -> null
                                        },
                                        accentArgb = accentArgb
                                    )
                                }
                                uri?.let { SessionCardRenderer.share(context, it) }
                                sharing = false
                            }
                        }
                        .padding(vertical = 12.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        "Share card",
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.primary,
                        letterSpacing = 1.sp
                    )
                }

                // Complete button — only way to dismiss
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(top = 4.dp, bottom = 16.dp)
                        .clip(RoundedCornerShape(4.dp))
                        .border(0.5.dp, outline.copy(alpha = 0.4f), RoundedCornerShape(4.dp))
                        .bounceClick { onDismiss(selectedMood, selectedTags.toList(), journal) }
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

            // Trophy confetti (#30): fires once when sheet opens with unlocked trophies
            if (showTrophyConfetti) {
                ConfettiOverlay(
                    modifier = Modifier.matchParentSize(),
                    onComplete = { showTrophyConfetti = false }
                )
            }
        }
    }
}

