package com.forge.app.ui.common

import android.graphics.RuntimeShader
import android.os.Build
import androidx.annotation.RequiresApi
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ShaderBrush
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.toArgb
import com.forge.app.appicon.AppIcon
import com.forge.app.appicon.IconFamily
import com.forge.app.ui.theme.ForgeMotion
import kotlin.math.PI
import kotlin.math.sin

/**
 * Icon-themed cold-launch scenes (DESIGN §9).
 *
 * ⚠ DELIBERATELY UNWIRED (2026-07-10, Antho's call): the every-launch theming moved into the
 * wordmark itself ([AvexWordmark]) — full-screen scenes wore out their welcome as a daily beat.
 * KEPT intact (device-approved: Aurora, Nebula, Molten, Gem; Stealth radar unreviewed) so they can
 * be re-wired by composing [IconLaunchScene] behind the wordmark in [AvexIntro]. Don't delete as
 * "dead code".
 *
 * One engine: a family supplies an effect STYLE (its
 * AGSL scene below), the icon supplies its PALETTE ([AppIcon.launchPalette], deep → mid → bright).
 * Every scene follows the same choreography law — an ignition verb ≤950ms, then living motion tuned
 * to READ WITHIN the ~1s beat the intro is on screen — behind one uniform contract:
 * `uSize` px · `uTime` seconds · `uReveal` 0→1 eased ignition · `uC0/uC1/uC2` palette.
 *
 * On API 33+ scenes render per-pixel via [RuntimeShader] (fbm filaments, micro-dither, premultiplied
 * output). Below 33 — or if a shader fails to parse — Aurora falls back to its strip renderer and
 * every other family to a shared two-wash gradient (the palette's story without particles). A scene
 * never crashes the launch. Reduce-motion renders each scene fully revealed and frozen.
 */

/** True when [icon] has both a palette and a built scene — gates the backdrop AND the longer hold. */
fun hasLaunchScene(icon: AppIcon): Boolean =
    icon.launchPalette != null && SCENE_AGSL.containsKey(icon.family)

/**
 * The scene backdrop, composed UNDER the wordmark. Owns its own ignition animatable and clock so
 * [AvexIntro] stays a plain wordmark plate.
 */
@Composable
fun IconLaunchScene(icon: AppIcon, reduceMotion: Boolean, modifier: Modifier = Modifier) {
    val palette = remember(icon) { icon.launchPalette.orEmpty().map { Color(it) } }
    if (palette.isEmpty()) return

    // Ignition — one decelerating sweep; each scene maps it to its own verb (aurora sweeps across,
    // the nebula blooms out of its core, the molten pool surges up from the bottom edge).
    val ignite = remember { Animatable(if (reduceMotion) 1f else 0f) }
    LaunchedEffect(Unit) {
        if (!reduceMotion) {
            ignite.animateTo(1f, tween(ForgeMotion.scaledDuration(950), easing = ForgeMotion.Decelerate))
        }
    }
    // Continuous clock in SECONDS (speeds live in the shaders, per-second, tuned to the beat).
    val timeSec = if (reduceMotion) 0.8f else {
        val transition = rememberInfiniteTransition(label = "launch-scene")
        val t by transition.animateFloat(
            initialValue = 0f,
            targetValue = 120f,
            animationSpec = infiniteRepeatable(tween(120_000, easing = LinearEasing)),
            label = "launch-clock"
        )
        t
    }

    val shader = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        remember(icon) { runCatching { buildLaunchShader(icon.family, palette) }.getOrNull() }
    } else null

    if (shader != null && Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        val brush = remember(shader) { ShaderBrush(shader) }
        Canvas(modifier) {
            shader.setFloatUniform("uSize", size.width, size.height)
            shader.setFloatUniform("uTime", timeSec)
            shader.setFloatUniform("uReveal", ignite.value)
            drawRect(brush)
        }
        return
    }

    // Fallbacks (pre-33 / parse failure): Aurora keeps its strip renderer; other families get the
    // palette's vertical story — deep sky above, bright base below — with the ignition as a fade.
    Canvas(modifier) {
        val progress = ignite.value
        if (icon.family == IconFamily.Aurora) {
            drawAuroraStrips(palette, frac(timeSec / 7f), progress)
        } else {
            drawRect(
                brush = Brush.verticalGradient(
                    0f to palette.first().copy(alpha = 0.30f * progress),
                    0.45f to Color.Transparent,
                    0.80f to Color.Transparent,
                    1f to palette.last().copy(alpha = 0.18f * progress)
                )
            )
        }
    }
}

