package com.forge.app.ui.gym.freestyle

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.circle
import com.forge.app.ui.common.icon
import com.forge.app.ui.common.strokePath
import com.forge.app.ui.experiment.CardMark
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.gym.history.formatHistoryDate
import com.forge.app.ui.gym.stats.components.MuscleFigure
import java.time.Instant
import java.time.ZoneId
import java.time.temporal.ChronoUnit

/** How many past workouts the start page offers to repeat before pointing at the full picker. */
private const val REPEAT_LIMIT = 2

/**
 * A move the user has performed recently, as the start page and the in-session rail show it: its
 * identity plus last time's top set and when that was, both null until last time has loaded.
 */
internal data class FsRecentMove(
    val libId: String,
    val name: String,
    val muscle: MuscleGroup,
    val lastReading: String?,
    val lastWhen: String?
)

/** "TODAY", "YESTERDAY", "4 DAYS AGO", then the history date past a week. Uppercase, for mono lines. */
internal fun lastDoneLabel(atMs: Long, nowMs: Long, zone: ZoneId = ZoneId.systemDefault()): String {
    val days = ChronoUnit.DAYS.between(
        Instant.ofEpochMilli(atMs).atZone(zone).toLocalDate(),
        Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate()
    )
    return when {
        days <= 0L -> "TODAY"
        days == 1L -> "YESTERDAY"
        days < 7L -> "$days DAYS AGO"
        else -> formatHistoryDate(atMs).uppercase()
    }
}

/** The two glyphs this page adds, drawn like the app's other families (24dp viewport, 1.8 strokes). */
private object FsIcons {
    /** Search: a lens with its handle running to the lower end corner. */
    val Search: ImageVector by lazy {
        icon("FsSearch") {
            strokePath(1.8f) { circle(10.6f, 10.6f, 6.2f) }
            strokePath(1.8f) { moveTo(15.2f, 15.2f); lineTo(19.8f, 19.8f) }
        }
    }

    /** Repeat: two arrows chasing each other round a loop. */
    val Repeat: ImageVector by lazy {
        icon("FsRepeat") {
            strokePath(1.8f) {
                moveTo(5f, 11f); lineTo(5f, 9.5f)
                quadTo(5f, 7f, 7.5f, 7f); lineTo(18f, 7f)
                moveTo(15.2f, 4.2f); lineTo(18f, 7f); lineTo(15.2f, 9.8f)
                moveTo(19f, 13f); lineTo(19f, 14.5f)
                quadTo(19f, 17f, 16.5f, 17f); lineTo(6f, 17f)
                moveTo(8.8f, 14.2f); lineTo(6f, 17f); lineTo(8.8f, 19.8f)
            }
        }
    }
}

/** A page section anchor: the app's editorial header, with an optional accent action at the end. */
@Composable
private fun FsSection(label: String, action: String? = null, onAction: (() -> Unit)? = null) {
    EditorialHeader(
        label,
        muted = MaterialTheme.colorScheme.onSurfaceVariant,
        accent = MaterialTheme.colorScheme.primary,
        action = action,
        onAction = onAction
    )
}

/** An accent mark in a fixed 40dp lane, so rows with a mark line up with rows with a muscle figure. */
@Composable
private fun FsMark(icon: ImageVector) {
    Box(Modifier.width(40.dp), contentAlignment = Alignment.Center) {
        CardMark(icon, MaterialTheme.colorScheme.primary, size = 36.dp, glyphSize = 18.dp)
    }
}

/** The accent add button at the end of a move row: drawn only, the whole row is the target. */
@Composable
private fun FsAddDot() {
    val cs = MaterialTheme.colorScheme
    Box(
        Modifier.size(32.dp).clip(CircleShape).background(cs.primary),
        contentAlignment = Alignment.Center
    ) {
        Text("+", style = MaterialTheme.typography.titleMedium, color = cs.onPrimary)
    }
}

/**
 * The shared row shape of the start page and the footer: a leading mark or figure, a name, a mono
 * meta line, an optional third line, and a trailing element. The whole row is one tap.
 */
