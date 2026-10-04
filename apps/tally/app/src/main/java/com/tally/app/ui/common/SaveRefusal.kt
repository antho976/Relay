package com.tally.app.ui.common

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp

/**
 * Why a save did not happen, said where it was pressed: directly over the save action, in the
 * quiet error line, and to TalkBack through a polite live region. The fields keep their red edge
 * and their own reason, but on a long form those sit a screen above the button; this line is
 * never out of sight of the press. [attempt] counts the presses, so a second refused press is
 * said again. Takes no height while there is nothing to say.
 */
@Composable
fun SaveRefusal(line: String?, attempt: Int, modifier: Modifier = Modifier) {
    Box(
        modifier
            .fillMaxWidth()
            .semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite },
    ) {
        if (line != null) {
            key(attempt) {
                Text(
                    line,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.fillMaxWidth().padding(start = ROW_PAD, end = ROW_PAD, bottom = 10.dp),
                )
            }
        }
    }
}

/** "Not saved · Name the bill · Enter an amount": what stops the save, in form order; null when nothing does. */
fun refusalLine(problems: List<String?>): String? {
    val found = problems.filterNotNull()
    return if (found.isEmpty()) null else (listOf("Not saved") + found).joinToString(" · ")
}
