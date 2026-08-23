package com.forge.app.ui.coach

import androidx.compose.foundation.layout.Arrangement
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
 * Where the account's spine runs: in the left margin, INSIDE the gutter, so every section of the
 * page keeps the one 24dp content column and the rule reads as a ledger's rule rather than as an
 * indent applied to half the screen.
 */
private val SPINE_X = 10.dp

/** Default distance from an item's top edge down to its node's centre — sits on the first line. */
internal val NODE_ON_TEXT = 10.dp

/** Where an open call's node sits: on the tile's own first line, past its 18dp top padding. */
internal val NODE_ON_TILE = 28.dp

/**
 * "2026-W27" → "Week of Jun 29" — machine ids never render. Null when the id doesn't parse;
 * callers drop the week rather than show the raw string.
 */
internal fun coachWeekLabel(weekId: String): String? = runCatching {
    val monday = LocalDate.parse("$weekId-1", DateTimeFormatter.ISO_WEEK_DATE)
    "Week of " + monday.format(DateTimeFormatter.ofPattern("MMM d", Locale.getDefault()))
}.getOrNull()

/**
 * A stored hold reason rendered as a short human line. Pass rows are immutable, so their text is
 * machine prose — em dashes, "(s)" plurals, trailing instructions. A learning hold becomes the
 * canonical "Baseline still forming · N of M sessions"; every other reason is de-punctuated and
 * trimmed to its lead clause.
 */
internal fun recordHoldLine(reason: String): String {
    if (AutoCoachPlanner.isLearningHold(reason)) {
        val n = Regex("""(\d+)\s+(?:of\s+\d+\s+)?session""").find(reason)?.groupValues?.get(1)
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
    /** The interactive tile fill — the ONE body on this page, spent on an open call. */
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
    tile = MaterialTheme.colorScheme.surfaceVariant,
    track = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.35f)
)

// ─────────────────────────────────────────────────────────────────────────────
// The ledger's own vocabulary: anchors, the spine, nodes and stamps.
// ─────────────────────────────────────────────────────────────────────────────

/**
 * One anchor in the account — a mono small-caps heading with an optional right-hand reading.
 * Anchors are the account's dated breaks ("THIS WEEK", "WEEK OF AUG 10") and its named regions
 * ("WHERE YOU STAND"). The whitespace above and the anchor itself are the separator; the page
 * carries no rules that are not data.
 */
@Composable
internal fun CoachAnchor(
    title: String,
    c: CoachColors,
    modifier: Modifier = Modifier,
    meta: String? = null
) {
    val heading = @Composable {
        Text(
            title.uppercase(),
            style = MonoSectionAnchor,
            color = c.muted,
            modifier = Modifier.semantics { heading() }
        )
    }
    // Mono earns its voice only in UPPERCASE micro-labels; a mixed-case mono sentence is the
    // machine voice leaking into prose, so the reading is uppercased rather than set in sans.
    val reading = @Composable {
        Text(meta.orEmpty().uppercase(), style = MaterialTheme.typography.labelSmall, color = c.muted)
    }
    // Side by side there is room for both; past ~130% there is not, and the reading was ellipsing
    // to "next brief in…" rather than wrapping. It stacks instead — nothing on this page truncates.
    if (LocalDensity.current.fontScale > 1.3f) {
        Column(modifier.fillMaxWidth()) {
            heading()
            if (meta != null) {
                Spacer(Modifier.height(4.dp))
                reading()
            }
        }
    } else {
        Row(
            modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.Bottom
        ) {
            heading()
            if (meta != null) {
                Spacer(Modifier.width(12.dp))
                reading()
            }
        }
    }
}

/**
 * What an entry's node says about the entry's life. The node carries the LIFECYCLE; the stamp
 * word beside the entry carries the outcome, so the two never say the same thing twice.
 */
internal enum class EntryNode { OPEN, APPLIED, FAILED, DECLINED, PLAIN }

/**
 * The account's spine, drawn behind an item in the left margin, with this item's node on it.
 *
 * This is the one line on the page, and it is data: it is the time axis every entry hangs from.
 * That is what lets an open proposal and a five-week-old outcome be the same kind of object — they
 * differ by their node and their stamp, not by living in different sections of the screen.
 *
 * Consecutive items each draw their own segment, so a run of them reads as one unbroken rule.
 * [top] and [bottom] clip it where the account starts and ends; [node] null draws rule only, which
 * is how an anchor lets the line pass behind it.
 */
internal fun Modifier.ledgerSpine(
    c: CoachColors,
    node: EntryNode? = null,
    nodeY: Dp = NODE_ON_TEXT,
    top: Boolean = true,
    bottom: Boolean = true
): Modifier = this.drawBehind {
    val rule = c.outline.copy(alpha = 0.35f)
    val cx = SPINE_X.toPx()
    val stroke = 1.dp.toPx()
    if (node == null) {
        val from = if (top) 0f else size.height / 2f
        val to = if (bottom) size.height else size.height / 2f
        if (to > from) drawLine(rule, Offset(cx, from), Offset(cx, to), stroke)
        return@drawBehind
    }
    val nodeColor = when (node) {
        EntryNode.OPEN, EntryNode.APPLIED -> c.accent
        EntryNode.FAILED -> c.error
        EntryNode.DECLINED, EntryNode.PLAIN -> c.outline
    }
    val filled = node != EntryNode.DECLINED && node != EntryNode.PLAIN
    // An open call's node is the larger one: it is the only entry still asking for something.
    val r = (if (node == EntryNode.OPEN) 5.dp else 3.5.dp).toPx()
    val cy = nodeY.toPx()
    // The rule breaks around the node, so the node reads as sitting ON the line, not over it.
    val gap = r + 3.dp.toPx()
    if (top && cy - gap > 0f) drawLine(rule, Offset(cx, 0f), Offset(cx, cy - gap), stroke)
    if (bottom && size.height > cy + gap) {
        drawLine(rule, Offset(cx, cy + gap), Offset(cx, size.height), stroke)
    }
    if (filled) drawCircle(nodeColor, r, Offset(cx, cy))
    else drawCircle(nodeColor, r - stroke / 2f, Offset(cx, cy), style = Stroke(stroke * 1.5f))
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
    Text(
        text,
        style = MaterialTheme.typography.titleSmall,
        color = color,
        modifier = Modifier
            .clickableLabeled(contentDescription) { onClick() }
            .padding(vertical = 8.dp, horizontal = 2.dp)
    )
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
