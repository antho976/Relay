package com.forge.app.ui.experiment

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.sparklineSeries
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.rememberDrawProgress
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.LocalForgeSettings

/**
 * # The surface-experiment kit — `design/surface-experiment`, 2026-08-15
 *
 * A FORK, not a promotion. Every primitive here exists because the card-led direction needs a
 * variant of something the shipped app deliberately does not have, and §1 forbids editing the
 * shared one to get it. Nothing in `ui/common/` was touched; nothing here is imported by any
 * screen other than Home and Profile. Deleting this package restores the app exactly.
 *
 * It is NOT in `ui/common/` on purpose: `DoctrineParityTest.commonInventoryMatchesUiCommon`
 * requires every composable there to be named in `DESIGN.md`, and this branch may not edit the
 * doctrine. Keeping the fork in its own package is what lets that test stay green.
 *
 * ## What this contradicts
 *
 * §1 ("surfaces and borders are EARNED by interactivity") — [SurfaceCard] puts a fill and a
 * hairline around passive content, which is the doctrine's central ban and `FAILURES.md`'s named
 * "boxed passive content". §5 (one accent) — [SurfacePalette] adds colour-as-data. §7 — the 18dp
 * radius is off `Shape.kt`'s 4/8/12/16/24 scale.
 *
 * ## What it does NOT contradict
 *
 * Motion stays on `ForgeMotion` and reduce-motion (§9). Press is still a bounce, never a ripple.
 * Touch targets, wrapping and Canvas semantics stay on §14: cards size to their content with
 * `heightIn(min=)` and never `height()`, because a fixed card height with text inside is exactly
 * §14's clipping failure and cards make it MORE likely, not less.
 */

// ── Palette ───────────────────────────────────────────────────────────────────────────────────

/**
 * The experiment's surface + data colours, resolved against the two theme switches the shipped app
 * already honours (AMOLED, monochrome). Read once per screen and thread it down.
 */
data class SurfacePalette(
    /** The one elevation. Cards do not stack on cards, so there is no second fill. */
    val card: Color,
    /** ~6% white. A boundary, not content — exempt from the contrast floor per §14. */
    val hairline: Color,
    val positive: Color,
    val negative: Color,
    /** Up to three per-metric hues for charts and sparklines. */
    val hues: List<Color>,
    /**
     * Muted at the floor that actually holds ON THE CARD FILL.
     *
     * §5's 0.65 rung is measured on the page (`#0E0E11`, 4.54:1). The card lifts the background,
     * and the same alpha measures **4.39:1 there — it fails AA**. Re-measured, the floor on a card
     * is **0.70 (4.90:1)**. This is the concrete cost of the fill: every caption inside a card has
     * to sit one rung brighter than §5's ladder allows.
     */
    val mutedOnCard: Color,
)

/** Alpha of the on-card muted floor. NOT 0.65 — see [SurfacePalette.mutedOnCard]. */
const val MUTED_ON_CARD_ALPHA = 0.7f

@Composable
fun surfacePalette(): SurfacePalette {
    val settings = LocalForgeSettings.current
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    // Monochrome (Appearance → accent off) must stay monochrome: colour-as-data is the first thing
    // that would break that promise. Direction still reads without it, because the ↑/↓ glyph
    // carries it and the three "hues" separate by tone. Meaning is never gated on colour (§14).
    val mono = !settings.accentEnabled
    return SurfacePalette(
        // On AMOLED the ground is pure black, so the card steps DOWN rather than up — a lit slab on
        // black reads as a panel, the opposite of one quiet elevation.
        card = if (settings.amoledMode) Color(0xFF120F0C) else Color(0xFF1A1512),
        // Warm white, not neutral white: a 6% pure-white edge on a brown-black page reads blue.
        hairline = Color(0xFFFFF3E8).copy(alpha = 0.055f),
        positive = if (mono) onBg else Color(0xFF7FB08C),
        negative = if (mono) muted else Color(0xFFC4756B),
        // The three metric hues were cool (blue / teal / violet) and now fight the warm ground.
        // They are neutral warm tones instead: a leading glyph is wayfinding, not a data channel, so
        // it costs the page nothing to stop spending colour on it. Ember is reserved (2026-08-16)
        // for the four places that carry a decision — see `OverviewScreen`'s colour budget.
        hues = listOf(onBg, muted, muted.copy(alpha = 0.7f)),
        mutedOnCard = muted.copy(alpha = MUTED_ON_CARD_ALPHA),
    )
}

