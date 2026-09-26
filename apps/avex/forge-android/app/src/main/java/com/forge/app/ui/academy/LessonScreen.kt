package com.forge.app.ui.academy

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.academy.readMinutes
import com.forge.app.ui.common.InlineEmptyHint

/**
 * One lesson, on its own page. A retired id (from a coach reason written before the 2026-09-26
 * cut) resolves to the lesson that absorbed it, so old links still land somewhere true.
 */
@Composable
fun LessonScreen(
    onBack: () -> Unit,
    onOpenLesson: (String) -> Unit = {},
    viewModel: LessonViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val lesson = state.lesson

    if (lesson == null) {
        // A retired or mistyped id. §12's error state: a quiet inline line wording the consequence,
        // never a dialog and never a crash.
        Column(Modifier.fillMaxSize().padding(horizontal = 24.dp)) {
            Spacer(Modifier.height(96.dp))
            InlineEmptyHint("That lesson is no longer in the Academy", muted.copy(alpha = 0.65f))
        }
        return
    }

    ReaderScreen(
        onBack = onBack,
        lessonId = lesson.id,
        meta = "${lesson.track.displayName} · ${lesson.blocks.readMinutes()} min",
        title = lesson.title,
        deck = lesson.summary,
        blocks = lesson.blocks,
        examples = state.examples,
        sources = lesson.sources,
        next = state.next,
        onReachedEnd = viewModel::onReachedEnd,
        onOpenNext = onOpenLesson
    )
}
