package com.forge.app.ui.profile

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.photo.PhotoPose
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.formatWeightDelta
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.rememberDrawProgress
import com.forge.app.ui.theme.ForgeMotion
import java.io.File
import java.text.SimpleDateFormat
import java.time.Instant
import java.time.ZoneId
import java.time.temporal.ChronoUnit
import java.util.Date
import java.util.Locale

// ── Hero ───────────────────────────────────────────────────────────────────

/** The screen's serif name over a mono eyebrow that carries the count + span (§3 hero rule). */
@Composable
internal fun GalleryHero(photos: List<ProgressPhoto>, zone: ZoneId, onBg: Color, muted: Color) {
    val eyebrow = remember(photos) {
        if (photos.isEmpty()) "PROGRESS PHOTOS"
        else {
            val count = "${photos.size} PHOTO${if (photos.size == 1) "" else "S"}"
            val oldest = photos.minByOrNull { it.takenAtMs }
            val since = oldest?.let {
                SimpleDateFormat("MMM yyyy", Locale.getDefault()).format(Date(it.takenAtMs)).uppercase()
            }
            if (since != null && photos.size > 1) "$count · SINCE $since" else count
        }
    }
    Text(eyebrow, style = MaterialTheme.typography.labelMedium, color = muted, letterSpacing = 1.sp)
    Spacer(Modifier.height(4.dp))
    // Serif hero — the screen names itself here, so the top bar carries only the wordmark (§2). No period.
    Text("Gallery", style = MaterialTheme.typography.headlineLarge, color = onBg)
}

// ── Progress band (the signature mark) ───────────────────────────────────────

/**
 * The overview's headline comparison: your first shot beside your latest, with the span, weight
 * change and a compare affordance between them. Tapping it opens the slider compare. Drawn at every
 * count — one photo shows the shot beside a ghost "next" frame, none shows two ghost frames with an
 * add prompt — so the section is a mark at zero, not a text row (§12).
 */
@Composable
internal fun ProgressBand(
    before: ProgressPhoto?,
    after: ProgressPhoto?,
    zone: ZoneId,
    weightUnit: WeightUnit,
    fileFor: (ProgressPhoto) -> File,
    onCompare: (ProgressPhoto, ProgressPhoto) -> Unit,
    onAdd: () -> Unit,
    onBg: Color, muted: Color, accent: Color, outline: Color
) {
    val pair = before != null && after != null && before.fileName != after.fileName
    val mod = if (pair) Modifier.fillMaxWidth().bounceClick { onCompare(before!!, after!!) }
    else Modifier.fillMaxWidth().bounceClick { onAdd() }

    Row(mod, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        BandFrame("FIRST", before, zone, fileFor, muted, outline, Modifier.weight(1f))
        BandMeta(before, after, zone, weightUnit, pair, muted, accent, onBg)
        BandFrame("NOW", after, zone, fileFor, muted, outline, Modifier.weight(1f))
    }
    if (!pair) {
        Spacer(Modifier.height(8.dp))
        Text(
            if (after == null) "Add your first shot to start the timeline" else "Add another to compare",
            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
        )
    }
}

