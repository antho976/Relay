package com.forge.app.ui.coach

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.CircularProgressIndicator
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.repo.CoachMilestone
import com.forge.app.data.repo.CoachRepository
import com.forge.app.data.repo.CoachTimeline
import com.forge.app.domain.coach.CoachOutcome
import com.forge.app.domain.coach.TypeTrust

/**
 * "Coach learning timeline" (Tier 6): how the coach has grown — the trust it's earned per change
 * type (segmented bars), the journey milestones it's hit, and the week-by-week record. Read-only.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CoachTimelineScreen(
    onBack: () -> Unit,
    viewModel: CoachTimelineViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("How I've learned.", style = MaterialTheme.typography.headlineMedium) },
                navigationIcon = {
                    IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        when {
            state.loading -> Box(Modifier.fillMaxSize().padding(inner), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
            state.error || state.timeline == null -> Box(
                Modifier.fillMaxSize().padding(inner).padding(horizontal = 32.dp),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    if (state.error) "Couldn't load the learning timeline — head back and try again."
                    else "No coaching history yet. Log a few weeks and the timeline fills in.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center
                )
            }
            else -> TimelineContent(state.timeline!!, Modifier.padding(inner))
        }
    }
}

@Composable
private fun TimelineContent(timeline: CoachTimeline, modifier: Modifier = Modifier) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val outline = MaterialTheme.colorScheme.outline
    val now = remember { System.currentTimeMillis() }

    Column(
        modifier = modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 24.dp)
    ) {
        Text(
            "The coach earns trust one accepted change at a time. Here's how far it's come.",
            style = MaterialTheme.typography.bodyMedium, color = onBg, fontStyle = FontStyle.Italic
        )

        // ── Trust earned (segmented bars) ─────────────────────────────────────
        Section("TRUST EARNED", outline, muted)
        if (timeline.trust.all { it.streak == 0 && !it.earned }) {
            Hint("No accepted proposals yet — every change you apply (and keep) builds a type's trust.", muted)
        }
        timeline.trust.forEach { t -> TrustRow(t, onBg, muted) }

        // ── Milestones ────────────────────────────────────────────────────────
        Section("MILESTONES", outline, muted)
        timeline.milestones.forEach { m -> MilestoneRow(m, onBg, muted) }

        // ── Week by week ──────────────────────────────────────────────────────
        Section("WEEK BY WEEK", outline, muted)
        if (timeline.weeks.isEmpty()) {
            Hint("No weekly passes yet.", muted)
        } else {
            timeline.weeks.forEach { week -> WeekBlock(week, now, onBg, muted) }
        }

        Spacer(Modifier.height(40.dp))
    }
}

@Composable
private fun TrustRow(t: TypeTrust, onBg: Color, muted: Color) {
    Column(Modifier.fillMaxWidth().padding(vertical = 8.dp)) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(t.label, style = MaterialTheme.typography.bodyMedium, color = onBg)
            Text(
                if (t.earned) "auto ✓" else "${t.streak}/${t.required}",
                style = MaterialTheme.typography.labelSmall,
                color = if (t.earned) MaterialTheme.colorScheme.primary else muted
            )
        }
        Spacer(Modifier.height(6.dp))
        TrustProgressBar(streak = t.streak, required = t.required, earned = t.earned, modifier = Modifier.fillMaxWidth())
        if (!t.earned) {
            Spacer(Modifier.height(4.dp))
            Text(
                "${(t.required - t.streak).coerceAtLeast(1)} more accepted week${if (t.required - t.streak == 1) "" else "s"} to auto-apply",
                style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.8f)
            )
        }
    }
}

@Composable
private fun MilestoneRow(m: CoachMilestone, onBg: Color, muted: Color) {
    Row(
        Modifier.fillMaxWidth().padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            if (m.reached) "●" else "○",
            style = MaterialTheme.typography.bodyMedium,
            color = if (m.reached) MaterialTheme.colorScheme.primary else muted.copy(alpha = 0.6f)
        )
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(
                m.label,
                style = MaterialTheme.typography.bodyMedium,
                color = if (m.reached) onBg else muted
            )
            Text(m.detail, style = MaterialTheme.typography.bodySmall, color = muted)
        }
    }
}

@Composable
private fun WeekBlock(week: CoachRepository.CoachHistoryEntry, now: Long, onBg: Color, muted: Color) {
    Text(
        "${week.pass.weekId.uppercase()} · ${week.pass.status.uppercase()}",
        style = MaterialTheme.typography.labelMedium, color = muted,
        modifier = Modifier.padding(top = 14.dp, bottom = 2.dp)
    )
    if (week.decisions.isEmpty()) {
        week.pass.holdReason?.let {
            Text(it, style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
        }
    } else {
        week.decisions.forEach { d ->
            val statusText = CoachOutcome.label(d.status, d.outcome, d.appliedAt, now) ?: d.status
            Text(
                "• ${d.summary} · $statusText",
                style = MaterialTheme.typography.bodySmall, color = muted,
                modifier = Modifier.padding(vertical = 2.dp)
            )
        }
    }
}

@Composable
private fun Section(title: String, outline: Color, muted: Color) {
    Spacer(Modifier.height(20.dp))
    HorizontalDivider(color = outline.copy(alpha = 0.3f))
    Spacer(Modifier.height(16.dp))
    Text(title, style = MaterialTheme.typography.labelMedium, color = muted)
    Spacer(Modifier.height(8.dp))
}

@Composable
private fun Hint(text: String, muted: Color) {
    Text(
        text, style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.8f),
        fontStyle = FontStyle.Italic, modifier = Modifier.padding(vertical = 4.dp)
    )
}
