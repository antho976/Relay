package com.forge.app.ui.profile

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.units.WeightUnit
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.statsEntrance
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.time.ZoneId

/**
 * The Gallery's default level, emitted into the screen's lazy list: masthead, browse bar, the
 * day-grouped grid, and the two readings that close the roll.
 *
 * It is a [LazyListScope] extension rather than a composable so the grid's rows are the list's own
 * items. Nesting a scrolling grid inside a scrolling page would either fix the grid's height (and
 * strand the rest of the library below a scroll trap) or compose every cell at once, which is the
 * exact stall this revamp removes.
 *
 * Kept apart from [MirrorTestScreen] so the composition can be rendered from plain data — the
 * screen owns the ViewModel, the pickers and the modals; this owns what the page looks like.
 */
@Suppress("LongParameterList")
internal fun LazyListScope.galleryLibrary(
    loading: Boolean,
    photos: List<ProgressPhoto>,
    visiblePhotos: List<ProgressPhoto>,
    days: List<GalleryDay>,
    knownTags: List<String>,
    tools: GalleryTools,
    bodyweight: List<BodyweightEntry>,
    bandBefore: ProgressPhoto?,
    bandAfter: ProgressPhoto?,
    zone: ZoneId,
    weightUnit: WeightUnit,
    fileFor: (ProgressPhoto) -> File,
    onOpenAlbums: () -> Unit,
    onStartCompare: () -> Unit,
    onLongPressPhoto: ((ProgressPhoto) -> Unit)?,
    onCompare: (ProgressPhoto, ProgressPhoto) -> Unit,
    onAdd: () -> Unit,
    onView: (ProgressPhoto) -> Unit,
    onBg: Color, muted: Color, accent: Color, outline: Color, background: Color
) {
    val filter = tools.filter

    item(key = "masthead") {
        Gutter {
            GalleryMasthead(
                loading = loading,
                photos = photos,
                before = bandBefore,
                after = bandAfter,
                lens = filter.soleMuscle,
                zone = zone,
                weightUnit = weightUnit,
                onOpenAlbums = onOpenAlbums,
                onBandCompare = onCompare,
                onAdd = onAdd,
                fileFor = fileFor,
                onBg = onBg, muted = muted, accent = accent, outline = outline
            )
        }
    }

    if (loading) return

    item(key = "browse") {
        Gutter {
            Spacer(Modifier.height(20.dp))
            GalleryFilterBar(
                knownTags = knownTags,
                tools = tools,
                onStartCompare = onStartCompare,
                onBg = onBg, muted = muted, accent = accent, outline = outline
            )
            Spacer(Modifier.height(if (filter.searching) 16.dp else 20.dp))
            if (filter.searching) {
                Text(
                    "${visiblePhotos.size} result${if (visiblePhotos.size == 1) "" else "s"}",
                    style = MaterialTheme.typography.labelMedium, color = muted
                )
                Spacer(Modifier.height(12.dp))
            }
        }
    }

    if (visiblePhotos.isEmpty()) {
        // An empty library is NOT an empty result: the masthead's ghost frames and its one filled
        // capsule already say "nothing here yet", and a second line saying it again would be the
        // page repeating itself (§4.3). Only a grid narrowed to nothing needs its way back out.
        if (photos.isNotEmpty()) {
            item(key = "no-results") {
                Gutter {
                    InlineEmptyHint(
                        if (filter.searching) "Nothing matches “${filter.query}”."
                        else "No photos match these filters.",
                        muted
                    )
                    if (filter.narrowed) {
                        Spacer(Modifier.height(12.dp))
                        Row { GalleryChip("Clear filters", selected = false) { tools.onClear() } }
                    }
                }
            }
        }
    } else {
        // A lone cell three-across reads as debris (§12). The day is the grid's row unit, so the
        // density that matters is the busiest DAY on screen, not the library's total: filter to one
        // muscle and every day may hold a single shot, which at three across is one speck beside two
        // thirds of nothing. Never drops below two, because a one-across grid is a list.
        val busiestDay = days.maxOfOrNull { it.photos.size } ?: 1
        galleryGrid(
            days,
            GalleryGridSpec(
                columns = filter.columns.coerceAtMost(busiestDay.coerceAtLeast(2)),
                fileFor = fileFor,
                onPhotoClick = onView,
                onPhotoLongClick = onLongPressPhoto,
                lensMuscle = filter.soleMuscle
            ),
            muted, accent, background
        )
    }

    item(key = "roll-end") {
        Gutter {
            Spacer(Modifier.height(8.dp))
            GalleryRollEnd(
                photos = photos,
                bodyweight = bodyweight,
                before = bandBefore,
                after = bandAfter,
                zone = zone,
                weightUnit = weightUnit,
                onCompare = onCompare,
                fileFor = fileFor,
                onBg = onBg, muted = muted, accent = accent
            )
        }
    }
}

