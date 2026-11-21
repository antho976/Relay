package com.forge.app.ui.coach

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.draw.clip
import com.forge.app.ui.common.ForgeGroupLabel
import com.forge.app.ui.common.GROUP_OUTER
import com.forge.app.ui.common.GROUP_SEAM
import com.forge.app.ui.common.ROW_H
import com.forge.app.ui.common.rowShape
import androidx.compose.foundation.layout.Box
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
import androidx.compose.foundation.layout.heightIn
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.data.repo.CoachRepository
import com.forge.app.domain.coach.AutoCoachPlanner
import com.forge.app.domain.coach.CoachOutcome
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.theme.MonoSectionAnchor
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.util.Locale

/** The page's horizontal gutter. */
internal val COACH_GUTTER = 24.dp



private val HOLD_SESSIONS_REGEX = Regex("""(\d+)\s+(?:of\s+\d+\s+)?session""")

/**
 * A stored hold reason rendered as a short human line. Pass rows are immutable, so their text is
 * machine prose — em dashes, "(s)" plurals, trailing instructions. A learning hold becomes the
 * canonical "Baseline still forming · N of M sessions"; every other reason is de-punctuated and
 * trimmed to its lead clause.
 */
internal fun recordHoldLine(reason: String): String {
    if (AutoCoachPlanner.isLearningHold(reason)) {
        val n = HOLD_SESSIONS_REGEX.find(reason)?.groupValues?.get(1)
        return if (n != null) "Baseline still forming · $n of ${AutoCoachPlanner.MIN_SESSIONS} sessions"
        else "Baseline still forming"
    }
    return humanizeMachineProse(reason).substringBefore(". ").trim().trimEnd('.', ';', ' ')
}

/**
 * Machine prose translated at the seam. Stored reasons and hold lines are immutable rows written
 * by the planner, so they still carry em dashes and paren plurals that are banned in anything
 * rendered. Every such literal in the UI lives HERE and nowhere else, so the translation has one
 * home and the ban stays enforceable everywhere it is read.
 */
internal fun humanizeMachineProse(text: String): String =
    text.replace(" — ", " · ").replace("—", " · ").replace("(s)", "s")

/** One decision's lifecycle as a status word — the stamp the account carries. */
internal fun coachDecisionStatusWord(d: CoachDecision, now: Long): String = when (d.status) {
    CoachRepository.STATUS_PROPOSED -> "open"
    CoachRepository.STATUS_APPLIED -> CoachOutcome.label(d.status, d.outcome, d.appliedAt, now) ?: "applied"
    CoachRepository.STATUS_SKIPPED -> "skipped"
    "reverted" -> "undone"
    CoachRepository.STATUS_FOLDED -> "absorbed"
    else -> ""
}

/** The page's palette, resolved once per composition. */
internal data class CoachColors(
    val onBg: Color,
    val muted: Color,
    val accent: Color,
    /** Accent at 0.6 — the secondary-series / weaker-step rung. */
    val secondary: Color,
    val outline: Color,
    val error: Color,
    val bg: Color,
    /** The grouped-surface fill every panel, tile and row group on the page sits on. */
    val tile: Color,
    /**
     * The unfilled rung of every meter, bar and rail.
     *
     * This was `outline` at 0.25, which lands around 1.6:1 on the near-black ground — on a real
     * phone in daylight an empty recovery meter was simply not there. Tinted from `muted` instead,
     * at the same sanctioned rung, it reads as an empty track rather than as nothing.
     */
    val track: Color
)

@Composable
internal fun rememberCoachColors(): CoachColors = CoachColors(
    onBg = MaterialTheme.colorScheme.onBackground,
    muted = MaterialTheme.colorScheme.onSurfaceVariant,
    accent = MaterialTheme.colorScheme.primary,
    secondary = MaterialTheme.colorScheme.secondary,
    outline = MaterialTheme.colorScheme.outline,
    error = MaterialTheme.colorScheme.error,
    bg = MaterialTheme.colorScheme.background,
    tile = MaterialTheme.colorScheme.surfaceContainerHigh,
    track = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.35f)
)
