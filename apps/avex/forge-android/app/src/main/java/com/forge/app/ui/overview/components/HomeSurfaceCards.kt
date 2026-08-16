package com.forge.app.ui.overview.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import com.forge.app.ui.experiment.CardEyebrow
import com.forge.app.ui.experiment.CardMark
import com.forge.app.ui.experiment.DeltaBadge
import com.forge.app.ui.experiment.GhostCard
import com.forge.app.ui.experiment.GhostTrack
import com.forge.app.ui.experiment.peekCardWidth
import com.forge.app.ui.experiment.SurfaceCard
import com.forge.app.ui.experiment.SurfaceCta
import com.forge.app.ui.experiment.SurfaceMeter
import com.forge.app.ui.experiment.SurfacePalette
import com.forge.app.ui.experiment.SurfaceSparkline
import com.forge.app.ui.experiment.WeekRail

/**
 * design/surface-experiment (2026-08-15) — Home's card bodies.
 *
 * Split out of `OverviewScreen` at the seam §15 asks for (screens ~300 lines): the screen owns
 * state, order and rhythm; this file owns what one card looks like. Everything here is
 * experiment-scoped and disappears with the branch.
 *
 * The whole file exists to argue with §1. `ui/experiment/SurfaceKit.kt` records what that costs.
 */

/** Minimum height of a goal card, so a strip of them keeps one baseline while still growing at 200%. */
val GOAL_CARD_MIN_HEIGHT = 118.dp

/**
 * The hero card: THE PLAN. The day's greeting and date as the eyebrow, the directive as the one
 * serif line on the screen, its reason, and the one action that matters.
 *
 * Rewritten 2026-08-15 (Antho: "the plan should be the focus and the 0 lb should be removed"). The
 * first version led with the week's volume, which put the screen's biggest voice on a number and
 * demoted the coach's answer to a supporting line — and at zero it opened Home with a serif "0".
 * §2① was right: the hero is the screen's most important *answer*, and on Home that is what to
 * train, not how much has been lifted. The volume figure kept its delta and curve; it moved to
 * [HomeVolumeCard] below the tiles, where it is a reading rather than the headline.
 */
@Composable
fun HomeHeroCard(
    palette: SurfacePalette,
    eyebrow: String,
    dateLabel: String,
    headline: String,
    reason: String?,
    secondary: String?,
    ctaText: String,
    ctaLabel: String,
    ctaHint: String?,
    onBg: Color,
    muted: Color,
    modifier: Modifier = Modifier,
    onLongCta: (() -> Unit)? = null,
    longCtaLabel: String? = null,
    onCta: () -> Unit
) {
    SurfaceCard(palette, modifier, padding = PaddingValues(18.dp)) {
        CardEyebrow(eyebrow, muted, trailing = dateLabel, trailingColor = palette.mutedOnCard)
        Spacer(Modifier.height(12.dp))

        // The one serif hero on Home (§2①), clamped at 1.3× so a long day name survives 200%.
        HeroHeadline(headline, onBg)

        if (!reason.isNullOrBlank()) {
            Spacer(Modifier.height(10.dp))
            Text(reason, style = MaterialTheme.typography.bodyMedium, color = palette.mutedOnCard)
        }
        if (!secondary.isNullOrBlank()) {
            Spacer(Modifier.height(4.dp))
            Text(secondary, style = MaterialTheme.typography.bodySmall, color = palette.mutedOnCard)
        }

        Spacer(Modifier.height(18.dp))
        SurfaceCta(
            text = ctaText,
            label = ctaLabel,
            onLongClick = onLongCta,
            longClickLabel = longCtaLabel,
            onClick = onCta
        )
        if (ctaHint != null) {
            Spacer(Modifier.height(8.dp))
            Text(
                ctaHint,
                style = MaterialTheme.typography.bodySmall,
                color = palette.mutedOnCard,
                textAlign = TextAlign.Center,
                modifier = Modifier.fillMaxWidth()
            )
        }
    }
}

/**
 * THIS WEEK's volume: the figure, its delta and the 8-week curve, as a reading rather than the
 * headline.
 *
 * The caller renders this only when there is volume history to show. A card reading "0 LB" over a
 * dead chart is the §12 trap in card form — an honest zero belongs on a figure that sits in a row
 * of siblings, not alone on a surface where it becomes the loudest thing on screen.
 */