/** One end of the band: a portrait photo (dated, tagged) or a ghost frame when there's nothing yet. */
@Composable
private fun BandFrame(
    tag: String,
    photo: ProgressPhoto?,
    zone: ZoneId,
    fileFor: (ProgressPhoto) -> File,
    muted: Color,
    outline: Color,
    modifier: Modifier = Modifier
) {
    // Rounded 16 — the photo idiom (§7), a step up from the grid's 12 so the band reads as the hero.
    Box(modifier.aspectRatio(0.8f).clip(RoundedCornerShape(16.dp))) {
        if (photo != null) {
            ProgressPhotoImage(fileFor(photo), Modifier.fillMaxSize(), reqPx = 480)
            Box(
                Modifier.matchParentSize().background(
                    Brush.verticalGradient(0.55f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.6f))
                )
            )
            Column(Modifier.align(Alignment.BottomStart).padding(horizontal = 9.dp, vertical = 8.dp)) {
                Text(tag, style = MaterialTheme.typography.labelSmall, color = Color.White.copy(alpha = 0.75f), fontSize = 8.sp, letterSpacing = 1.sp)
                Text(
                    SimpleDateFormat("MMM d", Locale.getDefault()).format(Date(photo.takenAtMs)).uppercase(),
                    style = MaterialTheme.typography.labelSmall, color = Color.White.copy(alpha = 0.92f), fontSize = 9.sp
                )
            }
        } else {
            Box(
                Modifier.matchParentSize()
                    .border(1.dp, outline.copy(alpha = 0.35f), RoundedCornerShape(16.dp)),
                contentAlignment = Alignment.Center
            ) {
                Text("＋", style = MaterialTheme.typography.headlineSmall, color = muted.copy(alpha = 0.6f))
            }
        }
    }
}

/**
 * The middle column: the span as a small serif figure ("4" over "MONTHS"), the weight change as a
 * direction-only ↑/↓ line (never a verdict — §11), and the `compare →` affordance that says the
 * whole band taps. Pre-pair it's just a quiet arrow between the frame and its ghost.
 */
@Composable
private fun BandMeta(
    before: ProgressPhoto?,
    after: ProgressPhoto?,
    zone: ZoneId,
    weightUnit: WeightUnit,
    pair: Boolean,
    muted: Color,
    accent: Color,
    onBg: Color
) {
    Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(3.dp)) {
        if (!pair || before == null || after == null) {
            Text("→", style = MaterialTheme.typography.titleMedium, color = muted.copy(alpha = 0.5f))
            return@Column
        }
        val span = gallerySpanLabel(before.takenAtMs, after.takenAtMs, zone)
        val parts = span.split(' ', limit = 2)
        if (parts.size == 2) {
            // Serif figure + mono caption — the EditorialFigure shape, centered between the frames.
            Text(parts[0], style = MaterialTheme.typography.headlineMedium, color = onBg)
            Text(parts[1].uppercase(), style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
        }
        val bw = before.weightLb
        val aw = after.weightLb
        if (bw != null && aw != null) {
            val diff = aw - bw
            Spacer(Modifier.height(3.dp))
            Text(
                if (kotlin.math.abs(diff) < 0.1) "SAME WT"
                else "${if (diff > 0) "↑" else "↓"} ${formatWeightDelta(kotlin.math.abs(diff), weightUnit)}",
                style = MaterialTheme.typography.labelMedium,
                color = if (kotlin.math.abs(diff) < 0.1) muted else onBg
            )
        }
        Spacer(Modifier.height(5.dp))
        Text("compare →", style = MaterialTheme.typography.labelSmall, color = accent)
    }
}

// ── Bodyweight sparkline ─────────────────────────────────────────────────────

/**
 * The bodyweight-through-time section under the band — the trend the photos are set against. A mono
 * section anchor, the latest reading as a small serif figure, and the line drawing in left-to-right
 * once (§9/§10). Only rendered with ≥2 weigh-ins (a lone point isn't a trend); the band already
 * carries the per-shot weight delta, so nothing is repeated when this is absent. Photo dates ride
 * the bottom axis as faint ticks.
 */