/** Parse a family's AGSL scene and bind the icon's palette (deep / mid / bright colour uniforms). */
@RequiresApi(Build.VERSION_CODES.TIRAMISU)
private fun buildLaunchShader(family: IconFamily, palette: List<Color>): RuntimeShader? {
    val scene = SCENE_AGSL[family] ?: return null
    return RuntimeShader(AGSL_PREAMBLE + scene).apply {
        setColorUniform("uC0", palette.first().toArgb())
        setColorUniform("uC1", palette[palette.size / 2].toArgb())
        setColorUniform("uC2", palette.last().toArgb())
    }
}

// ─── AGSL: shared preamble ────────────────────────────────────────────────────

/** Uniform contract + hash/value-noise/fbm helpers shared by every scene. */
private val AGSL_PREAMBLE = """
    uniform float2 uSize;
    uniform float uTime;    // seconds since the intro mounted — drives the living motion
    uniform float uReveal;  // 0→1 ignition sweep, eased on the Kotlin side
    layout(color) uniform half4 uC0;
    layout(color) uniform half4 uC1;
    layout(color) uniform half4 uC2;

    float hash21(float2 p) {
        p = fract(p * float2(123.34, 456.21));
        p += dot(p, p + 45.32);
        return fract(p.x * p.y);
    }
    float vnoise(float2 p) {
        float2 i = floor(p);
        float2 f = fract(p);
        float2 u = f * f * (3.0 - 2.0 * f);
        return mix(mix(hash21(i), hash21(i + float2(1.0, 0.0)), u.x),
                   mix(hash21(i + float2(0.0, 1.0)), hash21(i + float2(1.0, 1.0)), u.x), u.y);
    }
    float fbm(float2 p) {
        float v = 0.0;
        float a = 0.5;
        for (int i = 0; i < 4; i++) {
            v += a * vnoise(p);
            p = p * 2.13 + float2(17.3, 9.1);
            a *= 0.5;
        }
        return v;
    }

""".trimIndent()

// ─── AGSL: Aurora — curtain rays igniting across the sky ─────────────────────

