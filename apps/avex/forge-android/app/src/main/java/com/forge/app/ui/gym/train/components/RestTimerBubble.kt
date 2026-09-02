package com.forge.app.ui.gym.train.components

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import com.forge.app.domain.timer.RestTimerState
import com.forge.app.ui.theme.ForgeMotion

/**
 * Floating timer circle. Tap = open controls, long-press = +30s quick-extend (#96).
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun RestTimerBubble(
    state: RestTimerState,
    onOpenControls: () -> Unit,
    onLongClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val accent = MaterialTheme.colorScheme.primary
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val surface = MaterialTheme.colorScheme.surface
    val surfaceVar = MaterialTheme.colorScheme.surfaceVariant
    val isPaused = state.isPaused && !state.isFinished
    val fraction = if (state.totalSeconds > 0)
        (state.secondsRemaining.toFloat() / state.totalSeconds).coerceIn(0f, 1f) else 0f
    // Spoken label: the bubble otherwise reads only its raw "1:30" to TalkBack, with no context.
    val restA11y = if (state.isFinished) "Rest complete" else {
        val mins = state.secondsRemaining / 60
        val rem = state.secondsRemaining % 60
        val time = if (mins > 0) "$mins min ${rem}s" else "${rem}s"
        if (isPaused) "Rest timer paused, $time left" else "Rest timer, $time left"
    }
    // Deplete the ring smoothly between the per-second ticks instead of stepping each second.
    val animFraction by animateFloatAsState(
        targetValue = fraction,
        animationSpec = tween(durationMillis = 1000, easing = LinearEasing),
        label = "ring-fraction"
    )

    // Final-10s urgency: the ring warms toward the error colour over the last ten seconds, and the
    // bubble flashes once exactly at 0:10. Both collapse under reduced motion (ForgeMotion). The
    // haptic for that moment, like the one for the finish, is DayScreen's alone: it is the owner
    // that reads the Feedback strength setting, so Off plays nothing and nothing plays twice.
    val warn = MaterialTheme.colorScheme.error
    val urgency by animateFloatAsState(
        targetValue = if (!state.isFinished && state.secondsRemaining in 1..10) 1f else 0f,
        animationSpec = ForgeMotion.standardTween(ForgeMotion.DurationEmphasized),
        label = "ring-urgency"
    )
    // The depleting arc is now the hero, so it carries the accent (not a muted on-background) and
    // still warms toward the error colour over the final ten seconds.
    val ringColor = lerp(accent, warn, urgency)
    val flash = remember { Animatable(0f) }
    val haptic = LocalHapticFeedback.current
    // Track the previous tick so the 10s flash fires ONLY on the descending transition into 0:10,
    // never on first composition (a rest that simply starts at ≤10s, or a recompose while at 10).
    var prevSeconds by remember { mutableStateOf<Int?>(null) }
    LaunchedEffect(state.secondsRemaining) {
        // Clear any flash stranded by a prior run that was cancelled mid-fade (e.g. the user added
        // time within the 500ms window) — otherwise flash.value stays > 0 and draws indefinitely.
        flash.snapTo(0f)
        val prev = prevSeconds
        prevSeconds = state.secondsRemaining
        if (prev != null && prev > 10 && state.secondsRemaining == 10 &&
            !state.isFinished && ForgeMotion.durationScale > 0f
        ) {
            flash.snapTo(0.55f)
            flash.animateTo(0f, ForgeMotion.standardTween(500))
        }
    }

    // Springy pop-in each time the bubble appears (a rest begins).
    val appear = remember { Animatable(0f) }
    LaunchedEffect(Unit) {
        appear.animateTo(1f, animationSpec = ForgeMotion.bouncy<Float>())
    }
    // When the rest is up, the bubble breathes so the "ready" state pulls the eye. Instantiated
    // INSIDE the finished branch (P-12): the transition ran whatever the timer was doing, so a
    // running — or indefinitely paused — bubble advanced an animation every frame whose value was
    // multiplied by nothing. A paused bubble is now quiescent.
    val scale = appear.value * if (state.isFinished) finishedPulseScale() else 1f

    Box(
        modifier = modifier
            .graphicsLayer { scaleX = scale; scaleY = scale }
            .size(64.dp)
            // Soft cast shadow so the bubble reads as a physical object floating over the content
            // rather than a flat sticker. clip = false keeps the shadow outside the disc bounds.
            .shadow(elevation = 12.dp, shape = CircleShape, clip = false)
            .clip(CircleShape)
            // Dark "instrument" face: a subtle top-lit gradient gives the disc depth instead of a
            // single flat colour. The accent now lives in the ring, not the whole fill.
            .background(Brush.verticalGradient(listOf(surfaceVar, surface)))
            // Hairline rim to separate the disc from the background gradient at any accent.
            .border(width = 1.dp, color = onBg.copy(alpha = 0.08f), shape = CircleShape)
            // Countdown ring: a depleting accent arc on a faint track — the hero of the design.
            .drawBehind {
                val sw = 4.dp.toPx()
                val inset = Offset(sw / 2f + 1.dp.toPx(), sw / 2f + 1.dp.toPx())
                val arcSize = Size(size.width - sw - 2.dp.toPx(), size.height - sw - 2.dp.toPx())
                // Faint full-circle track so remaining rest reads as a shrinking arc on a track
                // rather than an arc appearing/vanishing.
                drawArc(
                    color = onBg.copy(alpha = 0.10f),
                    startAngle = -90f,
                    sweepAngle = 360f,
                    useCenter = false,
                    topLeft = inset,
                    size = arcSize,
                    style = Stroke(width = sw, cap = StrokeCap.Round)
                )
                when {
                    // Completed: a full accent ring crowns the ✓.
                    state.isFinished -> drawArc(
                        color = accent,
                        startAngle = -90f,
                        sweepAngle = 360f,
                        useCenter = false,
                        topLeft = inset,
                        size = arcSize,
                        style = Stroke(width = sw, cap = StrokeCap.Round)
                    )
                    // Paused: the remaining arc goes dashed and dim so the held state reads instantly.
                    isPaused -> drawArc(
                        color = accent.copy(alpha = 0.45f),
                        startAngle = -90f,
                        sweepAngle = 360f * animFraction,
                        useCenter = false,
                        topLeft = inset,
                        size = arcSize,
                        style = Stroke(
                            width = sw,
                            cap = StrokeCap.Round,
                            pathEffect = PathEffect.dashPathEffect(floatArrayOf(6f, 8f), 0f)
                        )
                    )
                    else -> drawArc(
                        color = ringColor,
                        startAngle = -90f,
                        sweepAngle = 360f * animFraction,
                        useCenter = false,
                        topLeft = inset,
                        size = arcSize,
                        style = Stroke(width = sw, cap = StrokeCap.Round)
                    )
                }
                // One-shot brighten at the 10-second mark — the visual half of the warning. Softer
                // now that it lands on a dark face rather than a saturated fill.
                if (flash.value > 0f) drawCircle(color = onBg.copy(alpha = flash.value * 0.4f))
            }
            .combinedClickable(
                onClick = {
                    haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                    onOpenControls()
                },
                onLongClick = {
                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                    onLongClick()
                }
            )
            // Merge so TalkBack reads the spoken state ("Rest timer, 1 min 30s left") in place of "1:30".
            .semantics(mergeDescendants = true) { contentDescription = restA11y },
        contentAlignment = Alignment.Center
    ) {
        if (state.isFinished) {
            Text(
                "✓",
                style = MaterialTheme.typography.titleLarge,
                color = accent,
                fontWeight = FontWeight.Bold
            )
        } else {
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                Text(
                    formatTime(state.secondsRemaining),
                    // Tabular figures keep the digits from jittering as the seconds count down.
                    style = MaterialTheme.typography.titleMedium.copy(fontFeatureSettings = "tnum"),
                    color = if (!isPaused) onBg else onBg.copy(alpha = 0.6f),
                    fontWeight = FontWeight.SemiBold
                )
                Text(
                    if (isPaused) "PAUSED" else "REST",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted,
                    fontSize = 8.sp,
                    letterSpacing = 1.5.sp,
                    fontWeight = FontWeight.Medium
                )
            }
        }
    }
}

@Composable
fun RestTimerControlsDialog(
    state: RestTimerState,
    onPause: () -> Unit,
    onResume: () -> Unit,
    onReset: () -> Unit,
    onSkip: () -> Unit,
    onAddSeconds: (Int) -> Unit,
    onDismiss: () -> Unit,
    /** How this duration was derived (adaptation engine) — every prescription is explainable. */
    reason: String? = null
) {
    Dialog(onDismissRequest = onDismiss) {
        val onBg = MaterialTheme.colorScheme.onBackground
        val muted = MaterialTheme.colorScheme.onSurfaceVariant
        val outline = MaterialTheme.colorScheme.outline
        val bg = MaterialTheme.colorScheme.background

        Column(
            modifier = Modifier
                .fillMaxWidth()
                .background(bg, RoundedCornerShape(8.dp))
                .padding(horizontal = 24.dp, vertical = 20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Text(
                "REST TIMER",
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                letterSpacing = 1.5.sp
            )

            Row(verticalAlignment = Alignment.Bottom) {
                Text(
                    formatTime(state.secondsRemaining),
                    style = MaterialTheme.typography.displayLarge,
                    color = onBg
                )
                val suffix = when {
                    state.isFinished -> " done."
                    state.isPaused  -> " paused."
                    else            -> " of ${formatTime(state.totalSeconds)}"
                }
                Text(
                    suffix,
                    style = MaterialTheme.typography.headlineSmall,
                    color = muted,
                    fontStyle = FontStyle.Italic,
                    modifier = Modifier.padding(start = 4.dp, bottom = 6.dp)
                )
            }

            if (reason != null) {
                Text(
                    reason,
                    style = MaterialTheme.typography.labelSmall,
                    color = muted.copy(alpha = 0.7f),
                    fontStyle = FontStyle.Italic
                )
            }

            HorizontalDivider(color = outline.copy(alpha = 0.2f))

            // −30s trims an over-long rest; the rest add time. addSeconds clamps at 0, so −30s
            // with under 30s left simply ends the rest.
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf(-30 to "−30s", 60 to "+1 min", 120 to "+2 min", 300 to "+5 min").forEach { (s, label) ->
                    Box(
                        modifier = Modifier
                            .border(1.dp, outline.copy(alpha = 0.4f), RoundedCornerShape(4.dp))
                            .clickable { onAddSeconds(s) }
                            .padding(horizontal = 10.dp, vertical = 6.dp),
                        contentAlignment = Alignment.Center
                    ) {
                        Text(
                            label,
                            style = MaterialTheme.typography.labelSmall,
                            color = muted,
                            letterSpacing = 0.5.sp
                        )
                    }
                }
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(20.dp, Alignment.End),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    "reset",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted.copy(alpha = 0.6f),
                    modifier = Modifier.clickable(onClick = onReset).padding(4.dp)
                )
                Text(
                    "done",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted.copy(alpha = 0.6f),
                    modifier = Modifier.clickable(onClick = onSkip).padding(4.dp)
                )
                Text(
                    if (state.isPaused) "resume →" else "pause →",
                    style = MaterialTheme.typography.bodyMedium,
                    color = onBg,
                    modifier = Modifier
                        .clickable { if (state.isPaused) onResume() else onPause() }
                        .padding(4.dp)
                )
            }
        }
    }
}

private fun formatTime(totalSeconds: Int): String {
    val s = totalSeconds.coerceAtLeast(0)
    return "%d:%02d".format(s / 60, s % 60)
}

/** The finished bubble's breathing scale. Its own composable so the transition exists only while
 *  the rest is actually up (P-12). */
@Composable
private fun finishedPulseScale(): Float {
    val pulse = rememberInfiniteTransition(label = "rest-finished-pulse")
    val pulseScale by pulse.animateFloat(
        initialValue = 1f,
        targetValue = 1.07f,
        animationSpec = infiniteRepeatable(tween(650), RepeatMode.Reverse),
        label = "pulse-scale"
    )
    return pulseScale
}
