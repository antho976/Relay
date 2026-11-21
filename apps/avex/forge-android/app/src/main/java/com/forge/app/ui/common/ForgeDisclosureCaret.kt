package com.forge.app.ui.common

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.forge.app.ui.theme.ForgeMotion

/** A chevron pointing down, drawn at the custom families' 1.7f stroke weight. */
private val DisclosureChevron: ImageVector by lazy {
    icon("DisclosureChevron") {
        strokePath(1.7f) {
            moveTo(6.5f, 9.5f)
            lineTo(12f, 15f)
            lineTo(17.5f, 9.5f)
        }
    }
}

/**
 * The one expand/collapse caret: points down while closed (the content opens below it) and turns
 * to point up once open. It replaced the `▸ ▾ ▲` characters that stood in for it on Stats, session
 * detail and the train screen (§8: content glyphs are drawn, never unicode).
 *
 * Decorative on purpose: every caller sits inside a row whose click label already says "Expand" or
 * "Collapse", so a content description here would be read twice. The turn is a graphics-layer
 * rotation, so animating it redraws without recomposing or relaying out the row.
 */
@Composable
fun ForgeDisclosureCaret(
    expanded: Boolean,
    modifier: Modifier = Modifier,
    tint: Color = MaterialTheme.colorScheme.onSurfaceVariant,
    size: Dp = 16.dp
) {
    val rotation by animateFloatAsState(
        targetValue = if (expanded) 180f else 0f,
        animationSpec = ForgeMotion.standardTween(ForgeMotion.DurationFast),
        label = "disclosureCaret"
    )
    Icon(
        DisclosureChevron,
        contentDescription = null,
        tint = tint,
        modifier = modifier
            .size(size)
            .graphicsLayer { rotationZ = rotation }
    )
}
