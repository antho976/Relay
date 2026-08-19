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

// `LessonDotRail` and `LessonRow` were deleted 2026-08-16 with the gated hub they belonged to.
// The rail counted how much of a track had unlocked and the row rendered a locked lesson's
// unlock condition; neither concept exists now that every lesson is open. See design/MAP.md.

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
