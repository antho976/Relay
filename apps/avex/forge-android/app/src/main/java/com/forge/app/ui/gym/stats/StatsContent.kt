package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.gym.stats.components.*
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.WeekFields
import java.util.Locale

@Composable
fun StatsContent(
    modifier: Modifier = Modifier,
    onOpenHistory: () -> Unit = {},
    onOpenNotes: () -> Unit = {},
    onOpenRecap: () -> Unit = {},
    viewModel: StatsViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()

    val zone = ZoneId.systemDefault()
    val today = LocalDate.now(zone)
    val weekNum = today.get(WeekFields.ISO.weekOfWeekBasedYear())
    val isoWeekStart = today.minusDays(today.dayOfWeek.value.toLong() - 1)
    val isoWeekEnd = isoWeekStart.plusDays(6)
    val weekLabel = remember(weekNum) {
        val fmt = DateTimeFormatter.ofPattern("MMM d", Locale.getDefault())
        "${isoWeekStart.format(fmt).uppercase()} — ${isoWeekEnd.format(fmt).uppercase()}"
    }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    val weekCurrent = state.weekComparison?.current
    val weekPrev = state.weekComparison?.previous
    val weekSessions = weekCurrent?.sessions ?: 0

    LazyColumn(
        modifier = modifier,
        contentPadding = PaddingValues(bottom = 56.dp)
    ) {

        // ── Hero ─────────────────────────────────────────────────────────
        item("hero") {
            StatsHeroSection(
                weekNum = weekNum,
                weekLabel = weekLabel,
                weekSessions = weekSessions,
                weekCurrentVolumeLb = weekCurrent?.volumeLb,
                weekCurrentPrs = weekCurrent?.prs ?: 0,
                cardioMin = state.thisWeekCardioMin,
                onBg = onBg,
                muted = muted
            )
        }

        // ── RHYTHM ───────────────────────────────────────────────────────
        item("rhythm") {
            Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
                Text("RHYTHM", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
                Spacer(Modifier.height(8.dp))
                RhythmRow(
                    weekActivity = state.weekActivity,
                    today = today,
                    onBg = onBg,
                    muted = muted,
                    outline = outline
                )
                Spacer(Modifier.height(20.dp))
                HorizontalDivider(color = outline.copy(alpha = 0.25f))
                Spacer(Modifier.height(20.dp))
            }
        }

        // ── What I did this week ─────────────────────────────────────────
        item("week-title") {
            Text(
                "What I did this week",
                style = MaterialTheme.typography.headlineSmall,
                color = onBg,
                fontStyle = FontStyle.Italic,
                modifier = Modifier.padding(horizontal = 24.dp)
            )
            Spacer(Modifier.height(12.dp))
        }
        items(
            items = state.weekActivity.ifEmpty { emptyWeekActivity() },
            key = { "day-${it.dayOfWeek}" }
        ) { row ->
            WeekDayRow(
                row = row,
                today = today,
                onBg = onBg,
                muted = muted,
                accent = accent,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 5.dp)
            )
        }

        // ── vs last week ─────────────────────────────────────────────────
        if (weekCurrent != null && weekPrev != null && weekPrev.sessions > 0) {
            item("vs-last") {
                Column(Modifier.padding(horizontal = 24.dp)) {
                    Spacer(Modifier.height(16.dp))
                    HorizontalDivider(color = outline.copy(alpha = 0.25f))
                    Spacer(Modifier.height(20.dp))
                    Text(
                        "vs last week",
                        style = MaterialTheme.typography.headlineSmall,
                        color = onBg,
                        fontStyle = FontStyle.Italic
                    )
                    Spacer(Modifier.height(12.dp))
                    VsLastWeekRow("Sessions", weekCurrent.sessions, weekPrev.sessions, muted, onBg, accent)
                    Spacer(Modifier.height(6.dp))
                    VsLastWeekRow("Volume lb", weekCurrent.volumeLb.toInt(), weekPrev.volumeLb.toInt(), muted, onBg, accent)
                    Spacer(Modifier.height(6.dp))
                    VsLastWeekRow("PRs", weekCurrent.prs, weekPrev.prs, muted, onBg, accent)
                    Spacer(Modifier.height(6.dp))
                    VsLastWeekRow("Sets", weekCurrent.sets, weekPrev.sets, muted, onBg, accent)
                }
            }
        }

        item("week-divider") {
            Column(Modifier.padding(horizontal = 24.dp)) {
                Spacer(Modifier.height(20.dp))
                HorizontalDivider(color = outline.copy(alpha = 0.25f))
                Spacer(Modifier.height(20.dp))
            }
        }

        // ── Where the weight went ────────────────────────────────────────
        if (state.volumeByMuscle.isNotEmpty()) {
            item("vol-title") {
                StatsVolumeSection(rows = state.volumeByMuscle, muted = muted, accent = accent, outline = outline, onBg = onBg)
            }
        }

        // ── Where I'm going (strength curve) ────────────────────────────
        state.strengthCurve?.let { curve ->
            item("strength") {
                StatsStrengthSection(curve = curve, accent = accent, muted = muted, outline = outline, onBg = onBg)
            }
        }

        // ── Records broken (recent PRs) ──────────────────────────────────
        if (state.recentPrs.isNotEmpty()) {
            item("pr-title") {
                Text(
                    "Records broken",
                    style = MaterialTheme.typography.headlineSmall,
                    color = onBg,
                    fontStyle = FontStyle.Italic,
                    modifier = Modifier.padding(horizontal = 24.dp)
                )
                Spacer(Modifier.height(12.dp))
            }
            items(state.recentPrs, key = { "pr-${it.date}-${it.exerciseName}" }) { pr ->
                PrEntryRow(
                    pr = pr, muted = muted, onBg = onBg, accent = accent,
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 5.dp)
                )
            }
            item("pr-divider") {
                Column(Modifier.padding(horizontal = 24.dp)) {
                    Spacer(Modifier.height(20.dp))
                    HorizontalDivider(color = outline.copy(alpha = 0.25f))
                    Spacer(Modifier.height(20.dp))
                }
            }
        }

        // ── All-time bests (hall of fame) — always visible ───────────────
        item("hof-title") {
            Text(
                "All-time bests",
                style = MaterialTheme.typography.headlineSmall,
                color = onBg,
                fontStyle = FontStyle.Italic,
                modifier = Modifier.padding(horizontal = 24.dp)
            )
            Spacer(Modifier.height(12.dp))
        }
        if (state.hallOfFame.isEmpty()) {
            item("hof-empty") {
                Text(
                    "No records yet. Finish a session to set your first PR.",
                    style = MaterialTheme.typography.bodySmall,
                    color = muted.copy(alpha = 0.6f),
                    fontStyle = FontStyle.Italic,
                    modifier = Modifier.padding(horizontal = 24.dp)
                )
            }
        } else {
            items(state.hallOfFame.take(8), key = { "hof-${it.exerciseId}" }) { record ->
                HallOfFameRow(
                    record = record, muted = muted, onBg = onBg, accent = accent,
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 5.dp)
                )
            }
        }
        item("hof-divider") {
            Column(Modifier.padding(horizontal = 24.dp)) {
                Spacer(Modifier.height(20.dp))
                HorizontalDivider(color = outline.copy(alpha = 0.25f))
                Spacer(Modifier.height(20.dp))
            }
        }

        // ── Exercise frequency (last 8 weeks) ────────────────────────────
        if (state.exerciseFrequency.isNotEmpty()) {
            item("freq-title") {
                StatsFreqSection(rows = state.exerciseFrequency, muted = muted, accent = accent, outline = outline, onBg = onBg)
            }
        }

        // ── Your body of work (lifetime metrics) — always visible ────────
        item("lifetime") {
            StatsLifetimeSection(lm = state.lifetimeMetrics, onBg = onBg, muted = muted, outline = outline)
        }

        // ── Behavioral insights ───────────────────────────────────────────
        if (state.insights.isNotEmpty()) {
            item("insights-title") {
                Text(
                    "Insights",
                    style = MaterialTheme.typography.headlineSmall,
                    color = onBg,
                    fontStyle = FontStyle.Italic,
                    modifier = Modifier.padding(horizontal = 24.dp)
                )
                Spacer(Modifier.height(12.dp))
            }
            items(state.insights, key = { "insight-${it.title}" }) { flag ->
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 24.dp, vertical = 6.dp)
                ) {
                    Text(
                        flag.title,
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontSize = 10.sp
                    )
                    Spacer(Modifier.height(2.dp))
                    Text(
                        flag.body,
                        style = MaterialTheme.typography.bodySmall,
                        color = onBg.copy(alpha = 0.8f)
                    )
                }
            }
            item("insights-bottom") { Spacer(Modifier.height(8.dp)) }
        }
    }
}

