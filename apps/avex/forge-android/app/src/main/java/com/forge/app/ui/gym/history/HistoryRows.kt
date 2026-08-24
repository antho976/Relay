package com.forge.app.ui.gym.history

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.entities.durationMinutes
import com.forge.app.domain.cardio.CardioActivity
import com.forge.app.domain.session.SessionType
import com.forge.app.domain.units.formatDistance
import com.forge.app.domain.units.formatVolume
import com.forge.app.program.Program
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.settings.SettingsIcons
import com.forge.app.ui.theme.LocalForgeSettings
import java.text.SimpleDateFormat
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.TextStyle
import java.util.Date
import java.util.Locale

/**
 * The History list rows, shared beyond the History screen (the Stats day-detail sheet renders the
 * same rows, so "a day" looks identical everywhere).
 *
 * ## What changed, and why (2026-08-24)
 *
 * The rows carried their own full date and closed on a hairline, which produced a seven-deep run of
 * "AUG 24, 2026" under a ladder of rules — a §1 violation twice over (a line is a claim about data,
 * and a separator is not data) and, worse, a list where every row's most prominent element was the
 * one thing it shared with its neighbours. The date now belongs to the day group above the rows
 * (`HistoryDay`), the rule is gone, and air separates.
 *
 * Three things took the space back:
 *
 *  1. **A leading glyph**, so gym rows finally sit on the same left rail cardio always had. It is
 *     the same clock Home's RECENT rows use, deliberately: a per-day-type glyph family was drawn
 *     for this slot and reverted on 2026-08-24 (`design/SETTLED.md`).
 *  2. **A meta line** carrying the session's markers, set count and duration. The trim on Home
 *     said more about a session than its own history entry did, which is backwards for the screen
 *     "view all →" points at.
 *  3. **One figure on the right**, not three lines. `formatVolume` already prints the unit, so the
 *     "KG" stacked beneath "149 kg" was the same fact twice (§4.3, one home). Duration moved to the
 *     meta line, where a qualifier belongs (§2①). Only a PR — the exception — earns a second line.
 */

/** The leading mark's footprint, and the indent every row's text shares because of it. */
private val GLYPH = 22.dp
private val GLYPH_GAP = 12.dp

/**
 * ONE vertical padding for every row on this lens, so the list reads as a single rhythm (§7).
 * Trim, because a List archetype's rows are (§3) — the air that replaced the hairlines belongs
 * BETWEEN the day groups, not inside every row.
 */
private val HISTORY_ROW_PAD = 8.dp

/**
 * One open gym-session row: focus glyph · day name over its markers · volume, with the day's date
 * carried by the group header above it.
 */
@Composable
internal fun SessionRow(
    session: Session,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val weightUnit = LocalForgeSettings.current.weightUnit
    val dayName = Program.dayDisplayName(session.dayKey)
    val marker = if (session.deloadMarkedHere) SessionType.DELOAD.pillLabel
    else SessionType.fromKey(session.sessionType)?.pillLabel
    val tags = session.tags.split(",").map { it.trim() }.filter { it.isNotEmpty() }
    val meta = listOfNotNull(
        marker,
        // "1 sets" is the machine talking (§11) — the same offence as "3 session(s)".
        session.setCount.takeIf { it > 0 }?.let { "$it set${if (it == 1) "" else "s"}" },
        session.durationMinutes()?.let { "$it min" }
    ).joinToString(" · ")

    HistoryRow(
        icon = SettingsIcons.Session,
        title = dayName,
        meta = meta.takeIf { it.isNotEmpty() },
        // Quick tags are the user's own words, so they keep their own line rather than being
        // truncated into the machine meta above them.
        tags = tags.takeIf { it.isNotEmpty() }?.joinToString(" · ") { "#$it" },
        figure = session.totalVolumeLb
            ?.takeIf { it > 0 }
            ?.let { formatVolume(it, weightUnit) },
        // Flag the exception only (§8): almost every session has no PR, and a column of "0 PR"
        // would be a grey dot column in words.
        figureNote = session.prCount.takeIf { it > 0 }?.let { "$it PR" },
        clickLabel = "Open $dayName",
        onClick = onClick,
        modifier = modifier
    )
}

