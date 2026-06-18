package com.forge.app.ui.settings

import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

// Small shared formatters for the settings screens, so the same strings aren't hand-rolled in
// SettingsDialogs / SettingsScreen / SettingsViewModel (one place to change wording or format).

/** "N progress photo(s)" — pluralized count used by the data-stake line and the factory-reset warning. */
internal fun photoCountLabel(n: Int): String = "$n progress photo${if (n == 1) "" else "s"}"

/** Short "MMM d" date for a millis timestamp (e.g. the progress-photo "last" line). */
internal fun formatShortDate(ms: Long): String =
    SimpleDateFormat("MMM d", Locale.getDefault()).format(Date(ms))

/** Medium "MMM d, yyyy" date for a millis timestamp (e.g. the last auto-backup date). */
internal fun formatMediumDate(ms: Long): String =
    SimpleDateFormat("MMM d, yyyy", Locale.getDefault()).format(Date(ms))
