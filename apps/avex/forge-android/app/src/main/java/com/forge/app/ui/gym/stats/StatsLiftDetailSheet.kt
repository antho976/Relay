package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.gym.stats.components.Sparkline
import com.forge.app.ui.gym.stats.components.rememberDrawProgress
import com.forge.app.ui.gym.stats.state.E1rmLift
import com.forge.app.ui.gym.stats.state.HistoryPoint
import com.forge.app.ui.gym.stats.state.PrRecord
import com.forge.app.ui.theme.ForgeLastGreen
import com.forge.app.ui.theme.LocalForgeSettings
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale
import kotlin.math.roundToInt

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun StatsLiftDetailSheet(
    lift: E1rmLift,
    hallRecord: PrRecord?,
    recentHistory: List<HistoryPoint>, // newest-first
    onDismiss: () -> Unit
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val useKg = LocalForgeSettings.current.useKg
    val unit = unitLabel(useKg)
    val dateFmt = DateTimeFormatter.ofPattern("MMM d, yyyy", Locale.getDefault())
    val zone = ZoneId.systemDefault()

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 24.dp)
                .padding(bottom = 32.dp)
        ) {
            // Lift name
            Text(
                lift.exerciseName,
                style = MaterialTheme.typography.headlineSmall,
                color = MaterialTheme.colorScheme.onBackground,
                fontStyle = FontStyle.Italic
            )
            Spacer(Modifier.height(4.dp))

            // Current e1RM
            Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(
                    "${toDisplayWeight(lift.currentE1rm, useKg).roundToInt()}",
                    style = MaterialTheme.typography.displaySmall,
                    color = MaterialTheme.colorScheme.onBackground,
                    fontWeight = FontWeight.Bold
                )
                Text(
                    unit.uppercase(),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 10.sp,
                    modifier = Modifier.padding(bottom = 8.dp)
                )
                Text(
                    "estimated 1RM",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 10.sp,
                    modifier = Modifier.padding(bottom = 8.dp)
                )
            }

            // Monthly rate (if available)
            lift.monthlyPct?.let { pct ->
                val sign = if (pct >= 0) "+" else ""
                val rateColor = when {
                    pct > 0 -> ForgeLastGreen
                    pct < 0 -> MaterialTheme.colorScheme.error
                    else -> MaterialTheme.colorScheme.onSurfaceVariant
                }
                Text(
                    "$sign${"%.1f".format(pct)}% / mo",
                    style = MaterialTheme.typography.bodySmall,
                    color = rateColor,
                    fontWeight = FontWeight.SemiBold
                )
            }

            // e1RM trend sparkline
            if (lift.history.size >= 2) {
                Spacer(Modifier.height(16.dp))
                Text(
                    "E1RM TREND",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 9.sp,
                    letterSpacing = 0.5.sp
                )
                Spacer(Modifier.height(6.dp))
                Sparkline(
                    values = lift.history,
                    lineColor = MaterialTheme.colorScheme.primary,
                    minValue = lift.history.min(),
                    maxValue = lift.history.max(),
                    modifier = Modifier.fillMaxWidth().height(48.dp),
                    progress = rememberDrawProgress("detail-${lift.exerciseId}")
                )
            }

            Spacer(Modifier.height(20.dp))
            HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.2f))
            Spacer(Modifier.height(16.dp))

            // All-time best set
            hallRecord?.let { rec ->
                Text(
                    "ALL-TIME BEST",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 9.sp,
                    letterSpacing = 0.5.sp
                )
                Spacer(Modifier.height(4.dp))
                Text(
                    "${toDisplayWeight(rec.maxWeightLb, useKg).roundToInt()} $unit × ${rec.bestReps} reps",
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onBackground,
                    fontWeight = FontWeight.SemiBold
                )
                Spacer(Modifier.height(16.dp))
                HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.2f))
                Spacer(Modifier.height(16.dp))
            }

            // Recent sessions (up to 5)
            val recent = recentHistory.take(5)
            if (recent.isNotEmpty()) {
                Text(
                    "RECENT SESSIONS",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 9.sp,
                    letterSpacing = 0.5.sp
                )
                Spacer(Modifier.height(8.dp))
                recent.forEach { point ->
                    val dateStr = runCatching {
                        Instant.ofEpochMilli(point.sessionDate)
                            .atZone(zone)
                            .toLocalDate()
                            .format(dateFmt)
                    }.getOrElse { "—" }
                    Row(
                        modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            dateStr,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        Text(
                            "${toDisplayWeight(point.maxWeightLb, useKg).roundToInt()} $unit",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onBackground,
                            fontWeight = FontWeight.Medium
                        )
                    }
                    HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.1f))
                }
            }
        }
    }
}
