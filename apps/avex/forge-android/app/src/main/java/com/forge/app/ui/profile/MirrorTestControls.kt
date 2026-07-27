package com.forge.app.ui.profile

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.unit.dp
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.photo.PhotoPose
import java.text.SimpleDateFormat
import java.time.Instant
import java.time.YearMonth
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.ChronoUnit
import java.util.Date
import java.util.Locale

/** Sort order for the gallery photo grid. */
internal enum class GallerySort(val label: String) { NEWEST("Newest"), OLDEST("Oldest") }

/** The grid densities the user can cycle through (photos per row). */
internal val GALLERY_DENSITIES = listOf(2, 3, 4)

/** The density the grid opens on (and the one the Filters chip treats as "untouched"). */
internal const val GALLERY_DEFAULT_COLUMNS = 3

/**
 * Below this many photos the whole grid already fits on one screen, so Search / Filters / sort chips
 * narrow nothing and only crowd the section — they appear once there is something to sift (§12: design
 * each section at its emptiest realistic state). Compare has its own gate (two photos).
 */
internal const val GALLERY_TOOLS_MIN = 4

/**
 * Everything that narrows or re-lays-out the photo grid, bundled so the timeline section takes one
 * parameter instead of a dozen. Held as plain screen state in [MirrorTestScreen]; the derived flags
 * below are what the chips and the empty grid read.
 */
internal data class GalleryTools(
    val query: String,
    val onQueryChange: (String) -> Unit,
    val range: GalleryRange,
    val onRangeChange: (GalleryRange) -> Unit,
    val pose: PhotoPose?,
    val onPoseChange: (PhotoPose?) -> Unit,
    val sort: GallerySort,
    val onToggleSort: () -> Unit,
    val columns: Int,
    val onCycleColumns: () -> Unit,
    val searchOpen: Boolean,
    val onToggleSearch: () -> Unit,
    val filtersOpen: Boolean,
    val onToggleFilters: () -> Unit,
    /** Drops the search text, the range window and the pose lens — everything that can empty the grid. */
    val onClear: () -> Unit,
    val searchFocus: FocusRequester
) {
    val searching: Boolean get() = query.isNotBlank()

    /** Something is hiding photos, so an empty grid can offer a way back out instead of dead-ending. */
    val narrowed: Boolean get() = searching || range != GalleryRange.ALL || pose != null

    /** Any chip off its default — lights the Filters chip so a narrowed grid is never silent. */
    val filtersActive: Boolean
        get() = range != GalleryRange.ALL || sort != GallerySort.NEWEST || columns != GALLERY_DEFAULT_COLUMNS
}

/**
 * The hero eyebrow: the count, plus the month the library starts in once there is a span to name.
 * Honest zero (§12) — a fresh gallery reads "0 PHOTOS", never a dash and never a hidden line. While
 * the index is still being read off disk the count is unknown, so the eyebrow names the screen instead
 * of claiming a zero it hasn't checked.
 */
internal fun galleryEyebrow(count: Int, oldestMs: Long?, loading: Boolean): String {
    if (loading) return "PROGRESS PHOTOS"
    val label = "$count PHOTO${if (count == 1) "" else "S"}"
    if (count < 2 || oldestMs == null) return label
    val since = SimpleDateFormat("MMM yyyy", Locale.getDefault()).format(Date(oldestMs)).uppercase()
    return "$label · SINCE $since"
}

// A few date spellings folded into the search haystack so "june", "jun", "2026" and "monday" all hit.
private val SEARCH_DATE_FMT: DateTimeFormatter =
    DateTimeFormatter.ofPattern("MMMM MMM d yyyy EEEE", Locale.getDefault())

/**
 * Lower-cased search haystack for a photo: everything the photo actually IS — its title, its note,
 * what it's a shot OF (the pose: "arms", "legs", "back"), its album, and its date in several
 * spellings.
 *
 * The pose and the title were missing until 2026-07-25, which made the two most natural searches in a
 * physique gallery — "arms", or whatever you named the shot — return nothing at all. Pose is matched
 * on the enum's LABEL, not its stored key, so what you type is what you see on the photo.
 */
private fun photoSearchText(photo: ProgressPhoto, zone: ZoneId): String {
    val date = Instant.ofEpochMilli(photo.takenAtMs).atZone(zone).toLocalDate()
    val pose = PhotoPose.fromKey(photo.pose)?.label.orEmpty()
    return "${photo.title} ${photo.note} $pose ${photo.album} ${date.format(SEARCH_DATE_FMT)}".lowercase()
}

/** True if every whitespace-separated token of [query] appears in the photo's note/album/date text. */
internal fun photoMatchesQuery(photo: ProgressPhoto, query: String, zone: ZoneId): Boolean {
    val q = query.trim().lowercase()
    if (q.isEmpty()) return true
    val hay = photoSearchText(photo, zone)
    return q.split(Regex("\\s+")).all { hay.contains(it) }
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

/** The search field — filters photos by note, album name or date across the whole gallery. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun GallerySearchBar(
    query: String,
    onQueryChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    focusRequester: FocusRequester? = null
) {
    OutlinedTextField(
        value = query,
        onValueChange = onQueryChange,
        modifier = modifier.fillMaxWidth().let { if (focusRequester != null) it.focusRequester(focusRequester) else it },
        // The placeholder is the field's documentation — it names the dimensions the query matches, so
        // "arms" or a title you chose are discoverable without a caption above the bar (§11). No verb:
        // the magnifier already says "search", and the full sentence WRAPPED TO TWO LINES on device,
        // doubling the field's height for a word the icon was already carrying.
        placeholder = { Text("Title, pose, note or date…", maxLines = 1) },
        leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null) },
        colors = bodyLogFieldColors(),
        trailingIcon = {
            if (query.isNotEmpty()) IconButton(onClick = { onQueryChange("") }) {
                Icon(Icons.Filled.Close, contentDescription = "Clear search")
            }
        },
        singleLine = true,
        shape = RoundedCornerShape(12.dp)
    )
}

