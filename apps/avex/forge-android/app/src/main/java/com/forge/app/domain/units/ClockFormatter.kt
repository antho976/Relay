package com.forge.app.domain.units

/**
 * Clock formatting for Settings → Format → Clock (12h/24h). The mirror of [formatWeight] for times:
 * every clock the app draws asks here instead of hard-coding "h:mm a", so the setting is the one
 * place the choice lives.
 *
 * The toggle used to be saved and previewed and then read by nothing: cardio times, the watch
 * import rows and the PDF export all printed 12-hour, and the reminder and quiet-hours steppers all
 * printed 24-hour, whatever the user picked (2026-09-26 audit, "Settings that do nothing").
 * Components read `LocalForgeSettings.current.timeFormat24h`; non-UI callers read
 * `SettingsRepository.timeFormat24h`.
 */

/** A `DateTimeFormatter`/`SimpleDateFormat` pattern for a time of day: "18:30" or "6:30 PM". */
fun clockPattern(use24h: Boolean): String = if (use24h) "HH:mm" else "h:mm a"

/**
 * A whole hour of the day (0..23) as a clock reading: "07:00" in 24h, "7 AM" in 12h. For the hour
 * steppers (training reminder, quiet hours), which only ever move in whole hours.
 */
fun formatClockHour(hour: Int, use24h: Boolean): String {
    val h = ((hour % 24) + 24) % 24
    if (use24h) return "${h.toString().padStart(2, '0')}:00"
    val h12 = if (h % 12 == 0) 12 else h % 12
    return "$h12 ${if (h < 12) "AM" else "PM"}"
}

/**
 * A whole hour in the compact form a caption range uses: "4am" in 12h, "04:00" in 24h, so the
 * busiest-hour label reads "4am–5am" or "04:00–05:00".
 */
fun formatClockHourShort(hour: Int, use24h: Boolean): String {
    if (use24h) return formatClockHour(hour, use24h = true)
    val h = ((hour % 24) + 24) % 24
    val h12 = if (h % 12 == 0) 12 else h % 12
    return "$h12${if (h < 12) "am" else "pm"}"
}
