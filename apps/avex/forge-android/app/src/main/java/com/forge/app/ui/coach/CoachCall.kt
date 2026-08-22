package com.forge.app.ui.coach

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.domain.adapt.DeloadAdvisor
import com.forge.app.ui.common.ForgePrimaryCapsule

/** The change a call makes, decomposed so the tile can rank it instead of printing a sentence. */
internal data class CallCopy(val subject: String, val change: String, val reason: String)

/**
 * One decision as subject + change + reason.
 *
 * The stored `summary` is a whole imperative sentence ("Add a set to Bench Press (3 → 4)"), which
 * is why every proposal used to render as one more line of body text. Split into its parts, the
 * change itself ("3 → 4 sets") can carry the serif rung and the lift can carry the title rung, so
 * a proposal reads as a decision at a glance rather than as prose to be parsed. Anything that
 * cannot be decomposed falls back to the summary, whole and unedited.
 */
internal fun callCopy(d: CoachDecision): CallCopy {
    val subject = if (d.targetKey == "week") "This week" else d.targetName
    val swing = Regex("""\((\d+)\s*→\s*(\d+)\)""").find(d.summary)
    val change = when (d.type) {
        "deload" -> "Deload week"
        "swap" -> d.summary.substringAfter("→", "").trim()
            .takeIf { it.isNotBlank() }?.let { "Rotate to $it" } ?: "Rotate out"
        "rep_shift" -> d.payload?.takeIf { it.isNotBlank() }?.let { "Move to $it reps" }
            ?: "New rep range"
        "volume_up", "volume_down" ->
            swing?.let { "${it.groupValues[1]} → ${it.groupValues[2]} sets" }
                ?: if (d.type == "volume_up") "Add a set" else "Drop a set"
        "revert" -> "Undo the last change"
        // Legacy and future types (weight_nudge among them) fall back to the stored sentence, so
        // the subject is stripped out of it first: the tile already prints the lift above.
        else -> d.summary.withoutSubject(d.targetName)
    }
    // Stored reasons are machine prose, translated at the seam rather than at the source
    // (pass rows are immutable). The literals live once, in humanizeMachineProse.
    val reason = humanizeMachineProse(d.reason).trim()
    return CallCopy(subject, change, reason)
}

/**
 * "Add load to Back Squat (275 → 285)" → "Add load (275 → 285)".
 *
 * The tile and the account entry both print the lift as their own line, so a summary that names it
 * again reads as a stutter ("Back Squat · Add load to Back Squat"). Falls back to the untouched
 * sentence whenever stripping would leave nothing meaningful.
 */
private fun String.withoutSubject(name: String): String {
    if (name.isBlank() || !contains(name, ignoreCase = true)) return this
    val stripped = replace(name, "", ignoreCase = true)
        .replace(Regex("""\s+(to|from|for|on)\s+(?=\(|$)"""), " ")
        .replace(Regex("""\s{2,}"""), " ")
        .trim().trimEnd(':', ' ')
    return stripped.ifBlank { this }.replaceFirstChar { it.uppercaseChar() }
}

/**
 * An open call: the ONE element on this page with a body.
 *
 * Everything else on the account is passive and therefore bare. A surface here is not decoration,
 * it is rank made visible — the only entry still asking the user for something is the only entry
 * that looks like it can be acted on. When it resolves it loses the body and becomes a stamped
 * line on the same spine, which is the page's one authored moment.
 */