@Composable
fun HomeVolumeCard(
    palette: SurfacePalette,
    figure: String,
    unit: String,
    /** Signed percent vs last week; null when there is no comparable week yet (never a fake 0%). */
    deltaPercent: Int?,
    /** Volume per week, oldest → newest. Drawn only when two weeks carry data (§12). */
    series: List<Double>,
    seriesReading: String,
    onBg: Color,
    muted: Color,
    modifier: Modifier = Modifier
) {
    SurfaceCard(palette, modifier, padding = PaddingValues(18.dp)) {
        CardEyebrow("Volume this week", muted)
        Spacer(Modifier.height(8.dp))
        Row(verticalAlignment = Alignment.Bottom) {
            Text(figure, style = MaterialTheme.typography.headlineMedium, color = onBg, maxLines = 1)
            Spacer(Modifier.width(6.dp))
            Text(
                unit,
                style = MaterialTheme.typography.labelMedium,
                color = palette.mutedOnCard,
                modifier = Modifier.padding(bottom = 6.dp)
            )
            Spacer(Modifier.weight(1f))
            if (deltaPercent != null) {
                DeltaBadge(deltaPercent, palette, muted, Modifier.padding(bottom = 6.dp))
            }
        }
        // A lone flat line reads as broken, so the curve appears only once two weeks carry data
        // (§12: a ghost mark shows only beside a live sibling).
        if (series.count { it > 0.0 } >= 2) {
            Spacer(Modifier.height(14.dp))
            SurfaceSparkline(
                values = series,
                color = palette.hues[0],
                reading = seriesReading,
                modifier = Modifier.fillMaxWidth().height(52.dp)
            )
        }
    }
}

/** The directive as the screen's one serif line, clamped at 1.3× exactly as a hero figure is. */
@Composable
private fun HeroHeadline(text: String, onBg: Color) {
    val density = LocalDensity.current
    CompositionLocalProvider(
        LocalDensity provides Density(density.density, density.fontScale.coerceAtMost(1.3f))
    ) {
        Text(
            text,
            // displayLarge, the same rung the shipped Home gave the directive. Inside a card the
            // measure is ~80dp narrower than the open page, so it wraps sooner — which is why the
            // 1.3× clamp and maxLines = 2 matter more here than they did there.
            style = MaterialTheme.typography.displayLarge,
            color = onBg,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis
        )
    }
}

/**
 * One half of the two-up row: a mark, a label, a figure, and the metric's own mark underneath.
 *
 * [mark] must work at zero (§12) — an empty week rail and an empty meter are both real readings,
 * so neither is hidden when there is nothing yet to show.
 */
@Composable
fun HomeTileCard(
    palette: SurfacePalette,
    icon: ImageVector,
    hue: Color,
    label: String,
    figure: String,
    suffix: String?,
    onBg: Color,
    muted: Color,
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
    clickLabel: String = "",
    mark: @Composable () -> Unit
) {
    SurfaceCard(
        palette,
        modifier,
        onClick = onClick,
        clickLabel = clickLabel,
        padding = PaddingValues(16.dp)
    ) {
        CardMark(icon, hue)
        Spacer(Modifier.height(12.dp))
        Text(
            label.uppercase(),
            style = MaterialTheme.typography.labelMedium,
            color = muted,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis
        )
        Spacer(Modifier.height(4.dp))
        Row(verticalAlignment = Alignment.Bottom) {
            Text(figure, style = MaterialTheme.typography.headlineMedium, color = onBg, maxLines = 1)
            if (suffix != null) {
                Spacer(Modifier.width(4.dp))
                Text(
                    suffix,
                    style = MaterialTheme.typography.labelSmall,
                    color = palette.mutedOnCard,
                    modifier = Modifier.padding(bottom = 5.dp)
                )
            }
        }
        Spacer(Modifier.height(14.dp))
        mark()
    }
}

/** A goal as one peek-row card: name, reading, and its progress meter in the goal's own hue. */
@Composable
fun HomeGoalCard(
    palette: SurfacePalette,
    title: String,
    valueLine: String,
    fraction: Float,
    achieved: Boolean,
    hue: Color,
    onBg: Color,
    muted: Color,
    onClick: () -> Unit
) {
    val percent = (fraction.coerceIn(0f, 1f) * 100).toInt()
    SurfaceCard(
        palette,
        Modifier.width(peekCardWidth()),
        onClick = onClick,
        clickLabel = "Open goals",
        padding = PaddingValues(14.dp),
        minHeight = GOAL_CARD_MIN_HEIGHT
    ) {
        // No maxLines: a goal's name is user content, and §14 forbids truncating that. The card
        // grows instead — it is the strip, not the page, so one tall card costs nothing.
        Text(title, style = MaterialTheme.typography.bodyMedium, color = onBg)
        Spacer(Modifier.height(4.dp))
        Text(
            valueLine,
            style = MaterialTheme.typography.labelSmall,
            color = palette.mutedOnCard,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis
        )
        Spacer(Modifier.height(14.dp))
        SurfaceMeter(
            fraction = fraction,
            color = if (achieved) palette.positive else hue,
            track = muted.copy(alpha = 0.25f),
            reading = "$title, $percent percent of target",
            modifier = Modifier.fillMaxWidth()
        )
        Spacer(Modifier.height(6.dp))
        Text(
            if (achieved) "REACHED" else "$percent%",
            style = MaterialTheme.typography.labelSmall,
            color = if (achieved) palette.positive else palette.mutedOnCard,
            maxLines = 1
        )
    }
}

/**
 * The zero-shape for the goals strip: cards that dissolve off the end edge exactly as the real ones
 * run off it, the lead one carrying an empty meter track — the goal card's own mark at zero
 * (§2②: "empty track, honest 0").
 */
