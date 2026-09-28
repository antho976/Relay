@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
package com.forge.app.ui.gym.freestyle

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.ForgeTopBar
import com.forge.app.ui.common.GROUP_SEAM
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.ROW_H
import com.forge.app.ui.gym.train.components.GymSearchRow
import com.forge.app.ui.gym.train.components.groupMember
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.gym.history.formatHistoryDate

/** Case-insensitive filter over the template rows — matches the day name or any move it contains. */
internal fun filterTemplates(templates: List<FreestyleTemplateSummary>, query: String): List<FreestyleTemplateSummary> {
    val q = query.trim().lowercase()
    if (q.isEmpty()) return templates
    return templates.filter { t ->
        t.title.lowercase().contains(q) || t.exerciseNames.any { it.lowercase().contains(q) }
    }
}

/**
 * The freestyle "start from a past workout" picker (GYMAP-48): a full-screen List/browser overlaid on
 * the logger. Search-first over every finished session; one tap seeds the logger with that workout's
 * exercises and sets so you can re-log it and just adjust the numbers. Paints its own background and
 * owns system back, matching the sibling [ExerciseBrowserScreen].
 */
@Composable
fun FreestyleTemplatePicker(
    templates: List<FreestyleTemplateSummary>,
    onClose: () -> Unit,
    onPick: (Long) -> Unit
) {
    var query by remember { mutableStateOf("") }
    val results = remember(query, templates) { filterTemplates(templates, query) }

    BackHandler(onBack = onClose)

    val amoled = com.forge.app.ui.theme.LocalForgeSettings.current.amoledMode
    val (gradTop, gradBottom) = com.forge.app.ui.theme.forgeBackgroundGradient(amoled)

    Box(Modifier.fillMaxSize().background(Brush.verticalGradient(listOf(gradTop, gradBottom)))) {
        Scaffold(
            containerColor = Color.Transparent,
            // §4.6: back only, never the screen's name.
            topBar = { ForgeTopBar(onBack = onClose, backLabel = "Close") }
        ) { inner ->
            LazyColumn(
                modifier = Modifier.fillMaxSize().padding(inner),
                contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 32.dp),
                // The past workouts are one group: filled members, 2dp seams.
                verticalArrangement = Arrangement.spacedBy(GROUP_SEAM)
            ) {
                item {
                    Column(Modifier.padding(bottom = 14.dp)) {
                        Column(Modifier.padding(horizontal = 8.dp)) {
                            Text(
                                "START FROM",
                                style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                letterSpacing = 1.sp
                            )
                            Text(
                                "Past workouts",
                                style = MaterialTheme.typography.headlineSmall,
                                color = MaterialTheme.colorScheme.onSurface
                            )
                        }
                        Spacer(Modifier.height(12.dp))
                        GymSearchRow(query = query, onQueryChange = { query = it }, placeholder = "Search past workouts")
                    }
                }
                itemsIndexed(results, key = { _, it -> it.sessionId }) { i, t ->
                    TemplateRow(
                        template = t,
                        onClick = { onPick(t.sessionId) },
                        modifier = Modifier.groupMember(i, results.size)
                    )
                }
                if (results.isEmpty()) {
                    item {
                        InlineEmptyHint(
                            "No workouts match.",
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(horizontal = 8.dp)
                        )
                    }
                }
            }
        }
    }
}

/** One template row: mono date eyebrow → day name → the moves it holds, with a set-count figure right. */
@Composable
private fun TemplateRow(template: FreestyleTemplateSummary, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    val muted = cs.onSurfaceVariant
    Row(
        modifier = modifier
            .clickableLabeled("Start from ${template.title}, ${formatHistoryDate(template.startedAtMs)}", onClick = onClick)
            .padding(horizontal = ROW_H, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Text(
                formatHistoryDate(template.startedAtMs).uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                fontSize = 9.sp,
                letterSpacing = 0.5.sp
            )
            Text(template.title, style = MaterialTheme.typography.titleMedium, color = cs.onSurface)
            // The moves it holds — the row's real content, so you pick by what's inside, not just its name.
            Text(
                template.exerciseNames.distinct().joinToString(" · "),
                style = MaterialTheme.typography.bodySmall,
                color = muted,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
        }
        if (template.setCount > 0) {
            Spacer(Modifier.width(12.dp))
            Column(horizontalAlignment = Alignment.End) {
                Text(template.setCount.toString(), style = MaterialTheme.typography.bodyMedium, color = cs.onSurface)
                Text("SETS", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
            }
        }
    }
}
