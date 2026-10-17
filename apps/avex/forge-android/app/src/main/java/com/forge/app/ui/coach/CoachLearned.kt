package com.forge.app.ui.coach

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextAlign
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
        CoachSection("Learned", modifier = Modifier.statsEntrance(7)) {
            // ── Autopilot, earned per change type ────────────────────────────
            if (trust.isNotEmpty()) {
                val on = state.watch?.autopilot == true
                val earned = trust.count { it.earned }
                val total = state.timeline?.trust?.size ?: trust.size
                CoachPanel(c) {
                    CoachFigure(
                        "$earned of $total earned",
                        if (on) "Autopilot on. A change applies on its own once its type has earned it."
                        else "Autopilot off. Earned changes still wait for your tap.",
                        c
                    )
                    Spacer(Modifier.height(10.dp))
                    // Each type carries its own distinct reading, which is what earns these a list.
                    trust.forEach { t -> TrustRow(t, c) }
                }
            }

            // ── The biases it carries ────────────────────────────────────────
            if (biases.isNotEmpty()) {
                Spacer(Modifier.height(18.dp))
                Text(
                    "Carried into every regenerated plan",
                    style = MaterialTheme.typography.bodySmall,
                    color = c.muted,
                    modifier = Modifier.padding(horizontal = 4.dp)
                )
                Spacer(Modifier.height(8.dp))
                CoachRows(c, biases.map { b ->
                    @Composable {
                        Text(b.label, style = MaterialTheme.typography.bodyMedium, color = c.onBg)
                        Text(b.detail, style = MaterialTheme.typography.bodySmall, color = c.muted)
                    }
                })
            }

            // ── The numbers it measured about you ────────────────────────────
            if (hasNumbers) {
                Spacer(Modifier.height(18.dp))
                Text(
                    "Measured from your training",
                    style = MaterialTheme.typography.bodySmall,
                    color = c.muted,
                    modifier = Modifier.padding(horizontal = 4.dp)
                )
                Spacer(Modifier.height(8.dp))
                val rows = buildList<@Composable () -> Unit> {
                    state.profile.recoveryDays?.let { days ->
                        add { NumberRow("Best spacing", "$days ${if (days == 1) "day" else "days"} between sessions", c) }
                    }
                    state.profile.volumeCaps.entries.take(3).forEach { (muscle, cap) ->
                        add { NumberRow(muscle.displayName, "Up to $cap sets a week", c) }
                    }
                }
                if (rows.isNotEmpty()) CoachRows(c, rows)
            }
        }
    }
}

/**
 * One change type's trust: its label, and its streak as a row of pips toward the autopilot line.
 * The pips are the reading ("2 of 3" drawn rather than set in mono caps); an earned type swaps them
 * for the word, since a full row would say the same thing less plainly.
 */
@Composable
private fun TrustRow(t: TypeTrust, c: CoachColors) {
    Row(
        Modifier
            .fillMaxWidth()
            .semantics(mergeDescendants = true) {
                contentDescription = if (t.earned) "${t.label}, applies on its own"
                else "${t.label}, ${t.streak} of ${t.required} toward autopilot"
            }
            .padding(vertical = COACH_ROW_PAD + 2.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            t.label,
            style = MaterialTheme.typography.bodyMedium,
            color = c.onBg,
            modifier = Modifier.weight(1f)
        )
        Spacer(Modifier.width(12.dp))
        if (t.earned) {
            Text("Applies on its own", style = MaterialTheme.typography.bodySmall, color = c.onBg)
        } else {
            val required = t.required.coerceAtLeast(1)
            Row(horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                repeat(required) { i ->
                    Box(
                        Modifier
                            .size(7.dp)
                            .clip(RoundedCornerShape(50))
                            .background(if (i < t.streak) c.accent else c.track)
                    )
                }
            }
        }
    }
}

@Composable
private fun NumberRow(label: String, value: String, c: CoachColors) {
    Row(
        Modifier
            .fillMaxWidth()
            .semantics(mergeDescendants = true) {},
        verticalAlignment = Alignment.Top
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, color = c.muted, modifier = Modifier.weight(1f))
        Spacer(Modifier.width(12.dp))
        Text(
            value,
            style = MaterialTheme.typography.bodyMedium.copy(fontFeatureSettings = "tnum"),
            color = c.onBg,
            textAlign = TextAlign.End,
            modifier = Modifier.weight(1f, fill = false)
        )
    }
}
