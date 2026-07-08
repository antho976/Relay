package com.forge.app.ui.goals

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgeWordmark
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.clickableLabeled

/**
 * The aggregated Goals list. Adding and editing happen on the routed [GoalEditorScreen] (a full
 * screen, not a dialog) — this list refreshes itself reactively when you come back from it.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun GoalsScreen(
    onBack: () -> Unit,
    onAddGoal: () -> Unit,
    onEditLift: (exerciseId: String) -> Unit,
    onEditCustom: (goalId: Long) -> Unit,
    viewModel: GoalsViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    var query by remember { mutableStateOf("") }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    val totalGoals = state.liftGoals.size + state.customGoals.size
    val searchVisible = totalGoals >= 4
    // Clear a stale filter if the goal count drops below the search threshold — otherwise the search
    // box hides while its query keeps filtering, leaving goals invisible with no way to clear it.
    LaunchedEffect(searchVisible) { if (!searchVisible) query = "" }
    val q = if (searchVisible) query.trim() else ""
    val lifts = remember(state.liftGoals, q) {
        state.liftGoals.filter { q.isBlank() || it.name.contains(q, ignoreCase = true) }
    }
    val customs = remember(state.customGoals, q) {
        state.customGoals.filter {
            q.isBlank() || it.label.contains(q, ignoreCase = true) ||
                metricDisplayName(it.metric).contains(q, ignoreCase = true)
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                // §2: the top bar never names the screen — wordmark + back; the serif hero below does.
                title = { ForgeWordmark() },
                navigationIcon = {
                    IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        when {
            // No spinners (§13): local DB is instant, the list simply appears when state lands.
            state.loading -> Box(Modifier.fillMaxSize().padding(inner))
            else -> Column(
                Modifier.fillMaxSize().padding(inner).verticalScroll(rememberScrollState()).padding(horizontal = 24.dp)
            ) {
                Spacer(Modifier.height(4.dp))
                Text("Goals", style = MaterialTheme.typography.headlineSmall, color = onBg)
                Spacer(Modifier.height(8.dp))
                Text(
                    "A weight on a lift, a weekly cardio or workout target, or a bodyweight goal. " +
                        "Custom goals track themselves from what you log.",
                    style = MaterialTheme.typography.bodySmall, color = muted,
                    fontSize = 11.sp, lineHeight = 15.sp
                )
                Spacer(Modifier.height(16.dp))

                if (searchVisible) {
                    OutlinedTextField(
                        value = query,
                        onValueChange = { query = it },
                        modifier = Modifier.fillMaxWidth(),
                        placeholder = { Text("Search goals") },
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Text)
                    )
                    Spacer(Modifier.height(16.dp))
                }

                if (totalGoals == 0) {
                    InlineEmptyHint("No goals yet. Add one below.", muted)
                } else if (lifts.isEmpty() && customs.isEmpty()) {
                    Text(
                        "No goals match “$q”.",
                        style = MaterialTheme.typography.bodyMedium, color = muted,
                        modifier = Modifier.padding(vertical = 24.dp)
                    )
                } else {
                    if (lifts.isNotEmpty()) {
                        EditorialHeader("Lift targets", muted, accent, Modifier.padding(bottom = 12.dp))
                        lifts.forEachIndexed { i, g ->
                            LiftGoalRow(g, i, onBg, muted, accent, outline) { onEditLift(g.exerciseId) }
                            Spacer(Modifier.height(18.dp))
                        }
                    }
                    if (customs.isNotEmpty()) {
                        EditorialHeader("Other goals", muted, accent, Modifier.padding(bottom = 12.dp))
                        customs.forEachIndexed { i, g ->
                            // Stagger continues across the two sections so the whole list cascades once.
                            CustomGoalRow(g, lifts.size + i, onBg, muted, accent, outline) { onEditCustom(g.id) }
                            Spacer(Modifier.height(18.dp))
                        }
                    }
                }

                Spacer(Modifier.height(8.dp))
                // §11 "+ log" idiom: an accent mono action line, bounce over ripple.
                Text(
                    "+ add goal",
                    style = MaterialTheme.typography.labelLarge,
                    color = accent,
                    modifier = Modifier.clickableLabeled("Add a goal") { onAddGoal() }.padding(vertical = 12.dp)
                )
                Spacer(Modifier.height(40.dp))
            }
        }
    }
}
