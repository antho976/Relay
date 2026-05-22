package com.forge.app.ui.overview.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

/**
 * Suggests a deload week after [com.forge.app.ui.overview.state.OverviewUiState.DELOAD_THRESHOLD]
 * sessions. Tapping the action records the current session count, hiding the banner until
 * another 24 are logged.
 */
@Composable
fun DeloadBanner(
    sessionsSinceLast: Int,
    onMarkDeloaded: () -> Unit,
    modifier: Modifier = Modifier
) {
    val accent = MaterialTheme.colorScheme.primary
    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(accent.copy(alpha = 0.12f))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        Text(
            text = "DELOAD WEEK?",
            color = accent,
            fontWeight = FontWeight.Black,
            style = MaterialTheme.typography.labelLarge
        )
        Text(
            text = "$sessionsSinceLast workouts since your last deload. Your joints and CNS will thank you for a back-off week of lighter loads and lower volume.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurface
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.End
        ) {
            TextButton(onClick = onMarkDeloaded) {
                Text("I'll deload this week")
            }
        }
    }
}
