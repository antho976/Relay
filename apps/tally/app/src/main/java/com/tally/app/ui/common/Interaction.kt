package com.tally.app.ui.common

import android.content.Context
import android.view.accessibility.AccessibilityManager
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
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
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
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

/**
 * Press = a 0.97 bounce, no ripple, the app's one press language. Under TalkBack a double tap
 * never shows the press, so the ripple comes back there.
 */
fun Modifier.bounceClick(
    enabled: Boolean = true,
    label: String? = null,
    role: Role? = null,
    onClick: () -> Unit,
): Modifier = composed {
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val scale by animateFloatAsState(
        targetValue = if (pressed && enabled && !TallyMotion.animationsOff) 0.97f else 1f,
        animationSpec = spring(dampingRatio = Spring.DampingRatioMediumBouncy, stiffness = Spring.StiffnessMediumLow),
        label = "bounce",
    )
    val talkBack = LocalTouchExploration.current
    Modifier
        .graphicsLayer { scaleX = scale; scaleY = scale }
        .clickable(
            interactionSource = source,
            indication = if (talkBack) ripple() else null,
            enabled = enabled,
            onClickLabel = label,
            role = role,
            onClick = onClick,
        )
}
