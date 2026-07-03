package com.forge.app.ui.profile

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
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.theme.LocalForgeSettings
import kotlin.math.abs
import kotlin.math.roundToInt

/** The comparison window for the little "vs a month ago" delta beside the current weight. */
private const val DELTA_WINDOW_MS = 30L * 86_400_000L

/**
 * BODYWEIGHT — your weight belongs on your profile, not buried in Stats (moved 2026-07-01). The
 * current weight as an open serif figure with a ~30-day delta beside it, the recent trend as a
 * quiet sparkline, and the quick-log behind the header's "+ log" action (the only manual weigh-in
 * entry point after onboarding).
 */
@Composable
internal fun BodySection(
    entries: List<BodyweightEntry>,
    onLog: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color
) {
    val useKg = LocalForgeSettings.current.useKg
    val unit = unitLabel(useKg)
    SectionHeader("BODYWEIGHT", muted, accent, action = "+ log", onAction = onLog)
    if (entries.isEmpty()) {
        // Just a quiet hint under the header — the "+ log" action IS the entry point. A bold CTA row
        // here read as out of place between the populated sections.
        InlineEmptyHint("Log a weigh-in and your weight trend charts here.", muted)
        return
    }
    val display = remember(entries, useKg) { entries.map { toDisplayWeight(it.weightLb, useKg) } }
    // Delta vs the earliest weigh-in inside the last ~30 days (falls back to the previous entry).
    val delta = remember(entries, display) {
        if (entries.size < 2) null else {
            val cutoff = entries.last().recordedAt - DELTA_WINDOW_MS
            val refIdx = entries.indexOfFirst { it.recordedAt >= cutoff }.coerceAtMost(entries.lastIndex - 1)
            display.last() - display[refIdx.coerceAtLeast(0)]
        }
    }
    Row(verticalAlignment = Alignment.Top) {
        Text(
            "${display.last().roundToInt()}",
            style = MaterialTheme.typography.headlineLarge,
            color = onBg
        )
        delta?.let {
            if (abs(it) >= 0.05) {
                Spacer(Modifier.width(8.dp))
                Text(
                    "${if (it > 0) "↑" else "↓"} %.1f $unit · 30 days".format(abs(it)),
                    style = MaterialTheme.typography.labelSmall,
                    color = muted, fontSize = 9.sp,
                    modifier = Modifier.padding(top = 6.dp)
                )
            }
        }
    }
    Spacer(Modifier.height(2.dp))
    Text("${unit.uppercase()} NOW", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
    if (display.size >= 2) {
        Spacer(Modifier.height(14.dp))
        ProfileSparkline(display, accent, Modifier.fillMaxWidth().height(56.dp))
        Spacer(Modifier.height(8.dp))
        ChartCaption(accent, "LAST ${entries.size} WEIGH-INS", muted)
    }
}
