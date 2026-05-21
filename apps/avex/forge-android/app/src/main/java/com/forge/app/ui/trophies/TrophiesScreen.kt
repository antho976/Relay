package com.forge.app.ui.trophies

import androidx.compose.runtime.Composable
import com.forge.app.ui.common.PlaceholderScreen

@Composable
fun TrophiesScreen(onBack: () -> Unit) {
    PlaceholderScreen(
        title = "Trophies",
        subtitle = "Phase 6 puts the trophy catalog and unlock animations here.",
        onBack = onBack
    )
}