@Composable
internal fun CoachCallTile(
    d: CoachDecision,
    state: CoachViewModel.UiState,
    c: CoachColors,
    onApply: (Long) -> Unit,
    onSkip: (Long) -> Unit
) {
    val copy = callCopy(d)
    CoachTile(c) {
        Text(copy.subject, style = MaterialTheme.typography.titleMedium, color = c.onBg)
        Spacer(Modifier.height(4.dp))
        // The change is the tile's one big thing. A numeric swing is short and takes the 28sp
        // rung; a rotation names a whole exercise and steps down to 22 so it does not wrap to
        // three lines on a 360dp screen. Size is the hierarchy here — colour is not spent on it.
        Text(
            copy.change,
            style = if (copy.change.length <= 18) MaterialTheme.typography.headlineMedium
            else MaterialTheme.typography.headlineSmall,
            color = c.onBg
        )
        Spacer(Modifier.height(10.dp))
        Text(
            copy.reason,
            style = MaterialTheme.typography.bodyMedium,
            color = c.muted
        )

        CallEvidence(d, state, c)

        Spacer(Modifier.height(18.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            // Every open call commits the same way. Demoting the second one to the outlined rung
            // put a 1.1:1 border on the tile fill and left a primary action with no visible edge,
            // which is a worse failure than the one it was avoiding. Two filled capsules on one
            // screen are two decisions owed, and the count of them is information, not noise.
            ForgePrimaryCapsule(
                label = "Apply",
                onClick = { onApply(d.id) },
                modifier = Modifier.weight(1f)
            )
            Spacer(Modifier.width(8.dp))
            CoachAction("Skip", c.muted, "Skip this change") { onSkip(d.id) }
            Spacer(Modifier.width(8.dp))
        }
    }
}

/**
 * The page's one body: a filled surface, spent only on an entry still asking the user for
 * something. Everything passive on the account is bare, so a fill here is not decoration, it is
 * rank made visible.
 */
@Composable
internal fun CoachTile(c: CoachColors, content: @Composable ColumnScope.() -> Unit) {
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(16.dp))
            .background(c.tile)
            .padding(horizontal = 18.dp, vertical = 18.dp),
        content = content
    )
}

/**
 * The reading the call was made from, drawn inside the call itself.
 *
 * This is the whole argument for folding the old Signals lens away: the evidence for a decision
 * belongs to that decision, not to a separate tab the user has to go and correlate by hand. A
 * lift call shows that lift's strength trend at full width; a deload call shows the recovery
 * meter and the checks that actually fired, with their own readings.
 */
@Composable
private fun CallEvidence(
    d: CoachDecision,
    state: CoachViewModel.UiState,
    c: CoachColors
) {
    if (d.type == "deload") {
        val watch = state.watch ?: return
        val score = watch.fatigueScore ?: return
        Spacer(Modifier.height(16.dp))
        CoachChartLabel("Recovery load · ${score} of ${watch.fatigueThreshold}", c)
        Spacer(Modifier.height(6.dp))
        CoachFatigueMeter(score, watch.fatigueThreshold, c)
        val fired = watch.fatigueChecks.filter { it.fired }
        if (fired.isNotEmpty()) {
            Spacer(Modifier.height(10.dp))
            fired.take(FIRED_CHECKS_SHOWN).forEach { check -> FiredCheckRow(check, c) }
            val more = fired.size - FIRED_CHECKS_SHOWN
            if (more > 0) {
                Text(
                    "And $more more crossed its line.",
                    style = MaterialTheme.typography.bodySmall,
                    color = c.muted
                )
            }
        }
        return
    }

    val series = state.e1rmBySlot[d.targetKey] ?: return
    if (series.size < 2) return
    Spacer(Modifier.height(16.dp))
    CoachChartLabel("Estimated 1RM · last ${series.size} sessions", c)
    Spacer(Modifier.height(4.dp))
    // Life size, not a thumbnail. The evidence is the reason to trust the call, so it is drawn at
    // the width of the decision it supports rather than squeezed beside a text row.
    CoachSparkline(
        series,
        c.accent,
        c.tile,
        height = 88.dp,
        modifier = Modifier.semantics {
            contentDescription = "${d.targetName} estimated one-rep max over the last " +
                "${series.size} sessions"
        }
    )
}

private const val FIRED_CHECKS_SHOWN = 3

/** One fatigue check that crossed its line, with the reading that took it there. */
@Composable
private fun FiredCheckRow(check: DeloadAdvisor.FatigueCheck, c: CoachColors) {
    Row(
        Modifier.fillMaxWidth().padding(vertical = COACH_ROW_PAD),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.Top
    ) {
        Text(
            check.name,
            style = MaterialTheme.typography.bodySmall,
            color = c.onBg,
            modifier = Modifier.weight(1f)
        )
        Spacer(Modifier.width(12.dp))
        Text(check.reading, style = MaterialTheme.typography.labelSmall, color = c.muted)
    }
}
