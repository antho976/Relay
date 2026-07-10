@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
package com.forge.app.ui.gym.freestyle

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.units.parseToLb
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.ExerciseUnit
import com.forge.app.ui.common.ExerciseLibraryPicker
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeWordmark
import com.forge.app.ui.common.GlyphButton
import com.forge.app.ui.common.clickableLabeled

private data class FsSet(val weight: String = "", val reps: String = "")
private data class FsExercise(
    val libId: String,
    val name: String,
    val bodyweight: Boolean,
    val sets: List<FsSet> = listOf(FsSet())
)

/**
 * Dedicated freestyle ("go with the flow") logger: log what you did at the gym after the fact, with
 * no fixed plan. Add exercises from the full library, type weight × reps per set, and save — it
 * persists as a normal finished session (history/stats/PRs all pick it up).
 */
@Composable
fun FreestyleLogScreen(
    onBack: () -> Unit,
    viewModel: FreestyleLogViewModel = hiltViewModel()
) {
    val useKg by viewModel.useKg.collectAsStateWithLifecycle()
    val unitLabel = if (useKg) "kg" else "lb"
    var items by remember { mutableStateOf<List<FsExercise>>(emptyList()) }
    var showPicker by remember { mutableStateOf(false) }
    // When the logger was opened — becomes the saved session's start so its duration isn't ~0.
    val openedAtMs = remember { System.currentTimeMillis() }

    fun updateExercise(i: Int, transform: (FsExercise) -> FsExercise) {
        items = items.mapIndexed { idx, e -> if (idx == i) transform(e) else e }
    }

    val canSave = items.any { ex -> ex.sets.any { (it.reps.toIntOrNull() ?: 0) > 0 } }

    fun save() {
        val payload = items.mapNotNull { ex ->
            val sets = ex.sets.mapNotNull { s ->
                val reps = s.reps.toIntOrNull()?.takeIf { it > 0 } ?: return@mapNotNull null
                val weightLb = if (ex.bodyweight) null else parseToLb(s.weight, useKg)
                FreestyleSetInput(weightText = if (ex.bodyweight) "" else s.weight.trim(), weightLb = weightLb, reps = reps)
            }
            if (sets.isEmpty()) null else FreestyleExerciseInput(ex.libId, sets)
        }
        if (payload.isNotEmpty()) viewModel.save(payload, openedAtMs) { onBack() }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                // §2: wordmark + back, never the screen's name (live-flow screen, no hero needed).
                title = { ForgeWordmark() },
                navigationIcon = {
                    IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        bottomBar = {
            ForgePrimaryCapsule(
                "Save workout",
                onClick = { save() },
                enabled = canSave,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 16.dp)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(inner),
            contentPadding = PaddingValues(horizontal = 24.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(0.dp)
        ) {
            item {
                Text(
                    if (items.isEmpty()) "Add the machines and exercises you did, with your weights and reps."
                    else "Tap a field to enter weight ($unitLabel) and reps.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(Modifier.height(8.dp))
            }
            itemsIndexed(items, key = { _, ex -> ex.libId }) { i, ex ->
                FsExerciseCard(
                    exercise = ex,
                    unitLabel = unitLabel,
                    onRemove = { items = items.filterIndexed { idx, _ -> idx != i } },
                    onSetChange = { setIdx, set ->
                        updateExercise(i) { e -> e.copy(sets = e.sets.mapIndexed { si, s -> if (si == setIdx) set else s }) }
                    },
                    onAddSet = { updateExercise(i) { e -> e.copy(sets = e.sets + FsSet()) } },
                    onRemoveSet = { setIdx -> updateExercise(i) { e -> e.copy(sets = e.sets.filterIndexed { si, _ -> si != setIdx }) } }
                )
            }
            item {
                ForgeOutlineCapsule("+ Add exercise", onClick = { showPicker = true }, modifier = Modifier.fillMaxWidth())
            }
        }
    }

    if (showPicker) {
        ExerciseLibraryPicker(
            exclude = items.map { it.libId }.toSet(),
            onDismiss = { showPicker = false },
            onConfirm = { picked ->
                val added = picked.mapNotNull { id ->
                    val def = ExerciseLibrary.byId(id) ?: return@mapNotNull null
                    FsExercise(libId = def.id, name = def.name, bodyweight = def.unit == ExerciseUnit.BODYWEIGHT)
                }
                items = items + added
                showPicker = false
            }
        )
    }
}

@Composable
private fun FsExerciseCard(
    exercise: FsExercise,
    unitLabel: String,
    onRemove: () -> Unit,
    onSetChange: (Int, FsSet) -> Unit,
    onAddSet: () -> Unit,
    onRemoveSet: (Int) -> Unit
) {
    val cs = MaterialTheme.colorScheme
    // Air alone separates exercise blocks (§1: no section hairlines).
    Spacer(Modifier.height(12.dp))
    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(exercise.name, style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.SemiBold,
                color = cs.onSurface, modifier = Modifier.weight(1f))
            // Text glyph over a stock icon (§8); GlyphButton guarantees the ≥48dp touch target.
            GlyphButton("×", "Remove exercise", cs.onSurfaceVariant, onClick = onRemove)
        }
        exercise.sets.forEachIndexed { setIdx, set ->
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text("${setIdx + 1}", style = MaterialTheme.typography.labelMedium,
                    color = cs.onSurfaceVariant)
                if (!exercise.bodyweight) {
                    OutlinedTextField(
                        value = set.weight,
                        onValueChange = { onSetChange(setIdx, set.copy(weight = it)) },
                        label = { Text(unitLabel) },
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                        singleLine = true,
                        modifier = Modifier.weight(1f)
                    )
                }
                OutlinedTextField(
                    value = set.reps,
                    onValueChange = { onSetChange(setIdx, set.copy(reps = it.filter { c -> c.isDigit() })) },
                    label = { Text("reps") },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    singleLine = true,
                    modifier = Modifier.weight(1f)
                )
                // Text glyph over a stock icon (§8); inert + dimmed when it's the only set.
                GlyphButton(
                    "×", "Remove set", cs.onSurfaceVariant,
                    onClick = { onRemoveSet(setIdx) },
                    enabled = exercise.sets.size > 1
                )
            }
        }
        // §11 "+ log" idiom: an accent mono action line.
        Text(
            "+ add set",
            style = MaterialTheme.typography.labelLarge,
            color = cs.primary,
            modifier = Modifier.clickableLabeled("Add set", onClick = onAddSet).padding(vertical = 8.dp)
        )
        Spacer(Modifier.height(8.dp))
    }
}
