package com.forge.app.ui.gym.stats

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
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.gym.stats.components.ExerciseFrequencySection
import com.forge.app.ui.gym.stats.components.LifetimeStat
import com.forge.app.ui.gym.stats.components.SparklineWithAxis
import com.forge.app.ui.gym.stats.components.VolumeBarsSection
import com.forge.app.ui.gym.stats.components.formatVolume
import com.forge.app.ui.gym.stats.components.numberWord
import com.forge.app.ui.gym.stats.components.weekCommentary
import com.forge.app.ui.gym.stats.state.ExerciseFrequency
import com.forge.app.ui.gym.stats.state.LifetimeMetrics
import com.forge.app.ui.gym.stats.state.MuscleVolume
import com.forge.app.ui.gym.stats.state.StrengthCurve

@Composable
internal fun StatsHeroSection(
    weekNum: Int,
    weekLabel: String,
    weekSessions: Int,
    weekCurrentVolumeLb: Double?,
    weekCurrentPrs: Int,
    cardioMin: Int,
    onBg: Color,
    muted: Color
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp)
            .padding(top = 20.dp, bottom = 4.dp)
    ) {
        Text(
            "WEEK $weekNum · $weekLabel",
            style = MaterialTheme.typography.labelSmall,
            color = muted,
            fontSize = 9.sp
        )
        Spacer(Modifier.height(8.dp))
        Text(
            "${numberWord(weekSessions)} sessions logged.",
            style = MaterialTheme.typography.displayLarge,
            color = onBg
        )
        Spacer(Modifier.height(4.dp))
        val subtitleParts = buildList {
            if ((weekCurrentVolumeLb ?: 0.0) > 0)
                add("${formatVolume(weekCurrentVolumeLb!!)} lb moved")
            if (weekCurrentPrs > 0)
                add("$weekCurrentPrs ${if (weekCurrentPrs == 1) "PR" else "PRs"}")
            if (cardioMin > 0)
                add("$cardioMin cardio min")
        }
        if (subtitleParts.isNotEmpty()) {
            Text(
                subtitleParts.joinToString(" · "),
                style = MaterialTheme.typography.bodySmall,
                color = muted,
                fontStyle = FontStyle.Italic
            )
        }
        val commentary = weekCommentary(weekSessions, weekCurrentPrs)
        if (commentary.isNotEmpty()) {
            Spacer(Modifier.height(2.dp))
            Text(
                commentary,
                style = MaterialTheme.typography.bodySmall,
                color = muted.copy(alpha = 0.7f),
                fontStyle = FontStyle.Italic
            )
        }
        Spacer(Modifier.height(20.dp))
    }
}

@Composable
internal fun StatsStrengthSection(
    curve: StrengthCurve,
    accent: Color,
    muted: Color,
    outline: Color,
    onBg: Color
) {
    Column(modifier = Modifier.padding(horizontal = 24.dp)) {
        Text(
            "Where I'm going",
            style = MaterialTheme.typography.headlineSmall,
            color = onBg,
            fontStyle = FontStyle.Italic
        )
        Spacer(Modifier.height(4.dp))
        val delta = if (curve.points.size >= 2) curve.points.last() - curve.points.first() else 0.0
        val deltaText = when {
            delta > 0 -> "+${delta.toInt()} lb since first session"
            delta < 0 -> "${delta.toInt()} lb since first session"
            curve.points.isNotEmpty() -> "${curve.points.size} sessions logged"
            else -> "no data yet"
        }
        Text(
            "${curve.plan.name} — $deltaText.",
            style = MaterialTheme.typography.bodySmall,
            color = muted,
            fontStyle = FontStyle.Italic
        )
        Spacer(Modifier.height(12.dp))
        SparklineWithAxis(
            values = curve.points,
            lineColor = accent,
            modifier = Modifier.fillMaxWidth().height(80.dp)
        )
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}

@Composable
internal fun StatsVolumeSection(
    rows: List<MuscleVolume>,
    muted: Color,
    accent: Color,
    outline: Color,
    onBg: Color
) {
    Column(modifier = Modifier.padding(horizontal = 24.dp)) {
        Text(
            "Where the weight went",
            style = MaterialTheme.typography.headlineSmall,
            color = onBg,
            fontStyle = FontStyle.Italic
        )
        Spacer(Modifier.height(4.dp))
        val totalLb = rows.sumOf { it.volumeLb }
        Text(
            "${formatVolume(totalLb)} lb this week, by muscle group.",
            style = MaterialTheme.typography.bodySmall,
            color = muted,
            fontStyle = FontStyle.Italic
        )
        Spacer(Modifier.height(16.dp))
        VolumeBarsSection(rows = rows, muted = muted, accent = accent)
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}

@Composable
internal fun StatsFreqSection(
    rows: List<ExerciseFrequency>,
    muted: Color,
    accent: Color,
    outline: Color,
    onBg: Color
) {
    Column(Modifier.padding(horizontal = 24.dp)) {
        Text(
            "What you train most",
            style = MaterialTheme.typography.headlineSmall,
            color = onBg,
            fontStyle = FontStyle.Italic
        )
        Spacer(Modifier.height(4.dp))
        Text(
            "Sessions per exercise, last 8 weeks.",
            style = MaterialTheme.typography.bodySmall,
            color = muted,
            fontStyle = FontStyle.Italic
        )
        Spacer(Modifier.height(16.dp))
        ExerciseFrequencySection(rows = rows.take(6), muted = muted, accent = accent)
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}

@Composable
internal fun StatsLifetimeSection(
    lm: LifetimeMetrics?,
    onBg: Color,
    muted: Color,
    outline: Color
) {
    Column(Modifier.padding(horizontal = 24.dp)) {
        Text(
            "Your body of work",
            style = MaterialTheme.typography.headlineSmall,
            color = onBg,
            fontStyle = FontStyle.Italic
        )
        Spacer(Modifier.height(4.dp))
        if (lm != null && lm.totalSessions > 0) {
            Text(
                "${numberWord(lm.totalSessions)} sessions · ${formatVolume(lm.lifetimeVolumeLb)} lb total.",
                style = MaterialTheme.typography.bodySmall,
                color = muted,
                fontStyle = FontStyle.Italic
            )
            Spacer(Modifier.height(12.dp))
            Row(modifier = Modifier.fillMaxWidth()) {
                LifetimeStat(
                    value = lm.totalSessions.toString(),
                    label = "SESSIONS",
                    muted = muted, onBg = onBg,
                    modifier = Modifier.weight(1f)
                )
                LifetimeStat(
                    value = formatVolume(lm.lifetimeVolumeLb),
                    label = "LB TOTAL",
                    muted = muted, onBg = onBg,
                    modifier = Modifier.weight(1f)
                )
                LifetimeStat(
                    value = formatVolume(lm.avgSessionVolumeLb),
                    label = "AVG / SESSION",
                    muted = muted, onBg = onBg,
                    modifier = Modifier.weight(1f)
                )
            }
        } else {
            Text(
                "Your stats will fill in as you train.",
                style = MaterialTheme.typography.bodySmall,
                color = muted.copy(alpha = 0.6f),
                fontStyle = FontStyle.Italic
            )
        }
        Spacer(Modifier.height(20.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(20.dp))
    }
}
