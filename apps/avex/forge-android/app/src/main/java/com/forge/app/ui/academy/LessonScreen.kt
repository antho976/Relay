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
import com.forge.app.domain.academy.AcademyRegistry
import com.forge.app.domain.academy.LessonTrack
import com.forge.app.domain.academy.readMinutes
import com.forge.app.ui.common.InlineEmptyHint

/**
 * One lesson, on the same page an article gets.
 *
 * Until 2026-08-20 this was a `ModalBottomSheet` over whichever surface opened it. The sheet is
 * gone: it capped a lesson at a sheet's height, could not carry a cover, and counted a dismissal as
 * a completed read. See [ReaderScreen] for what replaced it and why the two halves of the Academy
 * now share one reader.
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
        cover = AcademyCovers.forId(lesson.id),
        kicker = lessonKicker(lesson.track, lesson.id, lesson.blocks.readMinutes()),
        title = lesson.title,
        deck = lesson.summary,
        blocks = lesson.blocks,
        examples = state.examples,
        next = state.next,
        onReachedEnd = viewModel::onReachedEnd,
        onOpenNext = onOpenLesson
    )
}

/**
 * "FUNDAMENTALS · 03 · 4 MIN".
 *
 * The numeral rides Fundamentals only — the one track authored as a sequence — so a position never
 * implies an order that was never written. Everywhere else the line is the chapter and the length.
 */
private fun lessonKicker(track: LessonTrack, lessonId: String, minutes: Int): String {
    val numeral = if (track == LessonTrack.FUNDAMENTALS) {
        AcademyRegistry.byTrack(track)
            .indexOfFirst { it.id == lessonId }
            .takeIf { it >= 0 }
            ?.let { (it + 1).toString().padStart(2, '0') }
    } else {
        null
    }
    return listOfNotNull(track.displayName, numeral, "$minutes min").joinToString(" · ")
}
