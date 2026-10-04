package com.margin.app.ui.common

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import com.margin.core.MoneyFormatter
import java.util.Locale

/** The owner's currency formatter, provided once at the root. Main-thread only, like the UI. */
val LocalMoney = staticCompositionLocalOf { MoneyFormatter("CAD", Locale.getDefault()) }

@Composable
fun rememberMoney(currency: String): MoneyFormatter {
    val locale = Locale.getDefault()
    return remember(currency, locale) { MoneyFormatter(currency, locale) }
}