/** Radius near 18dp — deliberately off `Shape.kt`'s 4/8/12/16/24 scale (§7). */
val CardShape = RoundedCornerShape(18.dp)

/** The page gutter a [PeekCardRow] sits on, and the gap between its cards. */
private val PEEK_GUTTER = 24.dp
private val PEEK_GAP = 10.dp

/**
 * How much of the third card must stay on screen. The peek IS the affordance — there is no chevron
 * and no "swipe" instruction — so it has to read as a card that continues, not as a seam.
 *
 * 58dp was measured against the alternative: a fixed 168dp card left **13dp** showing on a 393dp
 * screen, which is a sliver you register as an edge rather than as more content. Antho could not
 * tell the BODY strip scrolled (2026-08-15), and the third card there is WAIST — a metric that was
 * therefore invisible.
 */
private val PEEK_MIN = 58.dp

/**
 * The width one card takes in a [PeekCardRow], derived from the screen so the peek survives.
 *
 * Two rules fight here and the order matters. §14 says a pinned width plus a long name is how text
 * gets truncated at 200%, which argues for growing the card with the font scale. The peek argues
 * for keeping it narrow. So: derive the base from the real screen width with [PEEK_MIN] reserved,
 * let it grow with the font scale, and then clamp it back so at least 32dp of the next card is
 * always showing. Legibility wins the middle ground; discoverability holds the floor.
 */
@Composable
fun peekCardWidth(): Dp {
    val screen = LocalConfiguration.current.screenWidthDp.dp
    // Two cards, one gutter, two gaps, and the peek — whatever is left is split between the cards.
    val base = ((screen - PEEK_GUTTER - PEEK_MIN - PEEK_GAP * 2) / 2).coerceIn(132.dp, 190.dp)
    val scaled = base * LocalDensity.current.fontScale.coerceIn(1f, 1.6f)
    // The hard floor: never let a grown card swallow the affordance entirely.
    val widest = (screen - PEEK_GUTTER - PEEK_GAP * 2 - 32.dp) / 2
    return minOf(scaled, widest.coerceAtLeast(132.dp))
}

// ── The card ──────────────────────────────────────────────────────────────────────────────────

/**
 * One card. Fill + hairline + 18dp radius, no shadow: separation comes from the fill step, not a
 * blur. [onClick] is optional — under this experiment a card is a container rather than a promise
 * of a tap, which is precisely the §1 claim being tested.
 */
@Composable
fun SurfaceCard(
    palette: SurfacePalette,
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
    clickLabel: String = "",
    padding: PaddingValues = PaddingValues(horizontal = 16.dp, vertical = 14.dp),
    minHeight: Dp = 0.dp,
    content: @Composable ColumnScope.() -> Unit
) {
    Column(
        modifier
            .clip(CardShape)
            .background(palette.card)
            .border(1.dp, palette.hairline, CardShape)
            .then(
                if (onClick != null) {
                    Modifier.bounceCombinedClick(onClickLabel = clickLabel, onClick = onClick)
                } else Modifier
            )
            .then(if (minHeight > 0.dp) Modifier.heightIn(min = minHeight) else Modifier)
            .padding(padding),
        content = content
    )
}

/** The card's mono eyebrow: a small-caps label, optionally with a right-aligned reading. */
@Composable
fun CardEyebrow(
    label: String,
    muted: Color,
    trailing: String? = null,
    trailingColor: Color = muted
) {
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            label.uppercase(),
            style = MaterialTheme.typography.labelMedium,
            color = muted,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f, fill = false)
        )
        if (trailing != null) {
            Spacer(Modifier.width(8.dp))
            Text(
                trailing.uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = trailingColor,
                maxLines = 1
            )
        }
    }
}

/**
 * The one serif figure on a screen, with its font scaling clamped at 1.3×.
 *
 * §14 allows exactly this for a hero and forbids it for content: a 52sp display figure at 200%
 * would be 104sp and would either wrap mid-number or shove its delta badge off the card. Everything
 * else on these pages scales all the way.
 */
