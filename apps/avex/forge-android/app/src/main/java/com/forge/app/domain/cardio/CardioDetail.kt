package com.forge.app.domain.cardio

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.formatDistance

/**
 * The ordered detail chips for one non-rest cardio entry — duration · intervals · distance · pace ·
 * (effort) · HR zone. The single source of truth shared by the week-detail row
 * ([com.forge.app.ui.cardio.components.CardioWeekDetailComponents]) and the PDF export
 * ([com.forge.app.data.repo.PdfExportRepository]) so the two can't drift when a field is added.
 * No kcal chip — calorie estimates were dropped from cardio (GYMAP-3); real burn arrives with
 * the watch sync.
 *
 * [includeEffort] inlines the self-rated effort label (the history row renders that in its own
 * column instead, so it passes false); [useMiles] switches distance + pace to miles. Callers handle
 * rest-day entries themselves — this builds active-work chips.
 */
fun cardioDetailParts(
    entry: CardioEntry,
    includeEffort: Boolean = false,
    useMiles: Boolean = false
): List<String> = buildList {
    if (entry.durationMin > 0) add("${entry.durationMin} min")
    entry.intervalCount?.takeIf { it > 0 }?.let { add("$it intervals") }
    // formatDistance pins Locale.US so the '.' separator matches what the log form parses (it only
    // accepts '.'); a comma-decimal device locale would otherwise render "5,0 km".
    entry.distanceKm?.let { add(formatDistance(it, useMiles)) }
    pacePerUnit(entry.durationMin, entry.distanceKm, useMiles)?.let { add("$it /${distanceUnitLabel(useMiles)}") }
    if (includeEffort) CardioEffort.fromCode(entry.effort)?.let { add(it.displayName) }
    entry.hrZone?.let { add("HR Z$it") }
}
