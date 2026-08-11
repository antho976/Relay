@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.academy

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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.academy.LessonTrack
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.statsEntrance

/**
 * The Academy (Coach v3 B3) — the knowledge half of the coach, and the reason it can make itself
 * optional. Everything the coach decides, you can learn to decide yourself.
 *
 * Overview archetype, and organised the way the plan already described but the first build ignored:
 * **five tracks**, not one flat list of 31 rows. Each track carries a dot rail of what it holds and
 * what has opened, and drills into its own page (§4.1 — overview first, deep dives in sub-screens).
 *
 * What this screen deliberately is NOT is a course index. `docs/ACADEMY_LESSONS.md` is explicit:
 * "just-in-time, not curriculum-first" — lessons attach to coach moments, and only Fundamentals is
 * sequential. So there is no "next up" ladder and no percentage complete; the one lesson offered is
 * one whose moment has already fired and that hasn't been read. Reading nothing changes nothing.
 */
@Composable
fun AcademyScreen(
    onBack: (() -> Unit)? = null,
    onOpenTrack: (LessonTrack) -> Unit = {},
    viewModel: AcademyViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    LessonSheet(state, viewModel, onBg)

    Scaffold(
        topBar = {
            TopAppBar(
                title = {},
                navigationIcon = {
                    if (onBack != null) IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back", tint = muted)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        val unlockedTotal = state.unlocked.size
        val allTotal = unlockedTotal + state.upcoming.size

        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(inner),
            contentPadding = PaddingValues(horizontal = 24.dp, vertical = 4.dp)
        ) {
            item("hero") {
                Column(Modifier.fillMaxWidth().statsEntrance(0).padding(vertical = 8.dp)) {
                    // §12: an honest count, and it reads correctly at zero.
                    Text(
                        "$unlockedTotal OF $allTotal UNLOCKED",
                        style = MaterialTheme.typography.labelMedium,
                        color = muted
                    )
                    Spacer(Modifier.height(2.dp))
                    Text("Academy", style = MaterialTheme.typography.headlineLarge, color = onBg)
                    Spacer(Modifier.height(6.dp))
                    Text(
                        "Everything the coach knows, explained the moment it matters.",
                        style = MaterialTheme.typography.bodyMedium.copy(fontStyle = FontStyle.Italic),
                        color = muted
                    )
                }
            }

            // The only thing this screen pushes: a lesson whose moment already fired, unread.
            state.continueLesson?.let { next ->
                item("continue") {
                    Spacer(Modifier.height(28.dp))
                    EditorialHeader(label = "Start here", muted = muted, accent = accent)
                    Spacer(Modifier.height(10.dp))
                    Column(
                        Modifier
                            .fillMaxWidth()
                            .statsEntrance(1)
                            .clickableLabeled("Read: ${next.lesson.title}") { viewModel.open(next.lesson.id) }
                            .padding(vertical = 2.dp)
                    ) {
                        Text(next.lesson.title, style = MaterialTheme.typography.titleMedium, color = onBg)
                        Spacer(Modifier.height(3.dp))
                        Text(next.lesson.summary, style = MaterialTheme.typography.bodySmall, color = muted)
                        Spacer(Modifier.height(6.dp))
                        Text("read →", style = MaterialTheme.typography.labelMedium, color = accent)
                    }
                }
            }

            item("tracks-header") {
                Spacer(Modifier.height(28.dp))
                EditorialHeader(label = "Tracks", muted = muted, accent = accent)
                Spacer(Modifier.height(10.dp))
            }

            itemsIndexed(state.tracks, key = { _, t -> t.track.code }) { index, t ->
                Column(
                    Modifier
                        .fillMaxWidth()
                        .statsEntrance(index + 2)
                        .clickableLabeled("Open ${t.track.displayName}") { onOpenTrack(t.track) }
                        .padding(vertical = 10.dp)
                ) {
                    Row(
                        Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(t.track.displayName, style = MaterialTheme.typography.titleSmall, color = onBg)
                        Text(
                            if (t.unread > 0) "${t.unread} NEW" else "${t.unlocked} OF ${t.total}",
                            style = MaterialTheme.typography.labelMedium,
                            color = if (t.unread > 0) accent else muted
                        )
                    }
                    Spacer(Modifier.height(3.dp))
                    Text(t.track.blurb, style = MaterialTheme.typography.bodySmall, color = muted)
                    Spacer(Modifier.height(10.dp))
                    LessonDotRail(total = t.total, unlocked = t.unlocked, accent = accent, outline = outline)
                }
            }

            item("tail") { Spacer(Modifier.height(56.dp)) }
        }
    }
}
