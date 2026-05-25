package com.forge.app.ui.overview

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.program.Program
import com.forge.app.program.Trophies
import com.forge.app.ui.overview.state.OverviewRecentItem
import java.time.LocalDate
import java.time.format.TextStyle
import java.time.temporal.WeekFields
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OverviewScreen(
    onStartSession: (dayKey: String) -> Unit,
    onStartSessionSkipWarmup: (dayKey: String) -> Unit = onStartSession,
    onGoToGym: () -> Unit,
    onGoToCardio: () -> Unit,
    onGoToTrophies: () -> Unit,
    onGoToStats: () -> Unit = {},
    onGoToNutrition: () -> Unit = {},
    onGoToSettings: () -> Unit = {},
    viewModel: OverviewViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()

    LaunchedEffect(state.pendingMilestone) {
        state.pendingMilestone?.let { event ->
            viewModel.onMilestoneShown(event.id)
        }
    }

    val today = LocalDate.now()
    val todayDow = today.dayOfWeek.value - 1
    val weekNumber = today.get(WeekFields.ISO.weekOfWeekBasedYear())
    val dayName = today.dayOfWeek.getDisplayName(TextStyle.SHORT, Locale.getDefault()).uppercase()

    val nextDay = Program.days.firstOrNull { it.key == state.nextUpDayKey }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    Scaffold(containerColor = Color.Transparent) { inner ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(inner)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp)
        ) {
            Spacer(Modifier.height(16.dp))

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.size(5.dp).background(accent, CircleShape))
                    Spacer(Modifier.width(6.dp))
                    Text(
                        "Forge",
                        style = MaterialTheme.typography.bodyMedium,
                        fontStyle = FontStyle.Italic,
                        color = onBg
                    )
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        "$dayName · WK $weekNumber",
                        style = MaterialTheme.typography.labelSmall,
                        fontSize = 13.sp,
                        color = muted
                    )
                    Spacer(Modifier.width(4.dp))
                    IconButton(onClick = onGoToSettings, modifier = Modifier.size(28.dp)) {
                        Icon(
                            Icons.Default.Settings,
                            contentDescription = "Settings",
                            tint = muted,
                            modifier = Modifier.size(16.dp)
                        )
                    }
                }
            }

            Spacer(Modifier.height(20.dp))

            Text("TODAY", style = MaterialTheme.typography.labelSmall, fontSize = 13.sp, color = muted)
            Spacer(Modifier.height(2.dp))
            Text(
                nextDay?.defaultName ?: "Ready",
                style = MaterialTheme.typography.displayLarge,
                color = onBg
            )
            if (nextDay != null) {
                Spacer(Modifier.height(10.dp))
                Text(
                    "${nextDay.subtitle} · ${nextDay.exercises.size} exercises",
                    style = MaterialTheme.typography.bodyMedium.copy(fontStyle = FontStyle.Italic),
                    color = muted
                )
            }

            Spacer(Modifier.height(20.dp))

            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp)
            ) {
                Button(
                    onClick = { onStartSession(state.nextUpDayKey) },
                    shape = RoundedCornerShape(50),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = Color.White,
                        contentColor = Color.Black
                    ),
                    contentPadding = PaddingValues(horizontal = 32.dp, vertical = 18.dp)
                ) {
                    Text(
                        "Start session →",
                        style = MaterialTheme.typography.bodyLarge,
                        fontWeight = FontWeight.SemiBold
                    )
                }
                Text(
                    "skip warmup",
                    style = MaterialTheme.typography.bodySmall,
                    color = muted,
                    modifier = Modifier.clickable { onStartSessionSkipWarmup(state.nextUpDayKey) }
                )
            }

            Spacer(Modifier.height(28.dp))
            HorizontalDivider(color = outline.copy(alpha = 0.3f))
            Spacer(Modifier.height(20.dp))

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text("THIS WEEK", style = MaterialTheme.typography.labelMedium, color = muted)
                Text(
                    "${state.workoutsThisWeek} of 6 target",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted
                )
            }
            Spacer(Modifier.height(12.dp))

            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                listOf("M", "T", "W", "T", "F", "S", "S").forEachIndexed { i, letter ->
                    WeekDayBox(
                        letter = letter,
                        trained = i in state.weekDaysTrained,
                        isToday = i == todayDow,
                        accentColor = accent,
                        outlineColor = outline,
                        modifier = Modifier.weight(1f)
                    )
                }
            }

            Spacer(Modifier.height(16.dp))

            Row(modifier = Modifier.fillMaxWidth()) {
                OverviewStat(
                    value = "${state.workoutsThisWeek}",
                    label = "WORKOUTS",
                    modifier = Modifier.weight(1f)
                )
                OverviewStat(
                    value = "${state.volumeThisWeekLb.toInt()}",
                    label = "LB",
                    modifier = Modifier.weight(1f)
                )
                OverviewStat(
                    value = "${state.cardioMinutesThisWeek}",
                    label = "CARDIO MIN",
                    modifier = Modifier.weight(1f)
                )
            }

            val recentToShow = state.recentItems.ifEmpty {
                listOf(
                    OverviewRecentItem("GYM", "Start a session", "Tap above to begin", ""),
                    OverviewRecentItem("CARDIO", "Log a cardio entry", "Run, walk, or bike", "")
                )
            }
            Spacer(Modifier.height(20.dp))
            HorizontalDivider(color = outline.copy(alpha = 0.3f))
            Spacer(Modifier.height(16.dp))
            Text("RECENT", style = MaterialTheme.typography.labelMedium, color = muted)
            Spacer(Modifier.height(10.dp))
            recentToShow.forEach { item ->
                RecentRow(item = item, muted = muted, onBg = onBg, accent = accent)
                Spacer(Modifier.height(12.dp))
            }

            Spacer(Modifier.height(20.dp))
            HorizontalDivider(color = outline.copy(alpha = 0.3f))
            Spacer(Modifier.height(4.dp))

            val cardioKmText = if (state.cardioDistanceKm == 0.0) "0km"
                else "%.1fkm".format(state.cardioDistanceKm)
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                BottomNavButton(
                    label = "Cardio",
                    metric = cardioKmText,
                    onClick = onGoToCardio,
                    outlineColor = outline,
                    onBg = onBg,
                    muted = muted,
                    modifier = Modifier.weight(1f)
                )
                BottomNavButton(
                    label = "Trophies",
                    metric = "${state.trophiesUnlocked}/${Trophies.all.size}",
                    onClick = onGoToTrophies,
                    outlineColor = outline,
                    onBg = onBg,
                    muted = muted,
                    modifier = Modifier.weight(1f)
                )
            }
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                BottomNavButton(
                    label = "Stats",
                    metric = "${state.totalFinishedSessions} sessions",
                    onClick = onGoToStats,
                    outlineColor = outline,
                    onBg = onBg,
                    muted = muted,
                    modifier = Modifier.weight(1f)
                )
                BottomNavButton(
                    label = "Nutrition",
                    metric = "coming soon",
                    onClick = onGoToNutrition,
                    outlineColor = outline,
                    onBg = onBg,
                    muted = muted,
                    modifier = Modifier.weight(1f)
                )
            }

            Spacer(Modifier.height(24.dp))
        }
    }
}

