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
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.currentLocale
import com.forge.app.ui.common.statsEntrance
import java.time.LocalDate
import java.time.format.DateTimeFormatter

/**
 * TODAY — what the coach's read means for you right now, in one panel.
 *
 * The answer leads at the serif rung: how today's weight targets move and why ("Targets 2%
 * lighter", "Last session felt off"). Under it, recovery load against its deload line, and the two
 * short readings that turn the page forward: when the next brief lands and which way the lifts are
 * going. On a new account the panel is the baseline instead, a count and the meter it fills.
 *
 * Under Detailed the panel also lists the fatigue checks that crossed their line; the lift count
 * steps aside there because the lift tiles below draw each one.
 */
internal fun LazyListScope.coachToday(state: CoachViewModel.UiState, c: CoachColors) {
    val brief = state.brief
    val forming = (brief?.sessionsToGo ?: 0) > 0
    val readiness = readinessLine(state)
    if (!forming && readiness == null) return
    item("today") {
        val locale = currentLocale()
        val date = remember(locale) {
            LocalDate.now().format(DateTimeFormatter.ofPattern("EEE, MMM d", locale))
        }
        CoachSection("Today", meta = date, modifier = Modifier.statsEntrance(1)) {
            CoachPanel(c) {
                if (forming && brief != null) BaselineHero(brief, c)
                else if (readiness != null) {
                    Text(readiness.value, style = MaterialTheme.typography.headlineMedium, color = c.onBg)
                    readiness.sub?.let {
                        Spacer(Modifier.height(4.dp))
                        Text(it, style = MaterialTheme.typography.bodyMedium, color = c.muted)
                    }
                }
                // While the baseline forms, recovery is forming too; it only says so under Detailed.
                if (!forming || state.advanced) {
                    recoveryLine(state)?.let { line ->
                        Spacer(Modifier.height(22.dp))
                        RecoveryReading(line, state, c)
                    }
                }
                Spacer(Modifier.height(20.dp))
                TodayFigures(state, c)
            }
        }
    }
}

/** A new account's Today: the baseline count, the meter it fills, and when the first call lands. */
@Composable
private fun BaselineHero(brief: com.forge.app.data.repo.CoachBrief, c: CoachColors) {
    val logged = brief.sessionsLogged
    val needed = brief.minSessions.coerceAtLeast(1)
    val toGo = brief.sessionsToGo
    Text("$logged of $needed sessions", style = MaterialTheme.typography.headlineMedium, color = c.onBg)
    Spacer(Modifier.height(4.dp))
    Text(
        "Learning your baseline. $toGo more ${if (toGo == 1) "session" else "sessions"} and the coach makes its first call.",
        style = MaterialTheme.typography.bodyMedium,
        color = c.muted
    )
    Spacer(Modifier.height(14.dp))
    TrustProgressBar(
        streak = logged.coerceAtMost(needed),
        required = needed,
        earned = toGo == 0,
        modifier = Modifier.fillMaxWidth().semantics {
            contentDescription = "$logged of $needed baseline sessions logged"
        }
    )
}

/**
 * Recovery load: the band word against its label, the meter with its deload tick, the score in
 * words under it. Below a gate the meter is progress toward the gate; paused after a deload it is
 * an honest empty track.
 */
@Composable
private fun RecoveryReading(line: StandingLine, state: CoachViewModel.UiState, c: CoachColors) {
    val watch = state.watch
    Row(Modifier.fillMaxWidth().semantics(mergeDescendants = true) {}, verticalAlignment = Alignment.Bottom) {
        Text(line.label, style = MaterialTheme.typography.bodyMedium, color = c.muted, modifier = Modifier.weight(1f))
        Spacer(Modifier.width(12.dp))
        Text(line.value, style = MaterialTheme.typography.titleSmall, color = c.onBg)
    }
    Spacer(Modifier.height(10.dp))
    val score = watch?.fatigueScore
    when {
        watch == null -> Unit
        score != null -> CoachFatigueMeter(score, watch.fatigueThreshold, c)
        line.value == "Paused" -> Box(
            Modifier.fillMaxWidth().height(5.dp).clip(RoundedCornerShape(50)).drawBehind { drawRect(c.track) }
        )
        watch.sessionsLogged < watch.recoveryGateSessions.coerceAtLeast(1) -> CoachWatchBar(
            watch.sessionsLogged.toFloat() / watch.recoveryGateSessions.coerceAtLeast(1), c.accent, c
        )
        else -> CoachWatchBar(
            watch.historyDays.toFloat() / watch.recoveryWindowDays.coerceAtLeast(1), c.accent, c
        )
    }
    line.sub?.let {
        Spacer(Modifier.height(8.dp))
        Text(it, style = MaterialTheme.typography.bodySmall, color = c.muted)
    }
    // Detailed lists every check that crossed its line, each with the reading that took it there.
    if (state.advanced && score != null) {
        val fired = watch.fatigueChecks.filter { it.fired }
        if (fired.isNotEmpty()) {
            Spacer(Modifier.height(8.dp))
            fired.forEach { check ->
                Row(Modifier.fillMaxWidth().padding(vertical = COACH_ROW_PAD), verticalAlignment = Alignment.Top) {
                    CoachFlagDot(c.error, Modifier.padding(top = 6.dp))
                    Spacer(Modifier.width(10.dp))
                    Text(check.name, style = MaterialTheme.typography.bodySmall, color = c.onBg, modifier = Modifier.weight(1f))
                    Spacer(Modifier.width(12.dp))
                    Text(check.reading, style = MaterialTheme.typography.bodySmall, color = c.muted)
                }
            }
        }
    }
}

/** The two short readings that close the panel: the next brief, and (Simple) the lifts. */
@Composable
private fun TodayFigures(state: CoachViewModel.UiState, c: CoachColors) {
    val days = state.daysToNextBrief
    val next = when {
        days <= 0 || days > 7 -> "Monday"
        days == 1 -> "Tomorrow"
        else -> "In $days days"
    }
    val locale = currentLocale()
    val landing = if (days in 2..7) {
        LocalDate.now().plusDays(days.toLong()).format(DateTimeFormatter.ofPattern("EEE, MMM d", locale))
    } else null
    val lifts = if (state.advanced) null else liftsLine(state)
    val figures = buildList {
        add(Triple("Next brief", next, landing))
        lifts?.let { add(Triple(it.label, it.value, it.sub)) }
    }
    val stacked = LocalDensity.current.fontScale > 1.3f
    if (stacked) {
        Column(verticalArrangement = Arrangement.spacedBy(12.dp)) { figures.forEach { TodayFigure(it, c) } }
    } else {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            figures.forEach { TodayFigure(it, c, Modifier.weight(1f)) }
        }
    }
}

@Composable
private fun TodayFigure(f: Triple<String, String, String?>, c: CoachColors, modifier: Modifier = Modifier) {
    Column(modifier.semantics(mergeDescendants = true) {}) {
        Text(f.first, style = MaterialTheme.typography.bodySmall, color = c.muted)
        Spacer(Modifier.height(2.dp))
        Text(f.second, style = MaterialTheme.typography.titleMedium, color = c.onBg)
        f.third?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = c.muted) }
    }
}
