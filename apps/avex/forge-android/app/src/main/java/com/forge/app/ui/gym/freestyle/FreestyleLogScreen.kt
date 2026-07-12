@file:OptIn(
    androidx.compose.material3.ExperimentalMaterial3Api::class,
    androidx.compose.foundation.layout.ExperimentalLayoutApi::class
)
package com.forge.app.ui.gym.freestyle

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.parseToLb
import com.forge.app.domain.units.weightInputValue
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeWordmark
import com.forge.app.ui.common.GlyphButton
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.gym.stats.components.MuscleFigure
import com.forge.app.ui.theme.ForgeMotion
import kotlinx.coroutines.delay
import java.util.Locale

private data class FsSet(val weight: String = "", val reps: String = "")
private data class FsExercise(
    val libId: String,
    val name: String,
    val muscle: MuscleGroup,
    val bodyweight: Boolean,
    val sets: List<FsSet> = listOf(FsSet())
)

/** Volume of one set in lb (weight × reps); bodyweight moves contribute no external load. */
private fun FsExercise.setVolumeLb(set: FsSet, useKg: Boolean): Double {
    if (bodyweight) return 0.0
    val reps = set.reps.toIntOrNull() ?: return 0.0
    val w = parseToLb(set.weight, useKg) ?: return 0.0
    return w * reps
}

/** Nudge a display-unit weight string by [delta] (kept ≥0, '.'-decimal so it re-parses in any locale). */
private fun stepWeightStr(cur: String, delta: Double): String {
    val next = ((cur.toDoubleOrNull() ?: 0.0) + delta).coerceAtLeast(0.0)
    return if (next % 1.0 == 0.0) next.toInt().toString() else String.format(Locale.US, "%.1f", next)
}

private fun stepRepsStr(cur: String, delta: Int): String =
    ((cur.toIntOrNull() ?: 0) + delta).coerceAtLeast(0).toString()

/** mm:ss, or h:mm:ss past an hour — the running session clock. */
private fun formatElapsed(ms: Long): String {
    val s = ms / 1000
    val h = s / 3600; val m = (s % 3600) / 60; val sec = s % 60
    return if (h > 0) "%d:%02d:%02d".format(h, m, sec) else "%d:%02d".format(m, sec)
}

/**
 * Dedicated freestyle ("go with the flow") logger: log what you did at the gym after the fact, with
 * no fixed plan. Add exercises from the browser, type or step weight × reps per set, and save — it
 * persists as a normal finished session (history/stats/PRs all pick it up). Live/flow archetype: serif
 * number fields on underlines (not boxed inputs), +/- steppers, a running clock, each move's
 * target-muscle thumbnail, and a "copy last time" panel seeded from your last performance.
 */
