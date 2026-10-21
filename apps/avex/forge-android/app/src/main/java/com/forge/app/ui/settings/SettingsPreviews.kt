package com.forge.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.theme.OpenType

/*
 * The live preview the Units & format page opens on: a sample of every unit, redrawn the instant a
 * control below moves. It reads the live units, so it can never disagree with what the setting does.
 * (Appearance shows itself through its own swatches, theme chips and icon strip, Relay design "sa".)
 */

/**
 * One sample of every unit on the Units & format page, each as a reading: the mono label over the
 * serif figure, two to a row, open on the page.
 */
@Composable
internal fun FormatPreview(state: SettingsUiState) {
    val time = java.time.LocalTime.of(18, 30).format(
        java.time.format.DateTimeFormatter.ofPattern(com.forge.app.domain.units.clockPattern(state.timeFormat24h))
    )
    val cells = listOf(
        "Weight" to com.forge.app.domain.units.formatWeight(135.0, state.weightUnit),
        "Distance" to com.forge.app.domain.units.formatDistance(5.0, state.useMiles),
        "Length" to com.forge.app.domain.units.formatLength(90.0, state.useCm),
        "Clock" to time
    )
    Column {
        cells.chunked(2).forEachIndexed { i, pair ->
            if (i > 0) Spacer(Modifier.height(24.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                pair.forEach { (label, value) ->
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        OpenLabel(label)
                        Text(value, style = OpenType.figureRow, color = MaterialTheme.colorScheme.onBackground)
                    }
                }
            }
        }
        SettingsGroupFooter("Every number in Avex follows these.")
    }
}