private val AURORA_AGSL = """
    half4 main(float2 frag) {
        float TAU = 6.2831853;
        float2 uv = frag / uSize;

        // Undulating crest line — two travelling waves, fast enough to visibly move within ~1s.
        float crest = 0.15
            + 0.055 * sin(TAU * uv.x * 1.35 + uTime * 1.4)
            + 0.030 * sin(TAU * uv.x * 2.60 - uTime * 2.2);
        float d = uv.y - crest;

        // Ignition: a soft front sweeps left→right (front runs -0.15 → 1.45 so at uReveal=1 the
        // whole width is fully lit); columns behind it are alive, ahead of it still dark sky.
        float front = uReveal * 1.60 - 0.15;
        float ignite = 1.0 - smoothstep(front - 0.45, front, uv.x);
        float wash = clamp(uReveal * 1.6, 0.0, 1.0);

        // Unfurl: a column's curtain drops to full length as it ignites (envelope stretches in).
        float drop = 0.25 + 0.75 * ignite;
        float de = d / drop;
        float env;
        if (de < 0.0) { env = 0.45 + 0.55 * exp(de * 9.0); }
        else { env = exp(-de * 5.0); }

        // Living motion: filaments rain downward, a broad bright/dim sweep glides along the ribbon,
        // fold ripples travel faster on top, and the whole sheet breathes slightly.
        float rays = fbm(float2(uv.x * 20.0 + uTime * 0.30, uv.y * 1.5 - uTime * 0.55));
        rays = pow(rays, 2.0) * 1.9;
        float sweep  = 0.55 + 0.45 * sin(TAU * uv.x * 1.10 + uTime * 0.8);
        float fold   = 0.80 + 0.20 * sin(TAU * uv.x * 4.20 + uTime * 2.6);
        float breath = 0.92 + 0.08 * sin(uTime * 1.1);

        // Palette slides along the ribbon; height shades toward the deep-sky colour above the crest.
        float t = fract(uv.x + 0.05 * uTime + 0.22 * sin(uTime * 0.45 + TAU * uv.x * 0.6));
        float3 c0 = float3(uC0.rgb);
        float3 c1 = float3(uC1.rgb);
        float3 c2 = float3(uC2.rgb);
        float3 col;
        if (t < 0.5) { col = mix(c0, c1, t * 2.0); }
        else { col = mix(c1, c2, (t - 0.5) * 2.0); }
        col = mix(col, c0, clamp(-d * 2.0, 0.0, 0.6));

        float aur = env * (0.25 + 0.95 * rays) * sweep * fold * breath * ignite * 0.75;

        // A luminous rim leads the ignition front through the curtain band, gone once revealed.
        float rimMask = exp(-max(d, 0.0) * 3.0 - max(-d, 0.0) * 6.0);
        float rim = exp(-pow((uv.x - front) * 5.0, 2.0)) * (1.0 - uReveal) * 0.55 * rimMask;
        aur = min(aur + rim, 0.70);

        // The icon's own vertical story: ambient high-sky wash + warm horizon band.
        float sky = (1.0 - smoothstep(0.0, 0.58, uv.y)) * 0.30 * wash;
        float hor = smoothstep(0.80, 1.0, uv.y) * 0.20 * wash;

        // Additive light layers, premultiplied; micro-dither kills 8-bit banding on the slow washes.
        float a = clamp(aur + sky + hor + (hash21(frag) - 0.5) * 0.015, 0.0, 0.88);
        float3 rgb = col * aur + c0 * sky + c2 * hor;
        rgb = min(rgb, float3(a));
        return half4(half3(rgb), half(a));
    }
""".trimIndent()

// ─── AGSL: Nebula — deep space blooming out of a core ────────────────────────

