package com.forge.app.ui.settings

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.drag
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.ExpandLess
import androidx.compose.material.icons.rounded.ExpandMore
import androidx.compose.material3.Icon
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
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.theme.ForgeMotion
import java.util.Locale
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.min
import kotlin.math.sin

private const val DEFAULT_ACCENT = "#E23D3D"

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

// Red leads: it is the default (2026-08-23), and the warm end of the list sits first so the picker
// opens on the temperature the app is designed at rather than on the cool presets (2026-08-16).
// This Red replaced a deeper `#8B3535`, which measured 2.42:1 as text and could not be a default;
// anyone still holding that hex keeps it, the swatch simply no longer highlights.
private val ACCENT_PRESETS = listOf(
    "#E23D3D" to "Red", "#D4761F" to "Ember", "#8B5A35" to "Copper", "#7A6435" to "Gold",
    "#6B4535" to "Rust", "#8B3556" to "Rose", "#556B35" to "Moss", "#4D6040" to "Olive",
    "#3E5E3E" to "Forest", "#356B6B" to "Teal", "#3D4F73" to "Navy", "#445A6B" to "Steel"
)

/** The accent's name: its preset's, or "Custom" for a colour picked on the wheel or typed. */
internal fun accentName(hex: String): String =
    ACCENT_PRESETS.firstOrNull { it.first.equals(hex.ifEmpty { DEFAULT_ACCENT }, ignoreCase = true) }?.second ?: "Custom"

/**
 * The accent picker, drawn as two rows of the Accent group: the twelve presets as a swatch grid,
 * then a Custom row showing the live colour and hex that opens the wheel and a hex field.
 */
@Composable
internal fun AccentColorPicker(currentHex: String, onSelect: (String) -> Unit) {
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

    SettingsGroupBlock {
        // One choice among twelve, announced as such: `selectableGroup` + a radio role per swatch,
        // each named by its colour.
        Column(Modifier.selectableGroup(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            ACCENT_PRESETS.chunked(6).forEach { row ->
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    row.forEach { (hex, label) ->
                        val selected = currentHex.equals(hex, ignoreCase = true) || (currentHex.isEmpty() && hex == DEFAULT_ACCENT)
                        AccentSwatch(hex, label, selected) { onSelect(hex) }
                    }
                }
            }
        }
    }

    val isCustom = accentName(currentHex) == "Custom"
    val liveColor = remember(liveHex) { parseHex(liveHex) }
    SettingsRowContainer(
        interaction = Modifier.clickableLabeled(if (wheelVisible) "Hide the colour wheel" else "Pick a custom colour") {
            wheelVisible = !wheelVisible
        }
    ) {
        // The live colour once a custom one is picked; until then a hue wheel, so the tile reads as
        // "any colour" rather than repeating the preset above.
        val hueRing = remember {
            Brush.sweepGradient(listOf(Color.Red, Color.Yellow, Color.Green, Color.Cyan, Color.Blue, Color.Magenta, Color.Red))
        }
        Box(
            Modifier
                .size(40.dp)
                .clip(RoundedCornerShape(12.dp))
                .then(
                    if (isCustom) Modifier.background(liveColor ?: MaterialTheme.colorScheme.surfaceContainerHighest)
                    else Modifier.background(MaterialTheme.colorScheme.surfaceContainerHighest)
                )
        ) {
            if (!isCustom) {
                Box(Modifier.align(Alignment.Center).size(24.dp).clip(CircleShape).background(hueRing))
            } else {
                Icon(
                    Icons.Rounded.Check, contentDescription = null,
                    tint = contentOn(liveColor),
                    modifier = Modifier.align(Alignment.Center).size(20.dp)
                )
            }
        }
        SettingsRowText("Custom color", if (isCustom) liveHex else "Pick any shade on a wheel, or type a hex")
        Icon(
            if (wheelVisible) Icons.Rounded.ExpandLess else Icons.Rounded.ExpandMore,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }

    AnimatedVisibility(
        visible = wheelVisible,
        enter = expandVertically(ForgeMotion.enterTween()) + fadeIn(ForgeMotion.enterTween()),
        exit = shrinkVertically(ForgeMotion.exitTween()) + fadeOut(ForgeMotion.exitTween(ForgeMotion.DurationFast))
    ) {
        SettingsGroupBlock {
            Column(
                Modifier.fillMaxWidth(),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(20.dp)
            ) {
                ColorWheel(
                    hex = liveHex,
                    onBg = MaterialTheme.colorScheme.onSurface,
                    onPickStart = { dragging = true },
                    onPick = { liveHex = it },
                    onPickEnd = { dragging = false; onSelect(it) }
                )
                BrightnessSlider(
                    hex = liveHex,
                    muted = MaterialTheme.colorScheme.onSurfaceVariant,
                    onPickStart = { dragging = true },
                    onPick = { liveHex = it },
                    onPickEnd = { dragging = false; onSelect(it) }
                )
                HexField(livePreview = liveHex, onSelect = onSelect)
            }
        }
    }
}

/** One preset: a 36dp disc inside a 48dp target, ringed and checked when it is the accent. */
@Composable
private fun AccentSwatch(hex: String, label: String, selected: Boolean, onClick: () -> Unit) {
    val color = remember(hex) { parseHex(hex) ?: Color.Gray }
    val ring = MaterialTheme.colorScheme.onSurface
    Box(
        Modifier
            .size(48.dp)
            .clip(CircleShape)
            .semantics(mergeDescendants = true) { contentDescription = label }
            .selectable(selected = selected, role = Role.RadioButton, onClick = onClick)
            .then(if (selected) Modifier.border(2.dp, ring, CircleShape) else Modifier)
            .padding(5.dp)
            .clip(CircleShape)
            .background(color),
        contentAlignment = Alignment.Center
    ) {
        if (selected) Icon(Icons.Rounded.Check, contentDescription = null, tint = contentOn(color), modifier = Modifier.size(20.dp))
    }
}

/**
 * Type any `#RRGGBB` for an accent outside the presets; applies live once valid and readable. It
 * tracks the wheel's live value so it never shows a stale hex mid-drag.
 */
@Composable
private fun HexField(livePreview: String, onSelect: (String) -> Unit) {
    val onSurface = MaterialTheme.colorScheme.onSurface
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    var text by remember(livePreview) { mutableStateOf(livePreview.takeIf { it.length == 7 }.orEmpty()) }
    Row(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(MaterialTheme.colorScheme.surfaceContainerLowest)
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text("Hex", style = MaterialTheme.typography.bodyMedium, color = muted)
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
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Characters, autoCorrectEnabled = false),
            textStyle = MaterialTheme.typography.bodyLarge.copy(color = onSurface, fontFeatureSettings = "tnum"),
            cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
            decorationBox = { inner ->
                Box(contentAlignment = Alignment.CenterStart) {
                    if (text.isEmpty() || text == "#") {
                        Text("#RRGGBB", style = MaterialTheme.typography.bodyLarge, color = muted)
                    }
                    inner()
                }
            },
            modifier = Modifier.weight(1f)
        )
        Text("Faint shades are skipped", style = MaterialTheme.typography.bodySmall, color = muted, modifier = Modifier.widthIn(max = 150.dp))
    }
}

private fun parseHex(hex: String): Color? =
    runCatching { Color(android.graphics.Color.parseColor(hex)) }.getOrNull()

/** Black or white, whichever reads on [color]. */
private fun contentOn(color: Color?): Color =
    if (color == null || color.luminance() > 0.4f) Color.Black else Color.White

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
