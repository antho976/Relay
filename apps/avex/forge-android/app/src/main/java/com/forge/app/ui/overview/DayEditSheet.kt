package com.forge.app.ui.overview

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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.db.entities.WarmupRoutineItem
import com.forge.app.program.Program
import com.forge.app.ui.overview.components.AddWarmupRow
import com.forge.app.ui.overview.components.ExerciseEditRow
import com.forge.app.ui.overview.components.NameSection

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DayEditSheet(
    initialDayKey: String,
    onSelectAsToday: (String) -> Unit,
    onDismiss: () -> Unit,
    vm: DayEditViewModel = hiltViewModel()
) {
    LaunchedEffect(initialDayKey) { vm.selectDay(initialDayKey) }

    val selectedDayKey by vm.selectedDayKey.collectAsStateWithLifecycle()
    val customName by vm.customName.collectAsStateWithLifecycle()
    val exerciseCustomizations by vm.exerciseCustomizations.collectAsStateWithLifecycle()
    val warmupItems by vm.warmupItems.collectAsStateWithLifecycle()

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val bg = MaterialTheme.colorScheme.background

    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = bg
    ) {
        val customByExId = remember(exerciseCustomizations) {
            exerciseCustomizations.associateBy { it.exerciseId }
        }
        val selectedDay = remember(selectedDayKey) {
            Program.days.firstOrNull { it.key == selectedDayKey }
        }

        LazyColumn(
            modifier = Modifier.fillMaxWidth(),
            contentPadding = androidx.compose.foundation.layout.PaddingValues(bottom = 48.dp)
        ) {

            // ── Day selector ──────────────────────────────────────────────────
            item("day-selector") {
                Column(modifier = Modifier.padding(horizontal = 24.dp)) {
                    Spacer(Modifier.height(4.dp))
                    Text(
                        "EDIT WORKOUT",
                        style = MaterialTheme.typography.labelSmall,
                        letterSpacing = 1.5.sp,
                        color = muted,
                        fontSize = 10.sp
                    )
                    Spacer(Modifier.height(12.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Program.days.forEach { day ->
                            val sel = day.key == selectedDayKey
                            Box(
                                modifier = Modifier
                                    .border(
                                        1.dp,
                                        if (sel) onBg else outline.copy(alpha = 0.4f),
                                        RoundedCornerShape(4.dp)
                                    )
                                    .background(
                                        if (sel) onBg else Color.Transparent,
                                        RoundedCornerShape(4.dp)
                                    )
                                    .clickable { vm.selectDay(day.key) }
                                    .padding(horizontal = 10.dp, vertical = 6.dp),
                                contentAlignment = Alignment.Center
                            ) {
                                Text(
                                    day.defaultName.uppercase(),
                                    style = MaterialTheme.typography.labelSmall,
                                    color = if (sel) bg else muted.copy(alpha = 0.7f),
                                    fontSize = 9.sp,
                                    letterSpacing = 0.5.sp
                                )
                            }
                        }
                    }
                    Spacer(Modifier.height(12.dp))
                    // "Set as today" row (only shown when not already today)
                    if (selectedDayKey != initialDayKey) {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { onSelectAsToday(selectedDayKey) }
                                .padding(vertical = 8.dp),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Text(
                                "Train ${selectedDay?.defaultName ?: selectedDayKey} today",
                                style = MaterialTheme.typography.bodySmall,
                                color = muted
                            )
                            Text("→", style = MaterialTheme.typography.bodySmall, color = onBg)
                        }
                    }
                    Spacer(Modifier.height(4.dp))
                }
                HorizontalDivider(color = outline.copy(alpha = 0.2f))
            }

            // ── Name ──────────────────────────────────────────────────────────
            item("name") {
                NameSection(
                    defaultName = selectedDay?.defaultName ?: "",
                    customName = customName,
                    onSave = { vm.setDayName(it) },
                    onBg = onBg, muted = muted, outline = outline
                )
                HorizontalDivider(color = outline.copy(alpha = 0.2f))
            }

            // ── Exercises ─────────────────────────────────────────────────────
            item("ex-header") {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 24.dp, vertical = 14.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        "EXERCISES",
                        style = MaterialTheme.typography.labelSmall,
                        letterSpacing = 1.5.sp,
                        color = muted,
                        fontSize = 10.sp
                    )
                    Text(
                        "reset",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted.copy(alpha = 0.5f),
                        fontSize = 10.sp,
                        modifier = Modifier.clickable { vm.resetExercises() }
                    )
                }
            }

            selectedDay?.exercises?.let { basePlans ->
                items(basePlans, key = { it.id }) { plan ->
                    val c = customByExId[plan.id]
                    ExerciseEditRow(
                        plan = plan,
                        customization = c,
                        onSetsChange = { vm.setSetsOverride(plan.id, it) },
                        onRepsChange = { vm.setRepsOverride(plan.id, it) },
                        onToggleRemoved = { vm.toggleExerciseRemoved(plan.id, c?.removed ?: false) },
                        onBg = onBg, muted = muted, outline = outline
                    )
                    HorizontalDivider(
                        modifier = Modifier.padding(horizontal = 24.dp),
                        color = outline.copy(alpha = 0.12f)
                    )
                }
            }

            // ── Warmup ────────────────────────────────────────────────────────
            item("warmup-header") {
                HorizontalDivider(color = outline.copy(alpha = 0.2f))
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 24.dp, vertical = 14.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        "WARMUP",
                        style = MaterialTheme.typography.labelSmall,
                        letterSpacing = 1.5.sp,
                        color = muted,
                        fontSize = 10.sp
                    )
                    Text(
                        "reset",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted.copy(alpha = 0.5f),
                        fontSize = 10.sp,
                        modifier = Modifier.clickable { vm.resetWarmup() }
                    )
                }
            }

            // Static warmup (shown only when no custom items set)
            if (warmupItems.isEmpty()) {
                val staticWarmup = selectedDay?.warmup ?: emptyList()
                items(staticWarmup, key = { "static_$it" }) { item ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 24.dp, vertical = 10.dp),
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Text(
                            item,
                            style = MaterialTheme.typography.bodySmall,
                            color = muted.copy(alpha = 0.4f),
                            fontStyle = FontStyle.Italic
                        )
                        Text(
                            "default",
                            style = MaterialTheme.typography.labelSmall,
                            color = muted.copy(alpha = 0.3f),
                            fontSize = 9.sp
                        )
                    }
                    HorizontalDivider(
                        modifier = Modifier.padding(horizontal = 24.dp),
                        color = outline.copy(alpha = 0.08f)
                    )
                }
            }

            items(warmupItems, key = { it.id }) { item ->
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 24.dp, vertical = 10.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        item.label,
                        style = MaterialTheme.typography.bodySmall,
                        color = onBg
                    )
                    Text(
                        "×",
                        style = MaterialTheme.typography.bodyMedium,
                        color = muted.copy(alpha = 0.5f),
                        modifier = Modifier
                            .clickable { vm.removeWarmupItem(item) }
                            .padding(start = 16.dp, top = 4.dp, bottom = 4.dp)
                    )
                }
                HorizontalDivider(
                    modifier = Modifier.padding(horizontal = 24.dp),
                    color = outline.copy(alpha = 0.12f)
                )
            }

            item("warmup-add") {
                AddWarmupRow(onAdd = { vm.addWarmupItem(it) }, muted = muted, outline = outline, onBg = onBg)
            }
        }
    }
}

