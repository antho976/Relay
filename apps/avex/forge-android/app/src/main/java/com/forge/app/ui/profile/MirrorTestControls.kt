package com.forge.app.ui.profile

import androidx.compose.runtime.Immutable
import androidx.compose.ui.focus.FocusRequester
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.photo.PhotoPose
import com.forge.app.domain.photo.PhotoTag
import com.forge.app.domain.photo.musclesFromCodes
import com.forge.app.program.MuscleGroup
import java.text.SimpleDateFormat
import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.YearMonth
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.ChronoUnit
import java.time.temporal.TemporalAdjusters
import java.util.Date
import java.util.Locale

// ── The library's own vocabulary ─────────────────────────────────────────────

/** Time windows for the photo grid. Calendar-aware (not rolling) so they read predictably. */
internal enum class GalleryRange(val label: String) {
    ALL("Any time"), WEEK("This week"), MONTH("This month"), LAST_MONTH("Last month"), QUARTER("Last 3 months")
}

/** Sort order for the photo grid. */
internal enum class GallerySort(val label: String) { NEWEST("Newest first"), OLDEST("Oldest first") }

/** The grid densities the user can cycle through (photos per row). */
internal val GALLERY_DENSITIES = listOf(2, 3, 4, 5)

/** The density the grid opens on (and the one the Filters chip treats as "untouched"). */
internal const val GALLERY_DEFAULT_COLUMNS = 3

// ── Filter state ─────────────────────────────────────────────────────────────

/**
 * Everything that narrows the library, as one immutable value.
 *
 * Faceted, not modal: the four axes AND together (a Back shot, tagged Chest, from this month, whose
 * text matches "fasted"), while the values WITHIN the muscle and tag facets OR together (Chest or
 * Triceps). That is the only combination that behaves the way a person expects when they tap two
 * chips in the same rail and two chips in different ones.
 *
 * Search used to REPLACE the facets rather than compose with them, so typing a word silently threw
 * away the window and the lens you had set and there was no way to search inside a filter.
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
    val sort: GallerySort = GallerySort.NEWEST,
    val columns: Int = GALLERY_DEFAULT_COLUMNS
) {
    val searching: Boolean get() = query.isNotBlank()

    /** How many axes are narrowing right now — the number the Filters chip carries. */
    val activeFacets: Int
        get() = (if (range != GalleryRange.ALL) 1 else 0) +
            (if (pose != null) 1 else 0) +
            (if (muscles.isEmpty()) 0 else 1) +
            (if (tags.isEmpty()) 0 else 1)

    /** Something is hiding photos, so an empty grid can offer a way back out instead of dead-ending. */
    val narrowed: Boolean get() = searching || activeFacets > 0

    /** The single muscle the library is currently read through, or null when it is zero or several.
     *  One muscle is a LENS (the band re-pairs inside it); several is a filter. */
    val soleMuscle: MuscleGroup?
        get() = muscles.singleOrNull()?.let { code -> MuscleGroup.entries.firstOrNull { it.code == code } }

    fun withMuscleToggled(code: String): GalleryFilter =
        copy(muscles = if (code in muscles) muscles - code else muscles + code)

    fun withTagToggled(tag: String): GalleryFilter =
        copy(tags = if (tag in tags) tags - tag else tags + tag)

    /** Drops everything that can empty the grid, keeping the presentation choices (sort, density). */
    fun cleared(): GalleryFilter = GalleryFilter(sort = sort, columns = columns)
}

/**
 * The callbacks the filter bar needs, bundled so the bar takes one parameter instead of a dozen.
 * The state itself is [filter]; every mutation goes back through [onChange] as a whole new value, so
 * there is exactly one place a filter can be written and no half-applied combination exists.
 */
