package com.forge.app.data.importer

import java.time.Instant
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

    /**
     * Rows that aren't workouts — cardio, coach goals. Only Avex's own export carries any, so this
     * defaults to empty and no other parser has to implement it.
     */
    fun parseExtras(text: String): ImportedExtras = ImportedExtras()

    /**
     * The export-format version the file declares, when the source versions its format at all.
     * A file from a NEWER format than this build knows is refused rather than parsed on a guess.
     * Null = the source doesn't version its exports, so there is nothing to check.
     */
    fun formatVersion(text: String): Int? = null
}

/** Shared parsing helpers: unit conversion, date parsing, header-indexed CSV row access. */
object ImportParsing {

    /** Reuse the app's single kg↔lb conversion (WeightFormatter) rather than a private constant. */
    fun kgToLb(kg: Double): Double = com.forge.app.domain.units.fromDisplayWeight(kg, useKg = true)

    /** Round to 0.1 to avoid float dust from kg→lb conversion showing as "20.000001 lb". */
    fun roundWeight(lb: Double): Double = Math.round(lb * 10.0) / 10.0

    /**
     * Zone-carrying formats, tried FIRST and read as an instant.
     *
     * The old list matched `"yyyy-MM-dd'T'HH:mm:ss'Z'"` — with the Z QUOTED, so it was a literal
     * character rather than the UTC designator. "2026-08-25T10:00:00Z" therefore parsed as 10:00
     * wall time and was then re-zoned into the device's zone, shifting every imported session by
     * the full local offset and, for anyone far enough east or west, onto the wrong calendar day
     * and the wrong ISO week. An explicit "+02:00" matched nothing at all, so those rows were
     * dropped as unparseable.
     */
    private val INSTANT_FORMATS = listOf(
        DateTimeFormatter.ISO_INSTANT,
        DateTimeFormatter.ISO_OFFSET_DATE_TIME,
        DateTimeFormatter.ISO_ZONED_DATE_TIME
    )

    // Zone-LESS formats. These genuinely carry no offset, so local time is the right reading.
    private val DATE_TIME_FORMATS = listOf(
        "yyyy-MM-dd HH:mm:ss", "yyyy-MM-dd'T'HH:mm:ss",
        "yyyy-MM-dd HH:mm", "yyyy/MM/dd HH:mm:ss",
        "d MMM yyyy, HH:mm", "dd MMM yyyy, HH:mm", "d MMM yyyy HH:mm", "dd MMM yyyy HH:mm",
        "MMM d yyyy, HH:mm", "EEE, dd MMM yyyy HH:mm:ss"
    ).map { DateTimeFormatter.ofPattern(it, java.util.Locale.ENGLISH) }

    // Numeric d/M/y and M/d/y are mutually ambiguous ("04/05/2024" is either), so try the device
    // locale's convention FIRST — an export made on this device then parses to the date the user
    // meant, with the other order kept as a cross-locale fallback. Month-first is US + its Pacific
    // territories and the Philippines; everywhere else is day-first.
    //
    // Read on every access, not frozen at class load. As a `val` on an object this was evaluated
    // once per PROCESS: a user who switched their phone from en-US to en-GB and imported without
    // rebooting still had "04/05/2024" tried as MM/dd first, so the row was stored as 4 May when
    // they — now on a day-first locale, reading a day-first export — meant 5 April. Silent,
    // permanent, and every ambiguous date in the file.
    private val monthFirstLocale: Boolean
        get() = java.util.Locale.getDefault().country in setOf("US", "PH", "FM", "MH", "PW", "GU", "AS")
    private val ambiguousSlashFormats: List<String>
        get() = if (monthFirstLocale) listOf("MM/dd/yyyy", "dd/MM/yyyy") else listOf("dd/MM/yyyy", "MM/dd/yyyy")

    private val DATE_ONLY_FORMATS: List<DateTimeFormatter>
        get() = (
            listOf("yyyy-MM-dd", "yyyy/MM/dd") + ambiguousSlashFormats +
                listOf("d MMM yyyy", "dd MMM yyyy", "MMM d, yyyy", "MMMM d, yyyy")
            ).map { DateTimeFormatter.ofPattern(it, java.util.Locale.ENGLISH) }

    private val zone: ZoneId get() = ZoneId.systemDefault()

    /** Parse a source date/time string to epoch millis, or null if no known format matches. */
    fun parseEpochMillis(raw: String): Long? {
        val s = raw.trim()
        if (s.isBlank()) return null
        // A string that states its own zone or offset is an instant, and must not be re-zoned.
        for (fmt in INSTANT_FORMATS) {
            try {
                return Instant.from(fmt.parse(s)).toEpochMilli()
            } catch (_: Exception) { /* try next */ }
        }
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

    /**
     * Reps can arrive as "10", "10.0", or empty; parse leniently to an int (null when not a count).
     *
     * Zero is NOT a count. [ImportedSet] models a resistance set and carries no duration, so a
     * 0-rep row is never a set that happened. Returning 0 here defeated every importer's
     * cardio-row guard, which reads `reps == null && (weight == null || weight == 0.0)`: a Strong
     * or Hevy distance row (Weight 0, Reps 0) passed it and became a phantom 0 x 0 set, inflating
     * set counts, streaks and trophies with sessions the user never lifted in.
     */
    fun parseReps(raw: String): Int? = raw.trim().toDoubleOrNull()?.toInt()?.takeIf { it > 0 }

    /**
     * Weight can be "", "0", "45.5", "45.5 kg", "100,5" (European exports use a comma decimal — see
     * [CsvParser]'s `;` handling) or "1,250" (a US thousands separator); strip a unit suffix,
     * normalise the separator, and parse.
     *
     * A lone comma is only a decimal point when it is followed by ONE OR TWO digits and nothing
     * else. Treating every lone comma that way read "1,250" as 1.25 — a 1250 lb leg press stored as
     * 1.3 lb, with the session's denormalised totalVolumeLb computed from it, so a 10,000 lb session
     * landed in history as a 10 lb one and dragged every volume chart down permanently. A comma
     * followed by exactly three digits, or more than one comma, is a thousands separator.
     */
    fun parseWeight(raw: String): Double? {
        val s = raw.trim().lowercase().removeSuffix("kg").removeSuffix("kgs").removeSuffix("lb")
            .removeSuffix("lbs").trim()
        if (s.isEmpty()) return null
        val normalised = when {
            // Both separators present: whichever comes LAST is the decimal point — "1,234.5" is US,
            // "1.250,75" is European.
            s.contains('.') && s.contains(',') ->
                if (s.lastIndexOf(',') > s.lastIndexOf('.')) s.replace(".", "").replace(',', '.')
                else s.replace(",", "")
            // "100,5" / "82,25" — a single comma with a 1-2 digit tail is the decimal point.
            COMMA_DECIMAL.matches(s) -> s.replace(',', '.')
            // "1,250" / "12,345" / "1,234,567" — thousands separators.
            else -> s.replace(",", "")
        }
        return normalised.toDoubleOrNull()
    }

    /** A single comma with a 1-2 digit tail: the only shape a lone comma is a decimal point in. */
    private val COMMA_DECIMAL = Regex("""^-?\d+,\d{1,2}$""")

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
