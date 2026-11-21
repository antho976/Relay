package com.forge.app.ui.common

import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.ui.platform.LocalConfiguration
import java.util.Locale

/**
 * The locale to format dates and names with, read from the COMPOSITION rather than from the process.
 *
 * `Locale.getDefault()` is a process-wide static. Read inside a composable it is right at the moment
 * it runs and never again: nothing in the composition depends on it, so when the locale changes
 * under a composition that survives — a per-app language pick (Android 13+), or any configuration
 * change this window absorbs rather than restarts on — every month name, weekday letter and
 * formatted date keeps the old language while the screen around it updates. Compose ships
 * `NonObservableLocale` as an ERROR for exactly this, which is how the thirteen call sites that had
 * it were found.
 *
 * [LocalConfiguration] is a composition local, so reading it SUBSCRIBES: a locale change recomposes
 * the readers and they reformat. A `LocaleList` is empty only in configurations Android does not
 * produce, and [Locale.ROOT] there is better than a crash on a date label.
 */
@Composable
@ReadOnlyComposable
fun currentLocale(): Locale {
    val locales = LocalConfiguration.current.locales
    return if (locales.isEmpty) Locale.ROOT else locales[0]
}
