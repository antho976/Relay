package com.forge.app.ui.gym.notes

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.program.Program
import com.forge.app.ui.common.ForgeTopBar
import com.forge.app.ui.common.GROUP_SEAM
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.ROW_H
import com.forge.app.ui.gym.train.components.GymSearchRow
import com.forge.app.ui.gym.train.components.groupMember
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NotesSearchScreen(
    onBack: () -> Unit,
    viewModel: NotesSearchViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()

    Scaffold(
        // §4.6: back only, never the screen's name; the serif hero below carries it.
        topBar = { ForgeTopBar(onBack = onBack) },
        containerColor = Color.Transparent
    ) { inner ->
        Column(modifier = Modifier.fillMaxSize().padding(inner)) {
            // List archetype (§3): a tiny serif hero names the screen in content.
            Text(
                "Notes",
                style = MaterialTheme.typography.headlineSmall,
                color = MaterialTheme.colorScheme.onBackground,
                modifier = Modifier.padding(horizontal = 24.dp)
            )
            Spacer(Modifier.height(16.dp))
            GymSearchRow(
                query = state.query,
                onQueryChange = viewModel::setQuery,
                placeholder = "Search your notes",
                modifier = Modifier.padding(horizontal = 16.dp)
            )
            Spacer(Modifier.height(16.dp))

            when {
                state.query.isBlank() -> InlineEmptyHint(
                    text = "Type to search across your exercise notes.",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 24.dp, vertical = 4.dp)
                )
                // "Searching" and "no matches" are different answers, and the screen used to give
                // the second one while the first was true — under the new query, over the previous
                // query's results, for the whole debounce window.
                state.searching -> InlineEmptyHint(
                    text = "Searching…",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 24.dp, vertical = 4.dp)
                )
                state.results.isEmpty() -> InlineEmptyHint(
                    text = "No notes match. Try another term.",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 24.dp, vertical = 4.dp)
                )
                // The hits are one group of filled members, 2dp seams, no lines.
                else -> LazyColumn(
                    contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 32.dp),
                    verticalArrangement = Arrangement.spacedBy(GROUP_SEAM)
                ) {
                    itemsIndexed(state.results) { i, result ->
                        NoteResultRow(result = result, modifier = Modifier.groupMember(i, state.results.size))
                    }
                }
            }
        }
    }
}

/** One search hit, a member of the results group. */
@Composable
private fun NoteResultRow(
    result: com.forge.app.data.db.dao.LoggedExerciseDao.NoteSearchResult,
    modifier: Modifier = Modifier
) {
    val exerciseName = Program.exerciseDisplayName(result.exerciseId, result.swappedName)
    Column(
        modifier = modifier.padding(horizontal = ROW_H, vertical = 14.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        Text(exerciseName, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.Bold)
        Text(
            result.note ?: "",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onBackground
        )
        Text(
            formatDate(result.sessionStartedAt),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}

private val dateFormat = SimpleDateFormat("MMM d, yyyy", Locale.getDefault())
private fun formatDate(epochMs: Long) = dateFormat.format(Date(epochMs))