internal data class GalleryTools(
    val filter: GalleryFilter,
    val onChange: (GalleryFilter) -> Unit,
    val filtersOpen: Boolean,
    val onToggleFilters: () -> Unit,
    val searchFocus: FocusRequester
) {
    fun set(next: GalleryFilter) = onChange(next)
    fun onQueryChange(q: String) = onChange(filter.copy(query = q))
    fun onRangeChange(r: GalleryRange) = onChange(filter.copy(range = r))
    fun onPoseChange(p: PhotoPose?) = onChange(filter.copy(pose = p))
    fun onToggleMuscle(code: String) = onChange(filter.withMuscleToggled(code))
    fun onToggleTag(tag: String) = onChange(filter.withTagToggled(tag))
    fun onToggleSort() = onChange(
        filter.copy(sort = if (filter.sort == GallerySort.NEWEST) GallerySort.OLDEST else GallerySort.NEWEST)
    )
    fun onCycleColumns() = onChange(
        filter.copy(columns = GALLERY_DENSITIES[(GALLERY_DENSITIES.indexOf(filter.columns) + 1) % GALLERY_DENSITIES.size])
    )
    fun onClear() = onChange(filter.cleared())
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
 * Lower-cased search haystack for a photo: everything the photo actually IS — its title, its note,
 * what it is a shot OF (the pose and the muscles), the tags you put on it, its album, and its date
 * in several spellings. Pose and muscles match on their DISPLAY names, not their stored keys, so
 * what you type is what you see on the photo.
 */
private fun photoSearchText(photo: ProgressPhoto, zone: ZoneId): String {
    val date = Instant.ofEpochMilli(photo.takenAtMs).atZone(zone).toLocalDate()
    val pose = PhotoPose.fromKey(photo.pose)?.label.orEmpty()
    val muscles = musclesFromCodes(photo.muscles).joinToString(" ") { it.displayName }
    val tags = photo.tags.joinToString(" ")
    return "${photo.title} ${photo.note} $pose $muscles $tags ${photo.album} ${date.format(SEARCH_DATE_FMT)}".lowercase()
}

/** True if every whitespace-separated token of [query] appears somewhere in the photo's own text. */
internal fun photoMatchesQuery(photo: ProgressPhoto, query: String, zone: ZoneId): Boolean {
    val q = query.trim().removePrefix("#").lowercase()
    if (q.isEmpty()) return true
    val hay = photoSearchText(photo, zone)
    return q.split(Regex("\\s+")).all { hay.contains(it) }
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
            photoMatchesQuery(p, filter.query, zone)
    }
    return when (filter.sort) {
        GallerySort.NEWEST -> kept.sortedByDescending { it.takenAtMs }
        GallerySort.OLDEST -> kept.sortedBy { it.takenAtMs }
    }
}

// ── Day grouping ─────────────────────────────────────────────────────────────

/** Day-group headers — "MON 21 JUL", widened to "MON 21 JUL 2025" outside the current year. */
private val DAY_HEADER_FMT: DateTimeFormatter = DateTimeFormatter.ofPattern("EEE d MMM", Locale.getDefault())
private val DAY_HEADER_YEAR_FMT: DateTimeFormatter = DateTimeFormatter.ofPattern("EEE d MMM yyyy", Locale.getDefault())

/** One day's worth of shots, with the header line the grid pins to the top while you scroll it. */
@Immutable
internal data class GalleryDay(
    val date: LocalDate,
    val label: String,
    val meta: String?,
    val photos: List<ProgressPhoto>
)

/**
 * Group [photos] into days, newest-or-oldest first following the order they arrive in.
 *
 * The day, not the month, is the unit: you shoot a set of angles in one sitting, so the day is what
 * makes several shots read as one session rather than as N anonymous thumbnails.
 */
internal fun galleryDays(photos: List<ProgressPhoto>, zone: ZoneId, today: LocalDate): List<GalleryDay> =
    photos.groupBy { Instant.ofEpochMilli(it.takenAtMs).atZone(zone).toLocalDate() }
        .map { (date, dayPhotos) ->
            GalleryDay(date, dayHeaderLabel(date, today), dayMetaLine(dayPhotos), dayPhotos)
        }

/** "TODAY" · "YESTERDAY" · "MON 21 JUL" · "MON 21 JUL 2025" once the year isn't the current one. */
private fun dayHeaderLabel(day: LocalDate, today: LocalDate): String = when (day) {
    today -> "TODAY"
    today.minusDays(1) -> "YESTERDAY"
    else -> day.format(if (day.year == today.year) DAY_HEADER_FMT else DAY_HEADER_YEAR_FMT).uppercase()
}

