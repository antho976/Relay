package com.forge.app.ui.profile

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
import com.forge.app.data.db.entities.BodyFatEntry
import com.forge.app.ui.common.InlineEmptyHint
import kotlin.math.abs

/** Comparison window for the little "vs a month ago" delta beside the current body-fat %. */
private const val BF_DELTA_WINDOW_MS = 30L * 86_400_000L

/**
 * BODY FAT (GYMAP-62) — sits in the Profile "your body" cluster beside bodyweight, mirroring its
 * shape: the current reading as an open serif figure with a ~30-day delta, the recent trend as a
 * quiet sparkline, and the quick-log behind the header's "+ log" action. Readings come from a smart
 * scale via Health Connect or manual entry; percentage is unitless, so the delta reads in points.
 * The raw readings drive the line directly (body fat is logged sparsely — a smoothing average would
 * just trace the same points), and the arrow is direction only, never a good/bad verdict.
 */
@Composable
internal fun BodyFatSection(
    entries: List<BodyFatEntry>,
    onLog: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color
) {
    SectionHeader("BODY FAT", muted, action = "+ log", onAction = onLog)
    if (entries.isEmpty()) {
        // A quiet hint under the header — the "+ log" action IS the entry point, mirroring bodyweight.
        InlineEmptyHint("Log body fat or sync a smart scale to chart it here.", muted)
        return
    }
    val values = remember(entries) { entries.map { it.percent } }
    // Trend vs the reading ~30 days ago (falls back to the previous point).
    val delta = remember(entries, values) {
        if (entries.size < 2) null else {
            val cutoff = entries.last().recordedAt - BF_DELTA_WINDOW_MS
            val refIdx = entries.indexOfFirst { it.recordedAt >= cutoff }.coerceAtMost(entries.lastIndex - 1)
            values.last() - values[refIdx.coerceAtLeast(0)]
        }
    }
    Row(verticalAlignment = Alignment.Top) {
        Text(
            "%.1f".format(values.last()),
            style = MaterialTheme.typography.headlineLarge,
            color = onBg
        )
        delta?.let {
            if (abs(it) >= 0.05) {
                Spacer(Modifier.width(8.dp))
                Text(
                    "${if (it > 0) "↑" else "↓"} %.1f pts · 30 days".format(abs(it)),
                    style = MaterialTheme.typography.labelSmall,
                    color = muted, fontSize = 9.sp,
                    modifier = Modifier.padding(top = 6.dp)
                )
            }
        }
    }
    Spacer(Modifier.height(2.dp))
    Text("% NOW", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
    if (values.size >= 2) {
        Spacer(Modifier.height(14.dp))
        ProfileSparkline(
            values = values,
            color = accent,
            modifier = Modifier.fillMaxWidth().height(56.dp)
        )
        Spacer(Modifier.height(8.dp))
        ChartCaption(accent, "LAST ${entries.size} READINGS", muted)
    }
}
