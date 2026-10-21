package com.forge.app.ui.coach

import com.forge.app.domain.units.formatWeight
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.domain.adapt.DeloadAdvisor
import com.forge.app.ui.common.ForgePrimaryCapsule

/** The change a call makes, decomposed so the tile can rank it instead of printing a sentence. */
internal data class CallCopy(val subject: String, val change: String, val reason: String)

// Compiled once: callCopy runs per decision on every recomposition of the Coach page.
private val SWING_REGEX = Regex("""\((\d+)\s*→\s*(\d+)\)""")
private val DANGLING_PREPOSITION_REGEX = Regex("""\s+(to|from|for|on)\s+(?=\(|$)""")
private val MULTI_SPACE_REGEX = Regex("""\s{2,}""")

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
    val swing = SWING_REGEX.find(d.summary)
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
        .replace(DANGLING_PREPOSITION_REGEX, " ")
        .replace(MULTI_SPACE_REGEX, " ")
        .trim().trimEnd(':', ' ')
    return stripped.ifBlank { this }.replaceFirstChar { it.uppercaseChar() }
}

/** A decision type as the word a person would use. Machine ids never render. */
internal fun callKind(type: String): String? = when (type) {
    "deload" -> "Deload"
    "swap" -> "Rotation"
    "rep_shift" -> "Rep range"
    "volume_up", "volume_down" -> "Volume"
    "weight_nudge", "progression" -> "Load"
    "revert" -> "Undo"
    else -> null
}
