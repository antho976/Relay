package com.forge.app.ui.gym.session

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.session.SessionType
import com.forge.app.ui.common.clickableLabeled

/**
 * Tiny input dialog (§3 modal) for tagging what kind of session a finished workout was.
 *
 * This is Coach v3 A1's session-type writer: the enum has existed since #109 with no way to set
 * anything but "normal". The picker lives on session DETAIL rather than the live day screen —
 * `ui/gym/train` is untouchable (§14), and retro-tagging is the better moment anyway: you know it
 * was a test day once it's done, and the engine only reads finished sessions.
 */
@Composable
internal fun SessionTypeDialog(
    current: String,
    onPick: (String) -> Unit,
    onDismiss: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accentWash = MaterialTheme.colorScheme.primaryContainer

    AlertDialog(
        onDismissRequest = onDismiss,
        confirmButton = {},
        containerColor = MaterialTheme.colorScheme.surface,
        title = {
            Text(
                "Session type",
                style = MaterialTheme.typography.titleMedium,
                color = onBg
            )
        },
        text = {
            Column(Modifier.fillMaxWidth()) {
                // One caption for the whole dialog (§4.3): the consequence, not the mechanics.
                Text(
                    "Test and technique days are left out of your progress and fatigue reads.",
                    style = MaterialTheme.typography.bodySmall,
                    color = muted,
                    fontSize = 12.sp
                )
                Spacer(Modifier.height(12.dp))
                SessionType.entries.forEach { type ->
                    val selected = type.key == current
                    // Accent-wash pick, no checkbox — the ExerciseLibraryPicker singleSelect idiom.
                    Text(
                        text = label(type),
                        style = MaterialTheme.typography.bodyLarge,
                        color = onBg,
                        fontWeight = if (selected) FontWeight.Medium else FontWeight.Normal,
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(12.dp))
                            .background(if (selected) accentWash else androidx.compose.ui.graphics.Color.Transparent)
                            .clickableLabeled(label(type)) {
                                onPick(type.key)
                                onDismiss()
                            }
                            .padding(horizontal = 14.dp, vertical = 12.dp)
                    )
                }
            }
        },
        dismissButton = {
            Text(
                "Close",
                style = MaterialTheme.typography.bodyMedium,
                color = muted,
                modifier = Modifier
                    .clip(RoundedCornerShape(50))
                    .clickableLabeled("Close") { onDismiss() }
                    .padding(horizontal = 12.dp, vertical = 8.dp)
            )
        }
    )
}

/** Sentence-case content labels; the mono UPPERCASE pill form stays [SessionType.pillLabel]'s job. */
private fun label(type: SessionType): String = when (type) {
    SessionType.NORMAL -> "Normal training"
    SessionType.DELOAD -> "Deload"
    SessionType.TEST -> "Test day"
    SessionType.TECHNIQUE -> "Technique"
    SessionType.FIRST_BACK -> "First back"
}
