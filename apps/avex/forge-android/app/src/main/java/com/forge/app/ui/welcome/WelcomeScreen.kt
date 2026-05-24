package com.forge.app.ui.welcome

import androidx.compose.runtime.Composable
import com.forge.app.ui.onboarding.OnboardingScreen

/** WelcomeScreen now delegates to the real onboarding flow (#1). */
@Composable
fun WelcomeScreen(onFinished: () -> Unit) {
    OnboardingScreen(onFinished = onFinished)
}
