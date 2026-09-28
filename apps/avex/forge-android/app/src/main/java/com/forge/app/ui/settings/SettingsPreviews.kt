package com.forge.app.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.unit.dp

/*
 * Live previews: the two pages whose settings change how the whole app LOOKS open on a sample of
 * the result, redrawn the instant a control below moves. They read the live theme and the live
 * units, so the preview can never disagree with what the setting will do.
 */

/** Seven days of sessions for the sample chart. Illustrative, and read as such to TalkBack. */
private val SAMPLE_WEEK = listOf(0.55f, 0f, 1f, 0.55f, 0f, 0.8f, 0.3f)

/**
 * A slice of Home in the current theme: the page ground (black under AMOLED), the serif voice, the
 * accent on the week's bars and the start button. Monochrome shows the neutral that replaces it.
 */
@Composable
internal fun ThemePreview() {
    val scheme = MaterialTheme.colorScheme
    Column {
        Column(
            Modifier
                .fillMaxWidth()
                .clip(KitGroupShape)
                .background(scheme.background)
                .border(1.dp, scheme.outline, KitGroupShape)
                .clearAndSetSemantics { contentDescription = "Preview of Avex in your current look" }
                .padding(20.dp)
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text("Pull B", style = MaterialTheme.typography.headlineMedium, color = scheme.onBackground)
                    Text("Today · 5 exercises", style = MaterialTheme.typography.bodySmall, color = scheme.onSurfaceVariant)
                }
                Box(
                    Modifier
                        .clip(RoundedCornerShape(50))
                        .background(scheme.primary)
                        .padding(horizontal = 18.dp, vertical = 10.dp)
                ) {
                    Text("Start", style = MaterialTheme.typography.labelLarge, color = scheme.onPrimary)
                }
            }
            Spacer(Modifier.height(20.dp))
            Row(
                Modifier.fillMaxWidth().height(52.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalAlignment = Alignment.Bottom
            ) {
                SAMPLE_WEEK.forEach { f ->
                    Box(
                        Modifier
                            .weight(1f)
                            .fillMaxHeight()
                            .clip(RoundedCornerShape(6.dp))
                            .background(scheme.outline.copy(alpha = 0.35f))
                    ) {
                        if (f > 0f) {
                            Box(
                                Modifier
                                    .fillMaxWidth()
                                    .fillMaxHeight(f)
                                    .align(Alignment.BottomCenter)
                                    .background(scheme.primary)
                            )
                        }
                    }
                }
            }
        }
        SettingsGroupFooter("A sample of Home in your current look.")
    }
}

/** One sample of every unit on the Units & format page, in the serif figure voice. */
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
    SettingsGroup(footer = "Every number in Avex follows these.") {
        SettingsGroupBlock(padding = PaddingValues(horizontal = 20.dp, vertical = 20.dp)) {
            cells.chunked(2).forEachIndexed { i, pair ->
                if (i > 0) Spacer(Modifier.height(18.dp))
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    pair.forEach { (label, value) ->
                        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                            Text(value, style = MaterialTheme.typography.headlineSmall, color = MaterialTheme.colorScheme.onSurface)
                            Text(label, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                }
            }
        }
    }
}