private val NEBULA_AGSL = """
    // One potential star per grid cell: jittered position, hashed brightness, per-star twinkle.
    float stars(float2 p, float scale, float density, float t) {
        float2 st = p * scale;
        float2 id = floor(st);
        float2 f = fract(st) - 0.5;
        float gate = step(hash21(id), density);
        float2 off = (float2(hash21(id + 1.7), hash21(id + 4.3)) - 0.5) * 0.7;
        float d = length(f - off);
        float b = hash21(id + 9.1);
        float tw = 0.55 + 0.45 * sin(t * (1.0 + 2.5 * b) + b * 41.0);
        float core = smoothstep(0.09, 0.01, d);
        float glow = smoothstep(0.30, 0.0, d) * 0.25;
        return gate * (core + glow) * (0.35 + 0.65 * b * b) * tw;
    }

    half4 main(float2 frag) {
        float2 uv = frag / uSize;
        float ar = uSize.y / uSize.x;
        float2 p = float2(uv.x, uv.y * ar);          // aspect-true, so stars and blooms stay round
        float2 core = float2(0.5, 0.26 * ar);
        float dcore = length(p - core);

        // Ignition: stars prick in first, then the nebula BLOOMS radially out of its core.
        float sIn = smoothstep(0.0, 0.40, uReveal);
        float bloomR = uReveal * uReveal * (ar * 0.9 + 0.55);
        float gIn = 1.0 - smoothstep(bloomR - 0.40, bloomR, dcore);

        // Gas: drifting fbm, shaped around the core and the upper sky, quiet across the wordmark.
        float2 drift = float2(uTime * 0.030, -uTime * 0.018);
        float g = fbm(p * 2.4 + drift + float2(0.0, dcore * 0.6));
        g = g * g * 1.7;
        float vquiet = 1.0 - 0.62 * smoothstep(0.32, 0.55, uv.y);
        float coreHalo = exp(-dcore * dcore * 7.0);
        float shape = clamp(coreHalo * 1.2 + 0.35 * (1.0 - smoothstep(0.0, 0.45, uv.y)), 0.0, 1.0);
        float breath = 0.90 + 0.10 * sin(uTime * 0.9);
        float gasA = g * shape * vquiet * gIn * 0.55 * breath;

        // Core bloom — overshoots while igniting, settles as it completes.
        float coreA = exp(-dcore * dcore * 16.0) * (1.0 + 0.7 * (1.0 - uReveal)) * gIn * 0.50 * breath;

        // Two star layers: dense faint dust + sparse bright twinklers.
        float starA = (stars(p, 34.0, 0.32, uTime) * 0.35
                     + stars(p + 3.7, 13.0, 0.16, uTime) * 0.80) * sIn;

        float3 c0 = float3(uC0.rgb);
        float3 c1 = float3(uC1.rgb);
        float3 c2 = float3(uC2.rgb);
        float3 gasCol = mix(c0, c1, clamp(g * 1.4, 0.0, 1.0));
        gasCol = mix(gasCol, c2, clamp(coreHalo * 0.8, 0.0, 0.8));
        float3 starCol = mix(c2, float3(1.0), 0.55);

        float neb = min(gasA + coreA, 0.62);
        float a = clamp(neb + starA * 0.85 + (hash21(frag) - 0.5) * 0.015, 0.0, 0.85);
        float3 rgb = gasCol * neb + starCol * starA * 0.85;
        rgb = min(rgb, float3(a));
        return half4(half3(rgb), half(a));
    }
""".trimIndent()

// ─── AGSL: Molten — a fissure cracks open along the bottom ───────────────────

