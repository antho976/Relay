package com.forge.app.ui.common

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.dp
import com.forge.app.ui.theme.ForgeMotion
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * The app's shared one-shot motion kit. Promoted here from ui/gym/stats/components/ (2026-07-24):
 * four feature packages use it, well past DESIGN §2⑥'s promote-on-third-use rule, and §8 already
 * listed it as living in ui/common/. A pure relocation — the tuning is untouchable (SETTLED.md).
 *
 * Primitives, all derived from
 * [ForgeMotion] so the system reduced-motion preference collapses everything
 * automatically (springs snap, tweens hit 0 ms, stagger delays vanish):
 *
 *  - [statsEntrance] — fade + slight slide-up on first appearance, staggered by index
 *  - [CountUpText]   — a number that animates from its "was X" value to current
 *  - [rememberDrawProgress] — a 0→1 reveal used by sparklines, bars, and grids
 *
 * Everything plays ONCE per appearance (tracked via rememberSaveable, so scrolling
 * away and back doesn't replay) and nothing loops.
 */

private const val STAGGER_STEP_MS = 45
private const val MAX_STAGGER_STEPS = 8
private const val DRAW_IN_MS = 600
private const val ENTRANCE_SLIDE_DP = 12

/** One-shot entrance: fade in + settle up from [ENTRANCE_SLIDE_DP] on a spring, staggered by [index]. */
fun Modifier.statsEntrance(index: Int = 0): Modifier = composed {
    var played by rememberSaveable { mutableStateOf(false) }
    val alpha = remember { Animatable(if (played) 1f else 0f) }
    val slide = remember { Animatable(if (played) 0f else 1f) }
    LaunchedEffect(Unit) {
        if (!played) {
            delay(ForgeMotion.scaledDuration(STAGGER_STEP_MS * index.coerceAtMost(MAX_STAGGER_STEPS)).toLong())
            launch { alpha.animateTo(1f, ForgeMotion.enterTween()) }
            slide.animateTo(0f, ForgeMotion.bouncy())
            played = true
        }
    }
    graphicsLayer {
        this.alpha = alpha.value
        translationY = slide.value * ENTRANCE_SLIDE_DP.dp.toPx()
    }
}

/**
 * Big-number count-up: animates from [fromValue] (the "was X" of the delta pattern) to
 * [value] on first appearance, then sits still. No prior value → starts at [value] (no
 * 0-to-N slot-machine effect on lifetime totals).
 */
@Composable
internal fun CountUpText(
    value: Double,
    style: TextStyle,
    color: Color,
    modifier: Modifier = Modifier,
    fromValue: Double? = null,
    format: (Double) -> String = { "${it.toInt()}" }
) {
    var played by rememberSaveable { mutableStateOf(false) }
    val anim = remember {
        Animatable((if (played) value else fromValue ?: value).toFloat())
    }
    LaunchedEffect(value) {
        anim.animateTo(value.toFloat(), ForgeMotion.snappy())
        played = true
    }
    Text(format(anim.value.toDouble()), style = style, color = color, modifier = modifier)
}

/**
 * Single 0→1 reveal driving draw-in: sparklines clip left-to-right, bars grow, tiles
 * fill. Re-keyed by [key] (pass the data identity) so a data refresh re-reveals only
 * when the series itself changes. [spec] is the curve the reveal rides — defaults to the
 * standard 600 ms enter tween; pass [ForgeMotion.drawTween] for a slower, gentler glide.
 */
@Composable
internal fun rememberDrawProgress(
    key: Any? = Unit,
    spec: FiniteAnimationSpec<Float> = ForgeMotion.enterTween(DRAW_IN_MS)
): Float {
    var played by rememberSaveable { mutableStateOf(false) }
    val anim = remember { Animatable(if (played) 1f else 0f) }
    LaunchedEffect(key) {
        if (anim.value < 1f) anim.animateTo(1f, spec)
        played = true
    }
    return anim.value
}

/**
 * Slice one overall [rememberDrawProgress] value into a per-tile progress so a row or
 * grid of [count] tiles fills with a stagger — no per-tile Animatable needed.
 */
internal fun staggeredProgress(overall: Float, index: Int, count: Int): Float {
    if (count <= 1) return overall
    val span = 0.6f // staggered starts occupy the first 60% of the timeline; each tile fills over the rest
    val start = span * index / count
    return ((overall - start) / (1f - span)).coerceIn(0f, 1f)
}

/** Convenience: a Box that plays the entrance animation — for wrapping LazyColumn item content. */
@Composable
internal fun EntranceItem(index: Int, content: @Composable () -> Unit) {
    Box(Modifier.statsEntrance(index)) { content() }
}
