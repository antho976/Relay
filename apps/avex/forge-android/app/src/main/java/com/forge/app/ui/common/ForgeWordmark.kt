package com.forge.app.ui.common

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp

/**
 * A process-wide "return to the Home tab" action, provided once by ForgeNavHost. It lets the shared
 * [ForgeWordmark] send the user home from any screen — however deep — without threading a callback
 * through every composable in between. Defaults to a no-op so previews / tests don't need it.
 */
val LocalGoHome = staticCompositionLocalOf<() -> Unit> { {} }

/**
 * The "• Avex" wordmark that sits in nav top bars. It's now a shortcut Home button: tapping it
 * pops back to the hub and selects Home, so the user never has to back out screen-by-screen. By
 * default it fires [LocalGoHome]; the cardio in-screen overlays pass a custom [onClick] that also
 * closes the overlay first.
 */
@Composable
fun ForgeWordmark(
    onClick: () -> Unit = LocalGoHome.current,
    modifier: Modifier = Modifier
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val accent = MaterialTheme.colorScheme.secondary
    Row(
        modifier = modifier.clickableLabeled("Go to Home", onClick = onClick),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        Text("•", style = MaterialTheme.typography.bodyMedium, color = accent)
        Text("Avex", style = MaterialTheme.typography.bodyMedium, color = onBg, fontStyle = FontStyle.Italic)
    }
}
