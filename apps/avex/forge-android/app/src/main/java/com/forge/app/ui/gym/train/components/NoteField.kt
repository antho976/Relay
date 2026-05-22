package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay

/**
 * Multiline note field for an exercise. Local state for snappy typing; persists
 * via [onCommit] after a 500 ms debounce of inactivity. This avoids one DB write
 * per keystroke (which would also re-trigger the session's Flow and rebuild the
 * exercise list mid-typing).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NoteField(
    initialNote: String?,
    onCommit: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    // Seed once and let the local state diverge from initialNote during typing —
    // otherwise a refresh between keystrokes would yank what the user is typing.
    var text by rememberSaveable { mutableStateOf(initialNote.orEmpty()) }
    val baseline = remember { initialNote.orEmpty() }

    LaunchedEffect(text) {
        if (text != baseline) {
            delay(500)
            onCommit(text)
        }
    }

    OutlinedTextField(
        value = text,
        onValueChange = { text = it },
        modifier = modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp),
        label = { Text("Notes") },
        placeholder = {
            Text(
                "Form cues, how it felt, what to try next time…",
                style = MaterialTheme.typography.bodySmall
            )
        },
        minLines = 1,
        maxLines = 4
    )
}