private val MOLTEN_AGSL = """
    // Sparse rising sparks with strong per-spark identity: squared-hash brightness (most dim, a few
    // bright), a slight vertical streak for motion, individual flicker, gentle wind.
    float sparks(float2 p, float scale, float rise, float density, float t) {
        float2 q = p + float2(0.03 * sin(t * 0.5 + p.y * 3.1), t * rise);
        float2 st = q * scale;
        float2 id = floor(st);
        float2 f = fract(st) - 0.5;
        float gate = step(hash21(id), density);
        float2 off = (float2(hash21(id + 2.3), hash21(id + 5.9)) - 0.5) * 0.75;
        float2 d2 = f - off;
        d2.y *= 0.62;
        float d = length(d2);
        float b = hash21(id + 8.4);
        b = b * b;
        float flick = 0.55 + 0.45 * sin(t * (6.0 + 7.0 * b) + b * 51.0);
        float core = smoothstep(0.085, 0.01, d);
        float halo = smoothstep(0.30, 0.0, d) * 0.35;
        return gate * (core + halo) * (0.15 + 0.85 * b) * flick;
    }

    half4 main(float2 frag) {
        float2 uv = frag / uSize;
        float ar = uSize.y / uSize.x;
        float2 p = float2(uv.x, uv.y * ar);

        float3 c0 = float3(uC0.rgb);
        float3 c1 = float3(uC1.rgb);
        float3 c2 = float3(uC2.rgb);

        // Ignition: the fissure CRACKS OPEN from the centre outward along the bottom, burning
        // ~2× brighter while it opens, then settling to a steady melt.
        float front = uReveal * 0.75;
        float lit = 1.0 - smoothstep(front - 0.20, front, abs(uv.x - 0.5));
        float flare = 1.0 + 1.1 * (1.0 - uReveal);

        // The molten surface: an undulating line low on the plate (high enough to give the melt a
        // real band). Veins of heat give the pool below it a crust instead of a flat gradient.
        float wob = fbm(float2(uv.x * 3.0 + uTime * 0.10, uTime * 0.16));
        float surf = 0.88 - 0.045 * (wob * 2.0 - 1.0);
        float dy = uv.y - surf;                      // >0 inside the pool
        float veins = fbm(float2(uv.x * 6.5 + uTime * 0.05, uv.y * 9.0 - uTime * 0.08));
        veins = smoothstep(0.45, 0.75, veins);

        float inPool = smoothstep(0.0, 0.012, dy);
        float3 poolCol = mix(c1, c2, veins * 0.85);
        float poolA = inPool * (0.40 + 0.32 * veins) * lit;

        // The crack itself: a tight white-hot seam along the surface — the scene's focal line.
        float crackA = exp(-abs(dy) * 34.0) * (0.55 + 0.45 * veins) * 0.75 * lit * flare;

        // Heat tongues licking upward from the seam (the aurora curtain language, inverted) —
        // the scene's sky-filling mass, reaching well past mid-screen before dissolving.
        float above = max(-dy, 0.0);
        float tongues = fbm(float2(uv.x * 13.0 - uTime * 0.22, uv.y * 2.4 + uTime * 0.85));
        tongues = pow(tongues, 2.0) * 2.2;
        float tongueA = exp(-above * 3.4) * tongues * 0.50 * lit
                      * (0.85 + 0.15 * sin(uTime * 1.7 + uv.x * 9.0));
        float3 tongueCol = mix(c1, c0, clamp(above * 2.6, 0.0, 0.75));
        tongueCol = mix(tongueCol, c2, clamp((0.10 - above) * 6.0, 0.0, 0.5));

        // The glow dome the melt throws on the air above — a lava field lights its own sky.
        float ambA = exp(-above * 1.6) * 0.20 * lit;

        // Crack-open flash: a warm bloom washes the plate as the fissure opens (peaks mid-reveal,
        // gone once settled) — the event beat that matches Aurora's travelling rim.
        float flashA = uReveal * (1.0 - uReveal) * 4.0 * 0.15 * (0.4 + 0.6 * exp(-above * 1.2));

        // Sparks pop off the seam once it's open, cooling as they climb; a few carry high.
        float alt = surf - uv.y;
        float gate = smoothstep(0.30, 0.90, uReveal) * lit;
        float climb = 1.0 - smoothstep(0.10, 0.70, alt);
        float sp = sparks(p, 9.0, 0.62, 0.10, uTime) * 1.15
                 + sparks(p + 7.7, 15.0, 0.40, 0.12, uTime) * 0.80;
        float3 sparkCol = mix(c2, c1, smoothstep(0.04, 0.35, alt));
        float sparkA = min(sp * gate * climb * 0.80, 0.80);

        float a = clamp(poolA + crackA + tongueA + ambA + flashA + sparkA + (hash21(frag) - 0.5) * 0.015, 0.0, 0.90);
        float3 rgb = poolCol * poolA + c2 * crackA + tongueCol * tongueA
                   + mix(c0, c1, 0.5) * ambA + c1 * flashA + sparkCol * sparkA;
        rgb = min(rgb, float3(a));
        return half4(half3(rgb), half(a));
    }
""".trimIndent()

// ─── AGSL: Stealth — a radar scope powers on in the dark ─────────────────────

