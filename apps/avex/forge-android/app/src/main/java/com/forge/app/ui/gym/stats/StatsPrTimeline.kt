package com.forge.app.ui.gym.stats

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.units.formatWeight
import com.forge.app.ui.gym.stats.components.CountUpText
import com.forge.app.ui.gym.stats.state.PrEntry
import com.forge.app.ui.theme.ForgeLastGreen
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.emphasized
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * The Strength tab's PR story (revives the deleted PrTimelineStory #93 against the
 * existing recentPrs pipeline): a drought header — big serif days-since-last-PR — and a
 * date-railed vertical timeline of recent records.
 */

/** "N days since your last PR." — counts up from 0; reads as a quiet challenge. */
@Composable
internal fun PrDroughtHeader(daysSince: Int, onBg: Color, muted: Color) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        if (daysSince == 0) {
            Text("PR today.", style = MaterialTheme.typography.displaySmall, color = emphasized(onBg))
            Spacer(Modifier.height(2.dp))
            Text("The drought is over.", style = MaterialTheme.typography.bodySmall, color = ForgeLastGreen, fontStyle = FontStyle.Italic)
        } else {
            Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                CountUpText(
                    value = daysSince.toDouble(),
                    fromValue = 0.0,
                    style = MaterialTheme.typography.displaySmall,
                    color = emphasized(onBg)
                )
                Text(
                    if (daysSince == 1) "day since your last PR." else "days since your last PR.",
                    style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic,
                    modifier = Modifier.padding(bottom = 8.dp)
                )
            }
        }
        Spacer(Modifier.height(16.dp))
    }
}

/**
 * One timeline row: mono date on the left rail, a dot on a hairline spine, the lift and
 * its numbers on the right. [isFirst] gets the accent dot (the most recent record).
 */
@Composable
internal fun PrTimelineRow(
    pr: PrEntry,
    isFirst: Boolean,
    isLast: Boolean,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color,
    modifier: Modifier = Modifier
) {
    val dateText = remember(pr.date) { SimpleDateFormat("MMM d", Locale.getDefault()).format(Date(pr.date)).uppercase() }
    val useKg = LocalForgeSettings.current.useKg
    Row(modifier = modifier.height(IntrinsicSize.Min), verticalAlignment = Alignment.Top) {
        Text(
            dateText, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp,
            modifier = Modifier.width(52.dp).padding(top = 3.dp)
        )
        // The spine: hairline above + below the dot, suppressed at the ends.
        Column(Modifier.width(16.dp).fillMaxHeight(), horizontalAlignment = Alignment.CenterHorizontally) {
            Box(Modifier.width(1.dp).height(4.dp).background(if (isFirst) Color.Transparent else outline.copy(alpha = 0.3f)))
            Box(
                Modifier.size(7.dp).background(
                    if (isFirst) accent else outline.copy(alpha = 0.6f),
                    CircleShape
                )
            )
            Box(Modifier.width(1.dp).weight(1f).background(if (isLast) Color.Transparent else outline.copy(alpha = 0.3f)))
        }
        Spacer(Modifier.width(10.dp))
        Column(Modifier.weight(1f).padding(bottom = 14.dp)) {
            Text(pr.exerciseName, style = MaterialTheme.typography.bodyMedium, color = onBg)
            Text(
                "${formatWeight(pr.weightLb, useKg)} × ${pr.reps}",
                style = MaterialTheme.typography.labelSmall,
                color = if (isFirst) accent else onBg.copy(alpha = 0.7f),
                fontWeight = FontWeight.SemiBold
            )
        }
    }
}
