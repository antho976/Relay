package com.forge.app.ui.profile

import androidx.compose.animation.core.InfiniteTransition
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.unit.Dp
import com.forge.app.domain.rank.RankTier
import com.forge.app.ui.theme.ForgeMotion

/** A Compose [Color] from a tier's packed ARGB — the palette lives in [RankTier]. */
internal fun RankTier.color(): Color = Color(colorArgb)

/**
 * The big, alive emblem for a rank tier — the focal point of the rank section. Every tier now has a
 * bespoke, hand-drawn Canvas identity with real internal motion (not a swaying glyph):
 * Ember/Flare a licking fire, Nova a twinkling star, Pulsar sweeping beams, Quasar orbiting rings,
 * Supernova an expanding shockwave. The `when` is exhaustive (no `else`) so a newly-added tier fails
 * to compile until it's given a drawing.
 *
 * [animated] is false under the system "Remove animations" preference — everything draws static.
 */
@Composable
internal fun HeroEmblem(tier: RankTier, animated: Boolean, size: Dp) {
    when (tier) {
        RankTier.EMBER, RankTier.FLARE ->
            FireEmblem(tier.color(), animated, Modifier.size(width = size * 0.82f, height = size))
        RankTier.NOVA -> NovaEmblem(tier.color(), animated, Modifier.size(size))
        RankTier.PULSAR -> PulsarEmblem(tier.color(), animated, Modifier.size(size))
        RankTier.QUASAR -> QuasarEmblem(tier.color(), animated, Modifier.size(size))
        RankTier.SUPERNOVA -> SupernovaEmblem(tier.color(), animated, Modifier.size(size))
    }
}

// ── Fire (Ember / Flare) ───────────────────────────────────────────────────────

/**
 * A live fire drawn from three stacked flame paths (deep outer body → bright mid → hot core) plus
 * a few rising embers. Each layer sways and breathes on its own out-of-phase oscillators, so the
 * flames themselves move while the base stays planted — fire, not a wobbling icon.
 */
@Composable
private fun FireEmblem(tierColor: Color, animated: Boolean, modifier: Modifier) {
    val outer = lerp(tierColor, Color.Black, 0.18f)
    val mid = tierColor
    val core = lerp(tierColor, Color.White, 0.74f)

    // A handful of slow linear phases at non-harmonic periods; sines of them (read at draw time)
    // give organic, never-quite-repeating motion. Null when animations are off → a still flame.
    val t = if (animated) rememberInfiniteTransition(label = "fire") else null
    val p1 = t?.phase(1700)
    val p2 = t?.phase(1130)
    val p3 = t?.phase(2270)
    val pe = t?.phase(1500)
    // Reuse one Path per flame layer across frames — the Canvas redraws every frame while animating,
    // so rebuilding into cached Paths avoids 3 Path allocations per frame on the hero.
    val paths = remember { List(3) { Path() } }

    Canvas(modifier) {
        val w = size.width
        val h = size.height
        val cx = w / 2f
        val baseY = h * 0.95f
        fun s(p: State<Float>?, phase: Float) = if (p == null) 0f else sinf(p.value * TAU + phase)

        val sway1 = s(p1, 0f)
        val sway2 = s(p2, 1.2f)
        val sway3 = s(p3, 2.4f)
        val flick = (s(p2, 0f) + s(p3, 1.7f)) * 0.5f // -1..1, the tip's height shimmer

        // Outer body — tallest, widest sway.
        drawPath(buildFlame(paths[0], cx, baseY, w * 0.34f, h * 0.10f - h * 0.05f * flick, sway1 * w * 0.06f), outer)
        // Mid flame — brighter, quicker.
        drawPath(buildFlame(paths[1], cx, baseY, w * 0.23f, h * 0.30f - h * 0.05f * sway2, sway2 * w * 0.05f), mid)
        // Hot core — small, subtle.
        drawPath(buildFlame(paths[2], cx, baseY, w * 0.12f, h * 0.50f - h * 0.04f * sway3, sway3 * w * 0.035f), core)

        // Embers: a few sparks rising off the tip and fading out.
        if (pe != null) {
            val n = 3
            for (k in 0 until n) {
                val local = (pe.value + k / n.toFloat()) % 1f
                val ey = baseY - h * 0.18f - local * h * 0.78f
                val ex = cx + sinf(local * TAU + k.toFloat()) * w * 0.11f
                val alpha = (1f - local).coerceIn(0f, 1f) * 0.7f
                drawCircle(core.copy(alpha = alpha), radius = w * 0.035f * (1f - local * 0.5f), center = Offset(ex, ey))
            }
        }
    }
}

