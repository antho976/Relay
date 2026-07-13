package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeight
import com.forge.app.ui.common.bounceClick
import java.io.File
import java.text.SimpleDateFormat
import java.time.ZoneId
import java.util.Date
import java.util.Locale
import kotlin.math.abs

// ── Same weight, different body ──────────────────────────────────────────────

/**
 * Two shots taken at the SAME bodyweight but far enough apart to show a different physique — the
 * "scale held, the body didn't" comparison. [before]/[after] are ordered oldest→newest and
 * [avgWeightLb] is their shared weight (the two differ by at most [SAME_WEIGHT_TOL_LB]).
 */
internal data class SameWeightPair(
    val before: ProgressPhoto,
    val after: ProgressPhoto,
    val avgWeightLb: Double,
    val daysApart: Long
)

// The scale reads essentially the same within this band (about a normal day-to-day fluctuation).
private const val SAME_WEIGHT_TOL_LB = 2.0
// Two shots must sit this far apart for a body change to be worth showing (a plateau, not a week).
private const val MIN_DAYS_APART = 30L
private const val MAX_SAME_WEIGHT_PAIRS = 3

/**
 * Auto-detect "same weight, different body" pairs among [photos]: two shots of the SAME pose (so the
 * angle compares fairly) whose snapshotted bodyweight is within [SAME_WEIGHT_TOL_LB] yet taken at
 * least [MIN_DAYS_APART] apart. Ranked longest-span first (the most dramatic hold), then closest
 * weight, and kept to distinct photos so each surfaced card is its own comparison. Photos with no
 * weight snapshot can't make the claim, so they're skipped; [exclude] drops the one pair the progress
 * band already shows (it carries "SAME WT" itself when its ends match), so the section never echoes it.
 */
internal fun sameWeightPairs(
    photos: List<ProgressPhoto>,
    zone: ZoneId,
    exclude: Set<String> = emptySet()
): List<SameWeightPair> {
    val weighed = photos.filter { it.weightLb != null }
    if (weighed.size < 2) return emptyList()

    val candidates = ArrayList<SameWeightPair>()
    for (i in weighed.indices) {
        for (j in i + 1 until weighed.size) {
            val a = weighed[i]
            val b = weighed[j]
            if (a.pose != b.pose) continue
            val wa = a.weightLb!!
            val wb = b.weightLb!!
            if (abs(wa - wb) > SAME_WEIGHT_TOL_LB) continue
            val days = daysBetween(a.takenAtMs, b.takenAtMs, zone)
            if (days < MIN_DAYS_APART) continue
            val (before, after) = if (a.takenAtMs <= b.takenAtMs) a to b else b to a
            candidates += SameWeightPair(before, after, (wa + wb) / 2.0, days)
        }
    }

    val ranked = candidates.sortedWith(
        compareByDescending<SameWeightPair> { it.daysApart }
            .thenBy { abs(it.after.weightLb!! - it.before.weightLb!!) }
    )
    val used = HashSet<String>()
    val picked = ArrayList<SameWeightPair>()
    for (p in ranked) {
        if (setOf(p.before.fileName, p.after.fileName) == exclude) continue
        if (p.before.fileName in used || p.after.fileName in used) continue
        picked += p
        used += p.before.fileName
        used += p.after.fileName
        if (picked.size >= MAX_SAME_WEIGHT_PAIRS) break
    }
    return picked
}

/**
 * The "same weight, different body" strip: each auto-paired shot at a matched bodyweight is two
 * thumbnails with the shared weight + span beneath; tapping opens the slider compare. Drawn only when
 * a pair exists (like the bodyweight sparkline), so it never shows an empty row.
 */
@Composable
internal fun SameWeightSection(
    pairs: List<SameWeightPair>,
    zone: ZoneId,
    weightUnit: WeightUnit,
    fileFor: (ProgressPhoto) -> File,
    onCompare: (ProgressPhoto, ProgressPhoto) -> Unit,
    muted: Color
) {
    if (pairs.isEmpty()) return
    // Mono anchor matches the sibling "BODYWEIGHT" label; the phrase carries the concept, so no caption.
    Text("SAME WEIGHT, DIFFERENT BODY", style = MaterialTheme.typography.labelMedium, color = muted, letterSpacing = 1.sp)
    Spacer(Modifier.height(10.dp))
    Row(
        Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(14.dp)
    ) {
        pairs.forEach { pair -> SameWeightCard(pair, zone, weightUnit, fileFor, onCompare, muted) }
    }
}

/** One pair: the two dated thumbnails side by side over the shared weight + span line. */
@Composable
private fun SameWeightCard(
    pair: SameWeightPair,
    zone: ZoneId,
    weightUnit: WeightUnit,
    fileFor: (ProgressPhoto) -> File,
    onCompare: (ProgressPhoto, ProgressPhoto) -> Unit,
    muted: Color
) {
    val span = remember(pair) { gallerySpanLabel(pair.before.takenAtMs, pair.after.takenAtMs, zone) }
    Column(Modifier.width(176.dp).bounceClick { onCompare(pair.before, pair.after) }) {
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            PairThumb(pair.before, fileFor, Modifier.weight(1f))
            PairThumb(pair.after, fileFor, Modifier.weight(1f))
        }
        Spacer(Modifier.height(7.dp))
        Text(
            if (span.isEmpty()) formatWeight(pair.avgWeightLb, weightUnit)
            else "${formatWeight(pair.avgWeightLb, weightUnit)} · $span",
            style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp
        )
    }
}

/** A portrait thumbnail with a date on a bottom scrim — the bare-photo idiom the progress band uses. */
@Composable
private fun PairThumb(
    photo: ProgressPhoto,
    fileFor: (ProgressPhoto) -> File,
    modifier: Modifier = Modifier
) {
    Box(modifier.aspectRatio(0.8f).clip(RoundedCornerShape(12.dp))) {
        ProgressPhotoImage(fileFor(photo), Modifier.fillMaxSize(), reqPx = 400)
        Box(
            Modifier.matchParentSize().background(
                Brush.verticalGradient(0.6f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.55f))
            )
        )
        Text(
            SimpleDateFormat("MMM d", Locale.getDefault()).format(Date(photo.takenAtMs)).uppercase(),
            style = MaterialTheme.typography.labelSmall, color = Color.White.copy(alpha = 0.92f), fontSize = 9.sp,
            modifier = Modifier.align(Alignment.BottomStart).padding(horizontal = 7.dp, vertical = 6.dp)
        )
    }
}