@Composable
internal fun BodyweightSparkline(
    entries: List<BodyweightEntry>,
    photos: List<ProgressPhoto>,
    weightUnit: WeightUnit,
    onBg: Color, muted: Color, accent: Color
) {
    if (entries.size < 2) return
    val minW = entries.minOf { it.weightLb }
    val maxW = entries.maxOf { it.weightLb }
    val t0 = entries.first().recordedAt
    val t1 = entries.last().recordedAt
    val span = (t1 - t0).coerceAtLeast(1L)
    val range = (maxW - minW).takeIf { it > 0.1 } ?: 1.0

    val latest = entries.last().weightLb
    val first = entries.first().weightLb
    val diff = latest - first
    val reveal = rememberDrawProgress(key = entries.size to t1, spec = ForgeMotion.drawTween())

    EditorialHeader("Bodyweight", muted, accent)
    Spacer(Modifier.height(10.dp))
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Column {
            Text(formatWeight(latest, weightUnit), style = MaterialTheme.typography.headlineSmall, color = onBg)
            if (kotlin.math.abs(diff) >= 0.1) {
                Spacer(Modifier.height(2.dp))
                Text(
                    "${if (diff > 0) "↑" else "↓"} ${formatWeightDelta(kotlin.math.abs(diff), weightUnit)} since first",
                    style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp
                )
            }
        }
        Spacer(Modifier.width(16.dp))
        Canvas(Modifier.weight(1f).height(56.dp)) {
            val w = size.width
            val h = size.height
            val topPad = 4f
            val botPad = 8f // room for the photo ticks
            fun px(ms: Long) = ((ms - t0).toFloat() / span) * w
            fun py(wt: Double) = topPad + (1f - ((wt - minW) / range).toFloat()) * (h - topPad - botPad)

            // One-shot left-to-right reveal: everything on the timeline clips to the draw progress.
            clipRect(right = w * reveal) {
                // Weight line + soft area fill.
                val line = Path()
                val area = Path()
                entries.forEachIndexed { i, e ->
                    val x = px(e.recordedAt)
                    val y = py(e.weightLb)
                    if (i == 0) { line.moveTo(x, y); area.moveTo(x, h - botPad); area.lineTo(x, y) }
                    else { line.lineTo(x, y); area.lineTo(x, y) }
                }
                area.lineTo(px(entries.last().recordedAt), h - botPad)
                area.close()
                drawPath(area, Brush.verticalGradient(listOf(accent.copy(alpha = 0.15f), Color.Transparent)))
                drawPath(line, color = accent, style = Stroke(width = 2.5f))
                // Endpoint dot on the newest reading.
                drawCircle(accent, radius = 3.5f, center = Offset(px(t1), py(latest)))

                // Faint ticks where photos fall along the same timeline.
                photos.forEach { p ->
                    if (p.takenAtMs in t0..t1) {
                        val x = px(p.takenAtMs)
                        drawLine(muted.copy(alpha = 0.4f), Offset(x, h - botPad + 1f), Offset(x, h), strokeWidth = 1.5f)
                    }
                }
            }
        }
    }
}

// ── Pose helpers ─────────────────────────────────────────────────────────────

/** The poses that actually appear in [photos], in enum order — drives the lens pills + tag chips. */
internal fun posesPresent(photos: List<ProgressPhoto>): List<PhotoPose> =
    PhotoPose.entries.filter { pose -> photos.any { it.pose == pose.name } }

/**
 * Pick the strongest pair for the progress band: the newest photo, paired with the OLDEST photo that
 * shares its pose (so "front vs front" beats "front vs a leg shot"); falls back to the oldest overall.
 */
internal fun bestComparePair(photos: List<ProgressPhoto>): Pair<ProgressPhoto?, ProgressPhoto?> {
    if (photos.isEmpty()) return null to null
    val newest = photos.maxByOrNull { it.takenAtMs }!!
    if (photos.size == 1) return null to newest
    val samePoseOldest = photos
        .filter { it.pose == newest.pose && it.fileName != newest.fileName }
        .minByOrNull { it.takenAtMs }
    val oldest = samePoseOldest ?: photos.filter { it.fileName != newest.fileName }.minByOrNull { it.takenAtMs }
    return oldest to newest
}

/** Days between two shots, for compare readouts. */
internal fun daysBetween(aMs: Long, bMs: Long, zone: ZoneId): Long {
    val a = Instant.ofEpochMilli(minOf(aMs, bMs)).atZone(zone).toLocalDate()
    val b = Instant.ofEpochMilli(maxOf(aMs, bMs)).atZone(zone).toLocalDate()
    return ChronoUnit.DAYS.between(a, b)
}