@Composable
fun HeroFigure(value: String, onBg: Color, modifier: Modifier = Modifier) {
    val density = LocalDensity.current
    CompositionLocalProvider(
        LocalDensity provides Density(density.density, density.fontScale.coerceAtMost(1.3f))
    ) {
        Text(
            value,
            style = MaterialTheme.typography.displayLarge,
            color = onBg,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = modifier
        )
    }
}

/**
 * A product mark: the glyph in a small tinted disc, the way the reference cards lead. The tint is
 * the metric's own hue at the tonal rung, so a mark and its chart agree without a legend.
 *
 * Decorative by §14 — the label beside it already speaks, so the glyph takes a null description.
 */
@Composable
fun CardMark(
    icon: ImageVector,
    hue: Color,
    modifier: Modifier = Modifier,
    /**
     * The disc's edge, and the glyph inside it. Defaulted to what this drew as a fixed 30/16 pair,
     * so every existing call site is byte-identical; the Goals ladder passes a larger pair because
     * its rows grew around it and a 30dp mark became the weakest thing on the row (2026-08-23).
     */
    size: Dp = 30.dp,
    glyphSize: Dp = 16.dp,
) {
    Box(
        modifier
            .size(size)
            .clip(RoundedCornerShape(10.dp))
            .background(hue.copy(alpha = 0.15f)),
        contentAlignment = Alignment.Center
    ) {
        Icon(icon, contentDescription = null, tint = hue, modifier = Modifier.size(glyphSize))
    }
}

/**
 * Colour as data: a signed percentage as ↑/↓ + magnitude, tinted positive/negative.
 *
 * The glyph carries the direction on its own, so this still reads with the accent switched off and
 * for a red-green-blind reader — the tint is a second channel, never the only one.
 */
@Composable
fun DeltaBadge(
    percent: Int,
    palette: SurfacePalette,
    muted: Color,
    modifier: Modifier = Modifier
) {
    val tone = when {
        percent == 0 -> muted
        percent > 0 -> palette.positive
        else -> palette.negative
    }
    Box(
        modifier
            .clip(RoundedCornerShape(50))
            .background(tone.copy(alpha = 0.15f))
            .padding(horizontal = 8.dp, vertical = 3.dp)
    ) {
        Text(
            when {
                percent == 0 -> "· 0%"
                percent > 0 -> "↑ $percent%"
                else -> "↓ ${-percent}%"
            },
            style = MaterialTheme.typography.labelSmall,
            color = tone,
            maxLines = 1
        )
    }
}

// ── Marks ─────────────────────────────────────────────────────────────────────────────────────

/**
 * A small sparkline with a soft area fade, drawn in once via `ForgeMotion.drawTween()` (§9/§10).
 *
 * Forked rather than reused: the profile's own `ProfileSparkline` lives in a package this branch
 * rewrites, and the kit has to keep working when that file is restored.
 *
 * [reading] is what TalkBack speaks — §14 wants the value, not the shape.
 *
 * ## The plot is inset from the canvas, and has to be (2026-08-24)
 *
 * The first version mapped the series straight onto the canvas box: the minimum landed on `y = h`
 * and the maximum on `y = 0`. A stroke is centred on its path, so at both of those the outer half
 * of a 2dp line fell outside the canvas and was clipped — **the flat run at the series minimum
 * rendered at half thickness**, which is exactly what a lifetime-volume curve opens with, and what
 * Antho spotted at the start of the graph ("the line looks less big in height"). The end dot lost
 * its right half to the same edge whenever the last point was also the peak.
 *
 * So the plot area is inset by whatever the widest mark needs — half the stroke, or the dot's
 * radius, whichever is larger — and the series is mapped into what is left. The area fill still
 * closes to the true bottom of the canvas, because a gradient that stops short of the baseline
 * leaves a visible band under it.
 */
