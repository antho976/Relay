package com.tally.app.ui.common

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.platform.LocalConfiguration
import com.tally.core.MoneyFormatter
import java.util.Locale

/** The owner's currency formatter, provided once at the root. Main-thread only, like the UI. */
val LocalMoney = staticCompositionLocalOf { MoneyFormatter("CAD", Locale.getDefault()) }

/**
 * The formatter for [currency] in the device's locale, read through the Configuration so a
 * language change re-formats every amount instead of leaving the old separators on screen.
 */
@Composable
fun rememberMoney(currency: String): MoneyFormatter {
    val locale: Locale = LocalConfiguration.current.locales[0]
    return remember(currency, locale) { MoneyFormatter(currency, locale) }
}
