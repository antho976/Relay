package com.forge.app.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.clickableLabeled

/**
 * Coach settings — configuration ONLY. The coach's content (this week's brief, the trust ledger,
 * the week-by-week record with undo) lives on the Coach tab (Now/Journey lenses); mirroring it
 * here made a second coach page. What configures the coach: the master switch, the
 * suggest-vs-auto mode, and an at-a-glance of the input feeds it needs (a silent Health Connect
 * feed taps through to Recovery, where it's switched on).
 */
@Composable
internal fun CoachSettingsPage(
    state: SettingsUiState,
    vm: SettingsViewModel,
    onOpenRecovery: () -> Unit,
    modifier: Modifier = Modifier
) {
    // Trust is loaded only to gate the "auto isn't active yet" note — the bars themselves render
    // on the Coach tab's Journey lens.
    val trust by vm.coachTrust.collectAsState()
    val signals by vm.coachSignals.collectAsState()
    LaunchedEffect(Unit) { vm.loadCoachData() }

    Column(modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        // The top bar never names the screen (§2) — the page opens with its own mono anchor.
        SettingsSectionHeader("Your coach", top = 12.dp)
        ToggleRow(
            "Coach",
            "When on, the coach gets its own tab and suggests weekly tweaks.",
            state.coachEnabled
        ) { vm.setCoachEnabled(it) }

        // The feeds and mode only mean something while the coach is on. Feeds lead (the live
        // on/off truth); mode — a preference, not a state — sits last.
        if (state.coachEnabled) {
            // The feeds glance (§12 filled-disc/muted-ring idiom, same drawing as Recovery's rail):
            // which of the coach's inputs are coming in. States and labels come verbatim from
            // CoachRepository.coachLab() — the same feed list the Coach tab's Signals lens reads;
            // there it carries readings and charts, here only the on/off config truth. The Health
            // Connect feeds are the fixable ones, so a silent one taps through to Recovery.
            if (signals.isNotEmpty()) {
                ProgramBlock("What it reads", "Filled feeds sharpen the weekly call; check-ins and flags come from your logging.") {
                    signals.forEach { sig ->
                        CoachFeedRow(
                            label = sig.label,
                            active = sig.active,
                            onConnect = if (sig.label in HC_FEEDS) onOpenRecovery else null
                        )
                    }
                }
            }

            ProgramBlock("Coach mode", "Suggest asks every time; earn auto-apply lets trusted change types self-apply.") {
                ChipFlow {
                    PillChip("Suggest", state.coachMode != "auto") { vm.setCoachMode("suggest") }
                    PillChip("Earn auto-apply", state.coachMode == "auto") { vm.setCoachMode("auto") }
                }
                // "Earn auto-apply" is a TARGET, not an on-switch: until a change type builds its
                // accepted streak, nothing self-applies. Say so explicitly so the user never believes
                // they've handed over control prematurely. trust.isNotEmpty() gates out the load
                // window (coachTrust starts empty, so without it the note would flash until
                // loadCoachData populates it).
                if (state.coachMode == "auto" && trust.isNotEmpty() && trust.none { it.earned }) {
                    Spacer(Modifier.height(8.dp))
                    Text(
                        "Nothing self-applies yet. Every proposal waits for your tap until a change type earns its streak, tracked on the Coach tab.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        fontStyle = FontStyle.Italic,
                        modifier = Modifier.padding(horizontal = 24.dp)
                    )
                }
            }
        }
        Spacer(Modifier.height(24.dp))
    }
}

/** The coach feeds whose switch lives on the Recovery page (one Health Connect grant). */
private val HC_FEEDS = setOf("Sleep", "Resting heart rate")

/**
 * One coach feed: the §12 dot (solid accent = feeding, muted ring = silent) + its name. A silent
 * Health Connect feed carries a compact outlined Connect pill (§8 — drawn, not independently
 * clickable) and the whole row taps through to Recovery; live and logging-born feeds are passive.
 */
@Composable
private fun CoachFeedRow(label: String, active: Boolean, onConnect: (() -> Unit)?) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val tappable = onConnect != null && !active
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .then(
                if (tappable) Modifier.clickableLabeled("Connect $label in Recovery", onClick = { onConnect?.invoke() })
                else Modifier
            )
            .padding(horizontal = 24.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        StatusDot(active)
        Spacer(Modifier.width(12.dp))
        Text(
            label,
            style = MaterialTheme.typography.bodyMedium,
            color = onBg,
            modifier = Modifier.weight(1f)
        )
        if (tappable) ConnectPill()
    }
}