@Composable
fun SurfaceSparkline(
    values: List<Double>,
    color: Color,
    reading: String,
    modifier: Modifier = Modifier
) {
    if (values.size < 2) return
    val progress = rememberDrawProgress(key = values, spec = ForgeMotion.drawTween())
    // Reduced to what the chart can actually show, once (P-13) — see [sparklineSeries].
    val plotted = remember(values) { sparklineSeries(values) }
    // The extrema are a property of the DATA, so they belong outside the draw scope: they were
    // recomputed on every recomposition, and the reveal recomposes ~54 times per entry (P-13).
    val min = remember(plotted) { plotted.min() }
    val range = remember(plotted) { (plotted.max() - min).takeIf { it > 0.0 } ?: 1.0 }
    // And so are the paths, once the canvas size is known — which the reveal does not change. Both
    // were rebuilt every frame, point by point, to draw the same shape under a moving clip.
    val geometry = remember(plotted) { SparklineGeometry() }
    Canvas(modifier.semantics { contentDescription = reading }) {
        val w = size.width
        val stroke = 2.dp.toPx()
        val dot = 3.dp.toPx()
        // Half a stroke would clear the line; the dot is fatter, so it sets the inset.
        val inset = maxOf(stroke / 2f, dot)
        geometry.ensure(size, plotted, min, range, inset)
        clipRect(right = (w * progress).coerceAtLeast(0.01f)) {
            drawPath(geometry.area, Brush.verticalGradient(listOf(color.copy(alpha = 0.15f), Color.Transparent)))
            drawPath(geometry.line, color, style = Stroke(width = stroke, cap = StrokeCap.Round))
        }
        // The only per-frame work left: the end dot riding the reveal frontier.
        val fx = (plotted.size - 1) * progress
        val i = fx.toInt().coerceIn(0, plotted.size - 2)
        val t = fx - i
        val ys = geometry.ys
        val y = ys[i] + (ys[i + 1] - ys[i]) * t
        drawCircle(color, radius = dot, center = Offset(geometry.stepX * fx, y))
    }
}

/**
 * The sparkline's geometry, built once per (series, canvas size) rather than once per frame (P-13).
 *
 * Capping the input at 512 points bounded the cost; it did not remove it. A reveal animates only
 * the clip frontier and the end dot, and both `Path`s were still constructed point by point on
 * every one of its frames — for a shape that had not changed since the first. Held in a `remember`
 * keyed on the series, so new data gets a fresh instance and a resize rebuilds in place.
 */
private class SparklineGeometry {
    private var builtFor: androidx.compose.ui.geometry.Size? = null

    var line: Path = Path(); private set
    var area: Path = Path(); private set

    /** The y of every plotted point, so the end dot interpolates without re-deriving them. */
    var ys: FloatArray = FloatArray(0); private set
    var stepX: Float = 0f; private set

    fun ensure(
        size: androidx.compose.ui.geometry.Size,
        plotted: List<Double>,
        min: Double,
        range: Double,
        inset: Float
    ) {
        if (builtFor == size) return
        builtFor = size
        val plotH = (size.height - inset * 2f).coerceAtLeast(1f)
        // Only the right edge needs the horizontal inset — the first point is a line end, the last
        // one carries the dot.
        val plotW = (size.width - inset).coerceAtLeast(1f)
        stepX = plotW / (plotted.size - 1)
        ys = FloatArray(plotted.size) { i ->
            inset + plotH - ((plotted[i] - min) / range * plotH).toFloat()
        }
        line = Path().apply {
            ys.forEachIndexed { i, y -> if (i == 0) moveTo(0f, y) else lineTo(stepX * i, y) }
        }
        area = Path().apply {
            moveTo(0f, size.height)
            ys.forEachIndexed { i, y -> lineTo(stepX * i, y) }
            lineTo(stepX * (ys.size - 1), size.height)
            close()
        }
    }
}

/**
 * One value against a target: a thin filled track. Honest at zero — an empty track, never hidden.
 * A fixed 4dp height is fine here: it holds no text, so §14's "container sizes to its content" does
 * not apply to it.
 */
@Composable
fun SurfaceMeter(
    fraction: Float,
    color: Color,
    track: Color,
    reading: String,
    modifier: Modifier = Modifier
) {
    val f = fraction.coerceIn(0f, 1f)
    Box(
        modifier
            .height(4.dp)
            .clip(RoundedCornerShape(50))
            .background(track)
            .semantics { contentDescription = reading }
    ) {
        if (f > 0f) {
            Box(
                Modifier
                    .fillMaxWidth(f)
                    .fillMaxHeight()
                    .clip(RoundedCornerShape(50))
                    .background(color)
            )
        }
    }
}

