@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package com.forge.app.ui.profile

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
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
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.photo.PhotoPose
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.formatWeightDelta
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.forgeShimmer
import com.forge.app.ui.common.rememberDrawProgress
import com.forge.app.ui.common.statsEntrance
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
internal fun GalleryHero(photos: List<ProgressPhoto>, loading: Boolean, onBg: Color, muted: Color) {
    val eyebrow = remember(photos, loading) {
        galleryEyebrow(photos.size, photos.minOfOrNull { it.takenAtMs }, loading)
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
 * count — one photo fills FIRST beside a ghost NOW (the slot the next shot lands in), none shows two
 * ghost frames — so this is the screen's mark at zero, not a text row (§12). Both ends keep their
 * FIRST / NOW tag empty or full, so the band says what it is without a sentence explaining it.
 * While the index is still loading the frames sit as shimmer plates: an add prompt shown over photos
 * that simply haven't been read off disk yet is a lie, and it was the first thing the screen said.
 */
@Composable
internal fun ProgressBand(
    before: ProgressPhoto?,
    after: ProgressPhoto?,
    loading: Boolean,
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
        pair -> Modifier.fillMaxWidth().bounceClick { onCompare(before!!, after!!) }
        else -> Modifier.fillMaxWidth().bounceClick { onAdd() }
    }

    Row(mod, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        BandFrame("FIRST", before, loading, fileFor, muted, outline, Modifier.weight(1f))
        BandMeta(before, after, zone, weightUnit, pair, muted, accent, onBg)
        BandFrame("NOW", after, loading, fileFor, muted, outline, Modifier.weight(1f))
    }
    if (!loading && !pair) {
        Spacer(Modifier.height(8.dp))
        // The one hint this lens spends (§12) — it captions the ghost frame rather than replacing it.
        InlineEmptyHint(
            if (before == null && after == null) "Add your first shot to start the timeline."
            else "Add a second shot and the compare opens.",
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
    outline: Color,
    modifier: Modifier = Modifier
) {
    // Rounded 16 — the photo idiom (§7), a step up from the grid's 12 so the band reads as the hero.
    Box(modifier.aspectRatio(0.8f).clip(RoundedCornerShape(16.dp))) {
        when {
            photo != null -> {
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
            }
            loading -> Box(Modifier.matchParentSize().forgeShimmer())
            else -> {
                // Empty end: the same frame, the same tag position, nothing in it (§12 ghost visual).
                // Border off the MUTED ramp at §12's `muted@0.55`, not the outline one: these frames
                // are this page's entire mark at zero, and at outline 0.35 the border measured 1.13:1
                // against the page on device — a frame you cannot see is a blank page, not a ghost.
                Box(
                    Modifier.matchParentSize()
                        .border(1.dp, muted.copy(alpha = 0.55f), RoundedCornerShape(16.dp)),
                    contentAlignment = Alignment.Center
                ) {
                    Text("＋", style = MaterialTheme.typography.headlineSmall, color = muted.copy(alpha = 0.6f))
                }
                Text(
                    tag,
                    style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.7f),
                    fontSize = 8.sp, letterSpacing = 1.sp,
                    modifier = Modifier.align(Alignment.BottomStart).padding(horizontal = 9.dp, vertical = 8.dp)
                )
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

// ── Zero-state start block ───────────────────────────────────────────────────

/**
 * What an empty Gallery offers instead of a blank page: the one filled capsule (§8 ①) that opens the
 * camera-or-import chooser, and — only when albums already exist without photos in them — the way
 * back into them. Shown at zero alone; once a photo lands the top-bar `+` and the band carry adding,
 * so the capsule leaves rather than sitting over a live grid.
 */
@Composable
internal fun GalleryStart(onAdd: () -> Unit, onOpenAlbums: (() -> Unit)?, accent: Color) {
    ForgePrimaryCapsule("Add a photo", onClick = onAdd, modifier = Modifier.fillMaxWidth())
    if (onOpenAlbums != null) {
        Spacer(Modifier.height(14.dp))
        Text(
            "Albums →",
            style = MaterialTheme.typography.labelSmall, color = accent,
            modifier = Modifier.bounceClick { onOpenAlbums() }.padding(vertical = 4.dp)
        )
    }
}

// ── Timeline (lens pills + tools + grid) ─────────────────────────────────────

/**
 * The browse half of the overview: the TIMELINE anchor (with "Albums →" as its header action), the
 * pose lens pills, the search / filters / compare text pills, and the month-grouped grid underneath.
 *
 * Every control here is gated on there being something for it to do — pose pills need two poses to
 * choose between, Search and Filters need more photos than fit on one screen ([GALLERY_TOOLS_MIN]),
 * Compare needs two shots. A control that can only ever return the same grid is an affordance that
 * does nothing (§4.5), which is exactly how this section read with one photo in it.
 */
@Composable
internal fun GalleryTimeline(
    photos: List<ProgressPhoto>,
    visiblePhotos: List<ProgressPhoto>,
    poses: List<PhotoPose>,
    tools: GalleryTools,
    zone: ZoneId,
    entrance: Int,
    onOpenAlbums: () -> Unit,
    onStartCompare: () -> Unit,
    onView: (ProgressPhoto) -> Unit,
    fileFor: (ProgressPhoto) -> File,
    onBg: Color, muted: Color, accent: Color, outline: Color
) {
    // Tool pills — one pill vocabulary with the range chips below, no stock icons in content (§8).
    val showSift = photos.size >= GALLERY_TOOLS_MIN
    val showCompare = photos.size >= 2
    val hasControls = poses.size >= 2 || showSift || showCompare

    Column(Modifier.fillMaxWidth().statsEntrance(entrance)) {
        EditorialHeader("Timeline", muted, accent, action = "Albums →", onAction = onOpenAlbums)
        Spacer(Modifier.height(10.dp))

        // Lens pills: only with two poses to switch between (one pose + "All" filters nothing).
        if (poses.size >= 2) {
            Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                SegmentPill("All", selected = tools.pose == null, onClick = { tools.onPoseChange(null) }, accent, onBg, muted, outline)
                poses.forEach { p ->
                    SegmentPill(p.label, selected = tools.pose == p, onClick = { tools.onPoseChange(p) }, accent, onBg, muted, outline)
                }
            }
            Spacer(Modifier.height(10.dp))
        }

        // The search field stands ALWAYS OPEN once the library is worth searching (2026-07-25). It
        // used to hide behind a "Search" chip, which is a tool-drawer idiom — a photo library's
        // search is the thing you reach for first, and a gallery that makes you find its search
        // doesn't read as a gallery. Its placeholder names what it matches, so the fields it covers
        // (title, note, pose, date) are discoverable without a caption explaining them.
        if (showSift) {
            GallerySearchBar(tools.query, tools.onQueryChange, focusRequester = tools.searchFocus)
            Spacer(Modifier.height(10.dp))
        }

        if (showSift || showCompare) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (showSift) {
                    GalleryChip("Filters", selected = tools.filtersOpen || tools.filtersActive) { tools.onToggleFilters() }
                }
                if (showCompare) GalleryChip("Compare", selected = false) { onStartCompare() }
            }
        }

        if (showSift && tools.filtersOpen) {
            Spacer(Modifier.height(10.dp))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                GalleryRange.entries.forEach { r -> GalleryChip(r.label, selected = r == tools.range) { tools.onRangeChange(r) } }
            }
            Spacer(Modifier.height(10.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                GalleryChip(
                    if (tools.sort == GallerySort.NEWEST) "Newest first" else "Oldest first",
                    selected = false
                ) { tools.onToggleSort() }
                GalleryChip("${tools.columns} across", selected = false) { tools.onCycleColumns() }
            }
        }
    }

    // With no controls drawn the header's own 10dp is already the section's air (§7).
    if (hasControls) Spacer(Modifier.height(16.dp))
    Column(Modifier.fillMaxWidth().statsEntrance(entrance + 1)) {
        if (visiblePhotos.isEmpty()) {
            // A narrowed-to-nothing grid always carries its way out, so the view is never a dead end.
            InlineEmptyHint(
                if (tools.searching) "Nothing matches “${tools.query}”." else "No photos in this window.",
                muted
            )
            if (tools.narrowed) {
                Spacer(Modifier.height(12.dp))
                GalleryChip(if (tools.searching) "Clear search" else "Clear filters", selected = false) { tools.onClear() }
            }
            return@Column
        }
        if (tools.searching) {
            Text(
                "${visiblePhotos.size} result${if (visiblePhotos.size == 1) "" else "s"}",
                style = MaterialTheme.typography.labelMedium, color = muted
            )
            Spacer(Modifier.height(12.dp))
        }
        // A lone cell three-across reads as debris (§12) — thin galleries widen their cells instead.
        val columns = if (visiblePhotos.size <= 2) minOf(tools.columns, 2) else tools.columns
        DayGroupedGrid(visiblePhotos, columns, zone, fileFor, muted, accent, onView)
    }
}

// ── Bodyweight sparkline ─────────────────────────────────────────────────────

/**
 * The bodyweight-through-time section under the band — the trend the photos are set against. A mono
 * section anchor, the latest reading as a small serif figure, and the line drawing in left-to-right
 * once (§9/§10). Only rendered with ≥2 weigh-ins (a lone point isn't a trend); the band already
 * carries the per-shot weight delta, so nothing is repeated when this is absent. Photo dates ride
 * the bottom axis as faint ticks — an empty [photos] just means no ticks, which is why this section
 * also draws on a photo-less gallery: it is the one live mark beside the band's ghost frames (§12).
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
    // A lone shot IS the first one — it fills FIRST and the empty NOW frame is where the next lands.
    // (It used to sit under NOW with a ghost FIRST, which read as a missing past rather than a start.)
    if (photos.size == 1) return newest to null
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
