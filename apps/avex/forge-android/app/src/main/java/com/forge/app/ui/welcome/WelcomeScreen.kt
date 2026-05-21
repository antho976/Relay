package com.forge.app.ui.welcome

import androidx.compose.runtime.Composable
import com.forge.app.ui.common.PlaceholderAction
import com.forge.app.ui.common.PlaceholderScreen

@Composable
fun WelcomeScreen(onFinished: () -> Unit) {
    PlaceholderScreen(
        title = "FORGE",
        subtitle = "Welcome / onboarding lands here in a later phase."
    ) {
        PlaceholderAction(label = "Skip onboarding", onClick = onFinished)
    }
}