/**
 * The week as seven cells: filled where trained, ringed for today, hollow otherwise. Works at zero
 * — all-hollow is a real reading, so the mark never disappears on a fresh week (§12).
 */
@Composable
fun WeekRail(
    trained: Set<Int>,
    todayIndex: Int,
    fill: Color,
    track: Color,
    reading: String,
    modifier: Modifier = Modifier
) {
    Row(
        modifier.semantics(mergeDescendants = true) { contentDescription = reading },
        horizontalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        repeat(7) { i ->
            Box(
                Modifier
                    .weight(1f)
                    .height(6.dp)
                    .clip(RoundedCornerShape(50))
                    .then(
                        when {
                            i in trained -> Modifier.background(fill)
                            i == todayIndex -> Modifier.border(1.dp, fill, RoundedCornerShape(50))
                            else -> Modifier.background(track)
                        }
                    )
            )
        }
    }
}

/**
 * The week as seven day cells with real mass — the mark that replaced Home's WORKOUTS tile.
 *
 * ## Why this exists
 *
 * Home already carried this exact data as [WeekRail]: seven 6dp dashes under a "1 OF 7" figure.
 * Antho picked the same element out of a reference app as one of the two things he liked on it, and
 * could not see it in his own — because at 6dp it is debris, not a mark. Same information, given a
 * day letter and a 26dp cell, becomes the most legible thing on the page, and it answers "how is the
 * week going" without a figure, a label or a sentence.
 *
 * ## Where the colour goes
 *
 * A trained day is filled in `onBg` and carries a check; only TODAY is accent. That is deliberate and
 * it is the whole colour budget of this mark: four trained days in ember would be four competing
 * warm blobs under a warm CTA, and the accent would stop meaning anything (§5). One lit dot in a row
 * of seven says "you are here" at a glance and spends one unit of colour to do it.
 *
 * Works at zero — an untrained week is seven hollow rings with one lit today, which is a real
 * reading, not an empty state (§12).
 */
@Composable
fun WeekStrip(
    /** 0=Mon..6=Sun indices that carry a finished session. */
    trained: Set<Int>,
    todayIndex: Int,
    /** Mon-first day initials, supplied by the caller so the locale/first-day setting stays its call. */
    dayLabels: List<String>,
    reading: String,
    modifier: Modifier = Modifier
) {
    val accent = MaterialTheme.colorScheme.primary
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val bg = MaterialTheme.colorScheme.background
    Row(
        modifier.semantics(mergeDescendants = true) { contentDescription = reading },
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        repeat(7) { i ->
            val done = i in trained
            val isToday = i == todayIndex
            // Past-but-untrained and still-ahead are both hollow, but the future recedes: a missed
            // day should read as a gap, an unreached one as simply not yet. Both were dimmer in the
            // first cut and the whole strip read as "too quiet" (Antho, 2026-08-16).
            val ringAlpha = if (i < todayIndex) 0.55f else 0.30f
            Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                modifier = Modifier.weight(1f)
            ) {
                Text(
                    dayLabels.getOrElse(i) { "" },
                    style = MaterialTheme.typography.labelMedium,
                    // Full muted, not a dimmed rung. The letters are the only thing naming the days,
                    // so they are content, not chrome.
                    color = if (isToday) onBg else muted,
                    maxLines = 1
                )
                Spacer(Modifier.height(8.dp))
                Box(
                    Modifier
                        // No .size(): a cell holds a glyph that scales, so it is sized by a min
                        // constraint and grows at 200% (§14). 34dp, up from 26 — see above.
                        .sizeIn(minWidth = 34.dp, minHeight = 34.dp)
                        .clip(CellShape)
                        .then(
                            when {
                                done -> Modifier.background(onBg)
                                isToday -> Modifier.background(accent)
                                else -> Modifier.border(1.5.dp, muted.copy(alpha = ringAlpha), CellShape)
                            }
                        )
                        // A trained TODAY is filled like any other trained day, so the accent ring
                        // around it is what still says "you are here".
                        .then(
                            if (done && isToday) Modifier.border(2.dp, accent, CellShape) else Modifier
                        ),
                    contentAlignment = Alignment.Center
                ) {
                    if (done) {
                        Text("✓", style = MaterialTheme.typography.titleSmall, color = bg, maxLines = 1)
                    }
                }
            }
        }
    }
}

