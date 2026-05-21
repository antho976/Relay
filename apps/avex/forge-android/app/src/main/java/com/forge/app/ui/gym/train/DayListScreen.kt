package com.forge.app.ui.gym.train

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.PlaceholderAction
import com.forge.app.ui.common.PlaceholderScreen

@Composable
fun DayListScreen(
    onBack: () -> Unit,
    onOpenDay: (String) -> Unit
) {
    PlaceholderScreen(
        title = "Gym",
        subtitle = "Phase 3 puts the 4 day cards (Upper A / Lower A / Upper B / Lower B) here.",
        onBack = onBack
    ) {
        Column(
            Modifier.fillMaxWidth(),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            PlaceholderAction(label = "Upper A", onClick = { onOpenDay("upper-a") })
            PlaceholderAction(label = "Lower A", onClick = { onOpenDay("lower-a") })
            PlaceholderAction(label = "Upper B", onClick = { onOpenDay("upper-b") })
            PlaceholderAction(label = "Lower B", onClick = { onOpenDay("lower-b") })
        }
    }
}
