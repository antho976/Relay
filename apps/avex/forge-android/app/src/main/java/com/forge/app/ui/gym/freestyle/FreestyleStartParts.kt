package com.forge.app.ui.gym.freestyle

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.common.ForgeRowPill
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

/**
 * A search field that is really a door: tapping it opens the full exercise browser. It sits where
 * the eye expects search, so "find a move" never needs explaining.
 */
@Composable
internal fun FsSearchLauncher(label: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 52.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(cs.surfaceVariant)
            .bounceCombinedClick(pressedScale = 0.98f, onClickLabel = label, onClick = onClick)
            .semantics { role = Role.Button }
            .padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(Icons.Filled.Search, contentDescription = null, tint = cs.onSurfaceVariant, modifier = Modifier.size(20.dp))
        Spacer(Modifier.width(12.dp))
        Text(label, style = MaterialTheme.typography.bodyLarge, color = cs.onSurfaceVariant)
    }
}

/** A mono section label for the start page and the rail, matched to the logger's other eyebrows. */
@Composable
private fun FsSectionLabel(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        letterSpacing = 1.sp,
        modifier = modifier
    )
}

/**
 * The empty log. Three ways in, fastest first: a move you already do (one tap adds it with last
 * time's numbers waiting in the slab), a whole past workout (one tap brings every set back), or a
 * search of the library. Someone with no history gets the three-step flow instead of empty lists.
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
        Spacer(Modifier.height(4.dp))
        FsSearchLauncher("Search $libraryCount exercises", onClick = onSearch)

        if (recent.isNotEmpty()) {
            Spacer(Modifier.height(28.dp))
            FsSectionLabel("YOUR MOVES")
            Spacer(Modifier.height(4.dp))
            recent.forEach { move -> FsRecentRow(move, onClick = { onAdd(move.libId) }) }
        }

        if (templates.isNotEmpty()) {
            Spacer(Modifier.height(28.dp))
            FsSectionLabel("REPEAT A WORKOUT")
            Spacer(Modifier.height(4.dp))
            templates.take(REPEAT_LIMIT).forEach { t ->
                FsTemplateRow(t, nowMs, onClick = { onRepeat(t.sessionId) })
            }
            if (templates.size > REPEAT_LIMIT) {
                Text(
                    "All past workouts →",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier
                        .heightIn(min = 48.dp)
                        .bounceCombinedClick(onClickLabel = "Show all past workouts", onClick = onAllTemplates)
                        .semantics { role = Role.Button }
                        .padding(vertical = 14.dp)
                )
            }
        }

        if (recent.isEmpty() && templates.isEmpty()) {
            Spacer(Modifier.height(32.dp))
            FsHowItWorks()
        }
        Spacer(Modifier.height(24.dp))
    }
}

/** One recent move: thumbnail, name, last time's top set and when. The whole row adds it. */
@Composable
private fun FsRecentRow(move: FsRecentMove, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    val meta = buildString {
        append(move.muscle.displayName.uppercase())
        move.lastReading?.let { append(" · LAST $it") }
        move.lastWhen?.let { append(" · $it") }
    }
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 64.dp)
            .clip(RoundedCornerShape(12.dp))
            .bounceCombinedClick(onClickLabel = "Add ${move.name}", onClick = onClick)
            .semantics(mergeDescendants = true) { role = Role.Button },
        verticalAlignment = Alignment.CenterVertically
    ) {
        FsThumb(move.muscle, Modifier.width(40.dp).height(44.dp))
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f).padding(vertical = 8.dp)) {
            Text(move.name, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold, color = cs.onSurface)
            Text(meta, style = MaterialTheme.typography.labelMedium, color = cs.onSurfaceVariant)
        }
        Spacer(Modifier.width(12.dp))
        ForgeRowPill("+ Add")
    }
}

/** One past workout: its name, when, and what it held. The whole row brings every set back. */
@Composable
private fun FsTemplateRow(template: FreestyleTemplateSummary, nowMs: Long, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    val moves = template.exerciseNames.distinct()
    val meta = buildString {
        append(lastDoneLabel(template.startedAtMs, nowMs))
        append(" · ${moves.size} ${if (moves.size == 1) "EXERCISE" else "EXERCISES"}")
        if (template.setCount > 0) append(" · ${template.setCount} ${if (template.setCount == 1) "SET" else "SETS"}")
    }
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 64.dp)
            .clip(RoundedCornerShape(12.dp))
            .bounceCombinedClick(onClickLabel = "Repeat ${template.title}", onClick = onClick)
            .semantics(mergeDescendants = true) { role = Role.Button }
            .padding(vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(Modifier.weight(1f)) {
            Text(template.title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold, color = cs.onSurface)
            Text(meta, style = MaterialTheme.typography.labelMedium, color = cs.onSurfaceVariant)
            Spacer(Modifier.height(2.dp))
            Text(moves.joinToString(" · "), style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
        }
        Spacer(Modifier.width(12.dp))
        ForgeRowPill("Repeat")
    }
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
                    modifier = Modifier.width(36.dp).padding(top = 3.dp)
                )
                Column(Modifier.weight(1f)) {
                    Text(title, style = MaterialTheme.typography.titleMedium, color = cs.onSurface)
                    Text(body, style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)
                }
            }
        }
    }
}

/**
 * Under a log in progress: the search door, then recent moves as a sideways rail of tiles, so
 * adding the next move is one tap without pushing the page down by a list.
 */
@Composable
internal fun FsAddFooter(recent: List<FsRecentMove>, onSearch: () -> Unit, onAdd: (String) -> Unit) {
    Column(Modifier.fillMaxWidth().padding(top = 28.dp, bottom = 16.dp)) {
        FsSearchLauncher("Add an exercise", onClick = onSearch)
        if (recent.isNotEmpty()) {
            Spacer(Modifier.height(20.dp))
            FsSectionLabel("RECENT")
            Spacer(Modifier.height(10.dp))
            Row(
                Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
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
            .heightIn(min = 124.dp)
            .clip(RoundedCornerShape(12.dp))
            .border(1.dp, cs.outline.copy(alpha = 0.35f), RoundedCornerShape(12.dp))
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
        Text(
            "+",
            style = MaterialTheme.typography.titleLarge,
            color = cs.primary,
            modifier = Modifier.align(Alignment.TopEnd)
        )
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
