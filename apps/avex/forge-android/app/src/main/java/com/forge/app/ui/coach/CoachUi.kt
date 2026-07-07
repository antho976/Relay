package com.forge.app.ui.coach

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.data.repo.CoachRepository
import com.forge.app.domain.coach.AutoCoachPlanner
import com.forge.app.domain.coach.CoachOutcome
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.gym.stats.components.statsEntrance
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.util.Locale

/** The Coach page's horizontal gutter (DESIGN.md §7: 24dp on the column). */
internal val COACH_GUTTER = 24.dp

/** The ONE vertical padding every coach data/list row uses (§7: 12 total), so the page reads as a
 *  single rhythm — recovery checks, signal inputs, watched lifts all share it, no per-row values. */
internal val COACH_ROW_PAD = 6.dp

/**
 * "2026-W27" → "Week of Jun 29" — machine ids never render (§11). Null when the id doesn't
 * parse; callers drop the week rather than show the raw string.
 */
internal fun coachWeekLabel(weekId: String): String? = runCatching {
    val monday = LocalDate.parse("$weekId-1", DateTimeFormatter.ISO_WEEK_DATE)
    "Week of " + monday.format(DateTimeFormatter.ofPattern("MMM d", Locale.getDefault()))
}.getOrNull()

/**
 * A stored hold reason rendered as a short human line (§11). Pass rows are immutable, so their
 * text is machine prose — em dashes, "(s)" plurals, trailing instructions. A learning hold
 * becomes the canonical "Baseline still forming · N of M sessions" (the count pulled from the
 * stored string, since a historical week's live count isn't in the model); every other reason is
 * de-punctuated and trimmed to its lead clause.
 */
internal fun recordHoldLine(reason: String): String {
    if (AutoCoachPlanner.isLearningHold(reason)) {
        val n = Regex("""(\d+)\s+(?:of\s+\d+\s+)?session""").find(reason)?.groupValues?.get(1)
        return if (n != null) "Baseline still forming · $n of ${AutoCoachPlanner.MIN_SESSIONS} sessions"
        else "Baseline still forming"
    }
    return reason
        .replace(" — ", " · ").replace("—", " · ").replace("(s)", "s")
        .substringBefore(". ").trim().trimEnd('.', ';', ' ')
}

/** One decision's lifecycle as a status word — shared by the call rows and the record. */
internal fun coachDecisionStatusWord(d: CoachDecision, now: Long): String = when (d.status) {
    CoachRepository.STATUS_PROPOSED -> "open"
    CoachRepository.STATUS_APPLIED -> CoachOutcome.label(d.status, d.outcome, d.appliedAt, now) ?: "applied"
    CoachRepository.STATUS_SKIPPED -> "skipped"
    "reverted" -> "undone"
    CoachRepository.STATUS_FOLDED -> "absorbed"
    else -> ""
}

/** The page's palette, resolved once per composition; alphas are applied per the §5 ladder. */
internal data class CoachColors(
    val onBg: Color,
    val muted: Color,
    val accent: Color,
    /** Accent at 0.6 — the secondary-series / weaker-step rung. */
    val secondary: Color,
    val outline: Color,
    val error: Color,
    val bg: Color
)

@Composable
internal fun rememberCoachColors(): CoachColors = CoachColors(
    onBg = MaterialTheme.colorScheme.onBackground,
    muted = MaterialTheme.colorScheme.onSurfaceVariant,
    accent = MaterialTheme.colorScheme.primary,
    secondary = MaterialTheme.colorScheme.secondary,
    outline = MaterialTheme.colorScheme.outline,
    error = MaterialTheme.colorScheme.error,
    bg = MaterialTheme.colorScheme.background
)

/**
 * Open editorial section on the §7 air rhythm: 28 → EditorialHeader → 10 → content. No hairline —
 * lines mean data now (§1); the whitespace above and the mono header ARE the separator. The next
 * section's own leading 28 supplies the gap, so this one carries no trailing spacer.
 * Built from the shared primitives, staggered in with the shared entrance.
 */
