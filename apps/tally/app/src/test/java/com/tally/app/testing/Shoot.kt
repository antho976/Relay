package com.tally.app.testing

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.junit4.ComposeContentTestRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.Density
import com.github.takahirom.roborazzi.RoborazziOptions
import com.github.takahirom.roborazzi.captureRoboImage
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.theme.TallyTheme
import com.tally.core.MoneyFormatter
import java.util.Locale

/** 0.1% absorbs font-rasterisation jitter between machines, nothing structural. */
val screenshotOptions = RoborazziOptions(
    compareOptions = RoborazziOptions.CompareOptions(changeThreshold = 0.001f),
)

/**
 * Renders [content] the way the app does (theme, ground, CAD in English Canada) at [fontScale]
 * and captures it as `src/test/screenshots/<name>.png`. Every screen is shot at 100% and 200%:
 * clipping at 200% is the failure no static rule can see.
 */
fun ComposeContentTestRule.shoot(
    name: String,
    fontScale: Float = 1f,
    amoled: Boolean = false,
    accentEnabled: Boolean = true,
    content: @Composable () -> Unit,
) {
    setContent {
        TallyTheme(accent = Color(0xFFD4761F), accentEnabled = accentEnabled, amoled = amoled) {
            CompositionLocalProvider(
                LocalDensity provides Density(LocalDensity.current.density, fontScale),
                LocalMoney provides MoneyFormatter("CAD", Locale.CANADA),
            ) {
                Box(Modifier.fillMaxSize()) { content() }
            }
        }
    }
    onRoot().captureRoboImage("src/test/screenshots/$name.png", screenshotOptions)
}
