package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.GridView
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.SwapVert
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.ui.common.bounceClick
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

/** A quiet one-line stat: total photo count and the span they cover ("18 photos · 5 months"). */
@Composable
internal fun GalleryStatsHeader(photos: List<ProgressPhoto>, muted: Color, modifier: Modifier = Modifier) {
    if (photos.isEmpty()) return
    val zone = remember { ZoneId.systemDefault() }
    val text = remember(photos) {
        val count = photos.size
        val label = "$count photo${if (count == 1) "" else "s"}"
        if (count < 2) label else {
            val span = gallerySpanLabel(photos.minOf { it.takenAtMs }, photos.maxOf { it.takenAtMs }, zone)
            if (span.isEmpty()) label else "$label · $span"
        }
    }
    Text(text, style = MaterialTheme.typography.labelMedium, color = muted, modifier = modifier)
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

/** Sort toggle (left) + grid-density cycle (right) — the compact controls under the range chips. */
@Composable
internal fun GalleryControlsRow(
    sort: GallerySort,
    onToggleSort: () -> Unit,
    columns: Int,
    onCycleColumns: () -> Unit,
    muted: Color,
    outline: Color,
    modifier: Modifier = Modifier
) {
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        ControlPill(icon = { Icon(Icons.Filled.SwapVert, null, tint = muted, modifier = Modifier.size(15.dp)) },
            label = sort.label, muted = muted, outline = outline, onClick = onToggleSort)
        Spacer(Modifier.width(8.dp))
        ControlPill(icon = { Icon(Icons.Filled.GridView, null, tint = muted, modifier = Modifier.size(15.dp)) },
            label = "$columns", muted = muted, outline = outline, onClick = onCycleColumns)
    }
}

@Composable
private fun ControlPill(
    icon: @Composable () -> Unit,
    label: String,
    muted: Color,
    outline: Color,
    onClick: () -> Unit
) {
    Row(
        Modifier
            .clip(RoundedCornerShape(50))
            .background(Color.Transparent)
            .border(1.dp, outline.copy(alpha = 0.35f), RoundedCornerShape(50))
            .bounceClick { onClick() }
            .padding(horizontal = 12.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(5.dp)
    ) {
        icon()
        Text(label, style = MaterialTheme.typography.labelMedium, color = muted)
    }
}