/** Rebuilds [path] in place as one flame tongue (a wavy teardrop from a rounded base at [baseY] to a
 *  tip near [tipY]) and returns it. Reusing the same Path each frame avoids per-frame allocation. */
private fun buildFlame(path: Path, cx: Float, baseY: Float, halfW: Float, tipY: Float, sway: Float): Path {
    val h = baseY - tipY
    val tipX = cx + sway
    return path.apply {
        rewind()
        moveTo(cx - halfW, baseY)
        // Left edge: bulge outward low, curl back in toward the tip.
        cubicTo(
            cx - halfW * 1.1f, baseY - h * 0.45f,
            cx - halfW * 0.15f - sway * 0.5f, tipY + h * 0.25f,
            tipX, tipY
        )
        // Right edge: tip back down to the base.
        cubicTo(
            cx + halfW * 0.15f - sway * 0.5f, tipY + h * 0.25f,
            cx + halfW * 1.1f, baseY - h * 0.45f,
            cx + halfW, baseY
        )
        // Gently rounded bottom.
        cubicTo(
            cx + halfW * 0.5f, baseY + h * 0.06f,
            cx - halfW * 0.5f, baseY + h * 0.06f,
            cx - halfW, baseY
        )
        close()
    }
}

// ── Nova: a twinkling eight-point star ─────────────────────────────────────────

@Composable
private fun NovaEmblem(tierColor: Color, animated: Boolean, modifier: Modifier) {
    val core = lerp(tierColor, Color.White, 0.72f)
    val t = if (animated) rememberInfiniteTransition(label = "nova") else null
    val spin = t?.phase(9000)   // slow turn
    val twk = t?.phase(900)     // twinkle
    val star = remember { Path() }
    Canvas(modifier) {
        val c = Offset(size.width / 2f, size.height / 2f)
        val r = size.minDimension / 2f
        val rot = (spin?.value ?: 0.06f) * TAU
        val tw = osc(twk)
        drawCircle(tierColor.copy(alpha = 0.16f + 0.12f * tw), radius = r * 0.92f, center = c)
        val long = r * (0.74f + 0.12f * tw)
        val short = r * 0.30f
        star.rewind()
        for (i in 0 until 8) {
            val a = rot + i * (TAU / 8f)
            val rad = if (i % 2 == 0) long else short
            val px = c.x + cosf(a) * rad
            val py = c.y + sinf(a) * rad
            if (i == 0) star.moveTo(px, py) else star.lineTo(px, py)
        }
        star.close()
        drawPath(star, tierColor)
        drawCircle(core, radius = r * (0.15f + 0.05f * tw), center = c)
    }
}

// ── Pulsar: two beams sweeping from a pulsing core ─────────────────────────────

@Composable
private fun PulsarEmblem(tierColor: Color, animated: Boolean, modifier: Modifier) {
    val core = lerp(tierColor, Color.White, 0.82f)
    val t = if (animated) rememberInfiniteTransition(label = "pulsar") else null
    val sweep = t?.phase(2600)
    val pulse = t?.phase(700)
    val beam = remember { Path() }
    Canvas(modifier) {
        val c = Offset(size.width / 2f, size.height / 2f)
        val r = size.minDimension / 2f
        val ang = (sweep?.value ?: 0.12f) * TAU
        val p = osc(pulse)
        drawCircle(tierColor.copy(alpha = 0.15f), radius = r * 0.9f, center = c)
        for (dir in 0 until 2) {
            val a = ang + dir * (TAU / 2f)
            val perp = a + TAU / 4f
            val half = r * 0.15f
            val tip = Offset(c.x + cosf(a) * r * 0.96f, c.y + sinf(a) * r * 0.96f)
            beam.rewind()
            beam.moveTo(c.x + cosf(perp) * half, c.y + sinf(perp) * half)
            beam.lineTo(tip.x, tip.y)
            beam.lineTo(c.x - cosf(perp) * half, c.y - sinf(perp) * half)
            beam.close()
            drawPath(beam, tierColor.copy(alpha = 0.85f))
        }
        drawCircle(core.copy(alpha = 0.45f * (1f - p)), radius = r * (0.28f + 0.6f * p), center = c, style = Stroke(width = r * 0.045f))
        drawCircle(core, radius = r * (0.17f + 0.07f * p), center = c)
    }
}