@Composable
private fun WeekDayBox(
    letter: String,
    trained: Boolean,
    isToday: Boolean,
    accentColor: Color,
    outlineColor: Color,
    modifier: Modifier = Modifier
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    Column(
        modifier = modifier,
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        Text(
            letter,
            style = MaterialTheme.typography.labelSmall,
            color = if (isToday) onBg else MaterialTheme.colorScheme.onSurfaceVariant,
            fontSize = 9.sp
        )
        val boxMod = Modifier
            .fillMaxWidth()
            .aspectRatio(1f)
        when {
            trained -> Box(
                boxMod.background(onBg.copy(alpha = 0.85f), RoundedCornerShape(6.dp))
            )
            isToday -> {
                val dashColor = outlineColor
                Box(
                    boxMod
                        .drawBehind {
                            val stroke = Stroke(
                                width = 1.5.dp.toPx(),
                                pathEffect = PathEffect.dashPathEffect(
                                    floatArrayOf(4.dp.toPx(), 3.dp.toPx()), 0f
                                )
                            )
                            drawRoundRect(
                                color = dashColor,
                                cornerRadius = CornerRadius(6.dp.toPx()),
                                style = stroke
                            )
                        },
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        "NOW",
                        style = MaterialTheme.typography.labelSmall,
                        fontSize = 7.sp,
                        color = outlineColor
                    )
                }
            }
            else -> Box(
                boxMod.border(
                    BorderStroke(1.dp, outlineColor.copy(alpha = 0.35f)),
                    RoundedCornerShape(6.dp)
                )
            )
        }
    }
}