@Composable
internal fun CoachSection(
    c: CoachColors,
    title: String,
    modifier: Modifier = Modifier,
    caption: String? = null,
    index: Int = 0,
    content: @Composable ColumnScope.() -> Unit
) {
    Column(modifier.fillMaxWidth().statsEntrance(index)) {
        Spacer(Modifier.height(28.dp))
        Column(Modifier.padding(horizontal = COACH_GUTTER)) {
            EditorialHeader(label = title, muted = c.muted, accent = c.accent)
            if (caption != null) {
                Spacer(Modifier.height(2.dp))
                Text(caption, style = MaterialTheme.typography.bodySmall, color = c.muted)
            }
            Spacer(Modifier.height(10.dp))
            content()
        }
    }
}

/** A mono accent action line ("Apply →"); padded per §7 to fatten the touch target. */
@Composable
internal fun CoachAction(
    text: String,
    color: Color,
    contentDescription: String,
    onClick: () -> Unit
) {
    Text(
        text,
        style = MaterialTheme.typography.labelMedium,
        color = color,
        modifier = Modifier
            .clickableLabeled(contentDescription) { onClick() }
            .padding(vertical = 6.dp, horizontal = 2.dp)
    )
}

/**
 * A leading marker that paints ONLY for a meaningful [color] — an exception the eye should catch
 * (a failure, a win, an active signal); null reserves the 7dp gutter so rows still align without a
 * grey dot on every neutral row (§8). Most rows pass null.
 */
@Composable
internal fun CoachFlagDot(color: Color?, modifier: Modifier = Modifier) {
    Box(modifier.size(7.dp)) {
        if (color != null) {
            Box(Modifier.size(7.dp).clip(RoundedCornerShape(50)).background(color))
        }
    }
}

/** Row with a leading marker then full-width text. The marker paints only for an active signal
 *  (§8) — an inactive row reserves the gutter but shows no dot. */
@Composable
internal fun CoachDotRow(text: String, active: Boolean, c: CoachColors) {
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(vertical = COACH_ROW_PAD)) {
        CoachFlagDot(if (active) c.accent else null)
        Spacer(Modifier.width(10.dp))
        Text(
            text,
            style = MaterialTheme.typography.bodySmall,
            color = if (active) c.onBg else c.muted
        )
    }
}

/** A muted mono micro-caption above a chart ("ESTIMATED 1RM · LAST 9 SESSIONS"). */
@Composable
internal fun CoachChartLabel(text: String, c: CoachColors) {
    Text(
        text.uppercase(),
        style = MaterialTheme.typography.labelSmall,
        color = c.muted,
        fontSize = 9.sp,
        letterSpacing = 1.sp
    )
}

/**
 * One watched lift on the Signals lens: name and its real delta on the left, the strength
 * trend on the right when there are ≥2 points to draw.
 */
@Composable
internal fun LiftTrendRow(
    name: String,
    statusWord: String,
    statusColor: Color,
    valueText: String?,
    series: List<Double>,
    c: CoachColors
) {
    Row(
        Modifier.fillMaxWidth().padding(vertical = COACH_ROW_PAD),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                name,
                style = MaterialTheme.typography.bodyMedium,
                color = c.onBg,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
            Row {
                Text(statusWord, style = MaterialTheme.typography.labelSmall, color = statusColor)
                if (valueText != null) {
                    Text(" · $valueText", style = MaterialTheme.typography.labelSmall, color = c.muted)
                }
            }
        }
        // A lift that's still forming has no 2-point trend to draw — the status row stands alone.
        if (series.size >= 2) {
            Spacer(Modifier.width(12.dp))
            CoachSparkline(series, c.accent, c.bg, modifier = Modifier.width(110.dp), height = 32.dp)
        }
    }
}
