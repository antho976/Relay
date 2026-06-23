package com.forge.app.ui.cardio

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowForward
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.cardio.components.BarGeom
import com.forge.app.ui.cardio.components.VerticalBarRow
import com.forge.app.ui.cardio.state.CardioDayCell

/**
 * A slim, glanceable week summary — week range + a one-line sessions/minutes tally with a "stats →"
 * affordance. No big headline; the full per-week breakdown (graphs, numbers, day bars) lives behind
 * the tap, in the swipeable stats overlay. The whole block is the tap target.
 */
@Composable
internal fun CardioHero(
    weekDays: Int,
    weekMinutes: Int,
    weekNum: Int,
    weekLabel: String,
    onBg: Color,
    muted: Color,
    onOpenDetail: () -> Unit
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onOpenDetail)
            .padding(horizontal = 24.dp)
            .padding(top = 12.dp, bottom = 10.dp)
    ) {
        Text("WEEK $weekNum · $weekLabel", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
        Spacer(Modifier.height(8.dp))
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            val summary = if (weekDays > 0) {
                val d = if (weekDays == 1) "day" else "days"
                "$weekDays $d · $weekMinutes min"
            } else {
                "Nothing logged yet this week"
            }
            Text(summary, style = MaterialTheme.typography.bodyLarge, color = onBg)
            Text("stats →", style = MaterialTheme.typography.labelMedium, color = onBg.copy(alpha = 0.75f))
        }
    }
}

@Composable
internal fun WeekBoxRow(
    days: List<CardioDayCell>,
    todayDow: Int,
    onBg: Color,
    muted: Color,
    outline: Color,
    onClick: () -> Unit
) {
    val dayLetters = listOf("M", "T", "W", "T", "F", "S", "S")
    val maxMin = (days.maxOfOrNull { it.minutes } ?: 0).coerceAtLeast(1)
    // (minutes, hasActivity, isRest) for a day. A rest day reads distinctly from an untrained one.
    fun cellAt(i: Int): Triple<Int, Boolean, Boolean> {
        val cell = days.getOrNull(i)
        val mins = cell?.minutes ?: 0
        val hasActivity = mins > 0
        return Triple(mins, hasActivity, !hasActivity && (cell?.isRest ?: false))
    }
    VerticalBarRow(
        count = 7,
        trackHeight = 48.dp,
        modifier = Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 24.dp),
        bar = { i ->
            val (mins, hasActivity, isRest) = cellAt(i)
            val frac = (mins.toFloat() / maxMin).coerceIn(0f, 1f)
            when {
                hasActivity -> BarGeom(height = (8 + 40 * frac).dp, fill = onBg)
                isRest -> BarGeom(height = 12.dp, fill = muted.copy(alpha = 0.5f))
                i == todayDow -> BarGeom(height = 4.dp, dashedOutline = onBg.copy(alpha = 0.6f))
                else -> BarGeom(height = 4.dp, fill = outline.copy(alpha = 0.35f))
            }
        },
        top = { i ->
            val (mins, hasActivity, isRest) = cellAt(i)
            when {
                hasActivity -> Text("${mins}m", fontSize = 9.sp, color = onBg, fontWeight = FontWeight.SemiBold)
                isRest -> Text("rest", fontSize = 8.sp, color = muted)
            }
        },
        bottom = { i ->
            Text(
                dayLetters[i],
                fontSize = 9.sp,
                color = if (i == todayDow) onBg else muted,
                fontWeight = if (i == todayDow) FontWeight.Bold else FontWeight.Normal
            )
        }
    )
    Spacer(Modifier.height(8.dp))
}

@Composable
internal fun LogTodayRow(onOpenLog: () -> Unit, onBg: Color, muted: Color, outline: Color) {
    // Prominent full-width CTA so logging never feels hidden.
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp)
            .clip(RoundedCornerShape(16.dp))
            .border(1.dp, onBg, RoundedCornerShape(16.dp))
            .clickable(onClick = onOpenLog)
            .padding(horizontal = 16.dp, vertical = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp)
    ) {
        Box(
            modifier = Modifier.size(40.dp).background(onBg, CircleShape),
            contentAlignment = Alignment.Center
        ) {
            Icon(Icons.Filled.Add, contentDescription = null, tint = MaterialTheme.colorScheme.background, modifier = Modifier.size(20.dp))
        }
        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text("Log today's cardio", style = MaterialTheme.typography.bodyLarge, color = onBg, fontWeight = FontWeight.SemiBold)
            Text("run · walk · treadmill · rest", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
        }
        Icon(Icons.AutoMirrored.Filled.ArrowForward, contentDescription = null, tint = onBg, modifier = Modifier.size(16.dp))
    }
    Spacer(Modifier.height(8.dp))
}
