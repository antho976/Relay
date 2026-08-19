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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.academy.LessonTrack
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.statsEntrance

/**
 * The Academy (Coach v3 B3) — the knowledge half of the coach, and the reason it can make itself
 * optional. Everything the coach decides, you can learn to decide yourself.
 *
 * Two lenses under one name, because the tab holds two different promises and a reader has to be
 * able to tell which one they are looking at:
 *
 *  - **Lessons** are earned. Each attaches to a coach moment and opens the first time that moment
 *    fires, so the track rails read as an inventory of what has happened to you.
 *  - **Library** is not. Every article is readable from install by anyone, with no gate, no order
 *    and no score. It answers "I want to read about this tonight" rather than "why did the coach
 *    just do that".
 *
 * §4.4 makes that split a `SegmentPill` row rather than two tabs or two screens. The serif hero
 * stays constant across both so the tab keeps ONE identity (§6, one hero per screen); only the
 * eyebrow and the aside change, since those are the parts that are actually lens-specific.
 */
@Composable
fun AcademyScreen(
    onBack: (() -> Unit)? = null,
    onOpenTrack: (LessonTrack) -> Unit = {},
    onOpenArticle: (String) -> Unit = {},
    /** Opens straight onto one lesson's sheet — how a notifications-feed row lands here. */
    initialLessonId: String? = null,
    viewModel: AcademyViewModel = hiltViewModel(),
    libraryViewModel: LibraryViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val libraryState by libraryViewModel.state.collectAsStateWithLifecycle()
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    // Saveable so the lens survives a rotation and a swipe away to another hub page. It does NOT
    // persist across launches: the Academy opens on Lessons, because that is the half that can
    // have something new in it.
    var lens by rememberSaveable { mutableStateOf(AcademyLens.LESSONS) }

    // Keyed on the id so a second arrival for a different lesson still opens, while a rotation on
    // the same one does not re-open a sheet the reader just dismissed.
    LaunchedEffect(initialLessonId) {
        initialLessonId?.let { viewModel.open(it) }
    }

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
                    // §12: an honest count, and it reads correctly at zero in either lens.
                    Text(
                        when (lens) {
                            AcademyLens.LESSONS -> "$unlockedTotal OF $allTotal UNLOCKED"
                            AcademyLens.LIBRARY -> libraryEyebrow(libraryState)
                        },
                        style = MaterialTheme.typography.labelMedium,
                        color = muted
                    )
                    Spacer(Modifier.height(2.dp))
                    Text("Academy", style = MaterialTheme.typography.headlineLarge, color = onBg)
                    Spacer(Modifier.height(6.dp))
                    Text(
                        when (lens) {
                            AcademyLens.LESSONS ->
                                "Everything the coach knows, explained the moment it matters."
                            AcademyLens.LIBRARY ->
                                "The research behind the training, condensed and cited."
                        },
                        style = MaterialTheme.typography.bodyMedium.copy(fontStyle = FontStyle.Italic),
                        color = muted
                    )
                }
            }

            item("lens") {
                Spacer(Modifier.height(16.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    AcademyLens.entries.forEach { option ->
                        SegmentPill(
                            text = option.label,
                            selected = lens == option,
                            onClick = { lens = option },
                            accent = accent, onBg = onBg, muted = muted, outline = outline
                        )
                    }
                }
            }

            when (lens) {
                AcademyLens.LESSONS -> academyLessonsPane(
                    state = state,
                    viewModel = viewModel,
                    onOpenTrack = onOpenTrack,
                    onBg = onBg, muted = muted, accent = accent, outline = outline
                )

                AcademyLens.LIBRARY -> libraryPane(
                    state = libraryState,
                    viewModel = libraryViewModel,
                    onOpenArticle = onOpenArticle,
                    onBg = onBg, muted = muted, accent = accent, outline = outline
                )
            }
        }
    }
}

/** The two halves of the Academy. §4.4: lens labels are ONE short word. */
enum class AcademyLens(val label: String) {
    LESSONS("Lessons"),
    LIBRARY("Library")
}
