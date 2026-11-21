package com.forge.app.ui.profile

import androidx.compose.runtime.Immutable
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.photo.PhotoPose
import com.forge.app.domain.photo.musclesFromCodes
import com.forge.app.program.MuscleGroup
import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.YearMonth
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.ChronoUnit
import java.time.temporal.TemporalAdjusters
import java.util.Locale

// ── The library's own vocabulary ─────────────────────────────────────────────

/** Time windows for the photo grid. Calendar-aware (not rolling) so they read predictably. */
internal enum class GalleryRange(val label: String) {
    ALL("Any time"), WEEK("This week"), MONTH("This month"), LAST_MONTH("Last month"), QUARTER("Last 3 months")
}

/** Sort order for the photo grid. */
internal enum class GallerySort(val label: String) { NEWEST("Newest first"), OLDEST("Oldest first") }

/** The grid densities a pinch or the overflow menu steps through (photos per row). */
internal val GALLERY_DENSITIES = listOf(2, 3, 4, 5)

/** The density the grid opens on. */
internal const val GALLERY_DEFAULT_COLUMNS = 3

// ── Filter state ─────────────────────────────────────────────────────────────

/**
 * Everything that narrows the library, as one immutable value.
 *
 * Faceted: the axes AND together (a Back shot, tagged Chest, from this month, in the "Cut" album,
 * whose text matches "fasted"), while values WITHIN the muscle and tag facets OR together (Chest or
 * Triceps). That is the combination that behaves the way a person expects when they pick two values
 * in one filter and one value in another.
 *
 * Albums are a filter like any other rather than a separate mode of the screen: picking one narrows
 * the same grid, keeps search and the other filters working inside it, and is undone the same way.
 */
@Immutable
internal data class GalleryFilter(
    val query: String = "",
    val range: GalleryRange = GalleryRange.ALL,
    val pose: PhotoPose? = null,
    /** [MuscleGroup] codes; empty = every muscle. */
    val muscles: Set<String> = emptySet(),
    /** Normalized free tags; empty = every tag. */
    val tags: Set<String> = emptySet(),
    /** null = every album; "" = photos in no album; otherwise one album's name. */
    val album: String? = null,
    val sort: GallerySort = GallerySort.NEWEST,
    val columns: Int = GALLERY_DEFAULT_COLUMNS
) {
    val searching: Boolean get() = query.isNotBlank()

    /** How many filter axes are narrowing right now (search excluded). */
    val activeFacets: Int
        get() = (if (range != GalleryRange.ALL) 1 else 0) +
            (if (pose != null) 1 else 0) +
            (if (muscles.isEmpty()) 0 else 1) +
            (if (tags.isEmpty()) 0 else 1) +
            (if (album != null) 1 else 0)

    /** Something is hiding photos, so an empty grid can offer a way back out instead of dead-ending. */
    val narrowed: Boolean get() = searching || activeFacets > 0

    fun withMuscleToggled(code: String): GalleryFilter =
        copy(muscles = if (code in muscles) muscles - code else muscles + code)

    fun withTagToggled(tag: String): GalleryFilter =
        copy(tags = if (tag in tags) tags - tag else tags + tag)

    /** Drops every filter axis, keeping search and the presentation choices (sort, density). */
    fun clearedFacets(): GalleryFilter = GalleryFilter(query = query, sort = sort, columns = columns)

    /** One density step. [denser] = more photos per row. Clamped at both ends. */
    fun stepColumns(denser: Boolean): GalleryFilter {
        val i = GALLERY_DENSITIES.indexOf(columns).coerceAtLeast(0)
        val next = (if (denser) i + 1 else i - 1).coerceIn(0, GALLERY_DENSITIES.lastIndex)
        return copy(columns = GALLERY_DENSITIES[next])
    }
}

// ── Matching ─────────────────────────────────────────────────────────────────

