package com.forge.app.ui.profile

import androidx.compose.animation.core.InfiniteTransition
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AutoAwesome
import androidx.compose.material.icons.filled.BlurOn
import androidx.compose.material.icons.filled.Flare
import androidx.compose.material.icons.filled.LocalFireDepartment
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.WbSunny
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.Dp
import com.forge.app.domain.rank.RankTier
import com.forge.app.ui.theme.ForgeMotion

/** A Compose [Color] from a tier's packed ARGB — the palette lives in [RankTier]. */
internal fun RankTier.color(): Color = Color(colorArgb)

/**
 * The big, alive emblem for a rank tier — the focal point of the rank section.
 *
 * Ember & Flare render as a custom, hand-drawn fire whose flame tongues actually lick and
 * flicker and throw embers (real internal motion, not a swaying glyph). The other tiers fall
 * back to their Material emblem with subtle in-character motion until they grow bespoke
 * drawings of their own (star twinkle, pulsar sweep, quasar orbit, supernova rings).
 *
 * [animated] is false under the system "Remove animations" preference — everything draws static.
 */
@Composable
internal fun HeroEmblem(tier: RankTier, animated: Boolean, size: Dp) {
    when (tier) {
        RankTier.EMBER, RankTier.FLARE ->
            FireEmblem(tier.color(), animated, Modifier.size(width = size * 0.82f, height = size))
        else ->
            GlyphEmblem(tier, animated, Modifier.size(size * 0.7f))
    }
}

// ── Fire ─────────────────────────────────────────────────────────────────────

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

// ── Glyph fallback (non-fire tiers) ───────────────────────────────────────────

// Exhaustive over every tier (no `else`) so a newly-added tier fails to compile until handled here,
// rather than being silently absorbed. Ember/Flare draw as fire and never reach the glyph path, but
// are mapped anyway to keep the function total.
private fun tierIcon(t: RankTier): ImageVector = when (t) {
    RankTier.EMBER -> Icons.Filled.LocalFireDepartment
    RankTier.FLARE -> Icons.Filled.Flare
    RankTier.NOVA -> Icons.Filled.Star
    RankTier.PULSAR -> Icons.Filled.AutoAwesome
    RankTier.QUASAR -> Icons.Filled.BlurOn
    RankTier.SUPERNOVA -> Icons.Filled.WbSunny
}

@Composable
private fun GlyphEmblem(tier: RankTier, animated: Boolean, modifier: Modifier) {
    val anim = rememberEmblemMotion(tier, animated)
    Icon(
        tierIcon(tier),
        contentDescription = tier.display,
        tint = tier.color(),
        modifier = modifier.graphicsLayer {
            transformOrigin = TransformOrigin(0.5f, anim.pivotY)
            rotationZ = anim.rotation
            scaleX = anim.scaleX
            scaleY = anim.scaleY
        }
    )
}

/** Per-tier "alive" motion for the glyph emblems — real movement (turn / sweep / breathe). */
private data class EmblemMotion(val rotation: Float, val scaleX: Float, val scaleY: Float, val pivotY: Float)

@Composable
private fun rememberEmblemMotion(tier: RankTier, enabled: Boolean): EmblemMotion {
    if (!enabled) return EmblemMotion(0f, 1f, 1f, 0.5f)
    val t = rememberInfiniteTransition(label = "emblem-${tier.name}")
    return when (tier) {
        // Ember/Flare render as fire (FireEmblem) and never reach the glyph path; mapped to identity
        // so this `when` stays exhaustive and a new tier won't silently fall through.
        RankTier.EMBER, RankTier.FLARE -> EmblemMotion(0f, 1f, 1f, 0.5f)
        RankTier.NOVA -> {
            val rot by t.animateFloat(0f, 360f, infiniteRepeatable(tween(4200, easing = LinearEasing)), label = "turn")
            val tw by t.animateFloat(0f, 1f, infiniteRepeatable(tween(820), RepeatMode.Reverse), label = "twinkle")
            EmblemMotion(rot, 0.94f + 0.10f * tw, 0.94f + 0.10f * tw, 0.5f)
        }
        RankTier.PULSAR -> {
            val rock by t.animateFloat(-1f, 1f, infiniteRepeatable(tween(700), RepeatMode.Reverse), label = "rock")
            val pl by t.animateFloat(0f, 1f, infiniteRepeatable(tween(520), RepeatMode.Reverse), label = "pulse")
            EmblemMotion(rock * 22f, 0.88f + 0.20f * pl, 0.88f + 0.20f * pl, 0.5f)
        }
        RankTier.QUASAR -> {
            val rot by t.animateFloat(0f, 360f, infiniteRepeatable(tween(7000, easing = LinearEasing)), label = "spin")
            val pl by t.animateFloat(0f, 1f, infiniteRepeatable(tween(680), RepeatMode.Reverse), label = "breathe")
            EmblemMotion(rot, 0.90f + 0.16f * pl, 0.90f + 0.16f * pl, 0.5f)
        }
        RankTier.SUPERNOVA -> {
            // Sun rays rotate steadily while the whole burst breathes.
            val rot by t.animateFloat(0f, 360f, infiniteRepeatable(tween(6000, easing = LinearEasing)), label = "rays")
            val br by t.animateFloat(0f, 1f, infiniteRepeatable(tween(900), RepeatMode.Reverse), label = "burst")
            EmblemMotion(rot, 0.95f + 0.10f * br, 0.95f + 0.10f * br, 0.5f)
        }
    }
}

// ── helpers ────────────────────────────────────────────────────────────────--

private const val TAU = 6.2831855f

private fun sinf(x: Float): Float = kotlin.math.sin(x.toDouble()).toFloat()

/** A 0f→1f sawtooth phase that loops every [periodMs] (honoring reduced-motion scaling). */
@Composable
private fun InfiniteTransition.phase(periodMs: Int): State<Float> {
    val d = ForgeMotion.scaledDuration(periodMs).coerceAtLeast(1)
    return animateFloat(0f, 1f, infiniteRepeatable(tween(d, easing = LinearEasing)), label = "phase-$periodMs")
}
