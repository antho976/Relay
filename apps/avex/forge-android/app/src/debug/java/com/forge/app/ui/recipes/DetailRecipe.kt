@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.recipes

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgeWordmark
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.rememberDrawProgress
import com.forge.app.ui.theme.ForgeTheme

/**
 * RECIPE — Detail page archetype (DESIGN §3).
 *
 * Scoped to ONE item: one session, one lift, one cardio entry. The defining constraint is that a
 * detail page is NOT a small dashboard — no figure wall, no lens pills for unrelated views.
 *
 *   top bar (wordmark + ← + ≤1 action)
 *   ├─ serif title + context line       the item names itself here, not in the top bar   §4.6
 *   ├─ metric SegmentPills              switching the METRIC of this one item            §4.4
 *   ├─ chart, open on the page          no plot frame, draws in ONCE                     §10
 *   └─ stat rows                        each with its comparison meta                    §4.9
 */
@Composable
fun DetailRecipe(
    onBack: () -> Unit = {},
    exercise: String = "Barbell Row",
    series: List<Float> = listOf(60f, 62.5f, 62.5f, 65f, 67.5f, 67.5f, 70f),
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    var metric by remember { mutableStateOf("Weight") }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { ForgeWordmark() },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        Column(
            Modifier
                .padding(inner)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp)
        ) {
            // ── Title + context ─────────────────────────────────────────────────────────────────
            // §6: ONE serif hero per screen. §11: no terminal period on a serif title.
            Spacer(Modifier.height(8.dp))
            Text(exercise, style = MaterialTheme.typography.headlineMedium, color = onBg)
            Spacer(Modifier.height(2.dp))
            Text(
                "12 sessions · since Feb",
                style = MaterialTheme.typography.labelMedium,
                color = muted
            )

            // ── Metric pills ────────────────────────────────────────────────────────────────────
            // §4.4: these switch the METRIC of this one item. Pills that jump to unrelated views
            // belong on an overview, not here.
            Spacer(Modifier.height(24.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf("Weight", "Volume", "e1RM").forEach { name ->
                    SegmentPill(
                        text = name,
                        selected = metric == name,
                        onClick = { metric = name },
                        accent = accent, onBg = onBg, muted = muted, outline = outline
                    )
                }
            }

            // ── Chart ───────────────────────────────────────────────────────────────────────────
            // §10: open on the page — no plot frame, no boxed card. Draws in ONCE (§9), never
            // re-triggering on scroll.
            Spacer(Modifier.height(24.dp))
            TrendLine(series, accent, muted)

            // ── Stat rows ───────────────────────────────────────────────────────────────────────
            // §4.9: show the reading, not just the conclusion. Each row carries the comparison that
            // makes its number mean something. §8: the right meta is a reading, never a state word.
            Spacer(Modifier.height(28.dp))
            EditorialHeader("THIS SESSION", muted, accent)
            Spacer(Modifier.height(10.dp))
            StatRow("Top set", "70 kg × 8", "△ LAST 67.5 × 8")
            StatRow("Volume", "4,480 kg", "+6% on last")
            StatRow("Best e1RM", "88.7 kg", "personal best")

            Spacer(Modifier.height(32.dp))
        }
    }
}

/** §14: the value-reading contentDescription is what makes a Canvas exist for TalkBack at all. */
@Composable
private fun TrendLine(series: List<Float>, accent: Color, muted: Color) {
    val progress = rememberDrawProgress()
    val min = series.minOrNull() ?: 0f
    val max = series.maxOrNull() ?: 1f
    val span = (max - min).takeIf { it > 0f } ?: 1f
    val reading = "Trend from ${min.toInt()} to ${max.toInt()} over ${series.size} sessions"

    Canvas(
        Modifier
            .fillMaxWidth()
            .height(120.dp)                       // no text inside — a fixed height is right here
            .semantics { contentDescription = reading }
    ) {
        if (series.size < 2) return@Canvas
        val step = size.width / (series.size - 1)
        val path = Path()
        val shown = (series.size * progress).toInt().coerceAtLeast(2)
        series.take(shown).forEachIndexed { i, v ->
            val x = i * step
            val y = size.height - ((v - min) / span) * size.height
            if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
        }
        drawPath(
            path = path,
            color = accent,                        // §5 ladder: accent 1.0 for chart strokes
            style = Stroke(width = 2.dp.toPx(), cap = StrokeCap.Round)
        )
    }
}

/** One data row. Passive content — so no box, no border, no fill (§1). */
@Composable
private fun StatRow(label: String, value: String, meta: String) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        Modifier
            .fillMaxWidth()
            // §7: ONE vertical padding for ALL of a lens's rows. Sibling sections never mix 4/5/6.
            .padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text(
            label.uppercase(),
            style = MaterialTheme.typography.labelSmall,
            color = muted,
            modifier = Modifier.weight(1f)
        )
        Text(value, style = MaterialTheme.typography.bodyMedium, color = onBg)
        Text(
            meta,
            style = MaterialTheme.typography.labelSmall,
            color = muted.copy(alpha = 0.65f)
        )
    }
}

@Preview(name = "Detail", showBackground = true, backgroundColor = 0xFF0E0E11)
@Composable
private fun DetailRecipePreview() {
    ForgeTheme { DetailRecipe() }
}

@Preview(name = "Detail · 200% font", showBackground = true, backgroundColor = 0xFF0E0E11, fontScale = 2.0f)
@Composable
private fun DetailRecipeLargeFontPreview() {
    ForgeTheme { DetailRecipe() }
}
