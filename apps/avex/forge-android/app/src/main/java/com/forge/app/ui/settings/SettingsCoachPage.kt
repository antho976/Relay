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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.ForgeOptionCard
import com.forge.app.ui.common.clickableLabeled
import androidx.compose.foundation.layout.Arrangement

/**
 * Coach settings — configuration ONLY. The coach's content (this week's brief, the trust ledger,
 * the week-by-week record with undo) lives on the Coach tab (Now/Journey lenses); mirroring it
 * here made a second coach page. What configures the coach: the master switch, the
 * suggest-vs-auto mode, the advanced-tracking switch that decides how much of the Coach page is
 * drawn, and an at-a-glance of the input feeds it needs (a silent Health Connect feed taps through
 * to Wearable, where it's switched on).
 */
@Composable
internal fun CoachSettingsPage(
    state: SettingsUiState,
    vm: SettingsViewModel,
    onOpenRecovery: () -> Unit,
    modifier: Modifier = Modifier,
) {
    // Trust is loaded only to gate the "auto isn't active yet" note — the bars themselves render
    // on the Coach tab's Journey lens.
    val trust by vm.coachTrust.collectAsState()
    val signals by vm.coachSignals.collectAsState()
    LaunchedEffect(Unit) { vm.loadCoachData() }

    Column(modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        // The title line is a reading, not a restatement of the switch below it: how many of the
        // coach's inputs are actually coming in (§4.9). Off, it names what "off" means for the plan.
        SettingsPageTitle(
            "Your coach",
            when {
                !state.coachEnabled -> "Off. Your plan stays exactly as you set it."
                signals.isEmpty() -> "Weekly calls on your training, each one yours to apply."
                else -> "Reads ${signals.count { it.active }} of ${signals.size} feeds"
            }
        )
        ToggleRow(
            "Coach",
            "Its own tab, and a weekly call on your training.",
            state.coachEnabled
        ) { vm.setCoachEnabled(it) }

        // The feeds and mode only mean something while the coach is on. Feeds lead (the live
        // on/off truth); mode — a preference, not a state — follows.
        if (state.coachEnabled) {
            // The feeds glance (§12 filled-disc/muted-ring idiom, same drawing as Wearable's rail).
            // States and labels come verbatim from CoachRepository.coachLab() — the same feed list
            // the Coach tab's Signals lens reads; here only the on/off config truth. The Health
            // Connect feeds are the fixable ones, so a silent one taps through to Wearable.
            if (signals.isNotEmpty()) {
                SettingsSectionHeader("What it reads")
                SettingsCaption("Filled feeds sharpen the weekly call. Check-ins and flags come from your logging.")
                signals.forEach { sig ->
                    CoachFeedRow(
                        label = sig.label,
                        active = sig.active,
                        onConnect = if (sig.label in HC_FEEDS) onOpenRecovery else null
                    )
                }
            }

            SettingsSectionHeader("Mode")
            // "Earn auto-apply" is a TARGET, not an on-switch: until a change type builds its accepted
            // streak, nothing self-applies. Its card carries that as a READING — how many change types
            // have earned it — instead of the paragraph that used to explain it (§4.9). The count
            // waits for loadCoachData (trust starts empty) so it never flashes a false "0".
            val earned = trust.count { it.earned }
            CoachModeCards(
                auto = state.coachMode == "auto",
                autoMeta = if (trust.isNotEmpty()) "$earned of ${trust.size} earned" else null,
                onSelect = { vm.setCoachMode(if (it) "auto" else "suggest") }
            )

            // How much of the Coach page to draw. Off, the page is the account alone: the calls and
            // what became of them. On, the readings behind the calls come back: signals, the block,
            // the inputs with their charts, and what the coach has learned. Nothing about the coach's
            // behaviour changes either way. The Coach page closes on the same switch.
            SettingsSectionHeader("Coach page")
            ToggleRow(
                "Advanced tracking",
                "Also show signals, block, inputs and what it has learned.",
                state.coachAdvanced
            ) { vm.setCoachAdvanced(it) }
        }
        Spacer(Modifier.height(32.dp))
    }
}

/** The two coach modes as option cards, each carrying what it means in one line. */
@Composable
private fun CoachModeCards(auto: Boolean, autoMeta: String?, onSelect: (Boolean) -> Unit) {
    Column(
        Modifier.fillMaxWidth().padding(horizontal = SETTINGS_GUTTER),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        ForgeOptionCard(
            label = "Suggest",
            description = "Every change waits for your tap.",
            selected = !auto,
            onClick = { onSelect(false) }
        )
        ForgeOptionCard(
            label = "Earn auto-apply",
            description = "A change type applies itself once you've accepted it enough times in a row.",
            meta = autoMeta,
            selected = auto,
            onClick = { onSelect(true) }
        )
    }
}

/** The coach feeds whose switch lives on the Wearable page (one Health Connect grant). */
private val HC_FEEDS = setOf("Sleep", "Resting heart rate")

/**
 * One coach feed: the §12 dot (solid accent = feeding, muted ring = silent) + its name. A silent
 * Health Connect feed carries a compact outlined Connect pill (§8 — drawn, not independently
 * clickable) and the whole row taps through to Wearable; live and logging-born feeds are passive.
 */
@Composable
private fun CoachFeedRow(label: String, active: Boolean, onConnect: (() -> Unit)?) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val tappable = onConnect != null && !active
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .then(
                if (tappable) Modifier.clickableLabeled("Connect $label in Wearable", onClick = { onConnect?.invoke() })
                else Modifier
            )
            .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD),
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
