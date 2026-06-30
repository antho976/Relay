@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.settings

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.drag
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.util.Locale
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.min
import kotlin.math.sin

private const val DEFAULT_ACCENT = "#3D4F73"

/** Precompiled once — a valid `#RRGGBB` accent. Avoids allocating a Regex on every keystroke/recompose. */
private val HEX_REGEX = Regex("#[0-9A-F]{6}")

/**
 * Below this WCAG relative luminance the accent — applied as `primary` (i.e. accent *text*) on the
 * near-black Pearl background — is too dim to read comfortably (this floor ≈ 2.9:1 contrast). The
 * custom controls never expose a colour beneath it: the brightness slider spans only the readable
 * range ([minReadableValue]) so the dim end isn't there to land on, the wheel lifts brightness to
 * the floor ([readableHsvToHex]), and typed hex is rejected ([isReadableAccent]). The curated
 * presets bypass this gate — they're hand-picked and always selectable, even ones a touch below.
 */
private const val MIN_ACCENT_LUMINANCE = 0.11

private val ACCENT_PRESETS = listOf(
    "#3D4F73" to "Navy", "#8B3535" to "Red", "#4D6040" to "Olive", "#7A6435" to "Gold",
    "#356B6B" to "Teal", "#5B4570" to "Purple", "#8B3556" to "Rose", "#3E5E3E" to "Forest",
    "#8B5A35" to "Copper", "#445A6B" to "Steel", "#6B4535" to "Rust", "#556B35" to "Moss"
)

@Composable
internal fun AccentColorRow(currentHex: String, onSelect: (String) -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    // The custom-hex preview swatch doubles as the wheel toggle.
    var wheelVisible by remember { mutableStateOf(false) }

    // Lag-free picking: the wheel/slider mutate `liveHex` synchronously, so the indicator and preview
    // swatch update instantly without waiting on a DataStore round-trip. The expensive work —
    // persisting the accent (disk write) and re-theming the whole app — runs ONCE per gesture, on
    // release (onPickEnd), instead of on every drag frame. `dragging` suppresses external re-sync
    // while a finger is down so a slow commit can't yank the indicator back.
    var liveHex by remember { mutableStateOf(currentHex.ifEmpty { DEFAULT_ACCENT }) }
    var dragging by remember { mutableStateOf(false) }
    LaunchedEffect(currentHex) {
        if (!dragging) liveHex = currentHex.ifEmpty { DEFAULT_ACCENT }
    }

    Column(
        modifier = Modifier.padding(horizontal = 24.dp, vertical = 4.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Text("Accent color", style = MaterialTheme.typography.bodyMedium, color = onBg)
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            ACCENT_PRESETS.forEach { (hex, label) ->
                val isSelected = currentHex == hex || (currentHex.isEmpty() && hex == DEFAULT_ACCENT)
                val swatchColor = remember(hex) { Color(android.graphics.Color.parseColor(hex)) }
                Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                    // Tapping a preset writes its hex into the custom field too (currentHex drives it).
                    modifier = Modifier.clickable { onSelect(hex) }
                ) {
                    Box(
                        modifier = Modifier
                            .size(28.dp)
                            .clip(CircleShape)
                            .background(swatchColor)
                            .border(2.dp, if (isSelected) onBg else Color.Transparent, CircleShape)
                    )
                    Text(
                        label,
                        style = MaterialTheme.typography.labelSmall,
                        color = muted.copy(alpha = if (isSelected) 0.9f else 0.45f),
                        fontSize = 9.sp
                    )
                }
            }
        }
        Spacer(Modifier.height(4.dp))
        CustomHexInput(
            currentHex = currentHex,
            livePreview = liveHex,
            onSelect = onSelect,
            onBg = onBg,
            muted = muted,
            wheelVisible = wheelVisible,
            onToggleWheel = { wheelVisible = !wheelVisible }
        )
        Text(
            "Enter a 6-digit #RRGGBB hex code, or tap the swatch for a color wheel.",
            style = MaterialTheme.typography.labelSmall,
            color = muted.copy(alpha = 0.55f),
            fontSize = 10.sp
        )
        AnimatedVisibility(visible = wheelVisible) {
            Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(16.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 8.dp, bottom = 8.dp)
            ) {
                ColorWheel(
                    hex = liveHex,
                    onBg = onBg,
                    onPickStart = { dragging = true },
                    onPick = { liveHex = it },
                    onPickEnd = { dragging = false; onSelect(it) }
                )
                BrightnessSlider(
                    hex = liveHex,
                    muted = muted,
                    onPickStart = { dragging = true },
                    onPick = { liveHex = it },
                    onPickEnd = { dragging = false; onSelect(it) }
                )
            }
        }
        Spacer(Modifier.height(4.dp))
    }
}

