package com.forge.app.ui.gym.train

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.gym.stats.PrsContent
import com.forge.app.ui.gym.stats.StatsContent
import com.forge.app.ui.gym.train.components.DayCard

/**
 * Gym hub. Hosts two tabs: **Train** (the day list, Phase 3) and **Stats** (Phase 5).
 * Tab selection is preserved via rememberSaveable so backing out and returning keeps you
 * on whichever subtab you last had open.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DayListScreen(
    onBack: () -> Unit,
    onOpenDay: (String) -> Unit,
    onOpenDayQuick: (String) -> Unit,
    onOpenHistory: () -> Unit = {},
    onOpenNotes: () -> Unit = {},
    onOpenRecap: () -> Unit = {},
    onEditProgram: (String) -> Unit = {},
    viewModel: DayListViewModel = hiltViewModel()
) {
    var selectedTab by rememberSaveable { mutableIntStateOf(0) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("GYM", style = MaterialTheme.typography.headlineLarge) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background,
                    titleContentColor = MaterialTheme.colorScheme.onBackground
                )
            )
        },
        containerColor = MaterialTheme.colorScheme.background
    ) { inner ->
        Column(Modifier.fillMaxSize().padding(inner)) {
            TabRow(
                selectedTabIndex = selectedTab,
                containerColor = MaterialTheme.colorScheme.background,
                contentColor = MaterialTheme.colorScheme.primary
            ) {
                Tab(
                    selected = selectedTab == 0,
                    onClick = { selectedTab = 0 },
                    text = { Text("Train") }
                )
                Tab(
                    selected = selectedTab == 1,
                    onClick = { selectedTab = 1 },
                    text = { Text("Stats") }
                )
                Tab(
                    selected = selectedTab == 2,
                    onClick = { selectedTab = 2 },
                    text = { Text("PRs") }
                )
            }

            AnimatedContent(
                targetState = selectedTab,
                transitionSpec = {
                    val dir = if (targetState > initialState) 1 else -1
                    slideInHorizontally { it * dir } togetherWith slideOutHorizontally { -it * dir }
                },
                modifier = Modifier.fillMaxSize(),
                label = "tab_transition"
            ) { tab ->
                when (tab) {
                    0 -> TrainTab(
                        onOpenDay = onOpenDay,
                        onOpenDayQuick = onOpenDayQuick,
                        onEditProgram = onEditProgram,
                        viewModel = viewModel
                    )
                    1 -> StatsContent(modifier = Modifier.fillMaxSize(), onOpenHistory = onOpenHistory, onOpenNotes = onOpenNotes, onOpenRecap = onOpenRecap)
                    2 -> PrsContent(modifier = Modifier.fillMaxSize())
                    else -> Unit
                }
            }
        }
    }
}

@Composable
private fun TrainTab(
    onOpenDay: (String) -> Unit,
    onOpenDayQuick: (String) -> Unit,
    onEditProgram: (String) -> Unit = {},
    viewModel: DayListViewModel
) {
    val state by viewModel.state.collectAsStateWithLifecycle()

    // Different-day warning state (#47)
    var pendingOpenDayKey by remember { mutableStateOf<String?>(null) }
    // Color picker state (#65)
    var colorPickerForDayKey by remember { mutableStateOf<String?>(null) }
    // Long-press action menu
    var longPressMenuForDayKey by remember { mutableStateOf<String?>(null) }

    Column(
        Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text(
            "Pick your day.",
            style = MaterialTheme.typography.headlineMedium,
            color = MaterialTheme.colorScheme.onSurface
        )
        state.days.forEach { item ->
            DayCard(
                item = item,
                onClick = {
                    val active = state.activeSession
                    if (active != null && active.dayKey != item.plan.key) {
                        pendingOpenDayKey = item.plan.key
                    } else {
                        onOpenDay(item.plan.key)
                    }
                },
                onQuickStart = if (item.isNextUp && !item.isActive) {
                    { onOpenDayQuick(item.plan.key) }
                } else null,
                onLongPress = { longPressMenuForDayKey = item.plan.key }
            )
        }
    }

    // Long-press action menu: change color or edit program
    longPressMenuForDayKey?.let { dayKey ->
        val item = state.days.firstOrNull { it.plan.key == dayKey }
        AlertDialog(
            onDismissRequest = { longPressMenuForDayKey = null },
            title = { Text(item?.displayName ?: dayKey) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    TextButton(modifier = Modifier.fillMaxWidth(), onClick = {
                        longPressMenuForDayKey = null
                        colorPickerForDayKey = dayKey
                    }) { Text("Change day color") }
                    TextButton(modifier = Modifier.fillMaxWidth(), onClick = {
                        longPressMenuForDayKey = null
                        onEditProgram(dayKey)
                    }) { Text("Edit program for this day") }
                }
            },
            confirmButton = {
                TextButton(onClick = { longPressMenuForDayKey = null }) { Text("Cancel") }
            }
        )
    }

    // Color picker dialog (#65)
    colorPickerForDayKey?.let { dayKey ->
        val item = state.days.firstOrNull { it.plan.key == dayKey }
        DayColorPickerDialog(
            dayName = item?.displayName ?: dayKey,
            currentHex = item?.customAccentHex,
            onPick = { hex -> viewModel.setDayColor(dayKey, hex); colorPickerForDayKey = null },
            onReset = { viewModel.setDayColor(dayKey, null); colorPickerForDayKey = null },
            onDismiss = { colorPickerForDayKey = null }
        )
    }

    // Different-day warning dialog (#47)
    pendingOpenDayKey?.let { pendingKey ->
        val activeDayName = state.days
            .firstOrNull { it.plan.key == state.activeSession?.dayKey }
            ?.displayName ?: "another day"
        AlertDialog(
            onDismissRequest = { pendingOpenDayKey = null },
            title = { Text("Session in progress") },
            text = {
                Text("You have an active session on $activeDayName. Open a different day anyway?")
            },
            confirmButton = {
                Button(onClick = {
                    pendingOpenDayKey = null
                    onOpenDay(pendingKey)
                }) { Text("Open anyway") }
            },
            dismissButton = {
                TextButton(onClick = { pendingOpenDayKey = null }) { Text("Keep going") }
            }
        )
    }
}

private val PRESET_COLORS = listOf(
    "#E85D4A" to "Red",   "#F97316" to "Orange", "#EAB308" to "Yellow",
    "#22C55E" to "Green", "#3B82F6" to "Blue",   "#8B5CF6" to "Purple",
    "#EC4899" to "Pink",  "#14B8A6" to "Teal",   "#EF4444" to "Crimson"
)

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun DayColorPickerDialog(
    dayName: String,
    currentHex: String?,
    onPick: (String) -> Unit,
    onReset: () -> Unit,
    onDismiss: () -> Unit
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Accent color — $dayName") },
        text = {
            androidx.compose.foundation.layout.FlowRow(
                horizontalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(8.dp),
                verticalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(8.dp)
            ) {
                PRESET_COLORS.forEach { (hex, label) ->
                    val color = androidx.compose.ui.graphics.Color(android.graphics.Color.parseColor(hex))
                    androidx.compose.foundation.layout.Box(
                        modifier = androidx.compose.ui.Modifier
                            .size(40.dp)
                            .clip(androidx.compose.foundation.shape.CircleShape)
                            .background(color)
                            .let { m ->
                                if (currentHex == hex) m.border(3.dp, androidx.compose.material3.MaterialTheme.colorScheme.onSurface,
                                    androidx.compose.foundation.shape.CircleShape) else m
                            }
                            .clickable { onPick(hex) }
                    )
                }
            }
        },
        confirmButton = {},
        dismissButton = {
            if (currentHex != null) {
                TextButton(onClick = onReset) { Text("Reset to default") }
            } else {
                TextButton(onClick = onDismiss) { Text("Cancel") }
            }
        }
    )
}
