package com.forge.app.ui.coach

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.forge.app.domain.coach.PersonalProfile
import com.forge.app.domain.coach.TypeTrust
import com.forge.app.ui.common.statsEntrance

/**
 * WHAT IT HAS LEARNED — the account's standing balance.
 *
 * A ledger closes on what is left after every entry: the trust each change type has earned, the
 * biases the coach now carries into every regenerated plan, and the numbers it measured about
 * this athlete specifically. None of it is dated, so none of it is an entry; it is the sum.
 */
internal fun LazyListScope.coachLearned(
    state: CoachViewModel.UiState,
    c: CoachColors
) {
    // Every change type is always in the ledger, so a non-empty `trust` proves nothing: a brand
    // new account has one row per type at zero. The section speaks only once a type has actually
    // started earning, a bias has been learned, or a number has been measured.
    val trust = state.timeline?.trust.orEmpty().filter { it.earned || it.streak > 0 }
    val biases = state.watch?.learnedBiases.orEmpty()
    val hasNumbers = state.profile.hasPersonalData
    if (trust.isEmpty() && biases.isEmpty() && !hasNumbers) return

    item("learned") {
        Column(Modifier.fillMaxWidth().padding(horizontal = COACH_GUTTER).statsEntrance(5)) {
            Spacer(Modifier.height(30.dp))
            CoachAnchor("Learned", c)
            Spacer(Modifier.height(18.dp))

            // ── Autopilot, earned per change type ────────────────────────────
            if (trust.isNotEmpty()) {
                val on = state.watch?.autopilot == true
                val earned = trust.count { it.earned }
                val total = state.timeline?.trust?.size ?: trust.size
                Row(
                    Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("AUTOPILOT", style = MaterialTheme.typography.labelLarge, color = c.muted)
                    Text(
                        "$earned OF $total EARNED",
                        style = MaterialTheme.typography.labelSmall,
                        color = c.muted
                    )
                }
                Spacer(Modifier.height(10.dp))
                Text(
                    if (on) "On. A change applies on its own once its type has earned it."
                    else "Off. Earned changes still wait for your tap.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = c.onBg
                )
                Spacer(Modifier.height(14.dp))
                // Each type carries its own distinct reading, which is what earns these a list.
                // They used to carry a segmented bar each as well: three identical rails stacked,
                // saying nothing the reading beside them did not already say.
                trust.forEach { t -> TrustRow(t, c) }
            }

            // ── The biases it carries ────────────────────────────────────────
            if (biases.isNotEmpty()) {
                Spacer(Modifier.height(28.dp))
                Text("BIASES", style = MaterialTheme.typography.labelLarge, color = c.muted)
                Spacer(Modifier.height(4.dp))
                Text(
                    "Carried into every regenerated plan.",
                    style = MaterialTheme.typography.bodySmall,
                    color = c.muted
                )
                Spacer(Modifier.height(12.dp))
                biases.forEach { b ->
                    Column(Modifier.padding(bottom = 10.dp)) {
                        Text(b.label, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
                        Text(b.detail, style = MaterialTheme.typography.bodySmall, color = c.muted)
                    }
                }
            }

            // ── The numbers it measured about you ────────────────────────────
            if (hasNumbers) {
                Spacer(Modifier.height(28.dp))
                Text("YOUR NUMBERS", style = MaterialTheme.typography.labelLarge, color = c.muted)
                Spacer(Modifier.height(12.dp))
                ProfileReadout(state.profile, c)
            }
        }
    }
}

/** One change type's trust: its label and how close it is to applying itself. */
@Composable
private fun TrustRow(t: TypeTrust, c: CoachColors) {
    Column(Modifier.fillMaxWidth().padding(vertical = COACH_ROW_PAD)) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Text(
                t.label,
                style = MaterialTheme.typography.bodyMedium,
                color = c.onBg,
                modifier = Modifier.weight(1f)
            )
            Spacer(Modifier.width(12.dp))
            Text(
                if (t.earned) "AUTO" else "${t.streak} OF ${t.required}",
                style = MaterialTheme.typography.labelSmall,
                color = c.muted
            )
        }
    }
}

/** What the coach has measured about this athlete — the numbers that replaced its defaults. */
@Composable
private fun ProfileReadout(profile: PersonalProfile.Profile, c: CoachColors) {
    Column(Modifier.fillMaxWidth()) {
        profile.recoveryDays?.let { days ->
            Text(
                "Best spacing: $days ${if (days == 1) "day" else "days"} between sessions",
                style = MaterialTheme.typography.bodyMedium,
                color = c.onBg,
                modifier = Modifier.padding(vertical = 3.dp)
            )
        }
        profile.volumeCaps.entries.take(3).forEach { (muscle, cap) ->
            Text(
                "${muscle.displayName}: up to $cap sets a week",
                style = MaterialTheme.typography.bodyMedium,
                color = c.onBg,
                modifier = Modifier.padding(vertical = 3.dp)
            )
        }
    }
}
