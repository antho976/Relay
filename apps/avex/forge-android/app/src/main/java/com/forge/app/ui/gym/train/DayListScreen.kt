package com.forge.app.ui.gym.train

import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.selectable
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
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
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
    // Null when this is a hub pager page (the bottom bar + system-back handle navigation) so no
    // redundant back arrow shows; a real callback when pushed as a deep route (e.g. PRs).
    onBack: (() -> Unit)? = null,
    onOpenDay: (String) -> Unit,
    onOpenDayQuick: (String) -> Unit,
    onOpenHistory: () -> Unit = {},
    onOpenNotes: () -> Unit = {},
    onOpenRecap: () -> Unit = {},
    onEditProgram: (String) -> Unit = {},
    onOpenCardio: () -> Unit = {},
    onLogFreestyle: () -> Unit = {},
    onBuildPlan: () -> Unit = {},
    /** Stats → consistency-heatmap day sheet → a gym session's detail screen. */
    onOpenSession: (Long) -> Unit = {},
    /** Stats → consistency-heatmap day sheet → a cardio session's detail screen. */
    onOpenCardioSession: (Long) -> Unit = {},
    initialTab: Int = 0,
    /** Top-bar heading — "Stats" / "PRs" when hosting those sub-screens, "GYM" by default. */
    title: String = "GYM",
    viewModel: DayListViewModel = hiltViewModel()
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(title, style = MaterialTheme.typography.headlineLarge) },
                navigationIcon = {
                    if (onBack != null) IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        Box(Modifier.fillMaxSize().padding(inner)) {
            // No Train/Stats tab row — each is its own screen reached from Overview
            // (Gym tile → Train, Stats tile → Stats). Content is chosen by the route.
            when (initialTab) {
                // onOpenHistory/onOpenNotes/onOpenRecap stay wired into DayListScreen (and HubScreen)
                // so the history/notes/recap routes keep an entry point, but the rebuilt Stats screen
                // no longer surfaces them, so they aren't forwarded here.
                1 -> StatsContent(
                    modifier = Modifier.fillMaxSize(),
                    onOpenSession = onOpenSession,
                    onOpenCardioSession = onOpenCardioSession
                )
                else -> TrainTab(
                    onOpenDay = onOpenDay,
                    onOpenDayQuick = onOpenDayQuick,
                    onEditProgram = onEditProgram,
                    onOpenCardio = onOpenCardio,
                    onLogFreestyle = onLogFreestyle,
                    onBuildPlan = onBuildPlan,
                    viewModel = viewModel
                )
            }
        }
    }
}

@Composable
private fun TrainTab(
    onOpenDay: (String) -> Unit,
    onOpenDayQuick: (String) -> Unit,
    onEditProgram: (String) -> Unit = {},
    onOpenCardio: () -> Unit = {},
    onLogFreestyle: () -> Unit = {},
    onBuildPlan: () -> Unit = {},
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
        if (state.days.isEmpty()) {
            if (state.freestyleMode) {
                // Freestyle: no plan by design — lead with logging, not a "build a plan" push.
                Text(
                    "No fixed plan.",
                    style = MaterialTheme.typography.headlineMedium,
                    color = MaterialTheme.colorScheme.onSurface
                )
                Text(
                    "Just log what you did, whenever you train. The Log a workout button below adds a session.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            } else {
                // Custom user who hasn't built their plan yet — a plan mode, not the logger.
                Text(
                    "No plan yet.",
                    style = MaterialTheme.typography.headlineMedium,
                    color = MaterialTheme.colorScheme.onSurface
                )
                Text(
                    "Build your own plan day by day, then train from it.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Button(onClick = onBuildPlan, modifier = Modifier.fillMaxWidth()) { Text("Build a plan") }
            }
        } else {
            Text(
                "Pick your day.",
                style = MaterialTheme.typography.headlineMedium,
                color = MaterialTheme.colorScheme.onSurface
            )
            Text(
                "Hold a day for options. Use Edit plan to add, rename, reorder or remove days.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            state.days.forEach { item ->
                DayCard(
                    item = item,
                    onClick = {
                        if (item.plan.key.startsWith("cardio")) {
                            onOpenCardio()
                        } else {
                            val active = state.activeSession
                            if (active != null && active.dayKey != item.plan.key) {
                                pendingOpenDayKey = item.plan.key
                            } else {
                                onOpenDay(item.plan.key)
                            }
                        }
                    },
                    onQuickStart = if (item.isNextUp && !item.isActive && !item.plan.key.startsWith("cardio")) {
                        { onOpenDayQuick(item.plan.key) }
                    } else null,
                    onLongPress = { longPressMenuForDayKey = item.plan.key }
                )
            }
            OutlinedButton(onClick = onBuildPlan, modifier = Modifier.fillMaxWidth()) { Text("Edit plan") }
        }
        // Freestyle logging belongs to the "go with the flow" user only. Plan modes (generated / custom)
        // train from their days, so a no-plan logger here would just muddy the screen.
        if (state.freestyleMode) {
            OutlinedButton(onClick = onLogFreestyle, modifier = Modifier.fillMaxWidth()) {
                Text("Log a workout")
            }
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
                    if (!dayKey.startsWith("cardio")) {
                        TextButton(modifier = Modifier.fillMaxWidth(), onClick = {
                            longPressMenuForDayKey = null
                            viewModel.rerollDay(dayKey)
                        }) { Text("Re-roll this day's exercises") }
                    }
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
                modifier = androidx.compose.ui.Modifier.selectableGroup(),
                horizontalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(4.dp),
                verticalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(4.dp)
            ) {
                PRESET_COLORS.forEach { (hex, label) ->
                    val color = androidx.compose.ui.graphics.Color(android.graphics.Color.parseColor(hex))
                    val selected = currentHex == hex
                    // The swatch is a labelled, selectable control, not a coloured square.
                    //
                    // The names were already in PRESET_COLORS and never reached anyone: a bare
                    // `clickable` Box announces "button" with no label, so TalkBack read nine
                    // identical unnamed buttons and the ring marking the current one is a purely
                    // visual cue. `selectableGroup` + `Role.RadioButton` + `selected` says what this
                    // set of controls IS — one choice among nine — and the swatch is now a 48 dp
                    // target with a 40 dp paint, meeting the minimum without changing the design.
                    androidx.compose.foundation.layout.Box(
                        modifier = androidx.compose.ui.Modifier
                            .size(48.dp)
                            .semantics { contentDescription = label }
                            .selectable(
                                selected = selected,
                                role = androidx.compose.ui.semantics.Role.RadioButton,
                                onClick = { onPick(hex) }
                            ),
                        contentAlignment = androidx.compose.ui.Alignment.Center
                    ) {
                        androidx.compose.foundation.layout.Box(
                            modifier = androidx.compose.ui.Modifier
                                .size(40.dp)
                                .clip(androidx.compose.foundation.shape.CircleShape)
                                .background(color)
                                .let { m ->
                                    if (selected) m.border(3.dp, androidx.compose.material3.MaterialTheme.colorScheme.onSurface,
                                        androidx.compose.foundation.shape.CircleShape) else m
                                }
                        )
                    }
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
