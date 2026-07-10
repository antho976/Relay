package com.forge.app.ui.common

import android.graphics.RenderEffect
import android.graphics.RuntimeShader
import android.os.Build
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.asComposeRenderEffect
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import com.forge.app.appicon.AppIcon
import com.forge.app.appicon.IconFamily
import kotlin.math.sin

private const val WORD = "Avex"

/** Families whose wordmark has a choreographed EXIT — AvexIntro shortens the hold and drives
 *  [AvexWordmark]'s `exit` so the destruction overlaps the plate fade instead of extending it.
 *  Only families that actually render a death on the running API count: Solid's wipe-out and Nebula's
 *  vortex (33+) / spin-shrink (pre-33) work everywhere, but Molten's melt is a 33+ shader with no
 *  pre-33 fallback — below 33 it fades plainly, so we must NOT shorten its hold or drive `exit`. */
fun wordmarkExitChoreographed(icon: AppIcon): Boolean {
    if (icon.launchPalette == null) return false
    return when (icon.family) {
        IconFamily.Solid, IconFamily.Nebula -> true
        IconFamily.Molten -> Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU
        else -> false
    }
}

/**
 * The launch wordmark, themed to the chosen app icon (DESIGN §9). The name has a narrative arc in
 * the icon's palette — it ENTERS with the family's verb, holds legible (the brand beat), then DIES
 * the family's death as the intro hands off:
 *
 * - Metal — enters as brushed metal, one specular sheen sweep · fades (its glow IS the identity)
 * - Gem — crystal fill, a prismatic glint travels through; angular crystal spikes GROW OUT of the
 *   letterforms during the hold and stay (33+) · fades
 * - Aurora — northern lights RISE OUT of the letterforms, waving and hue-shifting (33+; pre-33
 *   a drifting light-fill) · fades
 * - Nebula — floats weightless · then DRAGGED INTO A BLACK HOLE (vortex RenderEffect, 33+)
 * - Molten — lit white-hot, heat-shimmering · then MELTS decelerating — the word slumps and thin
 *   drip streams run ahead (33+)
 * - Solid — the plate colour wipes in · wipes back out
 * - Stealth — flickers in like a HUD · fades
 * - Default / no palette — today's plain settle and fade, untouched
 *
 * [reveal] drives the entrance (settle alpha/scale), [exit] the death (0 until the hold ends). Both
 * are DEFERRED reads (`() -> Float`) so the caller's snapshot state is read in the draw phase — the
 * plain path never recomposes per frame, and neither does the caller.
 * Reduce-motion: settled, still, plain plate fade ([exit] never leaves 0).
 */