@Composable
private fun OverviewStat(value: String, label: String, modifier: Modifier = Modifier) {
    Column(modifier = modifier) {
        Text(
            value,
            style = MaterialTheme.typography.headlineSmall,
            color = MaterialTheme.colorScheme.onBackground,
            fontWeight = FontWeight.Normal
        )
        Text(
            label,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}

@Composable
private fun RecentRow(
    item: OverviewRecentItem,
    muted: Color,
    onBg: Color,
    accent: Color
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.Top
    ) {
        Text(
            item.dayLabel,
            style = MaterialTheme.typography.labelSmall,
            color = muted,
            modifier = Modifier.width(72.dp),
            fontSize = 9.sp
        )
        Column(modifier = Modifier.weight(1f)) {
            Text(item.title, style = MaterialTheme.typography.bodyMedium, color = onBg)
            Text(
                item.subtitle,
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                fontSize = 10.sp
            )
        }
        Text(
            item.tag,
            style = MaterialTheme.typography.labelSmall,
            color = muted,
            fontSize = 9.sp
        )
    }
}

@Composable
private fun BottomNavButton(
    label: String,
    metric: String,
    onClick: () -> Unit,
    outlineColor: Color,
    onBg: Color,
    muted: Color,
    modifier: Modifier = Modifier
) {
    Box(
        modifier = modifier
            .height(72.dp)
            .border(BorderStroke(0.5.dp, outlineColor.copy(alpha = 0.35f)), RoundedCornerShape(10.dp))
            .clip(RoundedCornerShape(10.dp))
            .clickable { onClick() }
            .padding(horizontal = 14.dp, vertical = 10.dp)
    ) {
        Text(
            metric,
            style = MaterialTheme.typography.bodySmall,
            color = muted,
            modifier = Modifier.align(Alignment.TopEnd)
        )
        Text(
            label,
            style = MaterialTheme.typography.bodyMedium,
            color = onBg,
            modifier = Modifier.align(Alignment.Center)
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun PlanTomorrowSheet(
    currentPlan: String,
    onPick: (String) -> Unit,
    onDismiss: () -> Unit
) {
    val sheetState = rememberModalBottomSheetState()
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = sheetState) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 24.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Text(
                "PLAN TOMORROW",
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary,
                fontWeight = FontWeight.Black
            )
            Spacer(Modifier.height(4.dp))
            Program.days.forEach { day ->
                val isSelected = currentPlan == day.key
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(10.dp))
                        .background(
                            if (isSelected) MaterialTheme.colorScheme.primary.copy(alpha = 0.12f)
                            else MaterialTheme.colorScheme.surfaceVariant
                        )
                        .clickable { onPick(day.key) }
                        .padding(16.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        day.defaultName,
                        style = MaterialTheme.typography.bodyLarge,
                        fontWeight = if (isSelected) FontWeight.SemiBold else FontWeight.Normal,
                        color = if (isSelected) MaterialTheme.colorScheme.primary
                                else MaterialTheme.colorScheme.onSurface
                    )
                    if (isSelected) {
                        Text("✓", color = MaterialTheme.colorScheme.primary, fontWeight = FontWeight.Bold)
                    }
                }
            }
            Spacer(Modifier.height(16.dp))
        }
    }
}
