@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.academy

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.forge.app.domain.academy.AcademyRegistry
import com.forge.app.ui.common.clickableLabeled

/**
 * A track's lessons as a filled/hollow dot rail (DESIGN §2②: "set of items, some present").
 *
 * Deliberately a rail rather than a progress bar or a percentage. The plan bans XP in the Academy
 * outright — "learning is not gamified engagement bait" — and a bar that fills reads as a score to
 * chase. A rail reads as an inventory: this is how many lessons the track holds, and this is how
 * many exist for you so far. It works at zero (all hollow), which is exactly the state a new
 * reader sees.
 */
@Composable
fun LessonDotRail(
    total: Int,
    unlocked: Int,
    accent: Color,
    outline: Color,
    modifier: Modifier = Modifier
) {
    val label = "$unlocked of $total lessons unlocked"
    Canvas(
        modifier
            .height(8.dp)
            .fillMaxWidth()
            .semantics { contentDescription = label }
    ) {
        if (total <= 0) return@Canvas
        val r = 3.dp.toPx()
        val step = if (total > 1) (size.width - 2 * r) / (total - 1) else 0f
        repeat(total) { i ->
            val cx = r + step * i
            val centre = Offset(cx, size.height / 2f)
            if (i < unlocked) drawCircle(accent, radius = r, center = centre)
            // §8: the empty ring sits at 1.5dp so it still reads on near-black.
            else drawCircle(outline, radius = r, center = centre, style = Stroke(width = 1.5.dp.toPx()))
        }
    }
}

/**
 * One lesson row, in the three states a lesson can be in.
 *
 * A locked row is the one this rework exists for. It used to carry a single line naming an internal
 * moment ("Your first placement-driven prescription"), which told a reader nothing they could act
 * on. It now carries the lesson's own title, what opens it, and one line of how — with the accent
 * dot reserved for the ones the reader can go and do today (§8: a dot earns its colour by flagging
 * the exception, and "you can unlock this yourself" is the exception among locked lessons).
 */
@Composable
fun LessonRow(
    state: AcademyRegistry.LessonState,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color,
    onClick: (() -> Unit)?,
    modifier: Modifier = Modifier
) {
    val lesson = state.lesson
    val locked = !state.unlocked
    Row(
        modifier
            .fillMaxWidth()
            .then(if (onClick != null) Modifier.clickableLabeled(lesson.title) { onClick() } else Modifier)
            .padding(vertical = 10.dp),
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                lesson.title,
                style = MaterialTheme.typography.titleSmall,
                color = if (locked) muted else onBg
            )
            Spacer(Modifier.height(3.dp))
            if (locked) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    // Only the ones you can trigger yourself get the accent.
                    if (lesson.unlock.byYou) {
                        Canvas(Modifier.size(5.dp)) { drawCircle(accent) }
                    }
                    Text(
                        lesson.unlock.label,
                        style = MaterialTheme.typography.bodySmall,
                        color = if (lesson.unlock.byYou) onBg else muted
                    )
                }
                Spacer(Modifier.height(2.dp))
                Text(
                    lesson.unlock.detail,
                    style = MaterialTheme.typography.bodySmall,
                    color = muted.copy(alpha = 0.65f)
                )
            } else {
                Text(lesson.summary, style = MaterialTheme.typography.bodySmall, color = muted)
            }
        }
        if (state.isNew) {
            Text(
                "NEW",
                style = MaterialTheme.typography.labelSmall,
                color = accent
            )
        } else if (state.opened) {
            // A read lesson is marked, not scored — no checkmark parade, just a quiet state word
            // the eye can skip (§8: right meta is a reading, never a duplicate of the row's colour).
            Text("READ", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.65f))
        }
    }
}

/**
 * The lesson reader: a sheet over whichever Academy surface opened it (§3, modal archetype).
 * Shared by the hub and the track pages so a lesson reads identically wherever it was tapped, and
 * so closing it records the same completion either way.
 */
@Composable
fun LessonSheet(
    state: AcademyViewModel.UiState,
    viewModel: AcademyViewModel,
    onBg: Color
) {
    val id = state.openLessonId ?: return
    val lesson = AcademyRegistry.lesson(id) ?: return
    ModalBottomSheet(
        onDismissRequest = { viewModel.close(finished = true) },
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
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
