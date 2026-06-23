package com.forge.app.ui.gym.train

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
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
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.gym.train.state.DayListItem

/**
 * Read-only view of the whole program (reached from "view program" on the overview): each day as a
 * big spine word (PUSH, PULL…) you tap to reveal that day's exercises. The next-up day is highlighted
 * with its accent colour and a "NEXT UP" badge. Unlike the Gym day-list, tapping never starts a
 * session — this is purely for looking the plan over.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ProgramViewerScreen(
    onBack: () -> Unit,
    viewModel: DayListViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    var expandedKey by remember { mutableStateOf<String?>(null) }
    // Open the next-up day by default so the view lands on where you are in the week.
    LaunchedEffect(state.days) {
        if (expandedKey == null) expandedKey = state.days.firstOrNull { it.isNextUp }?.plan?.key
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("YOUR PROGRAM", style = MaterialTheme.typography.headlineLarge) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        Column(
            Modifier
                .fillMaxSize()
                .padding(inner)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Text(
                "Tap a day to see its exercises.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                fontStyle = FontStyle.Italic
            )
            state.days.forEach { item ->
                ProgramViewerDayCard(
                    item = item,
                    expanded = expandedKey == item.plan.key,
                    onToggle = { expandedKey = if (expandedKey == item.plan.key) null else item.plan.key }
                )
            }
            Spacer(Modifier.height(24.dp))
        }
    }
}

@Composable
private fun ProgramViewerDayCard(item: DayListItem, expanded: Boolean, onToggle: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val accent = remember(item.customAccentHex, item.plan.accentHex) {
        val hex = item.customAccentHex ?: item.plan.accentHex
        runCatching { Color(android.graphics.Color.parseColor(hex)) }.getOrDefault(onBg)
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .animateContentSize()
            .clip(RoundedCornerShape(16.dp))
            .border(
                width = if (item.isNextUp) 2.dp else 1.dp,
                color = if (item.isNextUp) accent else outline.copy(alpha = 0.3f),
                shape = RoundedCornerShape(16.dp)
            )
            .background(
                if (item.isNextUp) accent.copy(alpha = 0.08f) else MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.4f)
            )
            .clickable { onToggle() }
            .padding(16.dp)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(modifier = Modifier.weight(1f)) {
                if (item.isNextUp) {
                    Text("NEXT UP", style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 9.sp, fontWeight = FontWeight.Bold)
                    Spacer(Modifier.height(2.dp))
                }
                Text(
                    item.plan.word.ifBlank { item.displayName }.uppercase(),
                    style = MaterialTheme.typography.headlineLarge,
                    color = if (item.isNextUp) accent else onBg,
                    fontWeight = FontWeight.Black
                )
                Text(
                    "${item.displayName}  ·  ${item.exerciseCount} exercises",
                    style = MaterialTheme.typography.bodySmall,
                    color = muted,
                    fontStyle = FontStyle.Italic
                )
            }
            Text(if (expanded) "▲" else "▾", style = MaterialTheme.typography.bodyLarge, color = muted.copy(alpha = 0.6f))
        }

        if (expanded) {
            Spacer(Modifier.height(12.dp))
            if (item.plan.exercises.isEmpty()) {
                Text(
                    "No exercises in this day.",
                    style = MaterialTheme.typography.bodySmall,
                    color = muted,
                    fontStyle = FontStyle.Italic
                )
            } else {
                item.plan.exercises.forEachIndexed { i, ex ->
                    Row(modifier = Modifier.fillMaxWidth().padding(vertical = 6.dp), verticalAlignment = Alignment.Top) {
                        Text(
                            "%02d".format(i + 1),
                            style = MaterialTheme.typography.labelSmall,
                            color = muted,
                            fontSize = 9.sp,
                            modifier = Modifier.width(28.dp).padding(top = 4.dp)
                        )
                        Column(modifier = Modifier.weight(1f)) {
                            Text(ex.name, style = MaterialTheme.typography.bodyLarge, color = onBg)
                            Text(
                                "${ex.sets} × ${ex.reps}  ·  ${ex.muscle.displayName.lowercase()}",
                                style = MaterialTheme.typography.bodySmall,
                                color = muted,
                                fontStyle = FontStyle.Italic
                            )
                        }
                    }
                }
            }
        }
    }
}
