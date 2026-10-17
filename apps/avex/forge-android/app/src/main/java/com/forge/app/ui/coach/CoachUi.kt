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

/** The ONE vertical padding every coach data row uses, so the page reads as a single rhythm. */
internal val COACH_ROW_PAD = 6.dp

/**
 * "2026-W27" → "Week of Jun 29" — machine ids never render. Null when the id doesn't parse;
 * callers drop the week rather than show the raw string.
 */
internal fun coachWeekLabel(weekId: String): String? = runCatching {
    val monday = LocalDate.parse("$weekId-1", DateTimeFormatter.ISO_WEEK_DATE)
    "Week of " + monday.format(DateTimeFormatter.ofPattern("MMM d", Locale.getDefault()))
}.getOrNull()

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

// ─────────────────────────────────────────────────────────────────────────────
// The page's own vocabulary: section heads, panels, stamps and text actions.
// ─────────────────────────────────────────────────────────────────────────────

/**
 * One section of the page: 28dp of air, the app's mono group label (with an optional reading on
 * the right), 10dp, then the content. The air and the label are the separator; no rules.
 */
@Composable
internal fun CoachSection(
    label: String,
    modifier: Modifier = Modifier,
    meta: String? = null,
    content: @Composable ColumnScope.() -> Unit
) {
    Column(modifier.fillMaxWidth().padding(horizontal = COACH_GUTTER)) {
        Spacer(Modifier.height(28.dp))
        Box(Modifier.padding(horizontal = 4.dp)) { ForgeGroupLabel(label, meta) }
        Spacer(Modifier.height(10.dp))
        content()
    }
}

/** A section's head on its own, for a section whose content spans several list items. */
@Composable
internal fun CoachSectionHead(label: String, modifier: Modifier = Modifier, meta: String? = null) {
    Column(modifier.fillMaxWidth().padding(horizontal = COACH_GUTTER)) {
        Spacer(Modifier.height(28.dp))
        Box(Modifier.padding(horizontal = 4.dp)) { ForgeGroupLabel(label, meta) }
        Spacer(Modifier.height(10.dp))
    }
}

/**
 * A filled panel in the grouped-surface fill: the Today panel, an open call, the block. The app's
 * grouped kit, so the Coach reads as the same app as Stats and Settings rather than a ledger apart.
 */
@Composable
internal fun CoachPanel(
    c: CoachColors,
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit
) {
    Column(
        modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(GROUP_OUTER))
            .background(c.tile)
            .padding(horizontal = 18.dp, vertical = 18.dp),
        content = content
    )
}

/** Rows stacked as one group: 2dp seams, outer corners round, inner ones tight. */
@Composable
internal fun CoachRows(c: CoachColors, rows: List<@Composable () -> Unit>) {
    Column(verticalArrangement = Arrangement.spacedBy(GROUP_SEAM)) {
        rows.forEachIndexed { i, row ->
            Column(
                Modifier
                    .fillMaxWidth()
                    .clip(rowShape(i, rows.size))
                    .background(c.tile)
                    .padding(horizontal = ROW_H, vertical = 14.dp)
            ) { row() }
        }
    }
}

/**
 * An entry's stamp: what happened to it, in one word. Nothing on the account is deleted or moved
 * when it resolves — it is stamped in place, so the record is complete by construction.
 *
 * Always muted, never red. Error at this size is 3.67:1 on the near-black ground and fails AA, and
 * the exception is already flagged by the entry's own node, which is a data mark and only owes 3:1.
 * Colouring the word too would be saying it twice AND saying it illegibly.
 */
@Composable
internal fun CoachStamp(word: String, c: CoachColors) {
    if (word.isBlank()) return
    Text(word.uppercase(), style = MaterialTheme.typography.labelSmall, color = c.muted)
}

/**
 * A text action ("Start →"), padded to fatten the touch target.
 *
 * Sans, not mono: button text belongs to the prose voice, and mono is reserved for uppercase
 * micro-labels. Never accent-coloured either — accent-as-text is 4.53:1 on the default Red but
 * 2.42:1 on Red and 2.34:1 on Navy, so on four of the five accents this control was unreadable on
 * the device. The arrow marks it as an action; the colour never had to.
 */
@Composable
internal fun CoachAction(
    text: String,
    color: Color,
    contentDescription: String,
    onClick: () -> Unit
) {
    // 48dp from the target, not the text (§14): a titleSmall line plus 8dp of padding was about
    // 36dp, on Skip for every open call, Apply all and the block's start and end.
    Box(
        modifier = Modifier
            .heightIn(min = 48.dp)
            .clickableLabeled(contentDescription) { onClick() }
            .padding(horizontal = 2.dp),
        contentAlignment = Alignment.CenterStart
    ) {
        Text(text, style = MaterialTheme.typography.titleSmall, color = color)
    }
}

/** A muted mono micro-caption above a chart ("ESTIMATED 1RM · LAST 9 SESSIONS"). */
@Composable
internal fun CoachChartLabel(text: String, c: CoachColors) {
    Text(
        text.uppercase(),
        style = MaterialTheme.typography.labelSmall,
        color = c.muted
    )
}

/**
 * A leading marker that paints ONLY for a meaningful [color] — an exception the eye should catch;
 * null reserves the gutter so rows still align without a grey dot on every neutral row.
 */
@Composable
internal fun CoachFlagDot(color: Color?, modifier: Modifier = Modifier) {
    Box(modifier.width(7.dp).height(7.dp)) {
        if (color != null) {
            Box(
                Modifier
                    .width(7.dp)
                    .height(7.dp)
                    .drawBehind { drawCircle(color, size.minDimension / 2f) }
            )
        }
    }
}

/**
 * An instrument's reading as a figure: the number in the serif voice, the meaning in one sans line
 * under it. The account already speaks this way ("2 of 4" on a new baseline, the week's figures);
 * the regions under it used to open on a mono caption instead and read as a readout, not a coach.
 * One per region at most, and only where the region IS a measurement.
 */
@Composable
internal fun CoachFigure(value: String, line: String?, c: CoachColors, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxWidth().semantics(mergeDescendants = true) {}) {
        Text(value, style = MaterialTheme.typography.headlineSmall, color = c.onBg)
        if (!line.isNullOrBlank()) {
            Spacer(Modifier.height(2.dp))
            Text(line, style = MaterialTheme.typography.bodySmall, color = c.muted)
        }
    }
}

/**
 * A group inside a region ("Recovery load", "Lifts on watch", "Autopilot"). Sentence case in the
 * sans voice: mono caps belong to the region anchor alone, and a second tier of them under it is
 * what made the advanced regions read as a terminal. Muted, so the rows it heads stay the ink.
 */
@Composable
internal fun CoachSubhead(label: String, c: CoachColors, modifier: Modifier = Modifier) {
    Text(
        label,
        style = MaterialTheme.typography.titleSmall,
        color = c.muted,
        modifier = modifier.semantics { heading() }
    )
}
