package com.forge.app.ui.common

/**
 * "1 month ago" / "3 months ago" / "1 year ago" / "2 years ago" — the single relative-age phrase
 * for the "on this day" surfaces (Overview card + Profile block), so the two can't drift. Uppercase
 * at the call site when an eyebrow label needs it.
 */
fun monthsAgoPhrase(monthsAgo: Int): String = when {
    monthsAgo >= 12 -> { val years = monthsAgo / 12; "$years year${if (years == 1) "" else "s"} ago" }
    monthsAgo <= 1 -> "1 month ago"
    else -> "$monthsAgo months ago"
}