@Composable
fun AvexWordmark(
    icon: AppIcon,
    reveal: () -> Float,
    exit: () -> Float,
    reduceMotion: Boolean,
    modifier: Modifier = Modifier,
) {
    val base = MaterialTheme.typography.displayLarge
    val onBg = MaterialTheme.colorScheme.onBackground
    val palette = remember(icon) { icon.launchPalette.orEmpty().map { Color(it) } }

    if (palette.size < 3) {
        // Default (and any icon without a palette): the plain wordmark, exactly as before. Reading
        // reveal() inside graphicsLayer keeps this in the draw phase — no per-frame recomposition.
        Text(
            WORD, style = base, color = onBg,
            modifier = modifier.graphicsLayer {
                val r = reveal()
                alpha = r
                val s = 0.94f + 0.06f * r
                scaleX = s
                scaleY = s
            }
        )
        return
    }
    // Themed families recompose per frame via the wordmark clock (t) regardless, so resolve the
    // deferred reads to plain values here — downstream reads match the pre-lambda Float params.
    val reveal = reveal()
    val exit = exit()
    val c0 = palette[0]
    val c1 = palette[1]
    val c2 = palette[2]
    val t = wordmarkClock(reduceMotion)
    var size by remember { mutableStateOf(IntSize.Zero) }
    val w = size.width.toFloat()
    val h = size.height.toFloat()

    // Stealth's HUD power-up: irregular hard ON/OFF bursts (~14Hz hash slots — each state ~70ms,
    // slow enough to SEE), the ON probability rising across a full second of struggle, then steady.
    val flicker = if (icon.family == IconFamily.Stealth && t < 1.0f) {
        val slot = kotlin.math.floor(t * 14f)
        val h = ((sin(slot * 127.1f) * 43758.547f) % 1f + 1f) % 1f
        if (h < 0.25f + 0.75f * t) 1f else 0f
    } else 1f
    // Stealth ramps in faster so the bursts are visible from the first frames (off = fully off,
    // so the flicker carries the entrance instead of hiding under the fade).
    val entry = if (icon.family == IconFamily.Stealth) minOf(1f, reveal * 1.6f) else reveal

    val fill: Brush = when (icon.family) {
        IconFamily.Metal -> remember(palette) {
            Brush.verticalGradient(0f to c2, 0.45f to c1, 1f to lerp(c1, Color.Black, 0.40f))
        }
        IconFamily.Gem, IconFamily.Nebula, IconFamily.Stealth -> remember(palette) {
            Brush.verticalGradient(0f to c2, 1f to c1)
        }
        IconFamily.Aurora -> auroraFlow(c1, c2, t, w, h)
        IconFamily.Molten -> remember(palette) {
            Brush.verticalGradient(0f to lerp(c1, c0, 0.35f), 0.55f to c1, 1f to c2)
        }
        IconFamily.Solid ->
            if (exit > 0f) solidWipeOut(exit, w, plate = lerp(c1, c2, 0.30f))
            else solidWipe(t, w, dim = onBg.copy(alpha = 0.35f), plate = lerp(c1, c2, 0.30f))
        IconFamily.Forge -> remember(onBg) { Brush.verticalGradient(0f to onBg, 1f to onBg) }
    }

    // One travelling highlight pass over the fill (Metal's sheen, Gem's glint); null once finished.
    val sheen: Brush? = when (icon.family) {
        IconFamily.Metal -> sweepBrush(t, start = 0.25f, dur = 0.60f, band = 0.20f, bright = Color.White.copy(alpha = 0.90f), w = w, h = h)
        IconFamily.Gem -> sweepBrush(t, start = 0.30f, dur = 0.45f, band = 0.10f, bright = lerp(c2, Color.White, 0.5f), w = w, h = h)
        else -> null
    }

    // Glyph shaders (33+): Molten shimmers always and melts on exit; Nebula's vortex acts only on
    // exit (identity while uExit = 0); Gem's crystals grow during the hold and stay; Aurora's
    // lights rise from the letters throughout.
    val warpShader = remember(icon) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            when (icon.family) {
                IconFamily.Molten -> runCatching { RuntimeShader(MOLTEN_WORD_AGSL) }.getOrNull()
                IconFamily.Nebula -> runCatching { RuntimeShader(NEBULA_WORD_AGSL) }.getOrNull()
                IconFamily.Gem -> runCatching {
                    RuntimeShader(GEM_WORD_AGSL).apply {
                        // Crystal tones are fixed for the intro's life — bind once.
                        setColorUniform("uCA", c1.toArgb())
                        setColorUniform("uCB", c2.toArgb())
                    }
                }.getOrNull()
                IconFamily.Aurora -> runCatching {
                    RuntimeShader(AURORA_WORD_AGSL).apply {
                        setColorUniform("uCA", c1.toArgb())
                        setColorUniform("uCB", c2.toArgb())
                    }
                }.getOrNull()
                else -> null
            }
        } else null
    }
    // Warped families get breathing room in their layer so drips sag below the baseline, vortex
    // arcs swing wide, shards burst past the glyph box, and aurora curtains rise above the name
    // instead of clipping at the text bounds.
    val warped = icon.family == IconFamily.Molten || icon.family == IconFamily.Nebula ||
        icon.family == IconFamily.Gem || icon.family == IconFamily.Aurora

    Box(
        modifier = modifier
            .onSizeChanged { size = it }
            .graphicsLayer {
                alpha = entry * flicker
                val s = 0.94f + 0.06f * reveal
                scaleX = s
                scaleY = s
                if (icon.family == IconFamily.Nebula && !reduceMotion) {
                    // Weightless: a slow bob with a slower sideways drift.
                    translationY = 3.dp.toPx() * sin(t * 1.1f)
                    translationX = 2.dp.toPx() * sin(t * 0.7f + 1.3f)
                }
                if (icon.family == IconFamily.Nebula && warpShader == null && exit > 0f) {
                    // Pre-33 black hole: spin-shrink into the singularity point.
                    transformOrigin = TransformOrigin(0.68f, 0.42f)
                    rotationZ = exit * 160f
                    scaleX *= (1f - 0.95f * exit)
                    scaleY *= (1f - 0.95f * exit)
                }
                if (warpShader != null && size.width > 0 &&
                    Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU
                ) {
                    warpShader.setFloatUniform("uSize", size.width.toFloat(), size.height.toFloat())
                    warpShader.setFloatUniform("uTime", t)
                    warpShader.setFloatUniform("uExit", exit)
                    renderEffect = RenderEffect
                        .createRuntimeShaderEffect(warpShader, "content")
                        .asComposeRenderEffect()
                }
            }
    ) {
        val room = if (warped) Modifier.padding(horizontal = 32.dp, vertical = 44.dp) else Modifier
        Box(room) {
            Text(WORD, style = base.merge(TextStyle(brush = fill)))
            if (sheen != null) {
                Text(WORD, style = base.merge(TextStyle(brush = sheen)))
            }
        }
    }
}