@Composable
fun HomeGoalGhost(
    palette: SurfacePalette,
    index: Int,
    muted: Color,
    onBg: Color,
    onClick: () -> Unit
) {
    GhostCard(
        palette = palette,
        index = index,
        muted = muted,
        onBg = onBg,
        minHeight = GOAL_CARD_MIN_HEIGHT,
        prompt = if (index == 0) "First goal" else null,
        caption = if (index == 0) "Pick a lift and a number to work toward." else null,
        onClick = onClick,
        clickLabel = "Set your first goal"
    ) {
        GhostTrack(muted, depth = if (index == 0) 1f else 0.5f)
    }
}

/**
 * The zero-shape for RECENT: the week as seven cells, all unlit, with the line that says what fills
 * them.
 *
 * Replaces a three-row skeleton of grey bars (2026-08-15, Antho: "Recent looks like a loading
 * asset"). That was the right diagnosis — §12 bans skeletons here outright, because the local DB is
 * instant and nothing ever loads, so bars-where-text-goes can only mean "broken" or "waiting".
 *
 * The rail is an honest mark instead: it is the same seven-cell shape the WORKOUTS tile uses, at
 * zero, which is exactly what §12 asks for — the section's own visual, drawn empty, in the same
 * vocabulary it will use with data.
 */
@Composable
fun HomeRecentZero(
    palette: SurfacePalette,
    todayIndex: Int,
    hint: String,
    muted: Color
) {
    Column(Modifier.fillMaxWidth().padding(vertical = 10.dp)) {
        WeekRail(
            trained = emptySet(),
            todayIndex = todayIndex,
            fill = palette.hues[0],
            track = muted.copy(alpha = 0.25f),
            reading = "No sessions this week",
            modifier = Modifier.fillMaxWidth()
        )
        Spacer(Modifier.height(14.dp))
        Text(hint, style = MaterialTheme.typography.bodySmall, color = palette.mutedOnCard)
    }
}

/** TROPHIES as a card: the mark, the count, and an n-of-m meter against the REAL total (§12). */
@Composable
fun HomeTrophiesCard(
    palette: SurfacePalette,
    icon: ImageVector,
    unlocked: Int,
    total: Int,
    nearMiss: String?,
    onBg: Color,
    muted: Color,
    modifier: Modifier = Modifier,
    onClick: () -> Unit
) {
    SurfaceCard(palette, modifier, onClick = onClick, clickLabel = "Open trophies") {
        Row(verticalAlignment = Alignment.CenterVertically) {
            CardMark(icon, palette.hues[2])
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text("TROPHIES", style = MaterialTheme.typography.labelMedium, color = muted)
                Spacer(Modifier.height(2.dp))
                Text("$unlocked of $total unlocked", style = MaterialTheme.typography.bodyMedium, color = onBg)
            }
        }
        Spacer(Modifier.height(12.dp))
        SurfaceMeter(
            fraction = if (total <= 0) 0f else unlocked.toFloat() / total,
            color = palette.hues[2],
            track = muted.copy(alpha = 0.25f),
            reading = "$unlocked of $total trophies unlocked",
            modifier = Modifier.fillMaxWidth()
        )
        if (nearMiss != null) {
            Spacer(Modifier.height(10.dp))
            Text(
                "NEXT · ${nearMiss.uppercase()}",
                style = MaterialTheme.typography.labelSmall,
                color = palette.mutedOnCard,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis
            )
        }
    }
}

/** A quiet full-width card for one throwaway fact (ON THIS DAY, a coach nudge). */
@Composable
fun HomeNoteCard(
    palette: SurfacePalette,
    eyebrow: String,
    body: String,
    onBg: Color,
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
    clickLabel: String = ""
) {
    SurfaceCard(palette, modifier, onClick = onClick, clickLabel = clickLabel) {
        Text(
            eyebrow.uppercase(),
            style = MaterialTheme.typography.labelSmall,
            color = palette.mutedOnCard,
            maxLines = 1
        )
        Spacer(Modifier.height(4.dp))
        Text(body, style = MaterialTheme.typography.bodyMedium, color = onBg)
    }
}

/** The week rail sized for a tile card. Separated so the tile itself stays layout-only. */
@Composable
fun TileWeekRail(trained: Set<Int>, todayIndex: Int, hue: Color, muted: Color, reading: String) {
    WeekRail(
        trained = trained,
        todayIndex = todayIndex,
        fill = hue,
        track = muted.copy(alpha = 0.25f),
        reading = reading,
        modifier = Modifier.fillMaxWidth()
    )
}

/** A tile's meter, with its own spoken reading. */
@Composable
fun TileMeter(fraction: Float, hue: Color, muted: Color, reading: String) {
    SurfaceMeter(
        fraction = fraction,
        color = hue,
        track = muted.copy(alpha = 0.25f),
        reading = reading,
        modifier = Modifier.fillMaxWidth()
    )
}
