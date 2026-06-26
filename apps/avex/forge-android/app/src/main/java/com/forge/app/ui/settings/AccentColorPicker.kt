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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
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
                ColorWheel(currentHex = currentHex, onPick = onSelect, onBg = onBg)
                BrightnessSlider(currentHex = currentHex, onPick = onSelect, muted = muted)
            }
        }
        Spacer(Modifier.height(4.dp))
    }
}

/**
 * Type any `#RRGGBB` hex for an accent colour outside the preset palette; applies live once valid.
 * The preview swatch doubles as the show/hide toggle for the colour wheel.
 */
@Composable
private fun CustomHexInput(
    currentHex: String,
    onSelect: (String) -> Unit,
    onBg: Color,
    muted: Color,
    wheelVisible: Boolean,
    onToggleWheel: () -> Unit
) {
    // Mirror whatever accent is active (preset, wheel pick, or hand-typed) so the field always shows the live hex.
    var text by remember(currentHex) {
        mutableStateOf(currentHex.takeIf { it.length == 7 }.orEmpty())
    }
    val valid = text.matches(Regex("#[0-9A-F]{6}"))
    val preview = if (valid) remember(text) { Color(android.graphics.Color.parseColor(text)) } else null
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
                if (text.matches(Regex("#[0-9A-F]{6}"))) onSelect(text)
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
                .background(preview ?: Color.Transparent)
                .border(
                    width = if (wheelVisible) 2.dp else 1.dp,
                    color = if (wheelVisible) onBg else muted.copy(alpha = 0.4f),
                    shape = CircleShape
                )
        )
    }
}

@Composable
private fun ColorWheel(currentHex: String, onPick: (String) -> Unit, onBg: Color) {
    val hsv = hexToHsv(currentHex.ifEmpty { DEFAULT_ACCENT })
    // Keep brightness fixed while picking hue/saturation; read latest via state to dodge stale closures.
    val valueState = rememberUpdatedState(hsv[2])
    val pickState = rememberUpdatedState(onPick)
    val ringColor = onBg
    Canvas(
        modifier = Modifier
            .size(176.dp)
            .pointerInput(Unit) {
                awaitEachGesture {
                    val down = awaitFirstDown()
                    emitHueSat(down.position, size, valueState.value, pickState.value)
                    down.consume()
                    drag(down.id) { change ->
                        emitHueSat(change.position, size, valueState.value, pickState.value)
                        change.consume()
                    }
                }
            }
    ) {
        val radius = size.minDimension / 2f
        val center = Offset(size.width / 2f, size.height / 2f)
        // Hue around the circle (clockwise from 3 o'clock).
        drawCircle(
            brush = Brush.sweepGradient(
                colors = listOf(
                    Color.Red, Color.Yellow, Color.Green, Color.Cyan,
                    Color.Blue, Color.Magenta, Color.Red
                ),
                center = center
            ),
            radius = radius,
            center = center
        )
        // Saturation: white at the centre fading to transparent at the rim.
        drawCircle(
            brush = Brush.radialGradient(
                colors = listOf(Color.White, Color.Transparent),
                center = center,
                radius = radius
            ),
            radius = radius,
            center = center
        )
        // Brightness: dim the whole wheel toward black as value drops.
        if (hsv[2] < 1f) {
            drawCircle(Color.Black.copy(alpha = 1f - hsv[2]), radius = radius, center = center)
        }
        // Selector indicator at the current hue/saturation.
        val angle = Math.toRadians(hsv[0].toDouble())
        val sel = Offset(
            x = center.x + (radius * hsv[1] * cos(angle)).toFloat(),
            y = center.y + (radius * hsv[1] * sin(angle)).toFloat()
        )
        drawCircle(Color.White, radius = 7.dp.toPx(), center = sel)
        drawCircle(ringColor.copy(alpha = 0.6f), radius = 7.dp.toPx(), center = sel, style = Stroke(width = 1.5.dp.toPx()))
    }
}

@Composable
private fun BrightnessSlider(currentHex: String, onPick: (String) -> Unit, muted: Color) {
    val hsv = hexToHsv(currentHex.ifEmpty { DEFAULT_ACCENT })
    val hsvState = rememberUpdatedState(hsv)
    val pickState = rememberUpdatedState(onPick)
    val fullColor = remember(hsv[0], hsv[1]) {
        Color(android.graphics.Color.HSVToColor(floatArrayOf(hsv[0], hsv[1], 1f)))
    }
    Canvas(
        modifier = Modifier
            .fillMaxWidth()
            .height(20.dp)
            .pointerInput(Unit) {
                awaitEachGesture {
                    val down = awaitFirstDown()
                    emitValue(down.position.x, size, hsvState.value, pickState.value)
                    down.consume()
                    drag(down.id) { change ->
                        emitValue(change.position.x, size, hsvState.value, pickState.value)
                        change.consume()
                    }
                }
            }
    ) {
        val r = size.height / 2f
        drawRoundRect(
            brush = Brush.horizontalGradient(listOf(Color.Black, fullColor)),
            cornerRadius = androidx.compose.ui.geometry.CornerRadius(r, r)
        )
        val thumbX = (hsv[2].coerceIn(0f, 1f) * size.width).coerceIn(r, size.width - r)
        val thumbCenter = Offset(thumbX, size.height / 2f)
        drawCircle(Color.White, radius = r, center = thumbCenter)
        drawCircle(muted.copy(alpha = 0.6f), radius = r, center = thumbCenter, style = Stroke(width = 1.5.dp.toPx()))
    }
}

// ─── HSV helpers ─────────────────────────────────────────────────────────────

/** Convert a wheel/slider touch to a hue+saturation pick (brightness preserved) and apply it. */
private fun emitHueSat(pos: Offset, size: IntSize, value: Float, onPick: (String) -> Unit) {
    val radius = min(size.width, size.height) / 2f
    if (radius <= 0f) return
    val dx = pos.x - size.width / 2f
    val dy = pos.y - size.height / 2f
    val sat = (hypot(dx.toDouble(), dy.toDouble()) / radius).coerceIn(0.0, 1.0).toFloat()
    var deg = Math.toDegrees(atan2(dy.toDouble(), dx.toDouble())).toFloat()
    if (deg < 0f) deg += 360f
    onPick(hsvToHex(deg, sat, value))
}

/** Convert a horizontal touch on the brightness bar to a value pick (hue/sat preserved) and apply it. */
private fun emitValue(x: Float, size: IntSize, hsv: FloatArray, onPick: (String) -> Unit) {
    if (size.width <= 0) return
    // Floor at 0.12 so the colour never collapses to pure black (which loses hue/saturation).
    val v = (x / size.width).coerceIn(0.12f, 1f)
    onPick(hsvToHex(hsv[0], hsv[1], v))
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