/** Seconds since the wordmark mounted (120s ramp, never loops in-shot); frozen under reduce-motion. */
@Composable
private fun wordmarkClock(reduceMotion: Boolean): Float {
    if (reduceMotion) return 0.8f
    val transition = rememberInfiniteTransition(label = "wordmark")
    val t by transition.animateFloat(
        initialValue = 0f,
        targetValue = 120f,
        animationSpec = infiniteRepeatable(tween(120_000, easing = LinearEasing)),
        label = "wm-clock"
    )
    return t
}

/**
 * A narrow diagonal highlight band crossing the text once: enters at [start]s, takes [dur]s,
 * [band] = half-width as a fraction of text width. Null before it starts and after it exits
 * (so the overlay text stops composing entirely).
 */
private fun sweepBrush(t: Float, start: Float, dur: Float, band: Float, bright: Color, w: Float, h: Float): Brush? {
    if (w <= 0f) return null
    val u = (t - start) / dur
    if (u <= 0f || u >= 1f) return null
    val cx = (-0.25f + 1.5f * u) * w
    val half = w * band
    return Brush.linearGradient(
        0f to Color.Transparent, 0.5f to bright, 1f to Color.Transparent,
        start = Offset(cx - half, 0f), end = Offset(cx + half, h)
    )
}

/** Aurora light inside the glyphs: a periodic diagonal gradient sliding sideways, seamlessly. */
private fun auroraFlow(c1: Color, c2: Color, t: Float, w: Float, h: Float): Brush {
    if (w <= 0f) return Brush.verticalGradient(0f to c2, 1f to c1)
    val shift = ((t * 0.12f) % 1f) * w
    return Brush.linearGradient(
        0f to c1, 0.25f to c2, 0.5f to c1, 0.75f to c2, 1f to c1,
        start = Offset(shift - w, 0f), end = Offset(shift + w, h * 0.8f)
    )
}