/**
 * Type any `#RRGGBB` hex for an accent colour outside the preset palette; applies live once valid.
 * The preview swatch doubles as the show/hide toggle for the colour wheel and reflects the live
 * wheel/slider pick instantly ([livePreview]); the text field tracks the same live value so it never
 * shows a stale hex mid-drag (it settles on the committed value when the gesture ends).
 */
@Composable
private fun CustomHexInput(
    currentHex: String,
    livePreview: String,
    onSelect: (String) -> Unit,
    onBg: Color,
    muted: Color,
    wheelVisible: Boolean,
    onToggleWheel: () -> Unit
) {
    // Track the live (in-progress) pick so the field mirrors the wheel/slider during a drag — matching
    // the swatch — instead of lagging on the committed value. When idle livePreview == currentHex, and
    // it settles back to the committed hex on release, so typing/presets still round-trip cleanly.
    var text by remember(livePreview) {
        mutableStateOf(livePreview.takeIf { it.length == 7 }.orEmpty())
    }
    // Swatch tracks the live (in-progress) pick so dragging the wheel gives instant feedback.
    val swatch = remember(livePreview) {
        runCatching { Color(android.graphics.Color.parseColor(livePreview)) }.getOrNull()
    }
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text("Custom hex", style = MaterialTheme.typography.bodySmall, color = muted)
        BasicTextField(
            value = text,
            onValueChange = { raw ->
                // Normalize: a single leading '#', uppercase hex only, capped at #RRGGBB.
                val hex = raw.uppercase().filter { it in "0123456789ABCDEF" }.take(6)
                text = "#$hex"
                // Apply only valid AND bright-enough hex; a too-faint colour is ignored like an
                // incomplete one (it would render accent text invisible on the dark UI).
                if (HEX_REGEX.matches(text) && isReadableAccent(text)) onSelect(text)
            },
            singleLine = true,
            textStyle = MaterialTheme.typography.bodyMedium.copy(color = onBg),
            cursorBrush = SolidColor(onBg),
            decorationBox = { inner ->
                Box(contentAlignment = Alignment.CenterStart) {
                    if (text.isEmpty() || text == "#") {
                        Text(
                            "#RRGGBB",
                            style = MaterialTheme.typography.bodyMedium,
                            color = muted.copy(alpha = 0.4f)
                        )
                    }
                    inner()
                }
            },
            modifier = Modifier
                .clip(RoundedCornerShape(6.dp))
                .border(1.dp, muted.copy(alpha = 0.4f), RoundedCornerShape(6.dp))
                .padding(horizontal = 10.dp, vertical = 6.dp)
                .size(width = 96.dp, height = 20.dp)
        )
        // Tap the live-preview swatch to reveal/hide the colour wheel. Ring highlights when open.
        Box(
            modifier = Modifier
                .size(28.dp)
                .clip(CircleShape)
                .clickable(onClick = onToggleWheel)
                .background(swatch ?: Color.Transparent)
                .border(
                    width = if (wheelVisible) 2.dp else 1.dp,
                    color = if (wheelVisible) onBg else muted.copy(alpha = 0.4f),
                    shape = CircleShape
                )
        )
    }
}