/** True if [takenAtMs] falls inside [range]. [firstDayMonday] only affects the "This week" window. */
internal fun galleryRangeMatches(takenAtMs: Long, range: GalleryRange, zone: ZoneId, firstDayMonday: Boolean): Boolean {
    if (range == GalleryRange.ALL) return true
    val date = Instant.ofEpochMilli(takenAtMs).atZone(zone).toLocalDate()
    val today = LocalDate.now(zone)
    val thisMonth = YearMonth.from(today)
    return when (range) {
        GalleryRange.ALL -> true
        GalleryRange.WEEK -> {
            val firstDow = if (firstDayMonday) DayOfWeek.MONDAY else DayOfWeek.SUNDAY
            val weekStart = today.with(TemporalAdjusters.previousOrSame(firstDow))
            !date.isBefore(weekStart) && !date.isAfter(today)
        }
        GalleryRange.MONTH -> YearMonth.from(date) == thisMonth
        GalleryRange.LAST_MONTH -> YearMonth.from(date) == thisMonth.minusMonths(1)
        GalleryRange.QUARTER -> !date.isBefore(today.minusMonths(3)) && !date.isAfter(today)
    }
}

// A few date spellings folded into the search haystack so "june", "jun", "2026" and "monday" all hit.
private val SEARCH_DATE_FMT: DateTimeFormatter =
    DateTimeFormatter.ofPattern("MMMM MMM d yyyy EEEE", Locale.getDefault())

/**
 * Lower-cased search haystack for a photo: its title, its note, its pose and muscles (by DISPLAY
 * name, so what you type is what you see), its tags, its album, and its date in several spellings.
 */
private fun photoSearchText(photo: ProgressPhoto, zone: ZoneId): String {
    val date = Instant.ofEpochMilli(photo.takenAtMs).atZone(zone).toLocalDate()
    val pose = PhotoPose.fromKey(photo.pose)?.label.orEmpty()
    val muscles = musclesFromCodes(photo.muscles).joinToString(" ") { it.displayName }
    val tags = photo.tags.joinToString(" ")
    return "${photo.title} ${photo.note} $pose $muscles $tags ${photo.album} ${date.format(SEARCH_DATE_FMT)}".lowercase()
}

private val QUERY_SPLIT_REGEX = Regex("\\s+")

/**
 * Each photo's haystack, built once: the search re-filters every photo on every keystroke, and each
 * haystack formats a date, resolves muscle names and lower-cases the lot. Keyed by the photo's value
 * (a data class) and the zone, so an edited photo or a flight builds a fresh one.
 */
private val PHOTO_SEARCH_TEXT: MutableMap<Pair<ProgressPhoto, ZoneId>, String> =
    java.util.Collections.synchronizedMap(object : LinkedHashMap<Pair<ProgressPhoto, ZoneId>, String>(64, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Pair<ProgressPhoto, ZoneId>, String>?) =
            size > 1024
    })

/** True if every whitespace-separated token of [query] appears somewhere in the photo's own text. */
internal fun photoMatchesQuery(photo: ProgressPhoto, query: String, zone: ZoneId): Boolean {
    val q = query.trim().removePrefix("#").lowercase()
    if (q.isEmpty()) return true
    val hay = PHOTO_SEARCH_TEXT.getOrPut(photo to zone) { photoSearchText(photo, zone) }
    return q.split(QUERY_SPLIT_REGEX).all { hay.contains(it) }
}

/** Apply every axis of [filter] to [photos] and sort the survivors. AND across facets, OR within. */
internal fun applyGalleryFilter(
    photos: List<ProgressPhoto>,
    filter: GalleryFilter,
    zone: ZoneId,
    firstDayMonday: Boolean
): List<ProgressPhoto> {
    val kept = photos.filter { p ->
        galleryRangeMatches(p.takenAtMs, filter.range, zone, firstDayMonday) &&
            (filter.pose == null || p.pose == filter.pose.name) &&
            (filter.muscles.isEmpty() || p.muscles.any { it in filter.muscles }) &&
            (filter.tags.isEmpty() || p.tags.any { it in filter.tags }) &&
            (filter.album == null || p.album.equals(filter.album, ignoreCase = true)) &&
            photoMatchesQuery(p, filter.query, zone)
    }
    return when (filter.sort) {
        GallerySort.NEWEST -> kept.sortedByDescending { it.takenAtMs }
        GallerySort.OLDEST -> kept.sortedBy { it.takenAtMs }
    }
}