/**
 * The week cell's shape: a rounded square, not a circle (2026-08-16).
 *
 * Circles read soft and bubbly, which is a health-tracker register rather than a training one, and
 * seven of them in a row read as a progress-dot rail — the thing this element exists to stop being.
 * A squared cell has more visual mass at the same footprint and lines up with the rounded squares the
 * RECENT rows' leading marks already use, so the page shares one geometry.
 */
val CellShape = RoundedCornerShape(12.dp)

// The goal line Home renders is NOT here: it is `ui/goals/GoalsComponents.GoalProgressLine`, the
// same composable the Goals screen uses. Home's old horizontal card carousel was the fork, and
// forking it again as a kit primitive would have put the same element in two places with two
// treatments. "At a glance" and "swipe to see the second one" are contradictions anyway — the strip
// could only ever show one and a half cards (Antho, 2026-08-16).

// ── Rows ──────────────────────────────────────────────────────────────────────────────────────

/**
 * One full-width list row inside a card: leading glyph · label + sub · figure + delta. The WHOLE
 * row is the tap target and nothing inside it is separately clickable — §2③ still holds, because
 * this experiment argues about surfaces, not about nested taps.
 */
@Composable
fun SurfaceListRow(
    icon: ImageVector,
    hue: Color,
    label: String,
    sub: String?,
    figure: String?,
    delta: String?,
    deltaColor: Color,
    onBg: Color,
    muted: Color,
    onClick: (() -> Unit)? = null,
    clickLabel: String = ""
) {
    Row(
        Modifier
            .fillMaxWidth()
            .then(
                if (onClick != null) {
                    Modifier.bounceCombinedClick(onClickLabel = clickLabel, onClick = onClick)
                } else Modifier
            )
            .heightIn(min = 48.dp)
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        CardMark(icon, hue)
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(label, style = MaterialTheme.typography.bodyMedium, color = onBg)
            if (!sub.isNullOrBlank()) {
                Spacer(Modifier.height(2.dp))
                Text(sub, style = MaterialTheme.typography.bodySmall, color = muted)
            }
        }
        if (figure != null) {
            Spacer(Modifier.width(12.dp))
            Column(horizontalAlignment = Alignment.End) {
                Text(figure, style = MaterialTheme.typography.titleSmall, color = onBg)
                if (delta != null) {
                    Spacer(Modifier.height(2.dp))
                    Text(delta, style = MaterialTheme.typography.labelSmall, color = deltaColor, maxLines = 1)
                }
            }
        }
    }
}

// ── Horizontal section ────────────────────────────────────────────────────────────────────────

/**
 * A scrollable row of cards that PEEKS the next one at the end edge, so the scroll is discoverable
 * without a chevron or a "swipe" instruction. The peek IS the affordance.
 *
 * The gutter is carried as content padding so the first card lands on the same rail as everything
 * above it while the strip itself runs to the screen edge.
 */
@Composable
fun PeekCardRow(
    modifier: Modifier = Modifier,
    gutter: Dp = 24.dp,
    content: LazyListScope.() -> Unit
) {
    LazyRow(
        modifier.fillMaxWidth(),
        contentPadding = PaddingValues(start = gutter, end = gutter),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        content = content
    )
}

/**
 * A ghost card: the zero-shape for a [PeekCardRow]. Drawn, not written (§12) — the strip keeps its
 * rhythm with nothing in it, and only the LEAD cell is tappable.
 *
 * ## Two rounds of getting this wrong
 *
 * **v1 was a hollow outline.** Right on a bare page, where an outline is the only way to show an
 * empty slot; wrong beside filled cards, where a row of empty rectangles reads as unstyled.
 *
 * **v2 was a flat filled slab.** The colour was right and it still read badly — "stark and seeable"
 * (Antho, 2026-08-15) — because three identical blocks of solid fill are *louder* than the real
 * content they stand in for. An empty state should recede.
 *
 * **v3, this one, does three things instead.** The fill becomes a vertical gradient that dissolves
 * toward the page, borrowing the profile cover's own trick so an empty cell fades out rather than
 * ending on a hard edge. Each successive cell drops in opacity, so the strip reads as *continuing*
 * rather than as N equal boxes. And the lead cell carries real structure via [content] — its
 * section's own mark at zero, in the vocabulary it will use with data (§12) — instead of a bare
 * prompt floating in the middle of a blank.
 */
