@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package com.forge.app.ui.gym.train

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.ForgeChromeButton
import com.forge.app.ui.gym.train.state.DayUiEvent
import com.forge.app.ui.gym.train.state.DayUiState
import com.forge.app.ui.gym.train.state.TimeFitApplied

/** The "I have N minutes" choices on the day screen. */
internal val TIME_FIT_CHOICES = listOf(15, 20, 30, 45, 60)

/**
 * Shown before the first set, and after that only while a fit can still be undone: the day screen
 * isn't the place for a permanent control nobody needs mid-set.
 */
internal fun showTimeFit(state: DayUiState): Boolean =
    !state.isFinished && state.exercises.isNotEmpty() &&
        (state.timeFit != null || state.exercises.all { it.loggedSets.isEmpty() })

/** What the fit did, in one line: which exercises it skipped, or that nothing needed to go. */
internal fun timeFitSummary(fit: TimeFitApplied): String = when (fit.skippedNames.size) {
    0 -> "All of it fits in ${fit.minutes} minutes."
    1 -> "Skipping ${fit.skippedNames[0]} to fit ${fit.minutes} minutes. Your main lifts stay."
    else -> "Skipping ${fit.skippedNames.dropLast(1).joinToString(", ")} and ${fit.skippedNames.last()} " +
        "to fit ${fit.minutes} minutes. Your main lifts stay."
}

/**
 * "Short on time?" — tap how long you have and the day skips what doesn't fit (accessories first,
 * never work already started), through the ordinary per-exercise skip. Undo puts it all back.
 */
@Composable
internal fun TimeFitRow(state: DayUiState, onEvent: (DayUiEvent) -> Unit) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val fit = state.timeFit
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 10.dp)) {
        Text("SHORT ON TIME?", style = MaterialTheme.typography.labelSmall, color = muted)
        Spacer(Modifier.height(4.dp))
        Text(
            if (fit == null) "Say how long you have. Avex keeps the work that matters most and skips the rest."
            else timeFitSummary(fit),
            style = MaterialTheme.typography.bodySmall,
            color = if (fit == null) muted else MaterialTheme.colorScheme.onBackground
        )
        Spacer(Modifier.height(6.dp))
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            TIME_FIT_CHOICES.forEach { minutes ->
                val selected = fit?.minutes == minutes
                ForgeChromeButton(
                    onClick = { if (!selected) onEvent(DayUiEvent.FitToTime(minutes)) },
                    label = "I have $minutes minutes"
                ) {
                    Text(
                        "$minutes min",
                        style = MaterialTheme.typography.labelLarge,
                        color = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onBackground,
                        modifier = Modifier.padding(horizontal = 14.dp)
                    )
                }
            }
            if (fit != null) {
                ForgeChromeButton(
                    onClick = { onEvent(DayUiEvent.UndoFitToTime) },
                    label = "Undo, put the skipped exercises back"
                ) {
                    Text(
                        "Undo",
                        style = MaterialTheme.typography.labelLarge,
                        color = muted,
                        modifier = Modifier.padding(horizontal = 14.dp)
                    )
                }
            }
        }
    }
}