private val STEALTH_AGSL = """
    half4 main(float2 frag) {
        float TAU = 6.2831853;
        float2 uv = frag / uSize;
        float ar = uSize.y / uSize.x;
        // Scope space: centred on the wordmark, aspect-true so the dial is round.
        float2 p = float2(uv.x - 0.5, (uv.y - 0.5) * ar);
        float r = length(p);
        float theta = atan(p.y, p.x);

        float3 c0 = float3(uC0.rgb);
        float3 c1 = float3(uC1.rgb);
        float3 c2 = float3(uC2.rgb);

        // The dial fades at its rim, and dims through the centre so the wordmark stays the target.
        float rimFade = 1.0 - smoothstep(0.88, 1.05, r);
        float centerDim = 0.30 + 0.70 * smoothstep(0.06, 0.30, r);

        // Ignition: the scope POWERS ON — hub pips first (with a flare that settles), rings draw
        // outward, then the sweep starts its first rotation and blips wake as it passes them.
        float hubA = exp(-r * 30.0) * (0.35 + 0.55 * (1.0 - uReveal)) * smoothstep(0.02, 0.15, uReveal);
        float ringReveal1 = smoothstep(0.25, 0.42, uReveal);
        float ringReveal2 = smoothstep(0.40, 0.60, uReveal);
        float ringReveal3 = smoothstep(0.58, 0.80, uReveal);
        float rings = exp(-abs(r - 0.30) * 90.0) * ringReveal1
                    + exp(-abs(r - 0.58) * 90.0) * ringReveal2
                    + exp(-abs(r - 0.86) * 90.0) * ringReveal3;
        float ringsA = rings * 0.16 * rimFade;

        // The sweep: a rotating phosphor beam — crisp leading edge, decaying afterglow trail.
        float a0 = uTime * 4.5;
        float delta = fract((a0 - theta) / TAU);
        float beam = exp(-delta * 50.0) * 0.45;
        float trail = exp(-delta * 6.0) * 0.28;
        float sweepGate = smoothstep(0.15, 0.45, uReveal);
        float sweepA = (beam + trail) * rimFade * centerDim * sweepGate;

        // Blips: sparse contacts that flash as the sweep passes and decay until it returns.
        float2 st = (p + 10.0) * 6.5;
        float2 id = floor(st);
        float2 f = fract(st) - 0.5;
        float gate = step(hash21(id), 0.05);
        float2 off = (float2(hash21(id + 2.9), hash21(id + 7.1)) - 0.5) * 0.6;
        float d = length(f - off);
        float dotCore = smoothstep(0.10, 0.02, d);
        float dotHalo = smoothstep(0.28, 0.0, d) * 0.35;
        float ping = exp(-delta * 5.0);
        float inBand = step(0.16, r) * step(r, 0.90);
        float blipA = gate * (dotCore + dotHalo) * ping * inBand
                    * smoothstep(0.35, 0.75, uReveal) * 0.70;

        // A breath of tinted dark keeps the plate stealth-tinted rather than plain black.
        float baseA = 0.08 * smoothstep(0.05, 0.30, uReveal);

        float a = clamp(hubA + ringsA + sweepA + blipA + baseA + (hash21(frag) - 0.5) * 0.012, 0.0, 0.85);
        float3 rgb = c2 * hubA + c1 * ringsA + mix(c1, c2, 0.35) * sweepA + c2 * blipA + c0 * baseA;
        rgb = min(rgb, float3(a));
        return half4(half3(rgb), half(a));
    }
""".trimIndent()

// ─── AGSL: Gem — prismatic shards catching the light ─────────────────────────

