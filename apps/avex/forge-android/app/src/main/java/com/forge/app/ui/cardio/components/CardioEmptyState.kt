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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.cardio.LogTodayRow

/**
 * The first-run cardio screen — shown only when there are no entries at all. A dedicated,
 * calmer welcome (vs the old dashboard-with-an-empty-footer): a quiet empty week scaffold,
 * a one-line pitch, the prominent log CTA, and a soft hint at the watch-only extras. Once a
 * single session is logged the normal hero + week row + history take over.
 */
@Composable
internal fun CardioEmptyState(
    onOpenLog: () -> Unit,
    weekTargetMin: Int,
    onBg: Color,
    muted: Color,
    outline: Color
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp)
            .padding(top = 24.dp)
    ) {
        // A quiet empty week scaffold — seven low bars — so the page reads as "your week, waiting".
        EmptyWeekScaffold(outline = outline, muted = muted)
        Spacer(Modifier.height(28.dp))
        Row(verticalAlignment = Alignment.Bottom) {
            Text("Start ", style = MaterialTheme.typography.displaySmall, color = onBg)
            Text("moving.", style = MaterialTheme.typography.displaySmall, color = onBg, fontStyle = FontStyle.Italic)
        }
        Spacer(Modifier.height(10.dp))
        Text(
            "Log a run, walk, ride or swim. Avex tracks your days this week, your streak, and how it all trends — no account, all on your phone.",
            style = MaterialTheme.typography.bodyMedium,
            color = muted
        )
        if (weekTargetMin > 0) {
            Spacer(Modifier.height(6.dp))
            Text(
                "Your weekly goal: $weekTargetMin min.",
                style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
            )
        }
        Spacer(Modifier.height(24.dp))
    }
    // Reuse the same prominent CTA the populated screen uses, so logging looks identical everywhere.
    LogTodayRow(onOpenLog = onOpenLog, onBg = onBg, muted = muted, outline = outline)
    Column(modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Spacer(Modifier.height(4.dp))
        Text(
            "Connect a watch or ring later to see steps by the hour and your route.",
            style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.7f), fontSize = 10.sp
        )
    }
}

@Composable
private fun EmptyWeekScaffold(outline: Color, muted: Color) {
    val letters = listOf("M", "T", "W", "T", "F", "S", "S")
    // A quiet seven-bar week skeleton — fixed low bars, the day letter under each.
    VerticalBarRow(
        count = 7,
        trackHeight = 6.dp,
        modifier = Modifier.fillMaxWidth(),
        labelSpacing = 6.dp,
        corner = 3.dp,
        bar = { BarGeom(height = 6.dp, fill = outline.copy(alpha = 0.3f)) },
        bottom = { i -> Text(letters[i], fontSize = 9.sp, color = muted.copy(alpha = 0.6f)) }
    )
}
