package com.forge.app.ui.coach

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
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
import com.forge.app.domain.coach.CoachOutcome
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.repo.CoachBrief
import com.forge.app.data.repo.CoachRepository
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.emphasized

/**
 * The Week Brief (auto-coach Phase 1) — a coach's letter for the week, editorial hairline
 * style. Shadow mode: the "what I'd change" section is observations only; nothing applies
 * until Phase 3's propose mode.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CoachBriefScreen(
    onBack: () -> Unit,
    onOpenCoachLab: () -> Unit = {},
    viewModel: CoachBriefViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Week brief.", style = MaterialTheme.typography.headlineMedium) },
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
            state.brief == null -> Box(Modifier.fillMaxSize().padding(inner).padding(horizontal = 32.dp), contentAlignment = Alignment.Center) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(
                        "Your coach is still learning. It writes your first brief after about a week of " +
                            "training — finish a few workouts and it'll appear here.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = androidx.compose.ui.text.style.TextAlign.Center
                    )
                    Spacer(Modifier.height(16.dp))
                    Text(
                        "See what it's tracking →",
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.clickable { onOpenCoachLab() }.padding(vertical = 4.dp)
                    )
                }
            }
            else -> BriefContent(state.brief!!, viewModel, onOpenCoachLab, Modifier.padding(inner))
        }
    }
}

@Composable
private fun BriefContent(brief: CoachBrief, viewModel: CoachBriefViewModel, onOpenCoachLab: () -> Unit, modifier: Modifier = Modifier) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val outline = MaterialTheme.colorScheme.outline
    val useKg = LocalForgeSettings.current.useKg

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 24.dp)
    ) {
        Text(brief.pass.weekId.uppercase(), style = MaterialTheme.typography.labelMedium, color = emphasized(muted))
        Spacer(Modifier.height(4.dp))
        Text(
            when (brief.pass.status) {
                CoachRepository.STATUS_PROPOSED, CoachRepository.STATUS_APPLIED ->
                    "Proposals — nothing changes until you apply it. Every applied change can be undone."
                else -> "The coach is watching and learning — anything it would change shows up here as a suggestion. Nothing changes unless you apply it."
            },
            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
        )

        // ── Last week ─────────────────────────────────────────────────────────
        brief.review?.let { r ->
            Spacer(Modifier.height(24.dp))
            Text("Last week.", style = MaterialTheme.typography.headlineSmall, color = onBg)
            Spacer(Modifier.height(12.dp))
            BriefStatRow("Sessions", "${r.sessionsLastWeek} of ${r.sessionsTarget}")
            BriefStatRow(
                "Volume",
                formatVolume(r.volumeLastWeekLb, useKg) + (r.volumeDeltaPct?.let { d ->
                    "  (${if (d >= 0) "+" else ""}$d% vs prior)"
                } ?: "")
            )
            BriefStatRow("PRs", "${r.prsLastWeek}")
            if (r.trackedLifts > 0) BriefStatRow("Lifts", "${r.trackedLifts} tracked · ${r.stalledLifts} stalled")
            BriefStatRow("Recovery", r.fatigueBand + (r.fatigueScore?.let { "  (fatigue $it)" } ?: ""))
        }

        // ── What I'd change ───────────────────────────────────────────────────
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.3f))
        Spacer(Modifier.height(16.dp))
        Text("WHAT I'D CHANGE", style = MaterialTheme.typography.labelMedium, color = emphasized(muted))
        Spacer(Modifier.height(10.dp))
        when (brief.pass.status) {
            CoachRepository.STATUS_PROPOSED, CoachRepository.STATUS_SHADOW -> {
                brief.decisions.forEach { d -> DecisionRow(d, viewModel) }
                val openCount = brief.decisions.count { it.status == CoachRepository.STATUS_PROPOSED }
                if (openCount > 1) {
                    Spacer(Modifier.height(2.dp))
                    Text(
                        "Apply all ($openCount) →",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier
                            .clickable { viewModel.applyAll(brief.pass.weekId) }
                            .padding(vertical = 4.dp)
                    )
                }
            }
            CoachRepository.STATUS_ERROR -> Text(
                brief.pass.holdReason ?: "The pass failed this week.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.error
            )
            else -> Text(
                brief.pass.holdReason ?: "No changes this week.",
                style = MaterialTheme.typography.bodyMedium, color = onBg
            )
        }

        // ── Focus ─────────────────────────────────────────────────────────────
        brief.review?.let { r ->
            Spacer(Modifier.height(10.dp))
            HorizontalDivider(color = outline.copy(alpha = 0.3f))
            Spacer(Modifier.height(16.dp))
            Text("FOCUS", style = MaterialTheme.typography.labelMedium, color = emphasized(muted))
            Spacer(Modifier.height(8.dp))
            Text(r.focusLine, style = MaterialTheme.typography.bodyMedium, color = onBg, fontStyle = FontStyle.Italic)
        }

        // ── What I'm watching (Coach lab) ─────────────────────────────────────
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.3f))
        Spacer(Modifier.height(12.dp))
        Text(
            "What I'm watching →",
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.primary,
            modifier = Modifier.clickable { onOpenCoachLab() }.padding(vertical = 4.dp)
        )

        Spacer(Modifier.height(40.dp))
    }
}

@Composable
private fun DecisionRow(d: com.forge.app.data.db.entities.CoachDecision, viewModel: CoachBriefViewModel) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val accent = MaterialTheme.colorScheme.primary
    val now = remember { System.currentTimeMillis() }

    Column(Modifier.fillMaxWidth().padding(bottom = 14.dp)) {
        Text(d.summary, style = MaterialTheme.typography.bodyMedium, color = onBg)
        Spacer(Modifier.height(2.dp))
        Text(d.reason, style = MaterialTheme.typography.bodySmall, color = muted)
        Spacer(Modifier.height(6.dp))
        when (d.status) {
            CoachRepository.STATUS_PROPOSED -> Row(horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                Text(
                    "Apply →", style = MaterialTheme.typography.labelSmall, color = accent,
                    modifier = Modifier.clickable { viewModel.apply(d.id) }.padding(vertical = 2.dp)
                )
                Text(
                    "skip", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.7f),
                    modifier = Modifier.clickable { viewModel.skip(d.id) }.padding(vertical = 2.dp)
                )
            }
            CoachRepository.STATUS_APPLIED -> Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(20.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text("applied ✓", style = MaterialTheme.typography.labelSmall, color = muted)
                // Surface the otherwise-invisible 14-day watcher: "still watching · ~N days left" → verdict.
                // weight(1f) so the (possibly long) watcher label takes the slack and never pushes the
                // 'undo' tap target off-screen on a narrow display.
                CoachOutcome.label(d.status, d.outcome, d.appliedAt, now)?.let { wl ->
                    Text(wl, style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.7f),
                        modifier = Modifier.weight(1f, fill = false))
                }
                if (d.undoData != null) Text(
                    "undo", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.7f),
                    modifier = Modifier.clickable { viewModel.undo(d.id) }.padding(vertical = 2.dp)
                )
            }
            "reverted" -> Text("undone", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.7f))
            CoachRepository.STATUS_SKIPPED -> Text("skipped", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.7f))
            // A regenerate has baked this accepted change into the program itself — it lives in the
            // baseline now, not as a reversible overlay, so there's nothing left to undo.
            CoachRepository.STATUS_FOLDED -> Text("absorbed ✓", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.7f))
            else -> {} // shadow rows (Phase 1 history): display only
        }
    }
}

@Composable
private fun BriefStatRow(label: String, value: String) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Text(label, style = MaterialTheme.typography.bodySmall, color = muted)
        Text(value, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onBackground)
    }
}

private fun formatVolume(lb: Double, useKg: Boolean): String = formatVolumeCompact(lb, useKg)
