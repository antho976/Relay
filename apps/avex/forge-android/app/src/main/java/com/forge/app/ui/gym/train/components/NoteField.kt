package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.settings.SettingsViewModel
import kotlinx.coroutines.delay
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.rememberUpdatedState

/**
 * Multiline note field for an exercise. Includes quick-insert template chips (#113)
 * and a "Pin as cue" button (#112). Local state with 500ms commit debounce.
 *
 * Template behavior: each template starts its own line in the note with the cursor parked
 * right after it, ready to type the answer; a chip disappears once its prompt is already
 * in the note, so templates can't double-insert or mash together.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun NoteField(
    initialNote: String?,
    onCommit: (String) -> Unit,
    onPinNote: (String) -> Unit = {},
    currentPinnedNote: String = "",
    showTemplates: Boolean = true,
    modifier: Modifier = Modifier,
    settingsViewModel: SettingsViewModel = hiltViewModel()
) {
    var field by rememberSaveable(stateSaver = TextFieldValue.Saver) {
        mutableStateOf(TextFieldValue(initialNote.orEmpty(), TextRange(initialNote.orEmpty().length)))
    }
    // What the database is known to hold. Starts at the incoming note and advances on each commit,
    // so the dispose commit below can tell "unsaved" from "already written".
    val committed = remember { mutableStateOf(initialNote.orEmpty()) }
    val latestText = rememberUpdatedState(field.text)
    val latestCommit = rememberUpdatedState(onCommit)
    val settingsState by settingsViewModel.state.collectAsStateWithLifecycle()
    val templates = remember(settingsState.noteTemplates, field.text) {
        settingsState.noteTemplates
            .map { it.trim() }
            .filter { it.isNotBlank() }
            .distinctBy { it.lowercase() }
            .filterNot { field.text.contains(it, ignoreCase = true) }
            .sortedBy { it.lowercase() }
    }

    LaunchedEffect(field.text) {
        if (field.text != committed.value) {
            delay(500)
            latestCommit.value(field.text)
            committed.value = field.text
        }
    }

    // The debounce above was the ONLY commit path, so anything typed in the last 500ms died with
    // the composition — and this field's composition ends routinely, not exceptionally: the card
    // auto-collapses when the final set is logged, and "move to next" unmounts it outright. Typing
    // a note and immediately logging the last set therefore discarded the note, which is precisely
    // when a note is most likely to be written.
    DisposableEffect(Unit) {
        onDispose {
            val text = latestText.value
            if (text != committed.value) latestCommit.value(text)
        }
    }

    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        if (showTemplates && templates.isNotEmpty()) {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                templates.forEach { template ->
                    FilterChip(
                        selected = false,
                        onClick = {
                            val next =
                                if (field.text.isBlank()) "$template "
                                else field.text.trimEnd() + "\n" + template + " "
                            field = TextFieldValue(next, TextRange(next.length))
                        },
                        label = { Text(template, style = MaterialTheme.typography.labelSmall) },
                        colors = FilterChipDefaults.filterChipColors(
                            containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)
                        )
                    )
                }
            }
        }

        OutlinedTextField(
            value = field,
            onValueChange = { field = it },
            modifier = modifier.fillMaxWidth().heightIn(min = 56.dp),
            label = { Text("Notes") },
            placeholder = {
                Text("Form cues, how it felt, what to try next time…", style = MaterialTheme.typography.bodySmall)
            },
            minLines = 1,
            maxLines = 4
        )

        if (field.text.isNotBlank()) {
            Row(horizontalArrangement = Arrangement.End, modifier = Modifier.fillMaxWidth()) {
                TextButton(onClick = { onPinNote(if (currentPinnedNote == field.text.trim()) "" else field.text.trim()) }) {
                    Text(if (currentPinnedNote == field.text.trim()) "Unpin cue" else "Pin as cue")
                }
            }
        }
    }
}