@Composable
private fun ColorWheel(
    hex: String,
    onBg: Color,
    onPickStart: () -> Unit,
    onPick: (String) -> Unit,
    onPickEnd: (String) -> Unit
) {
    val hsv = remember(hex) { hexToHsv(hex) }
    // Read the latest values via state inside the gesture to dodge stale closures without restarting it.
    val valueState = rememberUpdatedState(hsv[2]) // brightness, sampled ONCE at each gesture's down
    val startState = rememberUpdatedState(onPickStart)
    val pickState = rememberUpdatedState(onPick)
    val endState = rememberUpdatedState(onPickEnd)
    val ringColor = onBg

    // Static gradients — identical every frame — so build them ONCE instead of allocating per draw
    // (avoids GC churn during a drag). Both auto-centre in the draw area; the radial's default radius
    // is size.minDimension / 2, matching the wheel radius drawn below.
    val hueBrush = remember {
        Brush.sweepGradient(
            listOf(Color.Red, Color.Yellow, Color.Green, Color.Cyan, Color.Blue, Color.Magenta, Color.Red)
        )
    }
    val satBrush = remember { Brush.radialGradient(listOf(Color.White, Color.Transparent)) }

    Canvas(
        modifier = Modifier
            .size(176.dp)
            .pointerInput(Unit) {
                awaitEachGesture {
                    val down = awaitFirstDown()
                    startState.value()
                    // Sample brightness ONCE, at down — picking hue/saturation keeps it fixed. Reading it
                    // live each frame would let readableHsvToHex's readability lift feed back through
                    // liveHex and ratchet brightness upward across the drag.
                    val value = valueState.value
                    var last = hueSatToHex(down.position, size, value)
                    pickState.value(last)
                    down.consume()
                    drag(down.id) { change ->
                        last = hueSatToHex(change.position, size, value)
                        pickState.value(last)
                        change.consume()
                    }
                    // Commit once, on release — the single point that persists to DataStore + re-themes the app.
                    endState.value(last)
                }
            }
    ) {
        val radius = size.minDimension / 2f
        val center = Offset(size.width / 2f, size.height / 2f)
        // Hue around the circle (clockwise from 3 o'clock).
        drawCircle(brush = hueBrush, radius = radius, center = center)
        // Saturation: white at the centre fading to transparent at the rim.
        drawCircle(brush = satBrush, radius = radius, center = center)
        // Brightness: dim the whole wheel toward black as value drops.
        if (hsv[2] < 1f) {
            drawCircle(Color.Black.copy(alpha = 1f - hsv[2]), radius = radius, center = center)
        }
        // Crisp rim so the wheel reads as a finished control rather than a raw gradient disc.
        drawCircle(
            ringColor.copy(alpha = 0.22f),
            radius = radius - 0.5.dp.toPx(),
            center = center,
            style = Stroke(width = 1.dp.toPx())
        )

        // Selector: a "lollipop" — the live colour filled inside a white ring with a faint dark
        // outline, so it stays legible over any hue (pale or dark) instead of a flat white dot.
        val angle = Math.toRadians(hsv[0].toDouble())
        val sel = Offset(
            x = center.x + (radius * hsv[1] * cos(angle)).toFloat(),
            y = center.y + (radius * hsv[1] * sin(angle)).toFloat()
        )
        val selColor = Color(android.graphics.Color.HSVToColor(hsv))
        val fillR = 7.dp.toPx()
        val ringW = 2.5.dp.toPx()
        drawCircle(selColor, radius = fillR, center = sel)
        drawCircle(Color.White, radius = fillR + ringW / 2f, center = sel, style = Stroke(width = ringW))
        drawCircle(
            Color.Black.copy(alpha = 0.25f),
            radius = fillR + ringW + 0.5.dp.toPx(),
            center = sel,
            style = Stroke(width = 1.dp.toPx())
        )
    }
}

