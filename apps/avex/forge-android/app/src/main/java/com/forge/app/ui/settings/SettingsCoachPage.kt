package com.forge.app.ui.settings

import androidx.compose.foundation.layout.Column
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import com.forge.app.ui.common.clickableLabeled

/**
 * Coach settings — configuration only. The coach's content (this week's brief, the trust ledger,
 * the record with undo) lives on the Coach tab. What configures it: the master switch, the feeds
 * it reads (a silent Health Connect feed taps through to Wearable, where it is switched on), the
 * suggest-or-earn mode, and how much of the Coach page is drawn.
 */
@Composable
internal fun CoachSettingsPage(
    state: SettingsUiState,
    vm: SettingsViewModel,
    onOpenRecovery: () -> Unit,
    onBack: () -> Unit,
) {
    // Trust is loaded to count the change types that have earned auto-apply; the bars themselves
    // render on the Coach tab.
    val trust by vm.coachTrust.collectAsState()
    val signals by vm.coachSignals.collectAsState()
    LaunchedEffect(Unit) { vm.loadCoachData() }

    SettingsScaffold(SettingsPage.Coach.title, onBack) {
        Column {
            SettingsMainSwitch("Use the coach", state.coachEnabled) { vm.setCoachEnabled(it) }
            SettingsGroupFooter(
                if (state.coachEnabled) "Its own tab, and a weekly call on your training that you apply or skip."
                else "Off. Your plan stays exactly as you set it."
            )
        }

        if (state.coachEnabled) {
            // States and labels come verbatim from CoachRepository.coachLab(), the same list the
            // Coach tab's Signals lens reads.
            if (signals.isNotEmpty()) {
                SettingsGroup(
                    "What it reads",
                    headerTrailing = "${signals.count { it.active }} of ${signals.size} receiving",
                    footer = "More feeds sharpen the weekly call. Check-ins and flags come from your logging."
                ) {
                    signals.forEach { sig ->
                        CoachFeedRow(
                            label = sig.label,
                            active = sig.active,
                            onConnect = if (sig.label in HC_FEEDS) onOpenRecovery else null
                        )
                    }
                }
            }

            // "Earn auto-apply" is a target, not an on-switch: nothing applies itself until a change
            // type builds its accepted streak. The count waits for loadCoachData so it never
            // flashes a false zero.
            val earned = trust.count { it.earned }
            SettingsGroup("Mode") {
                SettingsRadioRow("Suggest", state.coachMode != "auto", "Every change waits for your tap") {
                    vm.setCoachMode("suggest")
                }
                SettingsRadioRow(
                    "Earn auto-apply",
                    state.coachMode == "auto",
                    "A change type applies itself once you've accepted it enough times in a row" +
                        if (trust.isNotEmpty()) ". $earned of ${trust.size} earned so far." else ""
                ) { vm.setCoachMode("auto") }
            }

            // How much of the Coach page is drawn. Nothing about the coach's behaviour changes.
            SettingsGroup("Coach page") {
                SettingsSwitchRow(
                    "Advanced tracking",
                    "Also show signals, the training block, inputs and what it has learned",
                    state.coachAdvanced
                ) { vm.setCoachAdvanced(it) }
            }
        }
    }
}

/** The coach feeds whose switch lives on the Wearable page (one Health Connect grant). */
private val HC_FEEDS = setOf("Sleep", "Resting heart rate")

/**
 * One coach feed and whether it is coming in. A silent Health Connect feed is the fixable kind, so
 * its row taps through to Wearable with a drawn Connect pill; the rest are readings only.
 */
@Composable
private fun CoachFeedRow(label: String, active: Boolean, onConnect: (() -> Unit)?) {
    val tappable = onConnect != null && !active
    SettingsAdaptiveRow(
        title = label,
        interaction = if (tappable) Modifier.clickableLabeled("Connect $label in Wearable") { onConnect?.invoke() } else Modifier
    ) {
        when {
            active -> SettingsStatus("Receiving", live = true)
            tappable -> SettingsCompactButton("Connect")
            else -> SettingsStatus("Nothing yet", live = false)
        }
    }
}