@Composable
private fun FsRow(
    title: String,
    meta: String,
    clickLabel: String,
    onClick: () -> Unit,
    leading: @Composable () -> Unit,
    detail: String? = null,
    trailing: @Composable () -> Unit = {}
) {
    val cs = MaterialTheme.colorScheme
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 64.dp)
            .clip(RoundedCornerShape(12.dp))
            .bounceCombinedClick(onClickLabel = clickLabel, onClick = onClick)
            .semantics(mergeDescendants = true) { role = Role.Button }
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        leading()
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold, color = cs.onSurface)
            Text(meta, style = MaterialTheme.typography.labelMedium, color = cs.onSurfaceVariant)
            detail?.let {
                Spacer(Modifier.height(2.dp))
                Text(it, style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
            }
        }
        Spacer(Modifier.width(12.dp))
        trailing()
    }
}

/**
 * The empty log. Fastest way in first: a move you already do (one tap adds it with last time's
 * numbers waiting), then the library for anything else, then a whole past workout. Someone with no
 * history gets the search row and the three-step flow instead of empty sections.
 */
@Composable
internal fun FsStartPage(
    recent: List<FsRecentMove>,
    templates: List<FreestyleTemplateSummary>,
    libraryCount: Int,
    nowMs: Long,
    onSearch: () -> Unit,
    onAdd: (String) -> Unit,
    onRepeat: (Long) -> Unit,
    onAllTemplates: () -> Unit
) {
    Column(Modifier.fillMaxWidth()) {
        Spacer(Modifier.height(16.dp))
        FsSection(if (recent.isEmpty()) "Exercises" else "Your moves")
        Spacer(Modifier.height(6.dp))
        recent.forEach { move -> FsRecentRow(move, onClick = { onAdd(move.libId) }) }
        FsSearchRow(
            title = if (recent.isEmpty()) "Browse exercises" else "Find another exercise",
            meta = "SEARCH $libraryCount · OR NAME YOUR OWN",
            onClick = onSearch
        )

        if (templates.isNotEmpty()) {
            Spacer(Modifier.height(28.dp))
            FsSection(
                "Repeat a workout",
                action = if (templates.size > REPEAT_LIMIT) "all \u2192" else null,
                onAction = if (templates.size > REPEAT_LIMIT) onAllTemplates else null
            )
            Spacer(Modifier.height(6.dp))
            templates.take(REPEAT_LIMIT).forEach { t ->
                FsTemplateRow(t, nowMs, onClick = { onRepeat(t.sessionId) })
            }
        }

        if (recent.isEmpty() && templates.isEmpty()) {
            Spacer(Modifier.height(32.dp))
            FsHowItWorks()
        }
        Spacer(Modifier.height(24.dp))
    }
}

/** One recent move: figure, name, last time's top set and when, and the accent add button. */
@Composable
private fun FsRecentRow(move: FsRecentMove, onClick: () -> Unit) {
    val meta = buildString {
        append(move.muscle.displayName.uppercase())
        move.lastReading?.let { append(" · LAST $it") }
        move.lastWhen?.let { append(" · $it") }
    }
    FsRow(
        title = move.name,
        meta = meta,
        clickLabel = "Add ${move.name}",
        onClick = onClick,
        leading = { FsThumb(move.muscle, Modifier.width(40.dp).height(44.dp)) },
        trailing = { FsAddDot() }
    )
}

/** The library door, in the same row shape as the moves above it. */
@Composable
private fun FsSearchRow(title: String, meta: String, onClick: () -> Unit) {
    FsRow(
        title = title,
        meta = meta,
        clickLabel = title,
        onClick = onClick,
        leading = { FsMark(FsIcons.Search) },
        trailing = {
            Text("\u2192", style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.primary)
        }
    )
}

