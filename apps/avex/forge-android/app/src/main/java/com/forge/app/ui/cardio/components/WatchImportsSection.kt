package com.forge.app.ui.cardio.components

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.health.HcExerciseTypes
import com.forge.app.domain.cardio.CardioType
import com.forge.app.domain.health.WatchWorkout
import com.forge.app.domain.units.formatDistance
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.clickableLabeled
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * FROM YOUR WATCH (W5) — watch-recorded workouts with no matching entry, offered as one-tap imports.
 * Each row's whole surface imports it (opens the prefilled log sheet); the header's `hide` action
 * dismisses the batch permanently. Suggestions only, never auto-logged.
 *
 * It leads the WEEK lens because it is the only section still asking the user for a decision (§4.8).
 */
@Composable
internal fun WatchImportsSection(
    suggestions: List<WatchWorkout>,
    useMiles: Boolean,
    onImport: (WatchWorkout) -> Unit,
    onDismiss: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color
) {
    Column {
        Row(
            modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            EditorialHeader(
                label = "From your watch",
                muted = muted,
                accent = accent,
                modifier = Modifier.weight(1f)
            )
            Text(
                "hide",
                style = MaterialTheme.typography.labelMedium,
                color = muted,
                modifier = Modifier
                    .clickableLabeled("Hide watch workout suggestions", onClick = onDismiss)
                    .padding(horizontal = 4.dp, vertical = 8.dp)
            )
        }
        Spacer(Modifier.height(8.dp))
        suggestions.forEach { w ->
            val type = CardioType.entries
                .firstOrNull { it.code == HcExerciseTypes.toCardioCode(w.exerciseType) }
            val dayLabel = remember(w.startMs) {
                SimpleDateFormat("EEE h:mm a", Locale.getDefault()).format(Date(w.startMs))
            }
            val meta = buildList {
                add(dayLabel)
                add("${w.durationMin} min")
                w.distanceKm?.let { add(formatDistance(it, useMiles)) }
            }.joinToString(" · ")
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickableLabeled("Import ${type?.displayName ?: "workout"}", onClick = { onImport(w) })
                    .padding(horizontal = 24.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(Modifier.weight(1f)) {
                    Text(type?.displayName ?: "Workout", style = MaterialTheme.typography.bodyLarge, color = onBg)
                    Text(
                        meta.uppercase(),
                        style = MaterialTheme.typography.labelSmall,
                        color = muted, letterSpacing = 0.5.sp
                    )
                }
                Text("import →", style = MaterialTheme.typography.labelMedium, color = accent)
            }
        }
    }
}
