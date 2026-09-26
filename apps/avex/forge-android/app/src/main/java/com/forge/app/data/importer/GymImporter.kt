package com.forge.app.data.importer

import java.time.Instant
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.DateTimeFormatterBuilder
import java.time.temporal.ChronoField

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
     * Rows that aren't workouts — cardio, coach goals, weigh-ins. Avex's own exports carry all
     * three; Strong and Hevy carry cardio, logged as exercises with a distance or a duration. This
     * defaults to empty so a parser with none has nothing to implement. [assumeKg] resolves a
     * distance the source leaves unitless, as it does a weight.
     */
    fun parseExtras(text: String, assumeKg: Boolean): ImportedExtras = ImportedExtras()

    /**
     * Workouts, extras and the count of unreadable rows from ONE parse. The CSV parsers override it
     * to count what they skip; the default is for a source whose rows cannot be skipped one by one.
     */
    fun read(text: String, assumeKg: Boolean): ParsedImport =
        ParsedImport(parse(text, assumeKg), parseExtras(text, assumeKg))

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

    /**
     * Zone-LESS formats. These genuinely carry no offset, so local time is the right reading.
     *
     * ISO local date-time first, with either a `T` or a space and optional seconds and fraction:
     * "2024-05-06T18:30" and "2024-05-06T18:30:00.123" matched nothing in the old fixed-pattern
     * list, so those rows were dropped as unparseable.
     */
    private val DATE_TIME_FORMATS: List<DateTimeFormatter> = listOf(
        DateTimeFormatter.ISO_LOCAL_DATE_TIME,
        DateTimeFormatterBuilder().parseCaseInsensitive()
            .append(DateTimeFormatter.ISO_LOCAL_DATE).appendLiteral(' ').append(DateTimeFormatter.ISO_LOCAL_TIME)
            .toFormatter(java.util.Locale.ENGLISH)
    ) + patterns(
        "yyyy/MM/dd HH:mm:ss", "yyyy/MM/dd HH:mm",
        "d MMM yyyy, HH:mm", "d MMM yyyy HH:mm",
        "MMM d yyyy, HH:mm", "EEE, d MMM yyyy HH:mm:ss"
    )

    private val DATE_ONLY_FORMATS: List<DateTimeFormatter> = listOf(DateTimeFormatter.ISO_LOCAL_DATE) +
        patterns("yyyy/MM/dd", "d MMM yyyy", "MMM d, yyyy", "MMMM d, yyyy")

    /**
     * Numeric slash dates, in both orders. `M` and `d` read one OR two digits, so the Excel/Sheets
     * US default "5/4/2024" parses as well as "05/04/2024"; the old two-digit `MM`/`dd` patterns
     * rejected it and a whole US spreadsheet imported nothing. A time may follow, 24-hour or
     * "6:30 PM", and a two-digit year is accepted once the four-digit forms have failed.
     */
    private fun slashFormats(monthFirst: Boolean): Pair<List<DateTimeFormatter>, List<DateTimeFormatter>> {
        val date = if (monthFirst) "M/d" else "d/M"
        // Dotted dates (the German, Polish and Russian spreadsheet default) are always day-first.
        val dates = listOf("$date/yyyy", "$date/yy", "d.M.yyyy", "d.M.yy")
        val times = listOf("H:mm:ss", "H:mm", "h:mm:ss a", "h:mm a", "h:mma")
        val withTime = dates.flatMap { d -> times.flatMap { t -> listOf("$d $t", "$d, $t") } }
        return patterns(*withTime.toTypedArray()) to patterns(*dates.toTypedArray())
    }

    private val MONTH_FIRST_SLASH = slashFormats(monthFirst = true)
    private val DAY_FIRST_SLASH = slashFormats(monthFirst = false)

    /**
     * A two-digit `yy` reads within the last 80 years and next 20, not 2000–2099: `appendPattern`'s
     * fixed base put "5/4/99" in 2099, a session dated 73 years in the future.
     */
    private fun patterns(vararg p: String): List<DateTimeFormatter> = p.map { pattern ->
        val b = DateTimeFormatterBuilder().parseCaseInsensitive()
        val yy = Regex("""(?<!y)yy(?!y)""").find(pattern)
        if (yy == null) b.appendPattern(pattern)
        else {
            if (yy.range.first > 0) b.appendPattern(pattern.substring(0, yy.range.first))
            b.appendValueReduced(ChronoField.YEAR, 2, 2, LocalDate.now().minusYears(80))
            if (yy.range.last + 1 < pattern.length) b.appendPattern(pattern.substring(yy.range.last + 1))
        }
        b.toFormatter(java.util.Locale.ENGLISH)
    }

    // Month-first is US + its Pacific territories and the Philippines; everywhere else is day-first.
    //
    // Read on every access, not frozen at class load. As a `val` on an object this was evaluated
    // once per PROCESS: a user who switched their phone from en-US to en-GB and imported without
    // rebooting still had "04/05/2024" tried as MM/dd first, so the row was stored as 4 May when
    // they — now on a day-first locale, reading a day-first export — meant 5 April. Silent,
    // permanent, and every ambiguous date in the file.
    private val monthFirstLocale: Boolean
        get() = java.util.Locale.getDefault().country in setOf("US", "PH", "FM", "MH", "PW", "GU", "AS")

    /** "13/05/2024" → (13, 5): the two leading numbers of a slash date, or null for any other shape. */
    private val SLASH_DATE = Regex("""^(\d{1,2})/(\d{1,2})/\d{2,4}\b""")

    /**
     * Whether a file's numeric slash dates are month-first, decided ONCE from all of them.
     *
     * Numeric d/M/y and M/d/y are mutually ambiguous ("04/05/2024" is either). Deciding row by row
     * read a UK file on an en-US phone correctly for 13/05 (only one order parses) and wrongly for
     * 04/05 (both do, and the locale's order wins), so one file's workouts landed in two different
     * calendars. A file is written in one convention: the first date with a component above 12
     * settles it for every row, and only a file where every date is ambiguous falls back to the
     * device locale.
     */
    fun slashDatesMonthFirst(raws: Sequence<String>): Boolean {
        for (raw in raws) {
            val m = SLASH_DATE.find(raw.trim()) ?: continue
            val first = m.groupValues[1].toInt()
            val second = m.groupValues[2].toInt()
            if (first > 12 && second <= 12) return false
            if (second > 12 && first <= 12) return true
        }
        return monthFirstLocale
    }

    /**
     * A per-file date parser: the slash order is decided once for [raws] (see
     * [slashDatesMonthFirst]) and each distinct string is parsed once. A set-per-row export repeats
     * the same date on every row of a workout, and each miss costs a thrown exception per format.
     */
    fun dateReader(raws: Sequence<String>): (String) -> Long? {
        val monthFirst = slashDatesMonthFirst(raws)
        val memo = HashMap<String, Long?>()
        // Not getOrPut: an unparseable string memoises as null, which getOrPut would treat as absent.
        return { raw -> if (raw in memo) memo[raw] else parseEpochMillis(raw, monthFirst).also { memo[raw] = it } }
    }

    private val zone: ZoneId get() = ZoneId.systemDefault()

    /**
     * Parse a source date/time string to epoch millis, or null if no known format matches.
     * [monthFirst] resolves an ambiguous numeric slash date; importers pass the order decided for
     * the whole file ([dateReader]), and a lone string falls back to the device locale's.
     */
    fun parseEpochMillis(raw: String, monthFirst: Boolean = monthFirstLocale): Long? {
        val s = raw.trim()
        if (s.isBlank()) return null
        // A string that states its own zone or offset is an instant, and must not be re-zoned.
        for (fmt in INSTANT_FORMATS) {
            try {
                return Instant.from(fmt.parse(s)).toEpochMilli()
            } catch (_: Exception) { /* try next */ }
        }
        val (slashDateTimes, slashDates) = if (monthFirst) MONTH_FIRST_SLASH else DAY_FIRST_SLASH
        for (fmt in DATE_TIME_FORMATS + slashDateTimes) {
            try {
                return LocalDateTime.parse(s, fmt).atZone(zone).toInstant().toEpochMilli()
            } catch (_: Exception) { /* try next */ }
        }
        for (fmt in DATE_ONLY_FORMATS + slashDates) {
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

    /**
     * Seconds from a duration cell ("1800", "1800.0"), or null when blank, zero or not a number.
     * Strong writes `Seconds`, Hevy `duration_seconds`; both mean a hold or a cardio duration.
     */
    fun parseSeconds(raw: String): Int? = raw.trim().toDoubleOrNull()
        ?.takeIf { it.isFinite() && it > 0 && it <= Int.MAX_VALUE }?.toInt()?.takeIf { it > 0 }

    /** A time cell that is either plain seconds ("90") or a clock ("1:30", "0:01:30"), as seconds. */
    fun parseClockOrSeconds(raw: String): Int? =
        parseSeconds(raw) ?: parseDurationToMillis(raw)?.let { (it / 1000L).toInt() }?.takeIf { it > 0 }

    /**
     * A distance in kilometres from a value and its unit ("km", "mi", "m", "ft", "yd"), or null when
     * blank or zero. A unitless value is km for a metric user and miles otherwise, the same
     * fallback a unitless weight gets.
     */
    fun distanceKm(raw: String, unit: String, assumeKm: Boolean): Double? {
        val value = parseWeight(raw)?.takeIf { it.isFinite() && it > 0.0 } ?: return null
        val u = unit.trim().lowercase()
        val km = when {
            u.startsWith("km") || u.startsWith("kilomet") -> value
            u.startsWith("mi") -> value * KM_PER_MILE
            u == "m" || u.startsWith("met") || u.startsWith("metre") -> value / 1000.0
            u.startsWith("ft") || u.startsWith("feet") || u.startsWith("foot") -> value * 0.0003048
            u.startsWith("yd") || u.startsWith("yard") -> value * 0.0009144
            else -> if (assumeKm) value else value * KM_PER_MILE
        }
        return Math.round(km * 1000.0) / 1000.0
    }

    private const val KM_PER_MILE = 1.609344

    /**
     * The cardio activity a strength app's exercise name describes, as a
     * [com.forge.app.domain.cardio.CardioType] code, or null when the name names no cardio
     * activity. Order matters: "Running (Treadmill)" is a treadmill session, "Bike Erg" a ride.
     */
    fun cardioTypeFor(exerciseName: String): String? {
        val n = exerciseName.lowercase()
        if (LOADED_CARRY.containsMatchIn(n)) return null
        return CARDIO_KEYWORDS.firstOrNull { (re, _) -> re.containsMatchIn(n) }?.second
    }

    private val CARDIO_KEYWORDS: List<Pair<Regex, String>> = listOf(
        Regex("""treadmill""") to "treadmill",
        Regex("""elliptical|cross[\s-]?trainer""") to "elliptical",
        Regex("""\b(cycl\w*|bik(e|ing)|spin(ning)?)\b""") to "cycle",
        Regex("""\b(rowing|rower|erg|ergometer)\b""") to "row",
        Regex("""\b(run|running|jog|jogging|sprints?)\b""") to "run",
        Regex("""\b(walk|walking)\b""") to "walk",
        Regex("""\b(hike|hiking)\b""") to "hike",
        Regex("""\bswim(ming)?\b""") to "swim",
        Regex("""\bhiit\b""") to "hiit",
        // Cardio machines and conditioning with no activity of their own.
        Regex("""\b(stair\w*|step\s?mill|stepper|jump\s?rope|skipping|boxing|shadow\s?box\w*)\b""") to "other"
    )

    /** Strength moves whose names say "walk": a timed "Walking Lunge" or "Farmer's Walk" is a hold. */
    private val LOADED_CARRY = Regex("""\b(lunges?|farmers?'?s?|carry|carries|suitcase|yoke)\b""")

    /**
     * Whether a Strong or Hevy row is cardio rather than a set, and if so which activity.
     *
     * Both apps log a run as an exercise row with Weight 0, Reps 0, a distance and a duration. The
     * timed-hold support read any row with a duration as a hold, so a year of runs became ~150
     * finished strength sessions of one 1800-second "ext-running" hold each, with the distance
     * dropped: inflated workout counts, streaks, trophies and "workouts this week". A hold is a row
     * with no distance; a row that has reps or a load is always a set.
     *
     * A row with a distance is cardio whatever its name ("other" when the name says nothing). A row
     * with only a duration is cardio when its name names an activity, and otherwise stays a hold, so
     * a 60-second plank is still a plank.
     */
    fun cardioRowType(
        exerciseName: String, reps: Int?, weight: Double?, durationSeconds: Int?, distanceKm: Double?
    ): String? {
        if ((reps ?: 0) > 0 || (weight ?: 0.0) > 0.0) return null
        val type = cardioTypeFor(exerciseName)
        return when {
            distanceKm != null -> type ?: "other"
            durationSeconds != null -> type
            else -> null
        }
    }

    /** Whole minutes for a cardio entry, rounded to the nearest minute but never 0 for a real duration. */
    fun cardioMinutes(durationSeconds: Int?): Int =
        durationSeconds?.let { ((it + 30) / 60).coerceAtLeast(1) } ?: 0

    /** First header index whose name contains any of [needles], or null. */
    fun findCol(index: Map<String, Int>, vararg needles: String): Int? =
        index.entries.firstOrNull { e -> needles.any { e.key.contains(it) } }?.value

    fun at(row: List<String>, i: Int?): String = i?.let { row.getOrNull(it)?.trim() } ?: ""
}
