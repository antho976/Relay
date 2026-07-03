package com.forge.app.ui.goals

import androidx.compose.foundation.clickable
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
import androidx.compose.material3.CircularProgressIndicator
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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.ui.common.FirstTouchTip

/** The in-progress "add a goal" / "edit a goal" flow. Null = no dialog open. */
private sealed interface GoalsFlow {
    data object ChooseType : GoalsFlow
    data object LiftPicker : GoalsFlow
    /** Set/edit a lift's target weight. [currentTargetLb] null when adding. */
    data class LiftWeight(val exerciseId: String, val name: String, val currentTargetLb: Double?) : GoalsFlow
    data class CustomNew(val metric: GoalMetric) : GoalsFlow
    data class CustomEdit(val goal: ExtendedGoalRepository.Progress) : GoalsFlow
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun GoalsScreen(
    onBack: () -> Unit,
    viewModel: GoalsViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    var flow by remember { mutableStateOf<GoalsFlow?>(null) }
    var query by remember { mutableStateOf("") }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    val totalGoals = state.liftGoals.size + state.customGoals.size
    val q = query.trim()
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
                title = { Text("Goals.", style = MaterialTheme.typography.headlineMedium) },
                navigationIcon = {
                    IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        when {
            state.loading -> Box(Modifier.fillMaxSize().padding(inner), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
            else -> Column(
                Modifier.fillMaxSize().padding(inner).verticalScroll(rememberScrollState()).padding(horizontal = 24.dp)
            ) {
                Spacer(Modifier.height(8.dp))
                Text(
                    "Set a target and track your way to it — a weight on a lift, a weekly cardio or workout " +
                        "target, or a bodyweight goal. Custom goals update themselves from what you log.",
                    style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
                )
                Spacer(Modifier.height(16.dp))

                if (totalGoals >= 4) {
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
                    FirstTouchTip(
                        "No goals yet.",
                        "Tap + Add a goal below to pick a target — a lift weight, a weekly cardio/workout " +
                            "target, or a bodyweight goal."
                    )
                } else if (lifts.isEmpty() && customs.isEmpty()) {
                    Text(
                        "No goals match “$q”.",
                        style = MaterialTheme.typography.bodyMedium, color = muted,
                        modifier = Modifier.padding(vertical = 24.dp)
                    )
                } else {
                    if (lifts.isNotEmpty()) {
                        SectionLabel("LIFT TARGETS", muted)
                        lifts.forEach { g ->
                            LiftGoalRow(g, onBg, muted, accent, outline) {
                                flow = GoalsFlow.LiftWeight(g.exerciseId, g.name, g.targetLb)
                            }
                            Spacer(Modifier.height(16.dp))
                        }
                    }
                    if (customs.isNotEmpty()) {
                        SectionLabel("OTHER GOALS", muted)
                        customs.forEach { g ->
                            CustomGoalRow(g, onBg, muted, accent, outline) {
                                flow = GoalsFlow.CustomEdit(g)
                            }
                            Spacer(Modifier.height(16.dp))
                        }
                    }
                }

                Spacer(Modifier.height(8.dp))
                Text(
                    "+ Add a goal",
                    style = MaterialTheme.typography.labelLarge,
                    color = accent,
                    modifier = Modifier.clickable { flow = GoalsFlow.ChooseType }.padding(vertical = 8.dp)
                )
                Spacer(Modifier.height(40.dp))
            }
        }
    }

    when (val f = flow) {
        null -> Unit
        GoalsFlow.ChooseType -> AddGoalTypeDialog(
            onPickLift = { flow = GoalsFlow.LiftPicker },
            onPickMetric = { flow = GoalsFlow.CustomNew(it) },
            onDismiss = { flow = null }
        )
        GoalsFlow.LiftPicker -> LiftTargetPickerDialog(
            exclude = state.liftPickerExclude,
            onPick = { id, name -> flow = GoalsFlow.LiftWeight(id, name, null) },
            onDismiss = { flow = null }
        )
        is GoalsFlow.LiftWeight -> GoalWeightDialog(
            exerciseName = f.name,
            currentTargetLb = f.currentTargetLb,
            onSet = { lb -> viewModel.setLiftGoal(f.exerciseId, lb); flow = null },
            onClear = { viewModel.clearLiftGoal(f.exerciseId); flow = null },
            onDismiss = { flow = null }
        )
        is GoalsFlow.CustomNew -> CustomGoalDialog(
            metric = f.metric,
            onConfirm = { period, target, label ->
                viewModel.createCustomGoal(f.metric, period, target, label); flow = null
            },
            onDismiss = { flow = null }
        )
        is GoalsFlow.CustomEdit -> CustomGoalEditDialog(
            goal = f.goal,
            onSave = { target -> viewModel.updateCustomGoalTarget(f.goal.id, target); flow = null },
            onDelete = { viewModel.deleteCustomGoal(f.goal.id); flow = null },
            onDismiss = { flow = null }
        )
    }
}

@Composable
private fun SectionLabel(text: String, muted: Color) {
    Text(
        text,
        style = MaterialTheme.typography.labelSmall,
        color = muted,
        fontSize = 11.sp,
        modifier = Modifier.padding(bottom = 12.dp)
    )
}
