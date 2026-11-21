package com.forge.app.domain.units

/**
 * Elapsed seconds as a clock: "0:45", "5:07", then "1:05:12" past the hour. ASCII digits always,
 * padded by hand: `"%02d".format` writes the device locale's digits, so an Arabic or Persian
 * device printed a timer's seconds in its own digits beside ASCII minutes. Every running clock in
 * the app (rest, workout time, the session notification) reads through here.
 */
fun clockText(seconds: Long): String {
    val s = seconds.coerceAtLeast(0)
    val two = { n: Long -> n.toString().padStart(2, '0') }
    return if (s >= 3600) "${s / 3600}:${two(s % 3600 / 60)}:${two(s % 60)}" else "${s / 60}:${two(s % 60)}"
}
