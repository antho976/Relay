package com.forge.app.ui.common

import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.material3.ripple
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.graphics.graphicsLayer

/**
 * Clickable variant that scales the target down slightly while pressed. Use anywhere
 * a card / tile would otherwise just take `.clickable(onClick = ...)` — the press
 * feedback alone reads as "this is tappable" without needing any other affordance.
 *
 * Spring is intentionally fast + lightly damped: subtle bounce, not a wobble.
 */
fun Modifier.bounceClick(
    pressedScale: Float = 0.97f,
    enabled: Boolean = true,
    onClick: () -> Unit
): Modifier = composed {
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()
    val scale by animateFloatAsState(
        targetValue = if (isPressed && enabled) pressedScale else 1f,
        animationSpec = spring(
            dampingRatio = Spring.DampingRatioMediumBouncy,
            stiffness = Spring.StiffnessMediumLow
        ),
        label = "bounce-scale"
    )
    // The press-scale is invisible to a screen reader (a TalkBack double-tap never enters the pressed
    // state visibly), so with touch exploration on a tap reads as "nothing happened". Fall back to a
    // material ripple in that case; otherwise keep the clean, indication-free scale for sighted users.
    // Sourced from one app-level observer ([ProvideTouchExploration]) so this is live without every
    // bounceClick element registering its own AccessibilityManager listener.
    val touchExploration = LocalTouchExplorationEnabled.current
    Modifier
        .graphicsLayer { scaleX = scale; scaleY = scale }
        .clickable(
            interactionSource = interactionSource,
            indication = if (touchExploration) ripple() else null,
            enabled = enabled,
            onClick = onClick
        )
}

/**
 * [bounceClick] with a long-press hook. Use when a single control carries a primary tap and a
 * secondary hold (e.g. Start session vs. hold-to-skip-warmup). Same bounce-no-ripple feel; the
 * click/long-click labels feed TalkBack so both actions are announced.
 */
@OptIn(ExperimentalFoundationApi::class)
fun Modifier.bounceCombinedClick(
    pressedScale: Float = 0.97f,
    enabled: Boolean = true,
    onClickLabel: String? = null,
    onLongClickLabel: String? = null,
    onLongClick: (() -> Unit)? = null,
    onClick: () -> Unit
): Modifier = composed {
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()
    val scale by animateFloatAsState(
        targetValue = if (isPressed && enabled) pressedScale else 1f,
        animationSpec = spring(
            dampingRatio = Spring.DampingRatioMediumBouncy,
            stiffness = Spring.StiffnessMediumLow
        ),
        label = "bounce-scale"
    )
    val touchExploration = LocalTouchExplorationEnabled.current
    Modifier
        .graphicsLayer { scaleX = scale; scaleY = scale }
        .combinedClickable(
            interactionSource = interactionSource,
            indication = if (touchExploration) ripple() else null,
            enabled = enabled,
            onClickLabel = onClickLabel,
            onLongClickLabel = onLongClickLabel,
            onLongClick = onLongClick,
            onClick = onClick
        )
}

