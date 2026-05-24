package com.forge.app.ui.overview

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.Alignment
import androidx.compose.ui.draw.clip
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import com.forge.app.ui.theme.LocalForgeSettings
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.bodyweight.BodyweightSheet
import com.forge.app.ui.overview.components.ComebackBanner
import com.forge.app.ui.overview.components.ConsecutiveDayWarning
import com.forge.app.ui.overview.components.DeloadBanner
import com.forge.app.ui.overview.components.NavTile
import com.forge.app.ui.overview.components.OnThisDayCard
import com.forge.app.ui.overview.components.StreakCard
import com.forge.app.ui.overview.components.WeeklyStatsStrip

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OverviewScreen(
    onGoToGym: () -> Unit,
    onGoToCardio: () -> Unit,
    onGoToTrophies: () -> Unit,
    onGoToSettings: () -> Unit = {},
    viewModel: OverviewViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val snackbarHostState = remember { SnackbarHostState() }
    val hiddenTiles = LocalForgeSettings.current.hiddenOverviewTiles
    val tileOrder = LocalForgeSettings.current.overviewTileOrder
    var showBodyweightSheet by remember { mutableStateOf(false) }
    var showPlanTomorrowSheet by remember { mutableStateOf(false) }

    // Show milestone toast once; persist immediately so it never re-fires (#56)
    LaunchedEffect(state.pendingMilestone) {
        state.pendingMilestone?.let { event ->
            viewModel.onMilestoneShown(event.id)
            snackbarHostState.showSnackbar(event.message)
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Text(
                        text = "FORGE",
                        fontSize = 28.sp,
                        fontWeight = FontWeight.Black,
                        letterSpacing = 4.sp,
                        color = MaterialTheme.colorScheme.onBackground
                    )
                },
                actions = {
                    IconButton(onClick = onGoToSettings) {
                        Icon(Icons.Default.Settings, contentDescription = "Settings", tint = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background
                )
            )
        },
        snackbarHost = { SnackbarHost(snackbarHostState) },
        containerColor = MaterialTheme.colorScheme.background
    ) { inner ->
        Box(Modifier.fillMaxSize().padding(inner)) {
            Column(
                modifier = Modifier
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 16.dp, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                WeeklyStatsStrip(
                    workouts = state.workoutsThisWeek,
                    volumeLb = state.volumeThisWeekLb,
                    cardioMinutes = state.cardioMinutesThisWeek
                )

                if (state.streakDays > 0 && "streak" !in hiddenTiles) {
                    StreakCard(streakDays = state.streakDays)
                }

                if (state.showComebackBanner) {
                    ComebackBanner(daysSince = state.daysSinceLastSession ?: 0)
                } else if (state.showConsecutiveWarning) {
                    ConsecutiveDayWarning(consecutiveDays = state.streakDays)
                }

                if (state.needsDeload && "deload" !in hiddenTiles) {
                    DeloadBanner(
                        sessionsSinceLast = state.sessionsSinceLastDeload,
                        onMarkDeloaded = viewModel::onMarkDeloaded
                    )
                }

                // "On this day" memory card (#106)
                state.onThisDayMemory?.let { memory ->
                    OnThisDayCard(memory = memory)
                }

                // Plan tomorrow quick action (#147)
                if (state.plannedNextDay.isNotEmpty()) {
                    val dayName = com.forge.app.program.Program.days.firstOrNull { it.key == state.plannedNextDay }?.defaultName ?: state.plannedNextDay
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text("Tomorrow: $dayName", style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.primary)
                        androidx.compose.material3.TextButton(onClick = viewModel::clearPlanNextDay) { Text("Clear") }
                    }
                }
                androidx.compose.material3.TextButton(
                    onClick = { showPlanTomorrowSheet = true },
                    modifier = Modifier.fillMaxWidth()
                ) { Text("Plan tomorrow →") }

                // Bodyweight quick-log button (#76)
                androidx.compose.material3.TextButton(
                    onClick = { showBodyweightSheet = true },
                    modifier = Modifier.fillMaxWidth()
                ) { Text("Log bodyweight →") }

                // Render nav tiles in user-defined order (#64), respecting visibility (#121)
                val visibleSecondary = tileOrder.filter { it != "gym" && it !in hiddenTiles }
                if ("gym" !in hiddenTiles) {
                    NavTile(
                        label = "Gym",
                        word = "TRAIN",
                        primary = true,
                        onClick = onGoToGym,
                        heightDp = 160,
                        modifier = Modifier.fillMaxWidth()
                    )
                }
                if (visibleSecondary.isNotEmpty()) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(12.dp)
                    ) {
                        visibleSecondary.forEach { tileId ->
                            when (tileId) {
                                "cardio" -> NavTile(label = "Cardio", word = "MOVE", primary = false,
                                    onClick = onGoToCardio, heightDp = 110, modifier = Modifier.weight(1f))
                                "trophies" -> NavTile(label = "Trophies", word = "EARN", primary = false,
                                    onClick = onGoToTrophies, heightDp = 110, modifier = Modifier.weight(1f))
                            }
                        }
                    }
                }
            }
        }
    }

    if (showBodyweightSheet) {
        BodyweightSheet(onDismiss = { showBodyweightSheet = false })
    }
    if (showPlanTomorrowSheet) {
        PlanTomorrowSheet(
            currentPlan = state.plannedNextDay,
            onPick = { dayKey -> viewModel.setPlanNextDay(dayKey); showPlanTomorrowSheet = false },
            onDismiss = { showPlanTomorrowSheet = false }
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
    val sheetState = androidx.compose.material3.rememberModalBottomSheetState()
    androidx.compose.material3.ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Text(
                "PLAN TOMORROW",
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary,
                fontWeight = androidx.compose.ui.text.font.FontWeight.Black
            )
            com.forge.app.program.Program.days.forEach { day ->
                val isSelected = currentPlan == day.key
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(androidx.compose.foundation.shape.RoundedCornerShape(10.dp))
                        .background(
                            if (isSelected) MaterialTheme.colorScheme.primary.copy(alpha = 0.15f)
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
                        fontWeight = if (isSelected) androidx.compose.ui.text.font.FontWeight.SemiBold
                                     else androidx.compose.ui.text.font.FontWeight.Normal,
                        color = if (isSelected) MaterialTheme.colorScheme.primary
                                else MaterialTheme.colorScheme.onSurface
                    )
                    if (isSelected) {
                        Text("✓", color = MaterialTheme.colorScheme.primary,
                            fontWeight = androidx.compose.ui.text.font.FontWeight.Bold)
                    }
                }
            }
            androidx.compose.foundation.layout.Spacer(Modifier.height(8.dp))
        }
    }
}
