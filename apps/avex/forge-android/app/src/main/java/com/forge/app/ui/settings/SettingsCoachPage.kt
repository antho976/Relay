@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import com.forge.app.ui.coach.TrustProgressBar
import com.forge.app.ui.common.clickableLabeled

/**
 * Coach configuration — pulled out of the Program page (it governs ongoing weekly adjustments, not
 * building a plan). Master on/off, a jump to the week brief, the suggest-vs-auto mode, and the trust
 * ledger + pass history (with per-change undo). Reuses the settings [ProgramBlock] shape so it reads
 * as one voice with the rest of Settings.
 */
@Composable
internal fun CoachSettingsPage(
    state: SettingsUiState,
    vm: SettingsViewModel,
    onOpenCoachBrief: () -> Unit,
    modifier: Modifier = Modifier
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val trust by vm.coachTrust.collectAsState()
    val history by vm.coachHistory.collectAsState()
    val now = remember { System.currentTimeMillis() }
    LaunchedEffect(Unit) { vm.loadCoachData() }

    Column(modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        Spacer(Modifier.height(12.dp))
        ProgramBlock("Coach", "When on, the coach gets its own tab and suggests weekly tweaks.") {
            ChipFlow {
                PillChip("On", state.coachEnabled) { vm.setCoachEnabled(true) }
                PillChip("Off", !state.coachEnabled) { vm.setCoachEnabled(false) }
            }
        }

        // Everything below configures the coach — only meaningful while it's enabled.
        if (state.coachEnabled) {
            ProgramBlock("This week's brief", "Your coach's read on the week: last week's numbers, changes, and a focus.") {
                SettingsActionLink("View brief →") { onOpenCoachBrief() }
            }

            ProgramBlock("Coach mode", "Suggest asks every time; earn auto-apply lets trusted change types self-apply.") {
                ChipFlow {
                    PillChip("Suggest", state.coachMode != "auto") { vm.setCoachMode("suggest") }
                    PillChip("Earn auto-apply", state.coachMode == "auto") { vm.setCoachMode("auto") }
                }
                // "Earn auto-apply" is a TARGET, not an on-switch: until a change type builds its accepted
                // streak, nothing self-applies. Say so explicitly so the user never believes they've handed
                // over control prematurely. trust.isNotEmpty() gates out the load window (coachTrust starts
                // empty, so without it the note would flash until loadCoachData populates trust).
                if (state.coachMode == "auto" && trust.isNotEmpty() && trust.none { it.earned }) {
                    Spacer(Modifier.height(8.dp))
                    Text(
                        "Auto-apply isn't active yet. Every proposal still waits for your tap until a change type earns its streak below.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.primary,
                        fontStyle = FontStyle.Italic,
                        modifier = Modifier.padding(horizontal = 24.dp)
                    )
                }
            }

            ProgramBlock("Earned trust", "What the coach has earned so far, per change type.") {
                Column(Modifier.padding(horizontal = 24.dp)) {
                    if (trust.all { it.streak == 0 }) {
                        Text(
                            "No accepted proposals yet. Trust builds as you apply weekly suggestions.",
                            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
                        )
                    }
                    trust.forEach { t ->
                        Column(Modifier.fillMaxWidth().padding(vertical = 6.dp)) {
                            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                                Text(t.label, style = MaterialTheme.typography.bodySmall, color = onBg)
                                Text(
                                    when {
                                        t.earned -> "auto ✓"
                                        // Next milestone: how many more accepted weeks unlock auto-apply for this type.
                                        else -> "${t.streak}/${t.required} · ${(t.required - t.streak).coerceAtLeast(1)} more to auto-apply"
                                    },
                                    style = MaterialTheme.typography.labelSmall,
                                    color = if (t.earned) MaterialTheme.colorScheme.primary else muted
                                )
                            }
                            Spacer(Modifier.height(6.dp))
                            TrustProgressBar(
                                streak = t.streak, required = t.required, earned = t.earned,
                                modifier = Modifier.fillMaxWidth()
                            )
                        }
                    }
                }
            }

            ProgramBlock("History", "Every weekly pass, including holds. Applied changes can be undone here.") {
                Column(Modifier.padding(horizontal = 24.dp)) {
                    if (history.isEmpty()) {
                        Text("No passes yet.", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
                    }
                    history.forEach { entry ->
                        Text(
                            "${entry.pass.weekId.uppercase()} · ${entry.pass.status.uppercase()}",
                            style = MaterialTheme.typography.labelSmall, color = onBg,
                            modifier = Modifier.padding(top = 10.dp)
                        )
                        if (entry.decisions.isEmpty()) {
                            entry.pass.holdReason?.let {
                                Text(it, style = MaterialTheme.typography.bodySmall, color = muted)
                            }
                        }
                        entry.decisions.forEach { d ->
                            Row(
                                Modifier.fillMaxWidth().padding(vertical = 2.dp),
                                horizontalArrangement = Arrangement.SpaceBetween,
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                // Plain-English status: the watcher window/verdict for applied changes, else the raw status.
                                val statusText = com.forge.app.domain.coach.CoachOutcome
                                    .label(d.status, d.outcome, d.appliedAt, now) ?: d.status
                                Text(
                                    "${d.summary} · $statusText",
                                    style = MaterialTheme.typography.bodySmall, color = muted,
                                    modifier = Modifier.weight(1f)
                                )
                                if (d.status == "applied" && d.undoData != null) {
                                    Text(
                                        "undo",
                                        style = MaterialTheme.typography.labelSmall,
                                        color = MaterialTheme.colorScheme.primary,
                                        modifier = Modifier
                                            .clickableLabeled("Undo this change") { vm.undoCoachDecision(d.id) }
                                            .padding(start = 12.dp, top = 2.dp, bottom = 2.dp)
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
        Spacer(Modifier.height(24.dp))
    }
}