@Composable
fun FreestyleLogScreen(
    onBack: () -> Unit,
    viewModel: FreestyleLogViewModel = hiltViewModel()
) {
    val useKg by viewModel.useKg.collectAsStateWithLifecycle()
    val unitLabel = if (useKg) "kg" else "lb"
    var items by remember { mutableStateOf<List<FsExercise>>(emptyList()) }
    var showBrowser by remember { mutableStateOf(false) }
    // When the logger was opened — becomes the saved session's start so its duration isn't ~0.
    val openedAtMs = remember { System.currentTimeMillis() }
    val elapsedMs by produceState(0L, openedAtMs) {
        while (true) { value = System.currentTimeMillis() - openedAtMs; delay(1000) }
    }

    fun updateExercise(i: Int, transform: (FsExercise) -> FsExercise) {
        items = items.mapIndexed { idx, e -> if (idx == i) transform(e) else e }
    }

    val totalVolumeLb = items.sumOf { ex -> ex.sets.sumOf { ex.setVolumeLb(it, useKg) } }
    val loggedSets = items.sumOf { ex -> ex.sets.count { (it.reps.toIntOrNull() ?: 0) > 0 } }
    val canSave = loggedSets > 0

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

    Box(Modifier.fillMaxSize()) {
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
                    FsSessionHeader(
                        elapsedMs = elapsedMs,
                        exerciseCount = items.size,
                        setCount = loggedSets,
                        totalVolumeLb = totalVolumeLb,
                        useKg = useKg,
                        empty = items.isEmpty()
                    )
                }
                itemsIndexed(items, key = { _, ex -> ex.libId }) { i, ex ->
                    FsExerciseCard(
                        exercise = ex,
                        unitLabel = unitLabel,
                        useKg = useKg,
                        lastSetsProvider = { id -> viewModel.lastSets(id) },
                        onRemove = { items = items.filterIndexed { idx, _ -> idx != i } },
                        onSetChange = { setIdx, set ->
                            updateExercise(i) { e -> e.copy(sets = e.sets.mapIndexed { si, s -> if (si == setIdx) set else s }) }
                        },
                        onAddSet = { updateExercise(i) { e -> e.copy(sets = e.sets + FsSet()) } },
                        onRemoveSet = { setIdx -> updateExercise(i) { e -> e.copy(sets = e.sets.filterIndexed { si, _ -> si != setIdx }) } },
                        onReplaceSets = { newSets -> updateExercise(i) { e -> e.copy(sets = newSets) } }
                    )
                }
                item {
                    Spacer(Modifier.height(16.dp))
                    ForgeOutlineCapsule("+ Add exercise", onClick = { showBrowser = true }, modifier = Modifier.fillMaxWidth())
                    Spacer(Modifier.height(8.dp))
                }
            }
        }

        if (showBrowser) {
            ExerciseBrowserScreen(
                exclude = items.map { it.libId }.toSet(),
                onClose = { showBrowser = false },
                onConfirm = { picked ->
                    val added = picked.mapNotNull { id ->
                        val def = ExerciseLibrary.byId(id) ?: return@mapNotNull null
                        FsExercise(libId = def.id, name = def.name, muscle = def.muscle, bodyweight = def.unit == ExerciseUnit.BODYWEIGHT)
                    }
                    items = items + added
                    showBrowser = false
                }
            )
        }
    }
}

/** Running clock eyebrow + a live at-a-glance line (exercises · sets · total volume), or the first-run cue. */
@Composable
private fun FsSessionHeader(
    elapsedMs: Long,
    exerciseCount: Int,
    setCount: Int,
    totalVolumeLb: Double,
    useKg: Boolean,
    empty: Boolean
) {
    val cs = MaterialTheme.colorScheme
    Column {
        Text(
            "FREESTYLE · ${formatElapsed(elapsedMs)}",
            style = MaterialTheme.typography.labelMedium,
            color = cs.onSurfaceVariant,
            letterSpacing = 1.sp
        )
        Spacer(Modifier.height(4.dp))
        if (empty) {
            Text(
                "Log what you did — add the machines and exercises, with your weights and reps.",
                style = MaterialTheme.typography.bodySmall,
                color = cs.onSurfaceVariant
            )
        } else {
            val vol = if (totalVolumeLb > 0) " · ${formatWeight(totalVolumeLb, useKg)} volume" else ""
            Text(
                "$exerciseCount exercise${if (exerciseCount == 1) "" else "s"} · $setCount set${if (setCount == 1) "" else "s"}$vol",
                style = MaterialTheme.typography.bodyMedium,
                color = cs.onSurface
            )
        }
        Spacer(Modifier.height(8.dp))
    }
}