/** One past workout: when, how big, what it held. The whole row brings every set back. */
@Composable
private fun FsTemplateRow(template: FreestyleTemplateSummary, nowMs: Long, onClick: () -> Unit) {
    val moves = template.exerciseNames.distinct()
    val meta = buildString {
        append(lastDoneLabel(template.startedAtMs, nowMs))
        append(" · ${moves.size} ${if (moves.size == 1) "EXERCISE" else "EXERCISES"}")
        if (template.setCount > 0) append(" · ${template.setCount} ${if (template.setCount == 1) "SET" else "SETS"}")
    }
    FsRow(
        title = template.title,
        meta = meta,
        detail = moves.joinToString(" · "),
        clickLabel = "Repeat ${template.title}",
        onClick = onClick,
        leading = { FsMark(FsIcons.Repeat) },
        trailing = {
            Text("Repeat", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
        }
    )
}

/** First run, no history at all: the whole flow in three numbered lines instead of empty sections. */
@Composable
private fun FsHowItWorks() {
    val cs = MaterialTheme.colorScheme
    val steps = listOf(
        "Add a move" to "Search the library, or name your own if it isn't there.",
        "Log each set" to "Numbers carry over from the set before, so a repeat is one tap.",
        "Save" to "It lands in history, stats and records like any other session."
    )
    Column(verticalArrangement = Arrangement.spacedBy(18.dp)) {
        steps.forEachIndexed { i, (title, body) ->
            Row {
                Text(
                    "%02d".format(i + 1),
                    style = MaterialTheme.typography.labelLarge,
                    color = cs.primary,
                    modifier = Modifier.width(40.dp).padding(top = 3.dp)
                )
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(title, style = MaterialTheme.typography.titleMedium, color = cs.onSurface)
                    Text(body, style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)
                }
            }
        }
    }
}

/**
 * Under a log in progress: the library door, then recent moves as a sideways rail of tiles, so the
 * next move is one tap away without a list pushing the page down.
 */
@Composable
internal fun FsAddFooter(recent: List<FsRecentMove>, libraryCount: Int, onSearch: () -> Unit, onAdd: (String) -> Unit) {
    Column(Modifier.fillMaxWidth().padding(top = 24.dp, bottom = 16.dp)) {
        FsSearchRow(title = "Add an exercise", meta = "SEARCH $libraryCount · OR NAME YOUR OWN", onClick = onSearch)
        if (recent.isNotEmpty()) {
            Spacer(Modifier.height(16.dp))
            FsSection("Recent")
            Spacer(Modifier.height(10.dp))
            // Intrinsic height so every tile matches the tallest (a two-line name), not a fixed guess.
            Row(
                Modifier.fillMaxWidth().height(IntrinsicSize.Max).horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                recent.forEach { move -> FsRecentTile(move, onClick = { onAdd(move.libId) }) }
            }
        }
    }
}

@Composable
private fun FsRecentTile(move: FsRecentMove, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Box(
        Modifier
            .width(136.dp)
            .fillMaxHeight()
            .clip(RoundedCornerShape(12.dp))
            .background(cs.surfaceVariant)
            .bounceCombinedClick(onClickLabel = "Add ${move.name}", onClick = onClick)
            .semantics(mergeDescendants = true) { role = Role.Button }
            .padding(12.dp)
    ) {
        Column {
            FsThumb(move.muscle, Modifier.width(30.dp).height(34.dp))
            Spacer(Modifier.height(10.dp))
            Text(move.name, style = MaterialTheme.typography.titleSmall, color = cs.onSurface)
            move.lastReading?.let {
                Spacer(Modifier.height(2.dp))
                Text("LAST $it", style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant)
            }
        }
        Box(Modifier.align(Alignment.TopEnd)) { FsAddDot() }
    }
}

@Composable
private fun FsThumb(muscle: MuscleGroup, modifier: Modifier) {
    val cs = MaterialTheme.colorScheme
    MuscleFigure(
        muscle = muscle,
        lit = lerp(cs.primary, cs.onSurface, 0.32f),
        body = cs.onSurfaceVariant.copy(alpha = 0.15f),
        detail = cs.onSurfaceVariant.copy(alpha = 0.35f),
        modifier = modifier
    )
}
