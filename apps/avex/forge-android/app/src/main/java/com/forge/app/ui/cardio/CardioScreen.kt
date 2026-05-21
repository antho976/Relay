package com.forge.app.ui.cardio

import androidx.compose.runtime.Composable
import com.forge.app.ui.common.PlaceholderScreen

@Composable
fun CardioScreen(onBack: () -> Unit) {
    PlaceholderScreen(
        title = "Cardio",
        subtitle = "Phase 7 puts the cardio log here (run / walk / treadmill / rest).",
        onBack = onBack
    )
}
