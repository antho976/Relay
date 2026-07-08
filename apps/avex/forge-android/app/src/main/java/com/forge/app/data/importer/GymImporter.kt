package com.forge.app.data.importer

import java.time.LocalDate
import java.time.LocalDateTime
import java.time.ZoneId
import java.time.format.DateTimeFormatter

/**
 * A parser for one gym app's export format (#GYMAP-17). Each recognises its own file cheaply
 * ([canParse]) and turns it into the app-neutral [ImportedSession] model; [WorkoutImportRepository]
 * owns detection order and the DB write.
 */
interface GymImporter {
    val source: ImportSource

    /** Cheap recognition from the raw text (usually a header sniff). */
    fun canParse(text: String): Boolean

    /**
     * Parse the file into sessions. [assumeKg] is the fallback weight unit for rows a source leaves
     * unitless (Strong without a unit column, a bare spreadsheet) — set to the user's current unit.
     */
    fun parse(text: String, assumeKg: Boolean): List<ImportedSession>
}

/** Shared parsing helpers: unit conversion, date parsing, header-indexed CSV row access. */
object ImportParsing {

    /** Reuse the app's single kg↔lb conversion (WeightFormatter) rather than a private constant. */
    fun kgToLb(kg: Double): Double = com.forge.app.domain.units.fromDisplayWeight(kg, useKg = true)

    /** Round to 0.1 to avoid float dust from kg→lb conversion showing as "20.000001 lb". */
    fun roundWeight(lb: Double): Double = Math.round(lb * 10.0) / 10.0

    private val DATE_TIME_FORMATS = listOf(
        "yyyy-MM-dd HH:mm:ss", "yyyy-MM-dd'T'HH:mm:ss", "yyyy-MM-dd'T'HH:mm:ss'Z'",
        "yyyy-MM-dd HH:mm", "yyyy/MM/dd HH:mm:ss",
        "d MMM yyyy, HH:mm", "dd MMM yyyy, HH:mm", "d MMM yyyy HH:mm", "dd MMM yyyy HH:mm",
        "MMM d yyyy, HH:mm", "EEE, dd MMM yyyy HH:mm:ss"
    ).map { DateTimeFormatter.ofPattern(it, java.util.Locale.ENGLISH) }

    // Numeric d/M/y and M/d/y are mutually ambiguous ("04/05/2024" is either), so try the device
    // locale's convention FIRST — an export made on this device then parses to the date the user
    // meant, with the other order kept as a cross-locale fallback. Month-first is US + its Pacific
    // territories and the Philippines; everywhere else is day-first.
    private val monthFirstLocale: Boolean =
        java.util.Locale.getDefault().country in setOf("US", "PH", "FM", "MH", "PW", "GU", "AS")
    private val ambiguousSlashFormats: List<String> =
        if (monthFirstLocale) listOf("MM/dd/yyyy", "dd/MM/yyyy") else listOf("dd/MM/yyyy", "MM/dd/yyyy")

    private val DATE_ONLY_FORMATS = (
        listOf("yyyy-MM-dd", "yyyy/MM/dd") + ambiguousSlashFormats +
            listOf("d MMM yyyy", "dd MMM yyyy", "MMM d, yyyy", "MMMM d, yyyy")
        ).map { DateTimeFormatter.ofPattern(it, java.util.Locale.ENGLISH) }

    private val zone: ZoneId get() = ZoneId.systemDefault()

    /** Parse a source date/time string to epoch millis, or null if no known format matches. */
    fun parseEpochMillis(raw: String): Long? {
        val s = raw.trim()
        if (s.isBlank()) return null
        for (fmt in DATE_TIME_FORMATS) {
            try {
                return LocalDateTime.parse(s, fmt).atZone(zone).toInstant().toEpochMilli()
            } catch (_: Exception) { /* try next */ }
        }
        for (fmt in DATE_ONLY_FORMATS) {
            try {
                return LocalDate.parse(s, fmt).atStartOfDay(zone).toInstant().toEpochMilli()
            } catch (_: Exception) { /* try next */ }
        }
        return null
    }

