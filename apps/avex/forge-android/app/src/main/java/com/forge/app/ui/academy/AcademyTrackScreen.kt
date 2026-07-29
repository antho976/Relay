@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.academy

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.academy.LessonTrack
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.forgeItemMotion

/**
 * One track's lessons (list archetype): what's open, then what isn't and what opens it.
 *
 * The split is the whole point. A locked lesson used to be a title plus an internal moment, which
 * left a reader with a wall of things they could neither read nor reach. Here the locked ones say
 * plainly whether the next move is theirs or the coach's — the accent dot marks the ones they can
 * go and do today — so "ahead" stops being a list of vocabulary and becomes a list of answers.
 *
 * No search field: a track tops out at ten lessons, all visible at once.
 */
@Composable
fun AcademyTrackScreen(
    track: LessonTrack,
    onBack: () -> Unit,
    viewModel: AcademyViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    LessonSheet(state, viewModel, onBg)

    val lessons = state.lessonsIn(track)
    val open = lessons.filter { it.unlocked }
    val locked = lessons.filterNot { it.unlocked }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {},
                navigationIcon = {
                    IconButton(onClick = onBack) {
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
                Column(Modifier.fillMaxWidth().padding(vertical = 8.dp)) {
                    Text(
                        "${open.size} OF ${lessons.size} UNLOCKED",
                        style = MaterialTheme.typography.labelMedium,
                        color = muted
                    )
                    Spacer(Modifier.height(2.dp))
                    Text(track.displayName, style = MaterialTheme.typography.headlineSmall, color = onBg)
                    Spacer(Modifier.height(6.dp))
                    Text(
                        track.blurb,
                        style = MaterialTheme.typography.bodyMedium.copy(fontStyle = FontStyle.Italic),
                        color = muted
                    )
                    Spacer(Modifier.height(14.dp))
                    LessonDotRail(
                        total = lessons.size, unlocked = open.size,
                        accent = accent, outline = outline
                    )
                }
            }

            if (open.isNotEmpty()) {
                item("open-header") {
                    Spacer(Modifier.height(28.dp))
                    EditorialHeader(label = "Open", muted = muted, accent = accent)
                    Spacer(Modifier.height(8.dp))
                }
                items(open, key = { it.lesson.id }) { s ->
                    LessonRow(
                        state = s,
                        onBg = onBg, muted = muted, accent = accent, outline = outline,
                        onClick = { viewModel.open(s.lesson.id) },
                        modifier = forgeItemMotion()
                    )
                }
            }

            if (locked.isNotEmpty()) {
                item("locked-header") {
                    Spacer(Modifier.height(28.dp))
                    EditorialHeader(label = "Not yet", muted = muted, accent = accent)
                    Spacer(Modifier.height(8.dp))
                }
                items(locked, key = { it.lesson.id }) { s ->
                    // Passive: nothing looks tappable while doing nothing (§2③).
                    LessonRow(
                        state = s,
                        onBg = onBg, muted = muted, accent = accent, outline = outline,
                        onClick = null,
                        modifier = forgeItemMotion()
                    )
                }
            }

            if (lessons.isEmpty()) {
                item("empty") {
                    Spacer(Modifier.height(20.dp))
                    InlineEmptyHint("This track ships with a later phase.", muted.copy(alpha = 0.65f))
                }
            }

            item("tail") { Spacer(Modifier.height(56.dp)) }
        }
    }
}
