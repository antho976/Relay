package com.forge.app.ui.academy

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.academy.AcademyRegistry
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgeWordmark
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.statsEntrance

/**
 * The Academy (Coach v3 B3) — the knowledge half of the coach, and the reason it can make itself
 * optional. Everything the coach decides, you can learn to decide yourself.
 *
 * Overview archetype: a serif hero, unlocked lessons as real rows with their summaries, and
 * upcoming lessons visible-but-locked, each naming the moment that opens it. Reading nothing
 * changes nothing — there is no XP here and there never will be.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AcademyScreen(
    onBack: (() -> Unit)? = null,
    viewModel: AcademyViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary

    state.openLessonId?.let { id ->
        AcademyRegistry.lesson(id)?.let { lesson ->
            val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
            ModalBottomSheet(
                onDismissRequest = { viewModel.close(finished = true) },
                sheetState = sheetState,
                containerColor = MaterialTheme.colorScheme.surface
            ) {
                Column(
                    Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 24.dp)
                        .padding(bottom = 32.dp)
                ) {
                    Text(lesson.title, style = MaterialTheme.typography.headlineSmall, color = onBg)
                    Spacer(Modifier.height(12.dp))
                    LessonBody(lesson, state.examples)
                }
            }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { ForgeWordmark() },
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
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(inner),
            contentPadding = PaddingValues(horizontal = 24.dp, vertical = 4.dp)
        ) {
            item("hero") {
                Column(Modifier.fillMaxWidth().statsEntrance(0).padding(vertical = 8.dp)) {
                    Text(
                        "${state.unlocked.size} OF ${state.unlocked.size + state.upcoming.size} UNLOCKED",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted,
                        fontSize = 9.sp,
                        letterSpacing = 1.5.sp
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

            if (state.unlocked.isNotEmpty()) {
                item("unlocked-header") {
                    Spacer(Modifier.height(28.dp))
                    EditorialHeader(label = "Unlocked", muted = muted, accent = accent)
                    Spacer(Modifier.height(10.dp))
                }
                items(state.unlocked.size, key = { state.unlocked[it].lesson.id }) { i ->
                    val s = state.unlocked[i]
                    LessonRow(
                        title = s.lesson.title,
                        subtitle = s.lesson.summary,
                        isNew = s.isNew,
                        locked = false,
                        onBg = onBg, muted = muted, accent = accent,
                        onClick = { viewModel.open(s.lesson.id) }
                    )
                }
            }

            if (state.upcoming.isNotEmpty()) {
                item("upcoming-header") {
                    Spacer(Modifier.height(28.dp))
                    EditorialHeader(label = "Ahead", muted = muted, accent = accent)
                    Spacer(Modifier.height(10.dp))
                }
                items(state.upcoming.size, key = { state.upcoming[it].lesson.id }) { i ->
                    val s = state.upcoming[i]
                    // Visible but locked, naming its own moment: the product should be visibly
                    // growing, and a locked lesson is a promise the coach has to keep.
                    LessonRow(
                        title = s.lesson.title,
                        subtitle = s.lesson.unlockedBy,
                        isNew = false,
                        locked = true,
                        onBg = onBg, muted = muted, accent = accent,
                        onClick = null
                    )
                }
            }

            item("tail") { Spacer(Modifier.height(56.dp)) }
        }
    }
}

@Composable
private fun LessonRow(
    title: String,
    subtitle: String,
    isNew: Boolean,
    locked: Boolean,
    onBg: Color,
    muted: Color,
    accent: Color,
    onClick: (() -> Unit)?
) {
    Row(
        Modifier
            .fillMaxWidth()
            .then(if (onClick != null) Modifier.clickableLabeled(title) { onClick() } else Modifier)
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.Top
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                title,
                style = MaterialTheme.typography.titleSmall,
                color = if (locked) muted else onBg
            )
            Spacer(Modifier.height(2.dp))
            Text(
                subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = muted
            )
        }
        // The one flagged exception (§8): a lesson you haven't read yet.
        if (isNew) {
            Spacer(Modifier.width(12.dp))
            Text(
                "NEW",
                style = MaterialTheme.typography.labelSmall,
                color = accent,
                fontSize = 9.sp,
                letterSpacing = 1.5.sp
            )
        }
    }
}