private val GEM_AGSL = """
    // A soft-edged plane of refracted light: a band through `pos` with normal `nrm`, faded by
    // distance so it reads as a local facet catch, shimmering gently on its own phase.
    float shard(float2 p, float2 pos, float2 nrm, float sharp, float reach, float t, float ph) {
        float band = exp(-abs(dot(p - pos, nrm)) * sharp);
        float local = exp(-length(p - pos) * reach);
        float shim = 0.80 + 0.20 * sin(t * 0.9 + ph);
        return band * local * shim;
    }
    // Rare 4-point sparkles: mostly asleep (pow⁸ duty cycle), brilliant when they wake.
    float sparkle(float2 p, float scale, float density, float t) {
        float2 st = p * scale;
        float2 id = floor(st);
        float2 f = fract(st) - 0.5;
        float gate = step(hash21(id), density);
        float2 off = (float2(hash21(id + 1.3), hash21(id + 6.1)) - 0.5) * 0.6;
        float2 q = f - off;
        float b = hash21(id + 3.7);
        float tw = pow(0.5 + 0.5 * sin(t * (1.5 + 2.5 * b) + b * 44.0), 8.0);
        float core = smoothstep(0.06, 0.0, length(q));
        float arms = smoothstep(0.20, 0.0, abs(q.x) + abs(q.y))
                   * smoothstep(0.035, 0.0, min(abs(q.x), abs(q.y))) * 0.7;
        return gate * (core + arms) * tw;
    }

    half4 main(float2 frag) {
        float2 uv = frag / uSize;
        float ar = uSize.y / uSize.x;
        float2 p = float2(uv.x, uv.y * ar);
        float3 c0 = float3(uC0.rgb);
        float3 c1 = float3(uC1.rgb);
        float3 c2 = float3(uC2.rgb);

        // Ignition: facets catch the light one after another (staggered), while one razor caustic
        // flash sweeps the plate diagonally — turning a gem in the light.
        float gA = smoothstep(0.05, 0.32, uReveal);
        float gB = smoothstep(0.22, 0.50, uReveal);
        float gC = smoothstep(0.40, 0.68, uReveal);
        float gD = smoothstep(0.55, 0.85, uReveal);
        float sA = shard(p, float2(0.16, 0.14 * ar), normalize(float2(0.57, 0.82)), 16.0, 2.0, uTime, 0.0) * gA;
        float sB = shard(p, float2(0.86, 0.20 * ar), normalize(float2(-0.47, 0.88)), 13.0, 2.2, uTime, 2.1) * gB;
        float sC = shard(p, float2(0.50, 0.05 * ar), normalize(float2(0.26, -0.97)), 11.0, 1.8, uTime, 4.2) * gC;
        float sD = shard(p, float2(0.94, 0.62 * ar), normalize(float2(-0.87, 0.50)), 18.0, 2.6, uTime, 1.3) * gD * 0.7;

        // Prismatic dispersion: each facet takes its own stop on the palette.
        float3 colA = mix(c0, c1, 0.8);
        float3 colB = c1;
        float3 colC = mix(c1, c2, 0.6);
        float3 colD = mix(c0, c1, 0.5);

        float flashPos = uReveal * 1.55 - 0.15;
        float s = dot(uv, normalize(float2(0.80, 0.60)));
        float flashA = exp(-pow((s - flashPos) * 9.0, 2.0)) * (1.0 - uReveal) * 0.55;

        float sp = sparkle(p, 17.0, 0.30, uTime) + sparkle(p + 4.9, 9.0, 0.22, uTime * 1.1) * 1.2;
        float sparkleA = min(sp * 0.8, 0.8) * smoothstep(0.15, 0.55, uReveal);

        // A whisper of deep facet tint in the upper corners for cohesion.
        float amb = (exp(-length(p) * 1.6) + exp(-length(p - float2(1.0, 0.0)) * 1.6)) * 0.10;

        float aTot = (sA + sB + sC + sD) * 0.45 + flashA + sparkleA + amb;
        float a = clamp(aTot + (hash21(frag) - 0.5) * 0.012, 0.0, 0.85);
        float3 rgb = (colA * sA + colB * sB + colC * sC + colD * sD) * 0.45
                   + c2 * flashA + mix(c2, float3(1.0), 0.3) * sparkleA + c0 * amb;
        rgb = min(rgb, float3(a));
        return half4(half3(rgb), half(a));
    }
""".trimIndent()

/** Family → scene body. A family absent here launches with the plain wordmark even if its icons
 *  carry palettes (lets palettes land ahead of their scene). */
private val SCENE_AGSL: Map<IconFamily, String> = mapOf(
    IconFamily.Aurora to AURORA_AGSL,
    IconFamily.Nebula to NEBULA_AGSL,
    IconFamily.Molten to MOLTEN_AGSL,
    IconFamily.Stealth to STEALTH_AGSL,
    IconFamily.Gem to GEM_AGSL,
)

// ─── Aurora strip fallback (pre-33 / parse failure) ──────────────────────────

private fun frac(v: Float): Float = v - kotlin.math.floor(v)

