package com.forge.app.domain.cardio

import com.forge.app.data.db.entities.CardioEntry
import java.util.Locale

/**
 * The ordered detail chips for one non-rest cardio entry — duration · intervals · distance · pace ·
 * (effort) · HR zone · est. kcal. The single source of truth shared by the history row
 * ([com.forge.app.ui.cardio.components.CardioEntryRow]) and the PDF export
 * ([com.forge.app.data.repo.PdfExportRepository]) so the two can't drift when a field is added.
 *
 * [includeEffort] inlines the self-rated effort label (the history row renders that in its own
 * column instead, so it passes false); [bodyweightLb] scales the kcal estimate and the chip is
 * omitted when it's null. Callers handle rest-day entries themselves — this builds active-work chips.
 */
fun cardioDetailParts(
    entry: CardioEntry,
    type: CardioType,
    bodyweightLb: Double?,
    includeEffort: Boolean = false
): List<String> = buildList {
    if (entry.durationMin > 0) add("${entry.durationMin} min")
    entry.intervalCount?.takeIf { it > 0 }?.let { add("$it intervals") }
    // Pin to Locale.US so the '.' separator matches what the log form parses (it only accepts '.');
    // a comma-decimal device locale would otherwise render "5,0 km".
    entry.distanceKm?.let { add(String.format(Locale.US, "%.1f km", it)) }
    pacePerKm(entry.durationMin, entry.distanceKm)?.let { add("$it /km") }
    if (includeEffort) CardioEffort.fromCode(entry.effort)?.let { add(it.displayName) }
    entry.hrZone?.let { add("HR Z$it") }
    CardioCalorieEstimator.estimate(type, entry.durationMin, CardioEffort.fromCode(entry.effort), bodyweightLb)
        ?.let { add("≈ $it kcal") }
}