/** The Solid wipe: plate colour fills the name left→right over ~half a second, then holds. */
private fun solidWipe(t: Float, w: Float, dim: Color, plate: Color): Brush {
    if (w <= 0f) return Brush.verticalGradient(0f to dim, 1f to dim)
    val u = ((t - 0.10f) / 0.55f).coerceIn(0f, 1f)
    val x = (-0.10f + 1.2f * u) * w
    val e = w * 0.12f
    return Brush.horizontalGradient(0f to plate, 1f to dim, startX = x - e, endX = x + e)
}

/** The Solid death: the plate colour wipes back out right→left, leaving nothing. */
private fun solidWipeOut(exit: Float, w: Float, plate: Color): Brush {
    if (w <= 0f) return Brush.verticalGradient(0f to plate, 1f to plate)
    val x = (1.10f - 1.30f * exit) * w
    val e = w * 0.12f
    return Brush.horizontalGradient(0f to plate, 1f to Color.Transparent, startX = x - e, endX = x + e)
}

/**
 * Molten glyph warp (API 33+). Always: heat-shimmer displacement growing toward the glyph bases.
 * On exit (uExit 0→1): THE MELT — every column sags downward, lower pixels sagging further (the
 * letters stretch, not just slide), with per-column variance so some columns run ahead as drips.
 */
private val MOLTEN_WORD_AGSL = """
    uniform shader content;
    uniform float2 uSize;
    uniform float uTime;
    uniform float uExit;

    float h21(float2 p) {
        p = fract(p * float2(123.34, 456.21));
        p += dot(p, p + 45.32);
        return fract(p.x * p.y);
    }
    float vn(float2 p) {
        float2 i = floor(p);
        float2 f = fract(p);
        float2 u = f * f * (3.0 - 2.0 * f);
        return mix(mix(h21(i), h21(i + float2(1.0, 0.0)), u.x),
                   mix(h21(i + float2(0.0, 1.0)), h21(i + float2(1.0, 1.0)), u.x), u.y);
    }

    half4 main(float2 xy) {
        float2 uv = xy / uSize;
        // Heat shimmer, alive the whole time, strongest toward the glyph bases.
        float amp = uSize.y * 0.020 * smoothstep(0.30, 0.75, uv.y);
        float dx = (vn(float2(uv.x * 6.0, uv.y * 4.0 - uTime * 1.6)) - 0.5) * 2.0 * amp;
        float dy = (vn(float2(uv.x * 5.0 + 7.3, uv.y * 4.0 - uTime * 1.3)) - 0.5) * 2.0 * amp * 0.5;
        // The melt (uExit is eased DECELERATING on the Kotlin side — gives way fast, drips slow):
        // a smooth large-scale SLUMP across the word plus sparse narrow DRIP STREAMS that run
        // ahead of it. Smooth noise, not per-column steps, so it reads as liquid, not streaks.
        float m = uExit;
        float slump = vn(float2(uv.x * 3.0, 7.7));
        float sagBase = m * uSize.y * 0.28 * uv.y * (0.55 + 0.45 * slump);
        float dripField = vn(float2(uv.x * 22.0, 3.3));
        float drip = smoothstep(0.68, 0.85, dripField);
        float sag = sagBase + m * uSize.y * 0.85 * uv.y * drip;
        return content.eval(xy + float2(dx, dy - sag));
    }
""".trimIndent()

/**
 * Nebula glyph warp (API 33+): identity while uExit = 0; on exit the BLACK HOLE — pixels spiral
 * (twist strongest near the singularity) and the whole name collapses into the point, sampling
 * past the content bounds into nothing.
 */
private val NEBULA_WORD_AGSL = """
    uniform shader content;
    uniform float2 uSize;
    uniform float uTime;
    uniform float uExit;

    half4 main(float2 xy) {
        float2 c = uSize * float2(0.68, 0.42);
        float2 d = xy - c;
        float r = length(d);
        float t2 = uExit * uExit;
        // Modest twist — enough to read as a vortex, not a cartwheel; the collapse carries it.
        float twist = t2 * 4.5 * exp(-r / (uSize.x * 0.35));
        float s = sin(twist);
        float cs = cos(twist);
        float2 rd = float2(d.x * cs - d.y * s, d.x * s + d.y * cs);
        float stretch = 1.0 + t2 * (7.0 * exp(-r / (uSize.x * 0.5)) + 1.5);
        return content.eval(c + rd * stretch);
    }
""".trimIndent()