// ── Month grouping ───────────────────────────────────────────────────────────

/** Month headers: "September", widened to "September 2025" outside the current year. */
private val MONTH_FMT: DateTimeFormatter = DateTimeFormatter.ofPattern("LLLL", Locale.getDefault())
private val MONTH_YEAR_FMT: DateTimeFormatter = DateTimeFormatter.ofPattern("LLLL yyyy", Locale.getDefault())

/** One month of shots, with the header the grid pins to the top while you scroll it. */
@Immutable
internal data class GallerySection(
    /** The first day of the month; the section's stable key. */
    val date: LocalDate,
    val label: String,
    val meta: String,
    val photos: List<ProgressPhoto>
)

/**
 * Group [photos] into calendar months, keeping the order they arrive in.
 *
 * Months, not days: progress photos come a few at a time, a week or more apart, so day groups left
 * most rows one photo wide with the rest of the row empty, and a pose filter made every row that
 * way. A month flows as one grid, and each thumbnail carries its own date.
 */
internal fun gallerySections(photos: List<ProgressPhoto>, zone: ZoneId, today: LocalDate): List<GallerySection> =
    photos.groupBy { Instant.ofEpochMilli(it.takenAtMs).atZone(zone).toLocalDate().withDayOfMonth(1) }
        .map { (month, ps) ->
            val label = month.format(if (month.year == today.year) MONTH_FMT else MONTH_YEAR_FMT)
                .replaceFirstChar { it.titlecase(Locale.getDefault()) }
            GallerySection(month, label, photoCountLabel(ps.size), ps)
        }

// ── Readouts ─────────────────────────────────────────────────────────────────

/** "1 photo" / "24 photos". */
internal fun photoCountLabel(count: Int): String = "$count photo${if (count == 1) "" else "s"}"

/** Human span between the oldest and newest photo, e.g. "9 days" / "6 weeks" / "5 months". */
internal fun gallerySpanLabel(oldestMs: Long, newestMs: Long, zone: ZoneId): String {
    val d0 = Instant.ofEpochMilli(oldestMs).atZone(zone).toLocalDate()
    val d1 = Instant.ofEpochMilli(newestMs).atZone(zone).toLocalDate()
    val days = ChronoUnit.DAYS.between(d0, d1)
    return when {
        days <= 0 -> ""
        days < 14 -> "$days day${if (days == 1L) "" else "s"}"
        days < 60 -> "${days / 7} weeks"
        else -> {
            val months = ChronoUnit.MONTHS.between(YearMonth.from(d0), YearMonth.from(d1)).coerceAtLeast(1)
            "$months month${if (months == 1L) "" else "s"}"
        }
    }
}

/**
 * The strongest first-vs-latest pair in [photos]: the newest photo, paired with the OLDEST photo that
 * shares its pose (so "front vs front" beats "front vs a leg shot"); falls back to the oldest overall.
 * [photos] is whatever the grid is currently showing, so filtering to Back makes this answer "how has
 * my back changed". Null ends when there is no second photo to pair with.
 */
internal fun bestComparePair(photos: List<ProgressPhoto>): Pair<ProgressPhoto?, ProgressPhoto?> {
    if (photos.isEmpty()) return null to null
    val newest = photos.maxByOrNull { it.takenAtMs } ?: return null to null
    if (photos.size == 1) return newest to null
    val samePoseOldest = photos
        .filter { it.pose == newest.pose && it.fileName != newest.fileName }
        .minByOrNull { it.takenAtMs }
    val oldest = samePoseOldest ?: photos.filter { it.fileName != newest.fileName }.minByOrNull { it.takenAtMs }
    return oldest to newest
}

/** Display name for a muscle code, falling back to the code itself for a retired value. */
internal fun muscleName(code: String): String =
    MuscleGroup.entries.firstOrNull { it.code == code }?.displayName ?: code
