package com.forge.app.ui.overview

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
import com.forge.app.data.repo.StatsRepository
import com.forge.app.domain.units.formatDistance
import com.forge.app.domain.units.formatVolume
import com.forge.app.domain.units.formatWeight
import com.forge.app.ui.common.currentLocale
import com.forge.app.ui.theme.LocalForgeSettings
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter

// ─── Shared summary sheet (opened from OverviewScreen for a tapped recent item) ───────────

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SummarySheet(
    title: String,
    dateMs: Long,
    tag: String,
    durationMin: Int?,
    volumeLb: Double?,
    prCount: Int,
    vsAvgPct: Int?,
    isBest: Boolean,
    isGym: Boolean,
    distanceKm: Double? = null,
    exerciseLines: List<StatsRepository.SessionExerciseLine> = emptyList(),
    onDismiss: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val bg = MaterialTheme.colorScheme.background

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = bg
    ) {
        Column(modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
            val zone = ZoneId.systemDefault()
            val date = Instant.ofEpochMilli(dateMs).atZone(zone).toLocalDate()
            val dateStr = date.format(DateTimeFormatter.ofPattern("EEEE, d MMMM", currentLocale()))

            if (tag.isNotEmpty()) {
                Text(tag, style = MaterialTheme.typography.labelSmall,
                    letterSpacing = 1.5.sp, color = muted, fontSize = 9.sp)
                Spacer(Modifier.height(2.dp))
            }
            Text(title, style = MaterialTheme.typography.headlineSmall, color = onBg,
                fontWeight = FontWeight.Normal)
            Text(dateStr, style = MaterialTheme.typography.bodySmall,
                color = muted, fontSize = 11.sp, fontStyle = FontStyle.Italic)

            Spacer(Modifier.height(20.dp))

            val weightUnit = LocalForgeSettings.current.weightUnit
            val useMiles = LocalForgeSettings.current.useMiles
            // Stats strip
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                if (isGym && volumeLb != null && volumeLb > 0) {
                    SummaryStat(value = formatVolume(volumeLb, weightUnit), label = "VOLUME", muted = muted, onBg = onBg)
                }
                if (durationMin != null && durationMin > 0) {
                    SummaryStat(value = "$durationMin min", label = "DURATION", muted = muted, onBg = onBg)
                }
                if (!isGym && distanceKm != null && distanceKm > 0) {
                    SummaryStat(value = formatDistance(distanceKm, useMiles), label = "DISTANCE", muted = muted, onBg = onBg)
                }
                if (isGym && prCount > 0) {
                    SummaryStat(value = "$prCount", label = "PRs", muted = muted, onBg = onBg)
                }
                if (isGym) {
                    val compText = when {
                        isBest -> "BEST"
                        vsAvgPct != null -> "${if (vsAvgPct >= 0) "+" else ""}$vsAvgPct%"
                        else -> null
                    }
                    if (compText != null) {
                        SummaryStat(value = compText, label = "vs AVG", muted = muted, onBg = onBg)
                    }
                }
            }

            if (isGym && exerciseLines.isNotEmpty()) {
                Spacer(Modifier.height(20.dp))
                HorizontalDivider(color = outline.copy(alpha = 0.2f))
                Spacer(Modifier.height(14.dp))

                Text("EXERCISES", style = MaterialTheme.typography.labelSmall,
                    color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
                Spacer(Modifier.height(8.dp))

                exerciseLines.forEach { ex ->
                    Row(
                        modifier = Modifier.fillMaxWidth().padding(vertical = 5.dp),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            ex.exerciseName,
                            style = MaterialTheme.typography.bodySmall,
                            color = onBg,
                            modifier = Modifier.weight(1f)
                        )
                        val setInfo = if (ex.topWeightLb != null && ex.topWeightLb > 0)
                            "${ex.setCount} × ${formatWeight(ex.topWeightLb, weightUnit)}"
                        else "${ex.setCount} sets"
                        Text(setInfo, style = MaterialTheme.typography.labelSmall,
                            color = muted, fontSize = 10.sp)
                    }
                }
            }

            Spacer(Modifier.height(48.dp))
        }
    }
}
