package com.forge.app.ui.profile

import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.keyframes
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import com.forge.app.program.TrophyIcon
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.trophies.components.TrophyIconBadge
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin

/**
 * A [TrophyIconBadge] whose icon physically moves while [active] (the inspect popup is open): each
 * icon plays a transform-based motion that fits what it depicts — the flame sways and licks, the
 * recycle icon spins, the dumbbell lifts, lightning shakes, the heart beats, sparkles fly. No
 * brightness flicker — real movement. The motion is applied to the icon (via the badge's
 * iconModifier) so it dances within the static badge circle. Static when inactive / reduced-motion.
 */
@Composable
internal fun AnimatedTrophyBadge(
    icon: TrophyIcon,
    unlocked: Boolean,
    active: Boolean,
    size: Dp,
    accent: Color,
    modifier: Modifier = Modifier
) {
    if (!active || ForgeMotion.durationScale <= 0f) {
        TrophyIconBadge(icon = icon, unlocked = unlocked, modifier = modifier, size = size)
        return
    }
    val t = rememberInfiniteTransition(label = "trophy-icon-motion")
    when (icon.motion()) {
        Motion.SPIN -> {
            val a by t.animateFloat(0f, 360f, infiniteRepeatable(tween(1400, easing = LinearEasing)), label = "spin")
            TrophyIconBadge(icon = icon, unlocked = unlocked, modifier = modifier, size = size,
                iconModifier = Modifier.graphicsLayer { rotationZ = a })
        }
        Motion.FLAME -> {
            // The tip sways around the base (two speeds = organic) and licks upward.
            val s1 by t.animateFloat(-1f, 1f, infiniteRepeatable(tween(560, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "sway-1")
            val s2 by t.animateFloat(-1f, 1f, infiniteRepeatable(tween(340, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "sway-2")
            val lick by t.animateFloat(0f, 1f, infiniteRepeatable(tween(460, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "lick")
            TrophyIconBadge(icon = icon, unlocked = unlocked, modifier = modifier, size = size,
                iconModifier = Modifier.graphicsLayer {
                    transformOrigin = TransformOrigin(0.5f, 1f)
                    rotationZ = s1 * 8f + s2 * 4f
                    scaleX = 1f - 0.05f * lick
                    scaleY = 1f + 0.14f * lick
                })
        }
        Motion.TURN -> {
            val rot by t.animateFloat(0f, 360f, infiniteRepeatable(tween(3800, easing = LinearEasing)), label = "turn")
            val tw by t.animateFloat(0f, 1f, infiniteRepeatable(tween(820, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "twinkle")
            TrophyIconBadge(icon = icon, unlocked = unlocked, modifier = modifier, size = size,
                iconModifier = Modifier.graphicsLayer { rotationZ = rot; scaleX = 0.94f + 0.10f * tw; scaleY = 0.94f + 0.10f * tw })
        }
        Motion.HEARTBEAT -> {
            val s by t.animateFloat(
                1f, 1f,
                infiniteRepeatable(keyframes {
                    durationMillis = 900
                    1f at 0; 1.20f at 120; 1f at 260; 1.12f at 380; 1f at 560
                }),
                label = "beat"
            )
            TrophyIconBadge(icon = icon, unlocked = unlocked, modifier = modifier, size = size,
                iconModifier = Modifier.graphicsLayer { scaleX = s; scaleY = s })
        }
        Motion.SLIDE -> {
            val n by t.animateFloat(-1f, 1f, infiniteRepeatable(tween(560, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "slide")
            val dx = with(LocalDensity.current) { (size * 0.18f).toPx() }
            TrophyIconBadge(icon = icon, unlocked = unlocked, modifier = modifier, size = size,
                iconModifier = Modifier.graphicsLayer { translationX = n * dx })
        }
        Motion.BOB -> {
            val b by t.animateFloat(-1f, 1f, infiniteRepeatable(tween(680, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "bob")
            val dy = with(LocalDensity.current) { (size * 0.14f).toPx() }
            TrophyIconBadge(icon = icon, unlocked = unlocked, modifier = modifier, size = size,
                iconModifier = Modifier.graphicsLayer { translationY = b * dy })
        }
        Motion.TILT -> {
            val s1 by t.animateFloat(-1f, 1f, infiniteRepeatable(tween(720, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "tilt")
            val pl by t.animateFloat(0f, 1f, infiniteRepeatable(tween(900, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "tilt-pulse")
            TrophyIconBadge(icon = icon, unlocked = unlocked, modifier = modifier, size = size,
                iconModifier = Modifier.graphicsLayer { rotationZ = s1 * 9f; scaleX = 0.97f + 0.06f * pl; scaleY = 0.97f + 0.06f * pl })
        }
        Motion.PAGE -> {
            val r by t.animateFloat(-1f, 1f, infiniteRepeatable(tween(900, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "page")
            TrophyIconBadge(icon = icon, unlocked = unlocked, modifier = modifier, size = size,
                iconModifier = Modifier.graphicsLayer { rotationX = r * 18f })
        }
        Motion.SHAKE -> {
            // Fast, buzzy jitter — a lightning strike.
            val r by t.animateFloat(-1f, 1f, infiniteRepeatable(tween(110, easing = LinearEasing), RepeatMode.Reverse), label = "shake")
            val dx = with(LocalDensity.current) { (size * 0.05f).toPx() }
            TrophyIconBadge(icon = icon, unlocked = unlocked, modifier = modifier, size = size,
                iconModifier = Modifier.graphicsLayer { rotationZ = r * 7f; translationX = r * dx })
        }
        Motion.POP -> {
            val p by t.animateFloat(0f, 1f, infiniteRepeatable(tween(560, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "pop")
            TrophyIconBadge(icon = icon, unlocked = unlocked, modifier = modifier, size = size,
                iconModifier = Modifier.graphicsLayer { scaleX = 1f + 0.12f * p; scaleY = 1f + 0.12f * p; rotationZ = (p - 0.5f) * 8f })
        }
        Motion.SPARKLE -> {
            // The icon rocks + pulses while sparkle particles fly outward and fade.
            val p by t.animateFloat(0f, 1f, infiniteRepeatable(tween(1500, easing = LinearEasing)), label = "sparkle")
            val tw by t.animateFloat(0f, 1f, infiniteRepeatable(tween(620, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "twinkle")
            Box(modifier.size(size), contentAlignment = Alignment.Center) {
                TrophyIconBadge(icon = icon, unlocked = unlocked, size = size,
                    iconModifier = Modifier.graphicsLayer {
                        rotationZ = 16f * (tw * 2f - 1f)
                        scaleX = 0.9f + 0.16f * tw
                        scaleY = 0.9f + 0.16f * tw
                    })
                Canvas(Modifier.matchParentSize()) { drawSparkles(p, accent) }
            }
        }
    }
}

/** Sparkle particles emitting outward from the centre and fading — the "spark" flourish. */
private fun DrawScope.drawSparkles(progress: Float, color: Color, count: Int = 5) {
    val cx = size.width / 2f
    val cy = size.height / 2f
    val maxR = size.minDimension / 2f
    for (i in 0 until count) {
        val phase = (progress + i.toFloat() / count) % 1f
        val angle = (i * (360f / count) + progress * 50f) * (PI.toFloat() / 180f)
        val r = maxR * (0.18f + 0.82f * phase)
        val alpha = (1f - phase).coerceIn(0f, 1f)
        val dot = size.minDimension * 0.065f * (1f - phase * 0.6f)
        drawCircle(
            color = color.copy(alpha = alpha * 0.9f),
            radius = dot,
            center = Offset(cx + cos(angle) * r, cy + sin(angle) * r)
        )
    }
}

private enum class Motion { SPIN, FLAME, TURN, HEARTBEAT, SLIDE, BOB, TILT, PAGE, SHAKE, POP, SPARKLE }

/** Maps each trophy icon to the movement that best evokes what it depicts. */
private fun TrophyIcon.motion(): Motion = when (this) {
    TrophyIcon.REPEAT, TrophyIcon.CLOCK -> Motion.SPIN          // spins / ticks round
    TrophyIcon.FLAME -> Motion.FLAME                            // sways and licks
    TrophyIcon.STAR -> Motion.TURN                              // turns + twinkles
    TrophyIcon.SPARK -> Motion.SPARKLE                          // emits sparkle particles
    TrophyIcon.HEART -> Motion.HEARTBEAT                        // double-thump
    TrophyIcon.SWAP, TrophyIcon.DOOR -> Motion.SLIDE            // slides side to side
    TrophyIcon.STACK, TrophyIcon.DUMBBELL -> Motion.BOB         // lifts / settles up-down
    TrophyIcon.CROWN -> Motion.TILT                             // regal sway
    TrophyIcon.CALENDAR -> Motion.PAGE                          // page flip
    TrophyIcon.BOLT -> Motion.SHAKE                             // electric jitter
    TrophyIcon.CHECK, TrophyIcon.FOUR -> Motion.POP             // pops
}