/** Interpolate across the whole palette: t 0→1 sweeps first colour → last through each stop. */
private fun paletteLerp(palette: List<Color>, t: Float): Color {
    if (palette.size == 1) return palette[0]
    val pos = t.coerceIn(0f, 1f) * (palette.size - 1)
    val i = pos.toInt().coerceAtMost(palette.size - 2)
    return lerp(palette[i], palette[i + 1], pos - i)
}

/** The pre-33 aurora: washes + two passes of overlapping translucent gradient strips. */
private fun DrawScope.drawAuroraStrips(palette: List<Color>, phase: Float, progress: Float) {
    // High-sky wash — the aurora's ambient light, deepest palette colour, gone by mid-screen.
    drawRect(
        brush = Brush.verticalGradient(
            0f to palette.first().copy(alpha = 0.40f * progress),
            0.32f to paletteLerp(palette, 0.5f).copy(alpha = 0.12f * progress),
            0.58f to Color.Transparent
        )
    )
    // Horizon glow — the icon art's warm base, echoed as a thin band of dawn at the bottom.
    drawRect(
        brush = Brush.verticalGradient(
            0.80f to Color.Transparent,
            1f to palette.last().copy(alpha = 0.20f * progress)
        )
    )
    // Soft bloom sheet behind, then fine rays in front.
    drawCurtain(palette, phase, progress, strips = 40, overlap = 4.5f, alphaScale = 0.5f, lenScale = 1.15f)
    drawCurtain(palette, phase, progress, strips = 300, overlap = 3f, alphaScale = 1f, lenScale = 1f)
}

/**
 * One pass of aurora curtain rays: [strips] vertical gradient strips, each [overlap]× its pitch wide
 * so neighbours blend smoothly. A ray starts just OFF the top edge already lit (no visible upper
 * boundary), peaks at the undulating crest, and dissolves to nothing at its tail.
 */
private fun DrawScope.drawCurtain(
    palette: List<Color>,
    phase: Float,
    progress: Float,
    strips: Int,
    overlap: Float,
    alphaScale: Float,
    lenScale: Float,
) {
    val twoPi = 2f * PI.toFloat()
    val w = size.width
    val h = size.height
    val stripW = w / strips
    // Start slightly off-screen so the curtain bleeds past the top edge instead of being cropped.
    val top = -h * 0.02f
    for (i in 0 until strips) {
        val x = (i + 0.5f) * stripW
        val u = x / w
        val crest = h * (
            0.15f +
                0.055f * sin(twoPi * (u * 1.35f + phase)) +
                0.030f * sin(twoPi * (u * 2.60f - phase * 1.7f))
            )
        val tail = crest + h * lenScale * (
            0.22f +
                0.09f * sin(twoPi * (u * 3.1f + phase * 1.3f + 0.25f)) +
                0.045f * sin(twoPi * (u * 5.3f - phase))
            )
        // Palette position flows sideways over time — the colour bands slide along the ribbon.
        val t = frac(u + 0.22f * sin(twoPi * (phase * 0.8f + u * 0.6f)))
        val col = paletteLerp(palette, t)
        // Per-ray shimmer; floor keeps every ray faintly present, ceiling keeps the text readable.
        val peak = ((0.42f + 0.20f * sin(twoPi * (u * 4.2f + phase * 2f))) * progress * alphaScale)
            .coerceIn(0f, 0.60f)
        // Where the crest sits within this strip's own span — the gradient's brightest stop.
        val crestFrac = ((crest - top) / (tail - top)).coerceIn(0.05f, 0.85f)
        drawRect(
            brush = Brush.verticalGradient(
                0f to col.copy(alpha = peak * 0.45f),                        // lit at the screen edge
                crestFrac to col.copy(alpha = peak),                         // brightest at the crest
                crestFrac + (1f - crestFrac) * 0.45f to col.copy(alpha = peak * 0.30f), // soft decay
                1f to col.copy(alpha = 0f),                                  // dissolves at the tail
                startY = top,
                endY = tail
            ),
            topLeft = Offset(x - stripW * overlap / 2f, top),
            size = Size(stripW * overlap, tail - top)
        )
    }
}
