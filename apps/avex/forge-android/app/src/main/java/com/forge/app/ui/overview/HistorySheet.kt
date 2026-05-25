package com.forge.app.ui.overview

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.repo.StatsRepository
import java.time.Instant
import java.time.YearMonth
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HistorySheet(
    onDismiss: () -> Unit,
    vm: HistoryViewModel = hiltViewModel()
) {
    val entries by vm.entries.collectAsStateWithLifecycle()
    val selectedEntry by vm.selectedEntry.collectAsStateWithLifecycle()
    val exerciseLines by vm.sessionExerciseLines.collectAsStateWithLifecycle()

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val bg = MaterialTheme.colorScheme.background

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = bg
    ) {
        val zone = ZoneId.systemDefault()
        val grouped = entries
            .groupBy { YearMonth.from(Instant.ofEpochMilli(it.timestampMs).atZone(zone).toLocalDate()) }
            .entries
            .sortedByDescending { it.key }

        if (entries.isEmpty()) {
            Column(
                modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 32.dp),
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                Text("HISTORY", style = MaterialTheme.typography.labelSmall,
                    letterSpacing = 1.5.sp, color = muted, fontSize = 10.sp)
                Spacer(Modifier.height(24.dp))
                Text("no sessions yet.", style = MaterialTheme.typography.bodySmall,
                    color = muted.copy(alpha = 0.5f), fontStyle = FontStyle.Italic)
                Spacer(Modifier.height(48.dp))
            }
        } else {
            LazyColumn(contentPadding = PaddingValues(bottom = 48.dp)) {
                item {
                    Text(
                        "HISTORY",
                        style = MaterialTheme.typography.labelSmall,
                        letterSpacing = 1.5.sp,
                        color = muted,
                        fontSize = 10.sp,
                        modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp)
                    )
                }
                grouped.forEach { (month, monthEntries) ->
                    item(key = "hdr_$month") {
                        Text(
                            month.format(DateTimeFormatter.ofPattern("MMMM yyyy", Locale.getDefault())).uppercase(),
                            style = MaterialTheme.typography.labelSmall,
                            color = muted.copy(alpha = 0.45f),
                            fontSize = 9.sp,
                            letterSpacing = 1.sp,
                            modifier = Modifier.padding(horizontal = 24.dp, vertical = 6.dp)
                        )
                    }
                    items(monthEntries, key = { "${it.isGym}_${it.id}" }) { entry ->
                        HistoryRow(
                            entry = entry, muted = muted, onBg = onBg, outline = outline,
                            onClick = { vm.selectEntry(entry) }
                        )
                        HorizontalDivider(
                            modifier = Modifier.padding(horizontal = 24.dp),
                            color = outline.copy(alpha = 0.1f)
                        )
                    }
                }
            }
        }
    }

    if (selectedEntry != null) {
        val entry = selectedEntry!!
        SummarySheet(
            title = entry.title,
            dateMs = entry.timestampMs,
            tag = entry.tag,
            durationMin = entry.durationMin,
            volumeLb = entry.volumeLb,
            prCount = entry.prCount,
            vsAvgPct = entry.vsAvgPct,
            isBest = entry.isBest,
            isGym = entry.isGym,
            distanceKm = entry.distanceKm,
            exerciseLines = exerciseLines,
            onDismiss = { vm.clearSelectedEntry() }
        )
    }
}

@Composable
private fun HistoryRow(
    entry: HistoryViewModel.HistoryEntry,
    muted: Color,
    onBg: Color,
    outline: Color,
    onClick: () -> Unit
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onClick() }
            .padding(horizontal = 24.dp, vertical = 12.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            val zone = ZoneId.systemDefault()
            val date = Instant.ofEpochMilli(entry.timestampMs).atZone(zone).toLocalDate()
            val dateStr = date.format(DateTimeFormatter.ofPattern("d MMM", Locale.getDefault()))
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                Text(
                    dateStr,
                    style = MaterialTheme.typography.labelSmall,
                    color = muted.copy(alpha = 0.55f),
                    fontSize = 9.sp
                )
                if (entry.tag.isNotEmpty()) {
                    Box(
                        modifier = Modifier
                            .border(BorderStroke(0.5.dp, outline.copy(alpha = 0.3f)), RoundedCornerShape(2.dp))
                            .padding(horizontal = 5.dp, vertical = 1.dp)
                    ) {
                        Text(
                            entry.tag,
                            style = MaterialTheme.typography.labelSmall,
                            color = muted.copy(alpha = 0.5f),
                            fontSize = 7.sp,
                            letterSpacing = 0.5.sp
                        )
                    }
                }
            }
            Text(entry.title, style = MaterialTheme.typography.bodyMedium, color = onBg)
            if (entry.subtitle.isNotEmpty()) {
                Text(entry.subtitle, style = MaterialTheme.typography.bodySmall,
                    color = muted, fontSize = 10.sp)
            }
        }

        if (entry.isGym && entry.volumeLb != null && entry.volumeLb > 0) {
            Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(2.dp)) {
                val volText = if (entry.volumeLb >= 1000)
                    "%.1fk".format(entry.volumeLb / 1000) else "${entry.volumeLb.toInt()}"
                Text(
                    "$volText lb",
                    style = MaterialTheme.typography.bodySmall,
                    color = onBg,
                    fontWeight = FontWeight.SemiBold,
                    fontSize = 11.sp
                )
                when {
                    entry.isBest -> Text("BEST", style = MaterialTheme.typography.labelSmall,
                        color = onBg.copy(alpha = 0.85f), fontSize = 8.sp, letterSpacing = 0.6.sp)
                    entry.vsAvgPct != null -> {
                        val sign = if (entry.vsAvgPct >= 0) "+" else ""
                        Text("$sign${entry.vsAvgPct}% avg", style = MaterialTheme.typography.labelSmall,
                            color = muted, fontSize = 8.sp)
                    }
                }
                if (entry.prCount > 0) {
                    Text("${entry.prCount} PR", style = MaterialTheme.typography.labelSmall,
                        color = onBg.copy(alpha = 0.6f), fontSize = 8.sp)
                }
            }
        }
    }
}

// ─── Shared summary sheet (used from OverviewScreen + HistorySheet) ───────────

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
            val dateStr = date.format(DateTimeFormatter.ofPattern("EEEE, d MMMM", Locale.getDefault()))

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

            // Stats strip
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                if (isGym && volumeLb != null && volumeLb > 0) {
                    val volText = if (volumeLb >= 1000) "%.1fk lb".format(volumeLb / 1000) else "${volumeLb.toInt()} lb"
                    SummaryStat(value = volText, label = "VOLUME", muted = muted, onBg = onBg)
                }
                if (durationMin != null && durationMin > 0) {
                    SummaryStat(value = "$durationMin min", label = "DURATION", muted = muted, onBg = onBg)
                }
                if (!isGym && distanceKm != null && distanceKm > 0) {
                    SummaryStat(value = "%.1f km".format(distanceKm), label = "DISTANCE", muted = muted, onBg = onBg)
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
                            "${ex.setCount} × ${ex.topWeightLb.toInt()} lb"
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

@Composable
private fun SummaryStat(value: String, label: String, muted: Color, onBg: Color) {
    Column(verticalArrangement = Arrangement.spacedBy(1.dp)) {
        Text(value, style = MaterialTheme.typography.bodyMedium, color = onBg,
            fontWeight = FontWeight.Normal)
        Text(label, style = MaterialTheme.typography.labelSmall,
            color = muted.copy(alpha = 0.6f), fontSize = 9.sp, letterSpacing = 0.5.sp)
    }
}
