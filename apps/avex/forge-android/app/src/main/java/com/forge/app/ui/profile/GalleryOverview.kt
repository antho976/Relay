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
import androidx.compose.foundation.layout.widthIn
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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.formatWeightDelta
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.currentLocale
import com.forge.app.ui.common.forgeShimmer
import com.forge.app.ui.common.rememberDrawProgress
import com.forge.app.ui.theme.ForgeMotion
import java.io.File
import java.text.SimpleDateFormat
import java.time.ZoneId
import java.util.Date

// ── Hero ───────────────────────────────────────────────────────────────────

/**
 * The screen's serif name over a mono eyebrow carrying the count and span (§3), with the way into
 * Albums riding the end of that same line when there is a library to have albums of.
 */
@Composable
internal fun GalleryHero(
    photos: List<ProgressPhoto>,
    loading: Boolean,
    onBg: Color,
    muted: Color,
    action: (() -> Unit)? = null
) {
    val eyebrow = remember(photos, loading) { galleryEyebrow(photos, loading) }
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Top) {
        Text(eyebrow, style = MaterialTheme.typography.labelMedium, color = muted, modifier = Modifier.weight(1f))
        if (action != null) {
            Spacer(Modifier.width(12.dp))
            Text(
                "Albums →",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.bounceClick { action() }
            )
        }
    }
    Spacer(Modifier.height(4.dp))
    // Serif hero — the screen names itself here, so the top bar carries only its controls (§2). No period.
    Text("Gallery", style = MaterialTheme.typography.headlineLarge, color = onBg)
}

// ── Progress band (the signature mark) ───────────────────────────────────────

/**
 * The library's headline comparison: the first shot beside the latest, with the span, the weight
 * change and a compare affordance between them. Tapping it opens the slider compare.
 *
 * **It reads whatever the grid is lensed to.** With one muscle chip active the band re-pairs inside
 * that muscle and [lens] names it, so the mark answers "how has my back changed" and not only "how
 * have I changed" — which is the whole reason a physique library is worth tagging by muscle at all.
 *
 * Drawn at every count: one photo fills FIRST beside a ghost NOW (the slot the next shot lands in),
 * none shows two ghost frames, so this is the screen's mark at zero rather than a text row (§12).
 * Both ends keep their FIRST / NOW tag empty or full, so the band says what it is without a sentence
 * explaining it. While the index is still loading the frames sit as shimmer plates: an add prompt
 * shown over photos that simply have not been read off disk yet is a lie, and it was the first thing
 * the screen said.
 */
@Composable
internal fun ProgressBand(
    before: ProgressPhoto?,
    after: ProgressPhoto?,
    loading: Boolean,
    lens: MuscleGroup?,
    zone: ZoneId,
    weightUnit: WeightUnit,
    fileFor: (ProgressPhoto) -> File,
    onCompare: (ProgressPhoto, ProgressPhoto) -> Unit,
    onAdd: () -> Unit,
    onBg: Color, muted: Color, accent: Color, outline: Color
) {
    val pair = before != null && after != null && before.fileName != after.fileName
    val mod = when {
        loading -> Modifier.fillMaxWidth()
        pair -> Modifier.fillMaxWidth().bounceClick { onCompare(before, after) }
        else -> Modifier.fillMaxWidth().bounceClick { onAdd() }
    }

    Row(mod, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        BandFrame("FIRST", before, loading, fileFor, muted, Modifier.weight(1f))
        BandMeta(before, after, lens, zone, weightUnit, pair, muted, accent, onBg)
        BandFrame("NOW", after, loading, fileFor, muted, Modifier.weight(1f))
    }
    if (!loading && !pair) {
        Spacer(Modifier.height(8.dp))
        // The one hint this lens spends (§12) — it captions the ghost frame rather than replacing it.
        InlineEmptyHint(
            when {
                lens != null -> "Tag a second ${lens.displayName.lowercase()} shot and the compare opens."
                before == null && after == null -> "Add your first shot to start the timeline."
                else -> "Add a second shot and the compare opens."
            },
            muted
        )
    }
}

/** One end of the band: a portrait photo (dated, tagged) or a tagged ghost frame when nothing's there. */
@Composable
private fun BandFrame(
    tag: String,
    photo: ProgressPhoto?,
    loading: Boolean,
    fileFor: (ProgressPhoto) -> File,
    muted: Color,
    modifier: Modifier = Modifier
) {
    // Rounded 16 — the photo idiom (§7), a step up from the grid's 12 so the band reads as the hero.
    // 0.86 rather than the old 0.8: gallery-first means the grid starts as high as the mark allows,
    // and the frames lost 12dp of height each without losing the portrait read.
    Box(modifier.aspectRatio(0.86f).clip(RoundedCornerShape(16.dp))) {
        when {
            photo != null -> {
                ProgressPhotoImage(fileFor(photo), Modifier.fillMaxSize(), reqPx = 480)
                Box(
                    Modifier.matchParentSize().background(
                        Brush.verticalGradient(0.55f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.6f))
                    )
                )
                Column(Modifier.align(Alignment.BottomStart).padding(horizontal = 9.dp, vertical = 8.dp)) {
                    Text(tag, style = MaterialTheme.typography.labelSmall, color = Color.White.copy(alpha = 0.7f))
                    Text(
                        SimpleDateFormat("MMM d", currentLocale()).format(Date(photo.takenAtMs)).uppercase(),
                        style = MaterialTheme.typography.labelMedium, color = Color.White
                    )
                }
            }
            loading -> Box(Modifier.matchParentSize().forgeShimmer())
            else -> {
                // Empty end: the same frame, the same tag position, nothing in it (§12 ghost visual).
                // Border off the MUTED ramp, not the outline one: these frames are this page's entire
                // mark at zero, and at outline 0.35 the border measured 1.13:1 against the page on
                // device — a frame you cannot see is a blank page, not a ghost.
                Box(
                    Modifier.matchParentSize()
                        .border(1.dp, muted.copy(alpha = 0.6f), RoundedCornerShape(16.dp)),
                    contentAlignment = Alignment.Center
                ) {
                    Text("＋", style = MaterialTheme.typography.headlineSmall, color = muted.copy(alpha = 0.6f))
                }
                Text(
                    tag,
                    style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.7f),
                    modifier = Modifier.align(Alignment.BottomStart).padding(horizontal = 9.dp, vertical = 8.dp)
                )
            }
        }
    }
}