// ── Quasar: a bright core inside two orbited rings ─────────────────────────────

@Composable
private fun QuasarEmblem(tierColor: Color, animated: Boolean, modifier: Modifier) {
    val core = lerp(tierColor, Color.White, 0.78f)
    val t = if (animated) rememberInfiniteTransition(label = "quasar") else null
    val o1 = t?.phase(5200)
    val o2 = t?.phase(3400)
    val breathe = t?.phase(1500)
    Canvas(modifier) {
        val c = Offset(size.width / 2f, size.height / 2f)
        val r = size.minDimension / 2f
        val br = osc(breathe)
        drawCircle(tierColor.copy(alpha = 0.14f), radius = r * 0.95f, center = c)
        drawCircle(tierColor.copy(alpha = 0.5f), radius = r * 0.84f, center = c, style = Stroke(width = r * 0.025f))
        drawCircle(tierColor.copy(alpha = 0.38f), radius = r * 0.58f, center = c, style = Stroke(width = r * 0.02f))
        val a1 = (o1?.value ?: 0f) * TAU
        val a2 = (o2?.value ?: 0.3f) * TAU
        drawCircle(core, radius = r * 0.085f, center = Offset(c.x + cosf(a1) * r * 0.84f, c.y + sinf(a1) * r * 0.84f))
        drawCircle(core.copy(alpha = 0.85f), radius = r * 0.06f, center = Offset(c.x - cosf(a2) * r * 0.58f, c.y - sinf(a2) * r * 0.58f))
        drawCircle(core, radius = r * (0.19f + 0.05f * br), center = c)
    }
}

// ── Supernova: rotating burst rays + expanding shockwave rings ─────────────────

@Composable
private fun SupernovaEmblem(tierColor: Color, animated: Boolean, modifier: Modifier) {
    val core = lerp(tierColor, Color.White, 0.85f)
    val t = if (animated) rememberInfiniteTransition(label = "supernova") else null
    val wave = t?.phase(2000)
    val rays = t?.phase(8000)
    Canvas(modifier) {
        val c = Offset(size.width / 2f, size.height / 2f)
        val r = size.minDimension / 2f
        drawCircle(tierColor.copy(alpha = 0.2f), radius = r * 0.9f, center = c)
        val rot = (rays?.value ?: 0f) * TAU
        for (i in 0 until 12) {
            val a = rot + i * (TAU / 12f)
            drawLine(
                color = tierColor.copy(alpha = 0.7f),
                start = Offset(c.x + cosf(a) * r * 0.3f, c.y + sinf(a) * r * 0.3f),
                end = Offset(c.x + cosf(a) * r * 0.92f, c.y + sinf(a) * r * 0.92f),
                strokeWidth = r * 0.03f
            )
        }
        for (k in 0 until 3) {
            val local = if (wave == null) 0.5f else (wave.value + k / 3f) % 1f
            drawCircle(core.copy(alpha = (1f - local) * 0.55f), radius = r * (0.2f + local * 0.72f), center = c, style = Stroke(width = r * 0.03f))
        }
        drawCircle(core, radius = r * 0.19f, center = c)
    }
}

// ── helpers ────────────────────────────────────────────────────────────────--

private const val TAU = 6.2831855f

private fun sinf(x: Float): Float = kotlin.math.sin(x.toDouble()).toFloat()
private fun cosf(x: Float): Float = kotlin.math.cos(x.toDouble()).toFloat()

/** A sawtooth phase mapped to a 0f..1f sine oscillation; 0.5 (mid) when not animating. */
private fun osc(p: State<Float>?): Float = if (p == null) 0.5f else (sinf(p.value * TAU) + 1f) / 2f

/** A 0f→1f sawtooth phase that loops every [periodMs] (honoring reduced-motion scaling). */
@Composable
private fun InfiniteTransition.phase(periodMs: Int): State<Float> {
    val d = ForgeMotion.scaledDuration(periodMs).coerceAtLeast(1)
    return animateFloat(0f, 1f, infiniteRepeatable(tween(d, easing = LinearEasing)), label = "phase-$periodMs")
}