/**
 * Gem crystal growth (API 33+): chunky crystal shards GROW OUT of the letterforms during the hold
 * and stay — each an analytic trapezoid (wide base, beveled tip) rooted on a glyph stroke, heading
 * out of the surface, inflating from its root on its own staggered start so the growth reads as
 * crystallization rather than a fade-in. uExit declared for the shared uniform contract but unused
 * (Gem dies by fade).
 */
private val GEM_WORD_AGSL = """
    uniform shader content;
    uniform float2 uSize;
    uniform float uTime;
    uniform float uExit;
    layout(color) uniform half4 uCA;
    layout(color) uniform half4 uCB;

    float h21(float2 p) {
        p = fract(p * float2(123.34, 456.21));
        p += dot(p, p + 45.32);
        return fract(p.x * p.y);
    }

    half4 main(float2 xy) {
        half4 col = content.eval(xy);
        if (float(col.a) > 0.9) { return col; }
        float T = uTime - 0.32;                        // growth begins once the name has settled
        if (T < 0.0) { return col; }

        // At most one chunk per cell: a root point that must LAND ON a glyph stroke, a heading
        // probed BOTH ways so it grows toward open air, and a base that reaches INTO the stroke so
        // the crystal visibly emerges from the letter. Cells are STROKE-scale (small), keeping the
        // chunks proportionate to the letterforms; the silhouette is serrated per ~3px segment so
        // the edges carry small spikes. Each chunk inflates on its own staggered start. Pixels test
        // their 3x3 neighbourhood so chunks cross cell boundaries un-clipped.
        float cs = uSize.y * 0.055;
        float2 base = floor(xy / cs);
        float bestA = 0.0;
        float bestShade = 0.0;
        for (int i = -1; i <= 1; i++) {
            for (int j = -1; j <= 1; j++) {
                float2 cell = base + float2(float(i), float(j));
                float h = h21(cell);
                if (h <= 0.80) {
                    float2 root = (cell + float2(h21(cell + 2.2), h21(cell + 3.7))) * cs;
                    if (float(content.eval(root).a) >= 0.5) {
                        float ang = h21(cell + 1.3) * 6.2831853;
                        float2 dir = float2(cos(ang), sin(ang));
                        // Head toward open air: probe both ways, take the emptier side.
                        float aF = float(content.eval(root + dir * cs * 0.6).a);
                        float aB = float(content.eval(root - dir * cs * 0.6).a);
                        if (aB < aF) { dir = -dir; }
                        float g0 = clamp((T - h21(cell + 9.9) * 0.35) / 0.30, 0.0, 1.0);
                        float g = g0 * g0 * (3.0 - 2.0 * g0);
                        if (g > 0.0) {
                            float2 rel = xy - root;
                            float along = dot(rel, dir);
                            float lat = abs(dot(rel, float2(-dir.y, dir.x)));
                            float len = cs * (0.6 + 1.2 * h21(cell + 4.1)) * g;
                            float taper = 1.0 - 0.55 * along / max(len, 0.001);
                            // Serrated edge: per-segment width jitter = small spikes on the surface.
                            float seg = floor(along / (cs * 0.16));
                            float jag = 0.70 + 0.55 * h21(float2(seg, h * 91.0));
                            float edgeW = cs * (0.28 + 0.14 * h21(cell + 6.3)) * g * taper * jag;
                            if (along >= -cs * 0.30 * g && along <= len && lat <= edgeW) {
                                float a = 0.95 * smoothstep(0.0, 0.20, g);
                                if (a > bestA) {
                                    bestA = a;
                                    bestShade = 0.45 + 0.55 * h21(cell + 7.7);
                                }
                            }
                        }
                    }
                }
            }
        }
        if (bestA <= 0.0) { return col; }

        float3 cCol = mix(float3(uCA.rgb), float3(uCB.rgb), bestShade);
        // The glyph's own anti-aliased edge composites OVER the crystal chunk.
        float outA = float(col.a) + bestA * (1.0 - float(col.a));
        float3 outRGB = float3(col.rgb) + cCol * bestA * (1.0 - float(col.a));
        return half4(half3(outRGB), half(outA));
    }
""".trimIndent()