/**
 * The middle column: the muscle the band is read through when it is lensed to one, the span as a
 * small serif figure ("4" over "MONTHS"), the weight change as a direction-only line (never a
 * verdict — §11), and the `compare →` affordance that says the whole band taps. Pre-pair it is just
 * a quiet arrow between the frame and its ghost.
 */
@Composable
private fun BandMeta(
    before: ProgressPhoto?,
    after: ProgressPhoto?,
    lens: MuscleGroup?,
    zone: ZoneId,
    weightUnit: WeightUnit,
    pair: Boolean,
    muted: Color,
    accent: Color,
    onBg: Color
) {
    Column(
        // No fixed width. At 200% font scale an 84dp column broke "compare →" across two lines
        // mid-word; §14's rule against a fixed height on a text container is the same rule sideways.
        Modifier.widthIn(min = 72.dp).padding(horizontal = 4.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(3.dp)
    ) {
        // The lens names itself above the figure, so a narrowed band never silently claims to be
        // the whole library's story.
        if (lens != null) {
            Text(
                lens.displayName.uppercase(),
                style = MaterialTheme.typography.labelSmall, color = accent, textAlign = TextAlign.Center
            )
            Spacer(Modifier.height(2.dp))
        }
        if (!pair || before == null || after == null) {
            Text("→", style = MaterialTheme.typography.titleMedium, color = muted.copy(alpha = 0.6f))
            return@Column
        }
        val span = gallerySpanLabel(before.takenAtMs, after.takenAtMs, zone)
        val parts = span.split(' ', limit = 2)
        if (parts.size == 2) {
            // Serif figure + mono caption — the EditorialFigure shape, centered between the frames.
            Text(parts[0], style = MaterialTheme.typography.headlineMedium, color = onBg)
            Text(
                parts[1].uppercase(),
                style = MaterialTheme.typography.labelSmall, color = muted, textAlign = TextAlign.Center
            )
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
                color = if (kotlin.math.abs(diff) < 0.1) muted else onBg,
                textAlign = TextAlign.Center
            )
        }
        Spacer(Modifier.height(5.dp))
        Text("compare →", style = MaterialTheme.typography.labelSmall, color = accent)
    }
}

// ── Zero-state start block ───────────────────────────────────────────────────

/**
 * What an empty Gallery offers instead of a blank page: the one filled capsule (§8 ①) that opens the
 * camera-or-import chooser. Shown at zero alone; once a photo lands the top-bar `+` and the band
 * carry adding, so the capsule leaves rather than sitting over a live grid. Albums used to hang off
 * this block when any existed; it rides the hero line at every count now, so there is one door and
 * it is always in the same place.
 */
@Composable
internal fun GalleryStart(onAdd: () -> Unit, accent: Color) {
    ForgePrimaryCapsule("Add a photo", onClick = onAdd, modifier = Modifier.fillMaxWidth())
}

// ── Bodyweight sparkline ─────────────────────────────────────────────────────

/**
 * The bodyweight-through-time section — the trend the photos are set against. A mono section anchor,
 * the latest reading as a small serif figure, and the line drawing in left-to-right once (§9/§10).
 * Only rendered with ≥2 weigh-ins (a lone point is not a trend); the band already carries the
 * per-shot weight delta, so nothing is repeated when this is absent. Photo dates ride the bottom
 * axis as faint ticks — an empty [photos] just means no ticks, which is why this also draws on a
 * photo-less gallery: it is the one live mark beside the band's ghost frames (§12).
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
    val reading = "${formatWeight(latest, weightUnit)} now, " +
        if (kotlin.math.abs(diff) < 0.1) "level since the first weigh-in"
        else "${formatWeightDelta(kotlin.math.abs(diff), weightUnit)} ${if (diff > 0) "up" else "down"} since the first weigh-in"

    EditorialHeader("Bodyweight", muted, accent)
    Spacer(Modifier.height(10.dp))
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Column {
            Text(formatWeight(latest, weightUnit), style = MaterialTheme.typography.headlineSmall, color = onBg)
            if (kotlin.math.abs(diff) >= 0.1) {
                Spacer(Modifier.height(2.dp))
                Text(
                    "${if (diff > 0) "↑" else "↓"} ${formatWeightDelta(kotlin.math.abs(diff), weightUnit)} since first",
                    style = MaterialTheme.typography.labelSmall, color = muted
                )
            }
        }
        Spacer(Modifier.width(16.dp))
        Canvas(Modifier.weight(1f).height(56.dp).semantics { contentDescription = reading }) {
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
                        drawLine(muted.copy(alpha = 0.35f), Offset(x, h - botPad + 1f), Offset(x, h), strokeWidth = 1.5f)
                    }
                }
            }
        }
    }
}
