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
import java.time.Instant
import java.time.YearMonth
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.ChronoUnit
import java.util.Locale

/** Sort order for the gallery photo grid. */
internal enum class GallerySort(val label: String) { NEWEST("Newest"), OLDEST("Oldest") }

/** The grid densities the user can cycle through (photos per row). */
internal val GALLERY_DENSITIES = listOf(2, 3, 4)

// A few date spellings folded into the search haystack so "june", "jun", "2026" and "monday" all hit.
private val SEARCH_DATE_FMT: DateTimeFormatter =
    DateTimeFormatter.ofPattern("MMMM MMM d yyyy EEEE", Locale.getDefault())

/** Lower-cased search haystack for a photo: its note, album, and date in several spellings. */
private fun photoSearchText(photo: ProgressPhoto, zone: ZoneId): String {
    val date = Instant.ofEpochMilli(photo.takenAtMs).atZone(zone).toLocalDate()
    return "${photo.note} ${photo.album} ${date.format(SEARCH_DATE_FMT)}".lowercase()
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
        placeholder = { Text("Search notes, albums or dates…") },
        leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null) },
        trailingIcon = {
            if (query.isNotEmpty()) IconButton(onClick = { onQueryChange("") }) {
                Icon(Icons.Filled.Close, contentDescription = "Clear search")
            }
        },
        singleLine = true,
        shape = RoundedCornerShape(12.dp)
    )
}

