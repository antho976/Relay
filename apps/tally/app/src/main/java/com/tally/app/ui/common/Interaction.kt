package com.tally.app.ui.common

import android.content.Context
import android.view.accessibility.AccessibilityManager
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ripple
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawOutline
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.tally.app.ui.theme.TallyMotion

/** True while TalkBack (touch exploration) is on, from ONE app-level listener. */
val LocalTouchExploration = staticCompositionLocalOf { false }

@Composable
fun ProvideTouchExploration(content: @Composable () -> Unit) {
    val context = LocalContext.current
    var enabled by remember { mutableStateOf(false) }
    DisposableEffect(context) {
        val am = context.getSystemService(Context.ACCESSIBILITY_SERVICE) as? AccessibilityManager
        enabled = am?.isTouchExplorationEnabled == true
        val listener = AccessibilityManager.TouchExplorationStateChangeListener { enabled = it }
        am?.addTouchExplorationStateChangeListener(listener)
        // Removed on dispose: a listener left on the system service holds the Activity forever.
        onDispose { am?.removeTouchExplorationStateChangeListener(listener) }
    }
    CompositionLocalProvider(LocalTouchExploration provides enabled, content = content)
}

/** The focus ring's outline when a target does not pass its own shape: a row or a bare label. */
val FOCUS_SHAPE: Shape = RoundedCornerShape(12.dp)

/**
 * How far a bare row's focus ring stands outside it. A row inside a panel starts with its badge
 * at its very edge, so a ring drawn inside would run across the badge; the panel's padding has
 * room for it.
 */
val ROW_FOCUS_OUTSET = 4.dp

private val FOCUS_RING = 2.dp

/**
 * Press = a 0.97 bounce, no ripple, the app's one press language. Under TalkBack a double tap
 * never shows the press, so the ripple comes back there.
 *
 * Focus from a keyboard, a D-pad or a Chromebook draws a 2dp ring in [focusColor] (the accent
 * when unspecified) along [focusShape]. Touch never moves focus onto a clickable, so a finger never
 * sees the ring. With [focusOutset] 0 the ring sits just inside the target, so it survives a clip
 * placed before this modifier (pass the clip's shape); a positive outset moves it outside, for a
 * bare row ([ROW_FOCUS_OUTSET]).
 */
fun Modifier.bounceClick(
    enabled: Boolean = true,
    label: String? = null,
    role: Role? = null,
    focusShape: Shape = FOCUS_SHAPE,
    focusColor: Color = Color.Unspecified,
    focusOutset: Dp = 0.dp,
    onClick: () -> Unit,
): Modifier = composed {
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val focused by source.collectIsFocusedAsState()
    val scale by animateFloatAsState(
        targetValue = if (pressed && enabled && !TallyMotion.animationsOff) 0.97f else 1f,
        animationSpec = spring(dampingRatio = Spring.DampingRatioMediumBouncy, stiffness = Spring.StiffnessMediumLow),
        label = "bounce",
    )
    val talkBack = LocalTouchExploration.current
    val accent = MaterialTheme.colorScheme.primary
    val ring = if (focusColor != Color.Unspecified) focusColor else accent
    Modifier
        .graphicsLayer { scaleX = scale; scaleY = scale }
        .drawWithContent {
            drawContent()
            if (focused) drawFocusRing(focusShape, ring, focusOutset)
        }
        .clickable(
            interactionSource = source,
            indication = if (talkBack) ripple() else null,
            enabled = enabled,
            onClickLabel = label,
            role = role,
            onClick = onClick,
        )
}

/** A [FOCUS_RING] stroke along [shape], its outer edge [outset] outside the bounds (0: on them). */
private fun DrawScope.drawFocusRing(shape: Shape, color: Color, outset: Dp) {
    val stroke = FOCUS_RING.toPx()
    // The path runs down the middle of the stroke.
    val edge = stroke / 2f - outset.toPx()
    val width = size.width - edge * 2f
    val height = size.height - edge * 2f
    if (width <= 0f || height <= 0f) return
    val outline = shape.createOutline(Size(width, height), layoutDirection, this)
    translate(left = edge, top = edge) {
        drawOutline(outline, color, style = Stroke(width = stroke))
    }
}
