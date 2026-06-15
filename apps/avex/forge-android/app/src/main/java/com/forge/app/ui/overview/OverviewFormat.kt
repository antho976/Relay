package com.forge.app.ui.overview

/**
 * Session volume in lb, abbreviated past 1k — "850 lb" / "1.2k lb". Single source for the RECENT
 * row, the history list, and the history detail sheet so the three can't drift on the same format.
 * lb-only, matching the rest of the overview/history surfaces (they show stored lb directly).
 */
internal fun formatVolumeLb(volumeLb: Double): String =
    if (volumeLb >= 1000) "%.1fk lb".format(volumeLb / 1000) else "${volumeLb.toInt()} lb"