@Composable
private fun BrightnessSlider(
    hex: String,
    muted: Color,
    onPickStart: () -> Unit,
    onPick: (String) -> Unit,
    onPickEnd: (String) -> Unit
) {
    val hsv = remember(hex) { hexToHsv(hex) }
    val hsvState = rememberUpdatedState(hsv)
    val startState = rememberUpdatedState(onPickStart)
    val pickState = rememberUpdatedState(onPick)
    val endState = rememberUpdatedState(onPickEnd)
    // Smallest brightness for this hue/sat that still clears the readability floor — the bar starts
    // here, so the dim, unreadable end is hidden rather than blocked.
    val vMin = remember(hsv[0], hsv[1]) { minReadableValue(hsv[0], hsv[1]) }
    val fullColor = remember(hsv[0], hsv[1]) {
        Color(android.graphics.Color.HSVToColor(floatArrayOf(hsv[0], hsv[1], 1f)))
    }
    val minColor = remember(hsv[0], hsv[1]) {
        Color(android.graphics.Color.HSVToColor(floatArrayOf(hsv[0], hsv[1], vMin)))
    }
    // Gradient spans only the readable range [vMin … 1]; cache it so it isn't rebuilt per draw.
    val barBrush = remember(minColor, fullColor) { Brush.horizontalGradient(listOf(minColor, fullColor)) }
    Canvas(
        modifier = Modifier
            .fillMaxWidth()
            .height(26.dp)
            .pointerInput(Unit) {
                // Inset the touch track to match the thumb's drawn track ([outerR, width-outerR] below)
                // so the value a tap selects lines up with where the thumb renders. Equals outerR (12.dp).
                val thumbInset = 12.dp.toPx()
                awaitEachGesture {
                    val down = awaitFirstDown()
                    startState.value()
                    var last = valueToHex(down.position.x, size, hsvState.value, thumbInset)
                    pickState.value(last)
                    down.consume()
                    drag(down.id) { change ->
                        last = valueToHex(change.position.x, size, hsvState.value, thumbInset)
                        pickState.value(last)
                        change.consume()
                    }
                    endState.value(last)
                }
            }
    ) {
        // Track sits thinner than the canvas so the thumb can stand proud of it.
        val trackH = 12.dp.toPx()
        val rTrack = trackH / 2f
        val top = (size.height - trackH) / 2f
        val trackTopLeft = Offset(0f, top)
        val trackSize = Size(size.width, trackH)
        drawRoundRect(brush = barBrush, topLeft = trackTopLeft, size = trackSize, cornerRadius = CornerRadius(rTrack, rTrack))
        drawRoundRect(
            color = muted.copy(alpha = 0.25f),
            topLeft = trackTopLeft,
            size = trackSize,
            cornerRadius = CornerRadius(rTrack, rTrack),
            style = Stroke(width = 1.dp.toPx())
        )
        // Thumb matches the wheel selector: live colour, white ring, faint dark outline.
        val thumbR = 9.dp.toPx()
        val ringW = 2.5.dp.toPx()
        val outerR = thumbR + ringW + 0.5.dp.toPx()
        // Map the current value onto the readable [vMin, 1] span the bar represents.
        val span = 1f - vMin
        val frac = if (span <= 0.0001f) 1f else ((hsv[2] - vMin) / span).coerceIn(0f, 1f)
        // Place the thumb on the same inset track the touch maps over ([outerR, width-outerR]) so the
        // indicator and the value it represents agree at the extremes (and the thumb never clips).
        val thumbX = outerR + frac * (size.width - 2f * outerR)
        val thumbCenter = Offset(thumbX, size.height / 2f)
        val thumbColor = Color(android.graphics.Color.HSVToColor(hsv))
        drawCircle(thumbColor, radius = thumbR, center = thumbCenter)
        drawCircle(Color.White, radius = thumbR + ringW / 2f, center = thumbCenter, style = Stroke(width = ringW))
        drawCircle(Color.Black.copy(alpha = 0.25f), radius = outerR, center = thumbCenter, style = Stroke(width = 1.dp.toPx()))
    }
}

// ─── HSV helpers ─────────────────────────────────────────────────────────────

/** Convert a wheel touch to a hue+saturation pick (brightness preserved) and return its hex. */
private fun hueSatToHex(pos: Offset, size: IntSize, value: Float): String {
    val radius = min(size.width, size.height) / 2f
    if (radius <= 0f) return readableHsvToHex(0f, 0f, value)
    val dx = pos.x - size.width / 2f
    val dy = pos.y - size.height / 2f
    val sat = (hypot(dx.toDouble(), dy.toDouble()) / radius).coerceIn(0.0, 1.0).toFloat()
    var deg = Math.toDegrees(atan2(dy.toDouble(), dx.toDouble())).toFloat()
    if (deg < 0f) deg += 360f
    return readableHsvToHex(deg, sat, value)
}

/**
 * Convert a horizontal touch on the brightness bar to a value pick (hue/sat preserved). The bar
 * spans only the readable range [[minReadableValue] … 1], so the dim, unreadable end simply isn't
 * there to land on: x = 0 → the dimmest still-legible shade, x = width → full brightness.
 */
private fun valueToHex(x: Float, size: IntSize, hsv: FloatArray, insetPx: Float = 0f): String {
    val width = size.width
    val vMin = minReadableValue(hsv[0], hsv[1])
    if (width <= 0) return hsvToHex(hsv[0], hsv[1], hsv[2].coerceAtLeast(vMin))
    // Map over the inset track [insetPx, width-insetPx] so a tap matches the thumb's drawn position
    // (the thumb is clamped to the same inset so it never clips at the edges).
    val usable = (width - 2f * insetPx).coerceAtLeast(1f)
    val frac = ((x - insetPx) / usable).coerceIn(0f, 1f)
    val v = vMin + frac * (1f - vMin)
    return hsvToHex(hsv[0], hsv[1], v)
}