@Composable
private fun FsExerciseCard(
    exercise: FsExercise,
    unitLabel: String,
    useKg: Boolean,
    lastSetsProvider: suspend (String) -> List<LoggedSet>,
    onRemove: () -> Unit,
    onSetChange: (Int, FsSet) -> Unit,
    onAddSet: () -> Unit,
    onRemoveSet: (Int) -> Unit,
    onReplaceSets: (List<FsSet>) -> Unit
) {
    val cs = MaterialTheme.colorScheme
    val setsDone = exercise.sets.count { (it.reps.toIntOrNull() ?: 0) > 0 }
    val volumeLb = exercise.sets.sumOf { exercise.setVolumeLb(it, useKg) }
    val weightStep = if (useKg) 2.5 else 5.0

    // Last performance for this move — loaded once, powers the "copy last time" panel.
    var lastSets by remember(exercise.libId) { mutableStateOf<List<LoggedSet>>(emptyList()) }
    var showLast by remember(exercise.libId) { mutableStateOf(false) }
    var expanded by remember(exercise.libId) { mutableStateOf(true) }
    LaunchedEffect(exercise.libId) { lastSets = lastSetsProvider(exercise.libId) }

    // Air alone separates exercise blocks (§1: no section hairlines).
    Spacer(Modifier.height(20.dp))
    Column(modifier = Modifier.fillMaxWidth()) {
        // Header: target-muscle thumbnail + name + a mono meta line that stays visible when collapsed —
        // so a long workout can be tidied by folding done exercises down to a one-line summary.
        Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            MuscleFigure(
                muscle = exercise.muscle,
                lit = lerp(cs.primary, cs.onSurface, 0.32f),
                body = cs.onSurfaceVariant.copy(alpha = 0.14f),
                detail = cs.onSurfaceVariant.copy(alpha = 0.3f),
                modifier = Modifier.width(40.dp).height(44.dp)
            )
            Spacer(Modifier.width(12.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(exercise.name, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold, color = cs.onSurface)
                val vol = if (volumeLb > 0) " · ${formatWeight(volumeLb, useKg)}" else ""
                Text(
                    "${exercise.muscle.displayName.uppercase()} · $setsDone SET${if (setsDone == 1) "" else "S"}$vol",
                    style = MaterialTheme.typography.labelMedium,
                    color = cs.onSurfaceVariant
                )
            }
            GlyphButton(if (expanded) "▾" else "▸", if (expanded) "Collapse" else "Expand", cs.onSurfaceVariant, onClick = { expanded = !expanded })
            GlyphButton("×", "Remove exercise", cs.onSurfaceVariant, onClick = onRemove)
        }

      AnimatedVisibility(
          visible = expanded,
          enter = expandVertically(ForgeMotion.enterTween()) + fadeIn(ForgeMotion.enterTween()),
          exit = shrinkVertically(ForgeMotion.exitTween()) + fadeOut(ForgeMotion.exitTween())
      ) {
        Column(modifier = Modifier.fillMaxWidth()) {
        // Copy-last-time — only when there's a prior performance to copy.
        if (lastSets.isNotEmpty()) {
            Spacer(Modifier.height(8.dp))
            Text(
                "copy last time",
                style = MaterialTheme.typography.labelMedium,
                color = cs.primary,
                modifier = Modifier.clickableLabeled("Show last time you did this") { showLast = !showLast }
            )
            AnimatedVisibility(
                visible = showLast,
                enter = expandVertically(ForgeMotion.enterTween()) + fadeIn(ForgeMotion.enterTween()),
                exit = shrinkVertically(ForgeMotion.exitTween()) + fadeOut(ForgeMotion.exitTween())
            ) {
                LastTimePanel(
                    sets = lastSets,
                    useKg = useKg,
                    onCopy = {
                        onReplaceSets(
                            lastSets.map { s ->
                                FsSet(
                                    weight = s.weightLb?.let { lb -> weightInputValue(lb, useKg) } ?: "",
                                    reps = s.reps.toString()
                                )
                            }
                        )
                        showLast = false
                    }
                )
            }
        }

        Spacer(Modifier.height(10.dp))
        // Column labels (mono micro-caps), aligned to the set rows below.
        Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.Bottom) {
            Text("SET", style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant, fontSize = 9.sp, modifier = Modifier.width(24.dp))
            Text(
                if (exercise.bodyweight) "BODYWEIGHT" else "WEIGHT · ${unitLabel.uppercase()}",
                style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant, fontSize = 9.sp,
                modifier = Modifier.weight(1f).padding(start = if (exercise.bodyweight) 0.dp else 36.dp)
            )
            Text("REPS", style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant, fontSize = 9.sp, modifier = Modifier.padding(start = 36.dp))
            Spacer(Modifier.width(72.dp))
        }
        Spacer(Modifier.height(4.dp))

        exercise.sets.forEachIndexed { setIdx, set ->
            Row(
                modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Box(modifier = Modifier.width(24.dp)) {
                    Text("%02d".format(setIdx + 1), style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant, fontSize = 9.sp)
                }
                if (exercise.bodyweight) {
                    Text("BW", style = MaterialTheme.typography.headlineSmall, color = cs.onSurface, modifier = Modifier.weight(1f))
                } else {
                    StepBtn("−", "Decrease weight") { onSetChange(setIdx, set.copy(weight = stepWeightStr(set.weight, -weightStep))) }
                    FsUnderlineField(
                        value = set.weight,
                        onValueChange = { onSetChange(setIdx, set.copy(weight = it)) },
                        placeholder = "0",
                        keyboardType = KeyboardType.Decimal,
                        modifier = Modifier.weight(1f)
                    )
                    StepBtn("+", "Increase weight") { onSetChange(setIdx, set.copy(weight = stepWeightStr(set.weight, weightStep))) }
                }
                Spacer(Modifier.width(6.dp))
                StepBtn("−", "Decrease reps") { onSetChange(setIdx, set.copy(reps = stepRepsStr(set.reps, -1))) }
                FsUnderlineField(
                    value = set.reps,
                    onValueChange = { new -> onSetChange(setIdx, set.copy(reps = new.filter { it.isDigit() })) },
                    placeholder = "0",
                    keyboardType = KeyboardType.Number,
                    modifier = Modifier.width(36.dp)
                )
                StepBtn("+", "Increase reps") { onSetChange(setIdx, set.copy(reps = stepRepsStr(set.reps, 1))) }
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
        }
      }
    }
}

/** The last session's sets as tappable-to-copy chips + a copy CTA that fills this exercise. */
@Composable
private fun LastTimePanel(sets: List<LoggedSet>, useKg: Boolean, onCopy: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Column(modifier = Modifier.fillMaxWidth().padding(top = 8.dp, bottom = 4.dp)) {
        Text("LAST TIME", style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant, fontSize = 9.sp)
        Spacer(Modifier.height(6.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            sets.forEach { s ->
                val w = s.weightLb?.let { formatWeight(it, useKg) } ?: s.weightText.ifBlank { "BW" }
                Text(
                    "$w × ${s.reps}",
                    style = MaterialTheme.typography.labelMedium,
                    color = cs.onSurface,
                    modifier = Modifier
                        .border(1.dp, cs.outline.copy(alpha = 0.4f), RoundedCornerShape(50))
                        .padding(horizontal = 10.dp, vertical = 4.dp)
                )
            }
        }
        Spacer(Modifier.height(10.dp))
        ForgeOutlineCapsule("Copy to sets", onClick = onCopy)
    }
}

/** A compact −/+ tap target (≥44dp) that nudges a set field without opening the keyboard. */
@Composable
private fun StepBtn(symbol: String, label: String, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Box(
        modifier = Modifier.sizeIn(minWidth = 32.dp, minHeight = 44.dp).clickableLabeled(label, onClick = onClick),
        contentAlignment = Alignment.Center
    ) {
        Text(symbol, style = MaterialTheme.typography.titleLarge, color = cs.onSurfaceVariant)
    }
}

/** A big serif number on an underline — the app's set-entry field (matches the live session), never a boxed input. */
@Composable
private fun FsUnderlineField(
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    keyboardType: KeyboardType,
    modifier: Modifier = Modifier
) {
    val cs = MaterialTheme.colorScheme
    Column(modifier = modifier) {
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            textStyle = MaterialTheme.typography.headlineSmall.copy(color = cs.onSurface),
            singleLine = true,
            cursorBrush = SolidColor(cs.primary),
            keyboardOptions = KeyboardOptions(keyboardType = keyboardType),
            decorationBox = { inner ->
                Box {
                    if (value.isEmpty()) {
                        Text(placeholder, style = MaterialTheme.typography.headlineSmall, color = cs.onSurfaceVariant.copy(alpha = 0.35f))
                    }
                    inner()
                }
            }
        )
        HorizontalDivider(modifier = Modifier.padding(top = 2.dp), thickness = 1.dp, color = cs.outline.copy(alpha = 0.5f))
    }
}
