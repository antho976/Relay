package com.forge.app.ui.academy

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import com.forge.app.domain.academy.LessonTrack
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.statsEntrance

/**
 * The Lessons lens: five tracks, gated on coach moments.
 *
 * Extracted from `AcademyScreen` when the Library landed beside it, unchanged in behaviour. What it
 * deliberately still is NOT is a course index: `docs/ACADEMY_LESSONS.md` is explicit that lessons
 * are just-in-time, so there is no "next up" ladder and no percentage complete. The one lesson it
 * offers is one whose moment has already fired and that has not been read.
 *
 * This is the half of the Academy that is earned. The Library beside it is the half that is not,
 * and keeping the two contracts visibly different is the reason they can share a tab without
 * reading as duplicates.
 */
internal fun LazyListScope.academyLessonsPane(
    state: AcademyViewModel.UiState,
    viewModel: AcademyViewModel,
    onOpenTrack: (LessonTrack) -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    // The only thing this lens pushes: a lesson whose moment already fired, unread.
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

    item("lessons-tail") { Spacer(Modifier.height(56.dp)) }
}