    /** Reps can arrive as "10", "10.0", or empty; parse leniently to an int (null when not a count). */
    fun parseReps(raw: String): Int? = raw.trim().toDoubleOrNull()?.toInt()?.takeIf { it >= 0 }

    /** Weight can be "", "0", "45.5", "45.5 kg", or "100,5" (European exports use a comma decimal —
     *  see [CsvParser]'s `;` handling); strip a unit suffix, normalise the separator, and parse. */
    fun parseWeight(raw: String): Double? {
        val s = raw.trim().lowercase().removeSuffix("kg").removeSuffix("kgs").removeSuffix("lb")
            .removeSuffix("lbs").trim()
        if (s.isEmpty()) return null
        // "1,234.5" → comma is a thousands separator; "100,5" → comma is the decimal point.
        val normalised = when {
            s.contains(',') && s.contains('.') -> s.replace(",", "")
            s.contains(',') -> s.replace(',', '.')
            else -> s
        }
        return normalised.toDoubleOrNull()
    }

    /** The first non-blank line, lowercased — the cheap header sniff each CSV importer's canParse uses. */
    fun firstLine(text: String): String =
        text.lineSequence().firstOrNull { it.isNotBlank() }?.lowercase().orEmpty()

    /**
     * Resolve whether a row's weight is in kg, from the strongest signal available: a per-row unit
     * cell wins, then a unit baked into the weight header ("Weight (kgs)"), else the caller's
     * [assumeKg] fallback. Shared so FitNotes / the generic CSV resolve units identically.
     */
    fun rowIsKg(
        row: List<String>, unitCol: Int?, weightHeaderKg: Boolean, weightHeaderLb: Boolean, assumeKg: Boolean
    ): Boolean = when {
        unitCol != null -> at(row, unitCol).lowercase().startsWith("kg")
        weightHeaderKg -> true
        weightHeaderLb -> false
        else -> assumeKg
    }

    /** Build a lowercased-header → column-index map from a CSV header row. */
    fun headerIndex(header: List<String>): Map<String, Int> =
        header.mapIndexed { i, h -> h.trim().lowercase() to i }.toMap()

    /** Value at the column whose header equals [name] (lowercased), or "" if absent/short row. */
    fun cell(row: List<String>, index: Map<String, Int>, name: String): String {
        val i = index[name] ?: return ""
        return row.getOrNull(i)?.trim() ?: ""
    }

    private val HM_REGEX = Regex("(?:(\\d+)\\s*h)?\\s*(?:(\\d+)\\s*m)?", RegexOption.IGNORE_CASE)

    /** Parse a duration like "1h 15m", "45m", "45 min", or "1:15:00" to millis, or null. */
    fun parseDurationToMillis(raw: String): Long? {
        val s = raw.trim()
        if (s.isBlank()) return null
        if (s.contains(':')) {
            val parts = s.split(':').mapNotNull { it.trim().toIntOrNull() }
            return when (parts.size) {
                3 -> (parts[0] * 3600L + parts[1] * 60L + parts[2]) * 1000L
                2 -> (parts[0] * 60L + parts[1]) * 1000L
                else -> null
            }
        }
        val m = HM_REGEX.find(s) ?: return null
        val h = m.groupValues[1].toIntOrNull() ?: 0
        val min = m.groupValues[2].toIntOrNull() ?: 0
        val total = h * 3600L + min * 60L
        return if (total > 0) total * 1000L else null
    }

    /** First header index whose name contains any of [needles], or null. */
    fun findCol(index: Map<String, Int>, vararg needles: String): Int? =
        index.entries.firstOrNull { e -> needles.any { e.key.contains(it) } }?.value

    fun at(row: List<String>, i: Int?): String = i?.let { row.getOrNull(it)?.trim() } ?: ""
}
