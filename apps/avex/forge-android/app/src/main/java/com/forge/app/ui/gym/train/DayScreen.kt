package com.forge.app.ui.gym.train

import androidx.compose.runtime.Composable
import com.forge.app.ui.common.PlaceholderScreen

@Composable
fun DayScreen(dayKey: String, onBack: () -> Unit) {
    PlaceholderScreen(
        title = "Day · $dayKey",
        subtitle = "Phase 3 builds the live workout screen here: warmup gate, exercise list, set logging, rest timer.",
        onBack = onBack
    )
}