/**
 * Aurora rising from the letters (API 33+): the wordmark is the horizon — waving curtains of light
 * emanate UPWARD from the glyph tops, filamented and hue-shifting between the icon's colours,
 * brightest at the letters and dissolving above. A pixel above the name probes downward (with a
 * gentle sway) for the nearest stroke; proximity sets its light. uExit declared for the shared
 * uniform contract but unused (Aurora dies by fade).
 */
private val AURORA_WORD_AGSL = """
    uniform shader content;
    uniform float2 uSize;
    uniform float uTime;
    uniform float uExit;
    layout(color) uniform half4 uCA;
    layout(color) uniform half4 uCB;

    float h21(float2 p) {
        p = fract(p * float2(123.34, 456.21));
        p += dot(p, p + 45.32);
        return fract(p.x * p.y);
    }
    float vn(float2 p) {
        float2 i = floor(p);
        float2 f = fract(p);
        float2 u = f * f * (3.0 - 2.0 * f);
        return mix(mix(h21(i), h21(i + float2(1.0, 0.0)), u.x),
                   mix(h21(i + float2(0.0, 1.0)), h21(i + float2(1.0, 1.0)), u.x), u.y);
    }

    half4 main(float2 xy) {
        half4 col = content.eval(xy);
        if (float(col.a) > 0.9) { return col; }

        // Distance to the nearest glyph BELOW this pixel — the letters are the horizon the light
        // rises from. Probes sway hard enough that the curtain visibly WAVES.
        float2 uv = xy / uSize;
        float reach = uSize.y * 0.30;
        float hit = 0.0;
        float dist = 1.0;
        for (int k = 1; k <= 6; k++) {
            float f = float(k) / 6.0;
            float sway = sin(uTime * 2.2 + uv.x * 12.0 + f * 2.0) * uSize.y * 0.02 * f;
            float a = float(content.eval(float2(xy.x + sway, xy.y + reach * f)).a);
            if (a > 0.5 && hit < 0.5) { hit = 1.0; dist = f; }
        }
        if (hit < 0.5) { return col; }

        // Brightest at the letters, dissolving above. The light is DISCRETE BEAMS: two counter-
        // moving sets of sharp shafts sweeping along the word — motion that reads within the beat.
        float fall = pow(1.0 - dist, 1.6);
        float b1 = pow(0.5 + 0.5 * sin(6.2831853 * uv.x * 6.0 - uTime * 4.7), 3.0);
        float b2 = pow(0.5 + 0.5 * sin(6.2831853 * uv.x * 11.0 + uTime * 5.9), 4.0);
        float beams = 0.20 + 1.30 * (b1 * 0.75 + b2 * 0.45);
        float aA = min(fall * beams * 0.55, 0.80);
        // Hue slides between the icon's colours along the word and drifts noticeably.
        float tc = fract(uv.x * 0.9 + 0.28 * uTime);
        float tri = tc < 0.5 ? tc * 2.0 : 2.0 - tc * 2.0;
        float3 hue = mix(float3(uCA.rgb), float3(uCB.rgb), tri);

        // The glyph's own anti-aliased edge composites OVER the light.
        float outA = float(col.a) + aA * (1.0 - float(col.a));
        float3 outRGB = float3(col.rgb) + hue * aA * (1.0 - float(col.a));
        return half4(half3(outRGB), half(outA));
    }
""".trimIndent()
