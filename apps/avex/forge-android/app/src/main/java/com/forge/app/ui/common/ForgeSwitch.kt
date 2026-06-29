package com.forge.app.ui.common

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp

/**
 * Drop-in replacement for Material3 [androidx.compose.material3.Switch].
 *
 * M3's Switch animates the thumb with an internal, non-configurable spec that reads as
 * slow and slightly abrupt (the thumb resize "pops"). This rolls the same look with a
 * snappy, lightly-bouncy spring — quick to commit, smooth settle, a small satisfying
 * overshoot — consistent with the app's [bounceClick] motion language. The thumb also
 * stretches while pressed for tactile feedback.
 *
 * Colours default to the M3 equivalents; override per call site to match a styled toggle.
 */
@Composable
fun ForgeSwitch(
    checked: Boolean,
    onCheckedChange: ((Boolean) -> Unit)?,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    checkedTrackColor: Color = MaterialTheme.colorScheme.primary,
    checkedThumbColor: Color = MaterialTheme.colorScheme.onPrimary,
    checkedBorderColor: Color = Color.Transparent,
    uncheckedTrackColor: Color = MaterialTheme.colorScheme.surfaceVariant,
    uncheckedThumbColor: Color = MaterialTheme.colorScheme.outline,
    uncheckedBorderColor: Color = MaterialTheme.colorScheme.outline,
) {
    val trackWidth = 52.dp
    val trackHeight = 32.dp
    val inset = 5.dp
    val thumbUnchecked = 18.dp
    val thumbChecked = 22.dp
    val thumbPressed = 26.dp

    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()

    // Position: snappy commit + a tiny settle (lightly under-damped, no wobble).
    val fraction by animateFloatAsState(
        targetValue = if (checked) 1f else 0f,
        animationSpec = spring(dampingRatio = 0.68f, stiffness = 900f),
        label = "switch-pos"
    )
    // Size: crisp, no bounce — a wobbling thumb reads as cheap.
    val thumbSize by animateDpAsState(
        targetValue = when {
            pressed -> thumbPressed
            checked -> thumbChecked
            else -> thumbUnchecked
        },
        animationSpec = spring(dampingRatio = Spring.DampingRatioNoBouncy, stiffness = 1400f),
        label = "switch-thumb"
    )
    val trackColor by animateColorAsState(
        targetValue = if (checked) checkedTrackColor else uncheckedTrackColor,
        animationSpec = spring(stiffness = 700f),
        label = "switch-track"
    )
    val thumbColor by animateColorAsState(
        targetValue = if (checked) checkedThumbColor else uncheckedThumbColor,
        animationSpec = spring(stiffness = 700f),
        label = "switch-thumb-color"
    )
    val borderColor = if (checked) checkedBorderColor else uncheckedBorderColor

    val alpha = if (enabled) 1f else 0.38f

    Box(
        modifier = modifier
            .then(
                if (onCheckedChange != null) Modifier.toggleable(
                    value = checked,
                    interactionSource = interaction,
                    indication = null,
                    enabled = enabled,
                    role = Role.Switch,
                    onValueChange = onCheckedChange
                ) else Modifier
            )
            .size(trackWidth, trackHeight)
            .clip(CircleShape)
            .background(trackColor.copy(alpha = trackColor.alpha * alpha))
            .border(1.5.dp, borderColor.copy(alpha = borderColor.alpha * alpha), CircleShape),
        contentAlignment = Alignment.CenterStart
    ) {
        // Keep the thumb inside the track at every size by folding thumbSize into the travel.
        val thumbX = inset + (trackWidth - inset * 2 - thumbSize) * fraction
        Box(
            Modifier
                .offset(x = thumbX)
                .size(thumbSize)
                .clip(CircleShape)
                .background(thumbColor.copy(alpha = thumbColor.alpha * alpha))
        )
    }
}
