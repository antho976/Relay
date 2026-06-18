package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.repo.CoachWatch
import com.forge.app.ui.theme.ForgeLastGreen
import com.forge.app.ui.theme.ForgeWarning

/**
 * Compact Snapshot-tab surface: "what the coach is currently watching" — the still-learning
 * state made visible. NOT a verdict, just the inputs labelled. Tapping → Coach Lab (handled
 * by the nav layer if wired; here it's read-only).
 */
@Composable
internal fun CoachWatchCard(
    watch: CoachWatch,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    Column(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp)
    ) {
        // Header row
        Row(
            Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Text(
                "COACH · STILL LEARNING",
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                fontSize = 9.sp,
                letterSpacing = 1.sp
            )
            val statusText = when {
                watch.sessionsToGo > 0 -> "${watch.sessionsToGo} session${if (watch.sessionsToGo == 1) "" else "s"} to go"
                watch.autopilot -> "auto"
                else -> "active"
            }
            Text(
                statusText,
                style = MaterialTheme.typography.labelSmall,
                color = if (watch.sessionsToGo > 0) muted else ForgeLastGreen,
                fontSize = 9.sp
            )
        }
        Spacer(Modifier.height(8.dp))

        // Progress line
        val progressLine = if (watch.sessionsToGo > 0)
            "${watch.sessionsLogged} of ${watch.minSessions} sessions logged — watching and learning."
        else
            "${watch.sessionsLogged} sessions in — coach is active."
        Text(
            progressLine,
            style = MaterialTheme.typography.bodySmall,
            color = onBg.copy(alpha = 0.8f),
            fontStyle = FontStyle.Italic
        )

        // Tracked lifts (up to 3)
        if (watch.trackedLifts.isNotEmpty()) {
            Spacer(Modifier.height(10.dp))
            Text("TRACKING", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp, letterSpacing = 0.8.sp)
            Spacer(Modifier.height(4.dp))
            watch.trackedLifts.take(3).forEach { lift ->
                Row(
                    Modifier.fillMaxWidth().padding(vertical = 2.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(
                        lift.name,
                        style = MaterialTheme.typography.bodySmall,
                        color = if (lift.stalling) ForgeWarning else onBg
                    )
                    Text(
                        "${lift.bouts} session${if (lift.bouts == 1) "" else "s"}${if (!lift.concrete) " · forming" else ""}",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontSize = 9.sp
                    )
                }
            }
            if (watch.trackedLifts.size > 3) {
                Text(
                    "+${watch.trackedLifts.size - 3} more",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted,
                    fontSize = 9.sp
                )
            }
        }

        // Recovery signals that are active
        val activeSignals = watch.recoverySignals.filter { it.active }
        if (activeSignals.isNotEmpty()) {
            Spacer(Modifier.height(10.dp))
            Text("RECOVERY INPUTS", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp, letterSpacing = 0.8.sp)
            Spacer(Modifier.height(4.dp))
            activeSignals.forEach { sig ->
                Text(
                    "· ${sig.label}: ${sig.detail}",
                    style = MaterialTheme.typography.bodySmall,
                    color = onBg.copy(alpha = 0.7f),
                    fontSize = 11.sp
                )
            }
        }

        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}
