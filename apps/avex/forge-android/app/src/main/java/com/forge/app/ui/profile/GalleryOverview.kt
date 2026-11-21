package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.formatWeightDelta
import com.forge.app.ui.common.currentLocale
import java.io.File
import java.text.SimpleDateFormat
import java.time.ZoneId
import java.util.Date
import kotlin.math.abs

/** One ready-made comparison: two photos, oldest first, and why they were paired. */
@Immutable
internal data class CompareSuggestion(
    val before: ProgressPhoto,
    val after: ProgressPhoto,
    /** Set for a "same weight, different body" pair; null for the first-vs-latest pair. */
    val sameWeightLb: Double? = null
)

/**
 * The comparisons worth one tap, for whatever the grid is showing: the first shot against the
 * latest, then any pairs taken at the same bodyweight months apart. Empty below two photos.
 */
internal fun compareSuggestions(
    first: ProgressPhoto?,
    latest: ProgressPhoto?,
    sameWeight: List<SameWeightPair>
): List<CompareSuggestion> {
    val lead = if (first != null && latest != null && first.fileName != latest.fileName) {
        CompareSuggestion(first, latest)
    } else null
    return listOfNotNull(lead) + sameWeight.map { CompareSuggestion(it.before, it.after, it.avgWeightLb) }
}

/**
 * A horizontal row of comparison cards above the grid. Short on purpose: it is a shortcut into the
 * compare view, and the photos below it are the page.
 */
@Composable
internal fun GalleryCompareStrip(
    suggestions: List<CompareSuggestion>,
    zone: ZoneId,
    weightUnit: WeightUnit,
    fileFor: (ProgressPhoto) -> File,
    onCompare: (ProgressPhoto, ProgressPhoto) -> Unit
) {
    Column(Modifier.fillMaxWidth().padding(top = 4.dp, bottom = 8.dp)) {
        Text(
            "Compare",
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onBackground,
            modifier = Modifier.padding(horizontal = GALLERY_GUTTER, vertical = 8.dp)
        )
        LazyRow(
            contentPadding = PaddingValues(horizontal = GALLERY_GUTTER),
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            items(suggestions, key = { "${it.before.fileName}|${it.after.fileName}" }) { s ->
                CompareCard(s, zone, weightUnit, fileFor) { onCompare(s.before, s.after) }
            }
        }
    }
}

@Composable
private fun CompareCard(
    s: CompareSuggestion,
    zone: ZoneId,
    weightUnit: WeightUnit,
    fileFor: (ProgressPhoto) -> File,
    onClick: () -> Unit
) {
    val span = remember(s) { gallerySpanLabel(s.before.takenAtMs, s.after.takenAtMs, zone) }
    val headline = if (span.isEmpty()) "Same day" else "$span apart"
    val supporting = remember(s, weightUnit) { compareCardLine(s, weightUnit) }
    Surface(
        onClick = onClick,
        shape = MaterialTheme.shapes.large,
        color = MaterialTheme.colorScheme.surfaceContainer,
        modifier = Modifier.width(212.dp).semantics { contentDescription = "Compare, $headline, $supporting" }
    ) {
        Column(Modifier.padding(6.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                CompareThumb(s.before, fileFor, Modifier.weight(1f))
                CompareThumb(s.after, fileFor, Modifier.weight(1f))
            }
            Column(Modifier.padding(horizontal = 8.dp, vertical = 8.dp)) {
                Text(headline, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurface)
                Text(supporting, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

/** "First to latest · 4.2 kg down" · "Same weight · 181 lb". A direction and a size, never a verdict. */
private fun compareCardLine(s: CompareSuggestion, unit: WeightUnit): String {
    s.sameWeightLb?.let { return "Same weight · ${formatWeight(it, unit)}" }
    val bw = s.before.weightLb
    val aw = s.after.weightLb
    val weight = if (bw != null && aw != null) {
        val d = aw - bw
        if (abs(d) < 0.1) "no weight change" else "${formatWeightDelta(abs(d), unit)} ${if (d > 0) "up" else "down"}"
    } else null
    return listOfNotNull("First to latest", weight).joinToString(" · ")
}

@Composable
private fun CompareThumb(photo: ProgressPhoto, fileFor: (ProgressPhoto) -> File, modifier: Modifier) {
    val locale = currentLocale()
    val date = remember(photo.takenAtMs, locale) { SimpleDateFormat("MMM d", locale).format(Date(photo.takenAtMs)) }
    // Card radius 16 minus its 6dp padding, so the inner corners run parallel to the outer ones.
    Box(modifier.aspectRatio(0.75f).clip(RoundedCornerShape(10.dp))) {
        ProgressPhotoImage(fileFor(photo), Modifier.fillMaxSize(), reqPx = 320)
        Box(
            Modifier.matchParentSize().background(
                Brush.verticalGradient(0.6f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.65f))
            )
        )
        Text(
            date,
            style = MaterialTheme.typography.labelMedium,
            color = Color.White,
            modifier = Modifier.align(Alignment.BottomStart).padding(horizontal = 8.dp, vertical = 6.dp)
        )
    }
}