/**
 * The day's own reading: how many shots, what they are (the titles you gave them, else the angles
 * they were shot from), and the tags they carry. This is where a session's metadata belongs — at the
 * day's level, once, rather than repeated on every cell underneath it. Null when a lone untitled,
 * untagged photo would leave nothing worth saying (the cell already carries its own date).
 *
 * Muscles are deliberately NOT here. They ride the cells, and printing them in both places produced
 * headers like "BACK · BACK" over a row of thumbnails already saying BACK (§4.3, one home).
 */
private fun dayMetaLine(dayPhotos: List<ProgressPhoto>): String? {
    val count = if (dayPhotos.size > 1) "${dayPhotos.size} SHOTS" else null
    val what = dayPhotos.mapNotNull { p -> p.title.trim().ifBlank { PhotoPose.fromKey(p.pose)?.label } }
        .map { it.uppercase() }.distinct()
    val tags = dayPhotos.flatMap { it.tags }.distinct().map { PhotoTag.display(it) }
    val parts = listOfNotNull(count) + what + tags
    return parts.takeIf { it.isNotEmpty() }?.joinToString(" · ")
}

// ── Readouts ─────────────────────────────────────────────────────────────────

/**
 * The hero eyebrow: the count, then the month the library starts in once there is a span to name.
 * Honest zero (§12) — a fresh gallery reads "0 PHOTOS", never a dash and never a hidden line. While
 * the index is still being read off disk the count is unknown, so the eyebrow names the screen
 * instead of claiming a zero it has not checked.
 *
 * It carried a third segment counting muscle-tagged groups for a while. Two lines were enough at
 * 100% and it wrapped at 200%, and "9 TAGGED" never said WHAT was tagged, so it was read as nine
 * photos. A number nobody can interpret is not a reading (§2①).
 */
internal fun galleryEyebrow(photos: List<ProgressPhoto>, loading: Boolean): String {
    if (loading) return "PROGRESS PHOTOS"
    val count = photos.size
    val label = "$count PHOTO${if (count == 1) "" else "S"}"
    if (count < 2) return label
    val oldest = photos.minOfOrNull { it.takenAtMs } ?: return label
    val since = SimpleDateFormat("MMM yyyy", Locale.getDefault()).format(Date(oldest)).uppercase()
    return "$label · SINCE $since"
}

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
            "$months months"
        }
    }
}

/** Days between two shots, for compare readouts. */
internal fun daysBetween(aMs: Long, bMs: Long, zone: ZoneId): Long {
    val a = Instant.ofEpochMilli(minOf(aMs, bMs)).atZone(zone).toLocalDate()
    val b = Instant.ofEpochMilli(maxOf(aMs, bMs)).atZone(zone).toLocalDate()
    return ChronoUnit.DAYS.between(a, b)
}

/**
 * Pick the strongest pair for the progress band: the newest photo, paired with the OLDEST photo that
 * shares its pose (so "front vs front" beats "front vs a leg shot"); falls back to the oldest overall.
 * [photos] is whatever the band is currently reading — the whole library, or one muscle's worth of it
 * when the grid is lensed to a single muscle, which is what makes the band answer "how has my back
 * changed" rather than only "how have I changed".
 */
internal fun bestComparePair(photos: List<ProgressPhoto>): Pair<ProgressPhoto?, ProgressPhoto?> {
    if (photos.isEmpty()) return null to null
    val newest = photos.maxByOrNull { it.takenAtMs } ?: return null to null
    // A lone shot IS the first one — it fills FIRST and the empty NOW frame is where the next lands.
    if (photos.size == 1) return newest to null
    val samePoseOldest = photos
        .filter { it.pose == newest.pose && it.fileName != newest.fileName }
        .minByOrNull { it.takenAtMs }
    val oldest = samePoseOldest ?: photos.filter { it.fileName != newest.fileName }.minByOrNull { it.takenAtMs }
    return oldest to newest
}