private fun hsvToHex(h: Float, s: Float, v: Float): String {
    val argb = android.graphics.Color.HSVToColor(
        floatArrayOf(h, s.coerceIn(0f, 1f), v.coerceIn(0f, 1f))
    )
    return String.format(Locale.US, "#%06X", 0xFFFFFF and argb)
}

private fun hexToHsv(hex: String): FloatArray {
    val hsv = FloatArray(3)
    val color = runCatching { android.graphics.Color.parseColor(hex) }
        .getOrElse { android.graphics.Color.parseColor(DEFAULT_ACCENT) }
    android.graphics.Color.colorToHSV(color, hsv)
    return hsv
}

// ─── Readability gate ─────────────────────────────────────────────────────────
// The accent becomes `primary` (accent text) on the near-black Pearl UI, so a too-dark/faint pick
// renders unreadable. These keep faint colours off the picker — see [MIN_ACCENT_LUMINANCE].

/** sRGB → linear for one 0–255 channel, per the WCAG relative-luminance definition. */
private fun channelLuminance(c: Int): Double {
    val s = c / 255.0
    return if (s <= 0.03928) s / 12.92 else Math.pow((s + 0.055) / 1.055, 2.4)
}

/** WCAG relative luminance of an ARGB colour (0 = black … 1 = white). */
private fun relLuminance(argb: Int): Double =
    0.2126 * channelLuminance((argb shr 16) and 0xFF) +
        0.7152 * channelLuminance((argb shr 8) and 0xFF) +
        0.0722 * channelLuminance(argb and 0xFF)

/** A typed accent is pickable only if it's bright enough to read as text on the dark UI. */
private fun isReadableAccent(hex: String): Boolean {
    val argb = runCatching { android.graphics.Color.parseColor(hex) }.getOrNull() ?: return false
    return relLuminance(argb) >= MIN_ACCENT_LUMINANCE
}

/**
 * Like [hsvToHex], but nudges the colour up to clear [MIN_ACCENT_LUMINANCE] so the wheel never
 * commits an unreadable accent (its stated invariant). Two stages: (1) lift brightness (preserves
 * hue & saturation) — the common case; (2) if a deep hue (saturated blue/violet) can't clear the
 * floor at any brightness, desaturate toward white. Luminance rises monotonically as value rises
 * (hue/sat fixed) and as saturation drops (at full value), so both loops converge — white always
 * clears the floor — and the v = 1 / s = 0 caps bound them.
 */
private fun readableHsvToHex(h: Float, s: Float, v: Float): String {
    var sat = s.coerceIn(0f, 1f)
    var value = v.coerceIn(0f, 1f)
    while (value < 1f &&
        relLuminance(android.graphics.Color.HSVToColor(floatArrayOf(h, sat, value))) < MIN_ACCENT_LUMINANCE
    ) {
        value = (value + 0.02f).coerceAtMost(1f)
    }
    while (sat > 0f &&
        relLuminance(android.graphics.Color.HSVToColor(floatArrayOf(h, sat, value))) < MIN_ACCENT_LUMINANCE
    ) {
        sat = (sat - 0.02f).coerceAtLeast(0f)
    }
    return hsvToHex(h, sat, value)
}

/**
 * The lowest HSV value (brightness) at which [h]/[s] still clears [MIN_ACCENT_LUMINANCE] — i.e. the
 * dimmest legible shade of this hue/sat. The brightness bar starts here so its unreadable lower end
 * is hidden. Binary search over the monotonic luminance-vs-value curve; returns 1f when even full
 * brightness can't clear the floor (e.g. a fully-saturated deep blue), leaving only the brightest shade.
 */
private fun minReadableValue(h: Float, s: Float): Float {
    val sat = s.coerceIn(0f, 1f)
    if (relLuminance(android.graphics.Color.HSVToColor(floatArrayOf(h, sat, 1f))) < MIN_ACCENT_LUMINANCE) return 1f
    var lo = 0f
    var hi = 1f
    repeat(16) {
        val mid = (lo + hi) / 2f
        if (relLuminance(android.graphics.Color.HSVToColor(floatArrayOf(h, sat, mid))) < MIN_ACCENT_LUMINANCE) {
            lo = mid
        } else {
            hi = mid
        }
    }
    return hi
}