/** The page gutter (§7). The grid draws its own, because its rows are items of the lazy list. */
@Composable
internal fun Gutter(content: @Composable () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(horizontal = GALLERY_GUTTER)) { content() }
}

/**
 * The masthead: eyebrow, serif hero, the progress band, and the way into Albums.
 *
 * At ZERO the band's tagged ghost frames are the mark and one filled capsule is the way in (§12); at
 * ONE the shot fills FIRST with the empty NOW frame beside it. The page never ends after the hero.
 */
@Composable
private fun GalleryMasthead(
    loading: Boolean,
    photos: List<ProgressPhoto>,
    before: ProgressPhoto?,
    after: ProgressPhoto?,
    lens: com.forge.app.program.MuscleGroup?,
    zone: ZoneId,
    weightUnit: WeightUnit,
    onOpenAlbums: () -> Unit,
    onBandCompare: (ProgressPhoto, ProgressPhoto) -> Unit,
    onAdd: () -> Unit,
    fileFor: (ProgressPhoto) -> File,
    onBg: Color, muted: Color, accent: Color, outline: Color
) {
    Column(Modifier.fillMaxWidth().statsEntrance(0)) {
        // The eyebrow line doubles as the library's index row: the count and span on the left, the
        // way into Albums on the right. It used to sit under the band as its own stranded line.
        GalleryHero(photos, loading, onBg, muted, action = if (loading) null else ({ onOpenAlbums() }))
        Spacer(Modifier.height(18.dp))
        // Signature mark — works at zero (ghost frames + add prompt), so no separate empty text row.
        ProgressBand(
            before = before, after = after, loading = loading, lens = lens, zone = zone,
            weightUnit = weightUnit, fileFor = fileFor, onCompare = onBandCompare, onAdd = onAdd,
            onBg = onBg, muted = muted, accent = accent, outline = outline
        )
        if (!loading && photos.isEmpty()) {
            Spacer(Modifier.height(18.dp))
            GalleryStart(onAdd, accent)
        }
    }
}

/**
 * What closes the roll: the bodyweight trend the photos are set against, then the auto-paired
 * "scale held, body changed" shots. Both are readings ABOUT the library rather than part of it, so
 * they sit after it rather than between you and it (§4.8 — placement is rank).
 */
@Composable
private fun GalleryRollEnd(
    photos: List<ProgressPhoto>,
    bodyweight: List<BodyweightEntry>,
    before: ProgressPhoto?,
    after: ProgressPhoto?,
    zone: ZoneId,
    weightUnit: WeightUnit,
    onCompare: (ProgressPhoto, ProgressPhoto) -> Unit,
    fileFor: (ProgressPhoto) -> File,
    onBg: Color, muted: Color, accent: Color
) {
    if (bodyweight.size >= 2) {
        Spacer(Modifier.height(12.dp))
        BodyweightSparkline(bodyweight, photos, weightUnit, onBg, muted, accent)
    }

    // Pairing is O(n²) over every weighed photo, so it runs off the main thread (like the figure and
    // decode paths) to keep scrolling smooth on large galleries; empty until the first pass lands.
    val samePairs by produceState(emptyList<SameWeightPair>(), photos, before, after) {
        value = withContext(Dispatchers.Default) {
            sameWeightPairs(photos, zone, setOfNotNull(before?.fileName, after?.fileName))
        }
    }
    if (samePairs.isNotEmpty()) {
        Spacer(Modifier.height(28.dp))
        SameWeightSection(samePairs, zone, weightUnit, fileFor, onCompare, muted, accent)
    }
}
