package com.forge.app.domain.units

import java.util.Locale

/**
 * Formatting + parsing for timed-hold durations (GYMAP-51) — planks, dead hangs, wall sits.
 * Durations are stored as whole seconds (`logged_set.duration_seconds`); these convert at the
 * display/input boundary only. The mirror of [WeightFormatter]/[DistanceFormatter] for holds.
 *
 * Display is `m:ss` (no leading zero on minutes, seconds always two digits) — a stopwatch reading,
 * pinned to Locale.US so the ':' and digits never localise and always round-trip [parseHold].
 */

const val MAX_HOLD_SECONDS: Int = 3600 // 1 h — far above any real plank/hang; also the input ceiling.

/** Whole seconds as a stopwatch reading: 45 → "0:45", 90 → "1:30", 605 → "10:05". Negatives clamp to 0. */
fun formatHold(totalSeconds: Int): String {
    val s = totalSeconds.coerceAtLeast(0)
    return String.format(Locale.US, "%d:%02d", s / 60, s % 60)
}

/**
 * A compact hold label with a unit suffix, for read surfaces mixing holds with rep sets — under a
 * minute reads "45s"; a minute or more reads "1:30". Negatives clamp to 0.
 */
fun formatHoldLabel(totalSeconds: Int): String {
    val s = totalSeconds.coerceAtLeast(0)
    return if (s < 60) "${s}s" else formatHold(s)
}

/**
 * Parses a user-entered hold time to whole seconds, or null if blank/unparseable.
 *  - "1:30" → 90 (mm:ss; the seconds part is taken mod 60 so "1:90" can't out-mean "2:30")
 *  - "45"   → 45 (a bare number is seconds — the natural reading of a stopwatch field)
 *  - "45s"  → 45 (tolerates the suffix [formatHoldLabel] emits)
 * Result is clamped to [0, MAX_HOLD_SECONDS]. 0 parses to 0 (blank vs "0" is the caller's call).
 */
fun parseHold(input: String): Int? {
    val cleaned = input.trim().lowercase().removeSuffix("s").trim()
    if (cleaned.isEmpty()) return null
    val seconds = if (":" in cleaned) {
        val parts = cleaned.split(":")
        if (parts.size != 2) return null
        val minutes = parts[0].ifEmpty { "0" }.toIntOrNull() ?: return null
        val secs = parts[1].toIntOrNull() ?: return null
        if (minutes < 0 || secs < 0) return null
        minutes * 60 + (secs % 60)
    } else {
        cleaned.toIntOrNull() ?: return null
    }
    return seconds.coerceIn(0, MAX_HOLD_SECONDS)
}
