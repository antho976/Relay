package com.forge.app.ui.programeditor

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.ProgramCustomizationRepository
import com.forge.app.program.MuscleGroup
import com.forge.app.program.Program
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

data class ProgramEditorState(
    val dayKey: String = "",
    val dayName: String = "",
    val exercises: List<ProgramExerciseItem> = emptyList()
)

data class ProgramExerciseItem(
    val id: String,
    val name: String,
    val reps: String,
    val sets: Int,
    val muscle: String,
    val isCustom: Boolean,
    val removed: Boolean
)

@HiltViewModel
class ProgramEditorViewModel @Inject constructor(
    private val repo: ProgramCustomizationRepository
) : ViewModel() {

    private val _dayKey = MutableStateFlow("")

    val state: StateFlow<ProgramEditorState> = _dayKey.flatMapLatest { dayKey ->
        if (dayKey.isEmpty()) return@flatMapLatest flow { emit(ProgramEditorState()) }
        repo.observeForDay(dayKey).flatMapLatest { _ ->
            flow {
                val day = Program.days.firstOrNull { it.key == dayKey }
                    ?: return@flow emit(ProgramEditorState())
                val effective = runCatching { repo.effectivePlanForDay(dayKey) }
                    .getOrDefault(day.exercises)
                emit(ProgramEditorState(
                    dayKey = dayKey,
                    dayName = day.defaultName,
                    exercises = effective.map { plan ->
                        ProgramExerciseItem(
                            id = plan.id,
                            name = plan.name,
                            reps = plan.reps,
                            sets = plan.sets,
                            muscle = plan.muscle.displayName,
                            isCustom = plan.id.startsWith("custom_"),
                            removed = false
                        )
                    }
                ))
            }
        }
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), ProgramEditorState())

    fun setDay(dayKey: String) { _dayKey.value = dayKey }

    fun setRepRange(exerciseId: String, reps: String) = viewModelScope.launch {
        repo.setRepRange(_dayKey.value, exerciseId, reps)
    }

    fun setSets(exerciseId: String, sets: Int) = viewModelScope.launch {
        repo.setSetsOverride(_dayKey.value, exerciseId, sets)
    }

    fun removeExercise(exerciseId: String) = viewModelScope.launch {
        repo.removeExercise(_dayKey.value, exerciseId)
    }

    fun addCustom(name: String, muscle: MuscleGroup) = viewModelScope.launch {
        repo.addCustomExercise(_dayKey.value, name, muscle)
    }

    fun resetDay() = viewModelScope.launch { repo.resetDay(_dayKey.value) }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ProgramEditorScreen(
    dayKey: String,
    onBack: () -> Unit,
    viewModel: ProgramEditorViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsState()

    // Set the day key once
    remember(dayKey) { viewModel.setDay(dayKey); Unit }

    var showAddDialog by remember { mutableStateOf(false) }
    var editRepsFor by remember { mutableStateOf<ProgramExerciseItem?>(null) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("EDIT PROGRAM · ${state.dayName}") },
                navigationIcon = {
                    IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, null) }
                },
                actions = {
                    TextButton(onClick = viewModel::resetDay) { Text("Reset") }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = MaterialTheme.colorScheme.background)
            )
        },
        floatingActionButton = {
            androidx.compose.material3.FloatingActionButton(onClick = { showAddDialog = true }) {
                Icon(Icons.Default.Add, "Add exercise")
            }
        },
        containerColor = MaterialTheme.colorScheme.background
    ) { inner ->
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(inner),
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            item {
                Text("Tap reps to edit. Long-press to delete.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            items(state.exercises, key = { it.id }) { ex ->
                ExerciseEditorRow(
                    item = ex,
                    onEditReps = { editRepsFor = ex },
                    onRemove = { viewModel.removeExercise(ex.id) }
                )
            }
        }
    }

    editRepsFor?.let { ex ->
        RepRangeDialog(
            current = ex.reps,
            currentSets = ex.sets,
            exerciseName = ex.name,
            onDismiss = { editRepsFor = null },
            onSave = { reps, sets ->
                viewModel.setRepRange(ex.id, reps)
                viewModel.setSets(ex.id, sets)
                editRepsFor = null
            }
        )
    }

    if (showAddDialog) {
        AddExerciseDialog(
            onDismiss = { showAddDialog = false },
            onAdd = { name, muscle ->
                viewModel.addCustom(name, muscle)
                showAddDialog = false
            }
        )
    }
}

@Composable
private fun ExerciseEditorRow(
    item: ProgramExerciseItem,
    onEditReps: () -> Unit,
    onRemove: () -> Unit
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(10.dp))
            .padding(12.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(item.name, style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.SemiBold)
            Text("${item.muscle} · ${item.sets} sets · ${item.reps}",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        TextButton(onClick = onEditReps) { Text("Edit") }
        IconButton(onClick = onRemove) {
            Icon(Icons.Default.Delete, null, tint = MaterialTheme.colorScheme.error.copy(alpha = 0.7f))
        }
    }
}

@Composable
private fun RepRangeDialog(
    current: String,
    currentSets: Int,
    exerciseName: String,
    onDismiss: () -> Unit,
    onSave: (String, Int) -> Unit
) {
    var reps by remember { mutableStateOf(current) }
    var sets by remember { mutableStateOf(currentSets.toString()) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(exerciseName) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(value = reps, onValueChange = { reps = it },
                    label = { Text("Rep range (e.g. 6-8)") }, singleLine = true)
                OutlinedTextField(value = sets, onValueChange = { if (it.all { c -> c.isDigit() }) sets = it },
                    label = { Text("Sets") }, singleLine = true)
            }
        },
        confirmButton = {
            TextButton(onClick = { onSave(reps.trim(), sets.toIntOrNull() ?: currentSets) }) { Text("Save") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } }
    )
}

@Composable
private fun AddExerciseDialog(onDismiss: () -> Unit, onAdd: (String, MuscleGroup) -> Unit) {
    var name by remember { mutableStateOf("") }
    var muscle by remember { mutableStateOf(MuscleGroup.CHEST) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Add exercise") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(value = name, onValueChange = { name = it },
                    label = { Text("Exercise name") }, singleLine = true)
                @OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
                androidx.compose.foundation.layout.FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(4.dp)
                ) {
                    MuscleGroup.entries.forEach { mg ->
                        androidx.compose.material3.FilterChip(
                            selected = muscle == mg,
                            onClick = { muscle = mg },
                            label = { Text(mg.displayName, style = MaterialTheme.typography.labelSmall) }
                        )
                    }
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = { if (name.isNotBlank()) onAdd(name.trim(), muscle) },
                enabled = name.isNotBlank()
            ) { Text("Add") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } }
    )
}

@Composable
private fun <T> StateFlow<T>.collectAsState() = collectAsStateWithLifecycle()
