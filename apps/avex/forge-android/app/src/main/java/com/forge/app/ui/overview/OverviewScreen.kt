package com.forge.app.ui.overview

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.PlaceholderAction
import com.forge.app.ui.common.PlaceholderScreen

@Composable
fun OverviewScreen(
    onGoToGym: () -> Unit,
    onGoToCardio: () -> Unit,
    onGoToTrophies: () -> Unit
) {
    PlaceholderScreen(
        title = "FORGE",
        subtitle = "Overview hub. Phase 4 fills this with weekly stats and tiles."
    ) {
        Column(
            Modifier.fillMaxWidth(),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            PlaceholderAction(label = "Gym", onClick = onGoToGym)
            PlaceholderAction(label = "Cardio", onClick = onGoToCardio)
            PlaceholderAction(label = "Trophies", onClick = onGoToTrophies)
        }
    }
}