@Composable
fun GhostCard(
    palette: SurfacePalette,
    index: Int,
    muted: Color,
    onBg: Color,
    minHeight: Dp,
    modifier: Modifier = Modifier,
    prompt: String? = null,
    caption: String? = null,
    onClick: (() -> Unit)? = null,
    clickLabel: String = "",
    content: @Composable ColumnScope.() -> Unit = {}
) {
    val lead = index == 0
    // The recede ramp. A gradient/scrim interpolates freely between rungs (§5's named exception),
    // which is what lets the trailing cells fade instead of snapping to the next alpha down.
    val depth = when (index) {
        0 -> 1f
        1 -> 0.62f
        else -> 0.34f
    }
    Column(
        modifier
            .width(peekCardWidth())
            .heightIn(min = minHeight)
            .clip(CardShape)
            .background(
                Brush.verticalGradient(
                    listOf(
                        palette.card.copy(alpha = depth),
                        palette.card.copy(alpha = depth * 0.25f)
                    )
                )
            )
            .border(
                1.dp,
                if (lead) muted.copy(alpha = 0.25f) else palette.hairline.copy(alpha = depth),
                CardShape
            )
            .then(
                if (lead && onClick != null) {
                    Modifier.bounceCombinedClick(onClickLabel = clickLabel, onClick = onClick)
                } else Modifier
            )
            .padding(14.dp)
    ) {
        if (lead && prompt != null) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                // §14: the accent carries the glyph, the words stay on onBg — accent text measures
                // 2.35:1, so it never carries meaning on its own.
                Text("+ ", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary)
                Text(prompt, style = MaterialTheme.typography.labelMedium, color = onBg)
            }
            if (caption != null) {
                Spacer(Modifier.height(5.dp))
                Text(
                    caption,
                    style = MaterialTheme.typography.bodySmall,
                    // The on-card floor exactly, not a shade under it. `mutedOnCard.copy(alpha = …)`
                    // REPLACES the alpha rather than scaling it, so dimming this "slightly" landed
                    // at 0.595 — below the 0.70 the fill already forces. An empty cell's caption is
                    // still text.
                    color = palette.mutedOnCard
                )
            }
            Spacer(Modifier.height(12.dp))
        }
        // No weighted spacer pushing [content] to the bottom: a LazyRow measures its items with an
        // unbounded height, so a weight in here resolves to zero and the mark would silently
        // collapse against the prompt. The cell's min height carries the presence instead.
        content()
    }
}

/**
 * The empty track a meter leaves behind — the zero-shape for anything measured against a target
 * (§2②: "empty track, honest 0"). Dimmed by its cell's depth so it recedes with the rest.
 */
@Composable
fun GhostTrack(muted: Color, depth: Float = 1f, modifier: Modifier = Modifier) {
    Box(
        modifier
            .fillMaxWidth()
            .height(4.dp)
            .clip(RoundedCornerShape(50))
            .background(muted.copy(alpha = 0.25f * depth))
    )
}

// ── Controls ──────────────────────────────────────────────────────────────────────────────────

/**
 * The hero CTA: a full-width filled capsule, bounce press, no ripple (§8/§9). `heightIn(min=)` +
 * padding, never a fixed height (§14).
 *
 * **It is the accent now, not white** (2026-08-16). White is temperature-neutral, and this is the
 * single largest piece of colour on Home — the one place the app is allowed to be loud. Taking the
 * fill from `primary` means the whole page's heat follows the accent picker instead of being
 * hardcoded, and the ember default measures 4.94:1 against `onPrimary`, better than the 3.9:1 a
 * near-white content colour would have given.
 */