/** One open cardio row — the same rail and rhythm, with distance where a workout's volume sits. */
@Composable
internal fun CardioHistoryRow(
    entry: CardioEntry,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val activity = CardioActivity.resolve(entry.type, com.forge.app.ui.cardio.LocalCardioTypes.current)
    val useMiles = LocalForgeSettings.current.useMiles

    HistoryRow(
        icon = activity.icon,
        title = activity.displayName,
        meta = entry.durationMin.takeIf { it > 0 }?.let { "$it min" },
        tags = entry.note?.trim()?.takeIf { it.isNotEmpty() },
        figure = entry.distanceKm?.takeIf { it > 0 }?.let { formatDistance(it, useMiles) },
        figureNote = null,
        clickLabel = "Open ${activity.displayName}",
        onClick = onClick,
        modifier = modifier
    )
}

/**
 * The shared row shape. Gym and cardio differ only in what they put in each slot, so they share one
 * geometry — the left rail, the indent and the figure column line up down the whole list even when
 * a workout and a run sit next to each other.
 */
@Composable
private fun HistoryRow(
    icon: ImageVector,
    title: String,
    meta: String?,
    tags: String?,
    figure: String?,
    figureNote: String?,
    clickLabel: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val cs = MaterialTheme.colorScheme
    val muted = cs.onSurfaceVariant
    Row(
        modifier = modifier
            .fillMaxWidth()
            // Press = bounce, not a ripple (§9); the WHOLE row is the one tap target (§2③).
            .bounceCombinedClick(onClickLabel = clickLabel, onClick = onClick)
            // Touch target from padding, never a fixed height — the row grows with font scale (§14).
            .heightIn(min = 48.dp)
            .padding(vertical = HISTORY_ROW_PAD),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(
            icon,
            contentDescription = null,
            tint = muted,
            modifier = Modifier.size(GLYPH)
        )
        Spacer(Modifier.width(GLYPH_GAP))
        Column(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(2.dp)
        ) {
            Text(title, style = MaterialTheme.typography.bodyMedium, color = cs.onBackground)
            if (meta != null) {
                Text(meta, style = MaterialTheme.typography.labelMedium, color = muted)
            }
            if (tags != null) {
                Text(tags, style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.65f))
            }
        }
        if (figure != null) {
            Spacer(Modifier.width(12.dp))
            Column(
                horizontalAlignment = Alignment.End,
                verticalArrangement = Arrangement.spacedBy(2.dp)
            ) {
                Text(figure, style = MaterialTheme.typography.titleSmall, color = cs.onBackground)
                if (figureNote != null) {
                    Text(figureNote, style = MaterialTheme.typography.labelSmall, color = muted)
                }
            }
        }
    }
}

/**
 * The date a group of rows shares, said the way a person would.
 *
 * Inside the last week a weekday is what you actually remember a session by ("that was Saturday"),
 * so it leads and the date qualifies it. Beyond that the weekday stops meaning anything and the
 * date stands alone. The year appears only when it is not this one — printing "2026" on every row
 * of a log kept in 2026 is the machine talking (§11).
 */
internal fun historyDayLabel(epochMs: Long, today: LocalDate = LocalDate.now(ZoneId.systemDefault())): String {
    val date = Instant.ofEpochMilli(epochMs).atZone(ZoneId.systemDefault()).toLocalDate()
    val weekday = date.dayOfWeek.getDisplayName(TextStyle.SHORT, Locale.getDefault()).uppercase()
    return when {
        date == today -> "TODAY"
        date == today.minusDays(1) -> "YESTERDAY"
        date.isAfter(today.minusDays(7)) -> "$weekday · ${shortDateFormat.format(Date(epochMs)).uppercase()}"
        date.year == today.year -> shortDateFormat.format(Date(epochMs)).uppercase()
        else -> historyDateFormat.format(Date(epochMs)).uppercase()
    }
}

private val historyDateFormat = SimpleDateFormat("MMM d, yyyy", Locale.getDefault())
private val shortDateFormat = SimpleDateFormat("MMM d", Locale.getDefault())

/** The full date, for surfaces that name a single session rather than a day of them (freestyle
 *  templates). The History list itself groups by [historyDayLabel] instead. */
fun formatHistoryDate(epochMs: Long): String = historyDateFormat.format(Date(epochMs))