@Composable
fun SurfaceCta(
    text: String,
    label: String,
    modifier: Modifier = Modifier,
    /**
     * False renders the same capsule OUTLINED instead of filled.
     *
     * Home uses it when the day's answer is *not* to train: on a rest day the action still exists
     * (you may train anyway) but it is not the recommendation, and a filled ember capsule is this
     * app's one "do this now" signal. Shouting "Start session" under a headline reading "Done for
     * today" was the page contradicting itself (Antho, 2026-08-16).
     */
    filled: Boolean = true,
    onLongClick: (() -> Unit)? = null,
    longClickLabel: String? = null,
    onClick: () -> Unit
) {
    val accent = MaterialTheme.colorScheme.primary
    Box(
        modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(50))
            .then(
                if (filled) Modifier.background(accent)
                else Modifier.border(1.dp, MaterialTheme.colorScheme.outline, RoundedCornerShape(50))
            )
            .bounceCombinedClick(
                onClickLabel = label,
                onLongClickLabel = longClickLabel,
                onLongClick = onLongClick,
                onClick = onClick
            )
            .heightIn(min = 56.dp)
            .padding(horizontal = 24.dp, vertical = 16.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text,
            style = MaterialTheme.typography.bodyLarge,
            // Outlined takes onBackground, not the accent: accent-as-text clears AA on the Ember
            // default but not on the four alternate presets (§14), and this label must stay legible
            // whichever accent is picked.
            color = if (filled) MaterialTheme.colorScheme.onPrimary
            else MaterialTheme.colorScheme.onBackground,
            textAlign = TextAlign.Center
        )
    }
}

// The accent bloom behind Home's hero was REMOVED (Antho, 2026-08-16). It was an attempt to buy
// atmosphere without a photograph, and it worked — but a tinted wash across the top of the page is
// exactly the kind of decorative colour §5 spends the accent budget on for nothing. The warm base
// carries the temperature on its own; the CTA carries the heat. Do not re-add a background glow.

/**
 * A section anchor: the mono 15sp rung + an optional navigation link.
 *
 * §7 survives this experiment intact — sections still separate by AIR and their mono header, never
 * by a line or a box. What changed is that a section's CONTENT sits on a fill. Keeping this anchor
 * outside the card is what stops the page reading as a generic dashboard.
 *
 * Forked from `EditorialHeader` for one reason: that one renders its action in the accent, which
 * §14 records as failing AA (2.35:1 on the page, worse on a card). This one uses `onBg`.
 */
@Composable
fun SectionAnchor(
    label: String,
    muted: Color,
    onBg: Color,
    modifier: Modifier = Modifier,
    action: String? = null,
    actionLabel: String = "",
    /**
     * A passive right-hand reading, for a section whose meta is a number rather than a link
     * ("3 / 4 target"). It exists so such a section still gets THIS anchor: Home used to hand-roll
     * a sans `titleMedium` header purely because it needed a figure on the right, and the page
     * ended up with two section-header treatments a few hundred dp apart. Ignored when [action] is
     * set — a section head carries one thing on its right, not two.
     */
    meta: String? = null,
    onAction: (() -> Unit)? = null
) {
    Row(
        modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            label.uppercase(),
            style = com.forge.app.ui.theme.MonoSectionAnchor,
            color = muted,
            modifier = Modifier.semantics { heading() }
        )
        when {
            action != null && onAction != null -> CardLink(action, actionLabel, onBg, onAction)
            meta != null -> Text(
                meta,
                style = MaterialTheme.typography.labelMedium,
                color = muted
            )
        }
    }
}

/**
 * A quiet navigation link inside a card — mono, on `onBg` rather than accent. §14 records accent
 * text as failing AA (2.35:1) and forbids adding new accent body text until that is resolved, and
 * the card fill makes the ratio worse, not better.
 */
@Composable
fun CardLink(text: String, label: String, onBg: Color, onClick: () -> Unit) {
    Box(
        // §14: touch comes from padding, not from the glyph. "view all" is the most-tapped control
        // on Home and it was ~24dp tall.
        Modifier
            .clickableLabeled(label, onClick = onClick)
            .heightIn(min = 48.dp)
            .padding(vertical = 4.dp),
        contentAlignment = Alignment.CenterEnd
    ) {
        Text(
            "$text →",
            style = MaterialTheme.typography.labelMedium,
            color = onBg,
            maxLines = 1
        )
    }
}
