package com.forge.app.ui.common

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import com.forge.app.appicon.AppIcon
import com.forge.app.appicon.IconFamily
import com.forge.app.ui.launch.LaunchMascot
import com.forge.app.ui.launch.MascotChoreography
import com.forge.app.ui.launch.MascotStage
import com.forge.app.ui.launch.WordFrame
import com.forge.app.ui.launch.rememberMascotWordDrop
import com.forge.app.ui.launch.rollLaunchMascot
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.forgeBackgroundGradient
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

/**
 * One-shot launch wordmark. The serif app name settles in on the brand gradient, holds a beat, then
 * the whole plate fades to reveal the first screen composed beneath it. Shown once per cold launch
 * (MainActivity gates it). Honors the system "Remove animations" setting: no motion, just a short
 * still hold before it hands off. A tap anywhere skips straight to the fade.
 *
 * The launch is themed to the user's chosen app icon ([iconKey]) through the WORDMARK ITSELF — each
 * family gives the name one subtle verb in the icon's palette ([AvexWordmark]). The full-screen
 * scenes (LaunchScenes.kt) are deliberately UNWIRED, kept for re-wiring: compose
 * `IconLaunchScene(icon, reduceMotion, Modifier.fillMaxSize())` behind the wordmark and restore the
 * longer (950ms) themed hold.
 *
 * Every launch a MASCOT joins (`ui/launch`), a random one of the four: it arrives before the name,
 * acts on it in the family's style (runs the Solid wipe, hammers the Molten name, presses the Gym
 * name overhead…) and the hold stretches to let it finish. [mascot] forces one (the debug intro lab);
 * by default the launch rolls for one. Never under reduce-motion, never when the themed intro is off.
 *
 * [themed] is the user's "Custom startup animation" setting (Appearance). Off resolves the intro to
 * [AppIcon.Default] — no palette — so the shared plain path in [AvexWordmark] plays the black-and-
 * white Avex settle instead of the icon-family effect (and no choreographed family exit fires).
 */
@Composable
fun AvexIntro(
    iconKey: String,
    themed: Boolean = true,
    mascot: LaunchMascot? = null,
    rollMascot: Boolean = true,
    onDone: () -> Unit,
) {
    val (gradTop, gradBottom) = forgeBackgroundGradient(LocalForgeSettings.current.amoledMode)
    val reduceMotion = ForgeMotion.durationScale <= 0f
    val icon = if (themed) AppIcon.fromKey(iconKey) else AppIcon.Default
    val cast = remember {
        when {
            reduceMotion || !themed -> null
            mascot != null -> mascot
            rollMascot -> rollLaunchMascot()
            else -> null
        }
    }
    // The mascot beat needs its lead-in and a longer hold; without one, nothing changes.
    val leadSeconds = if (cast != null) MascotChoreography.leadFor(icon.family) else 0f
    val lead = (leadSeconds * 1000).toInt()
    val mascotHold = if (cast != null) MASCOT_EXTRA_HOLD else 0
    // Its reaction to the death (Molten's tears, Nebula's spiral) needs the plate to stay up longer.
    val mascotLinger = if (cast != null) MASCOT_EXIT_LINGER else 0

    // Text settles: alpha 0→1 with a hair of upward scale. Plate: the whole overlay fades out to
    // reveal. Exit: families with a choreographed wordmark death (melt, black hole, CRT-off, wipe)
    // play it over a shortened hold so the total stays in the stock envelope.
    val reveal = remember { Animatable(if (reduceMotion) 1f else 0f) }
    val plateAlpha = remember { Animatable(1f) }
    val exit = remember { Animatable(0f) }
    var skipped by remember { mutableStateOf(false) }
    // Seconds since the intro began, on the animator scale: the mascot's clock.
    var clock by remember { mutableFloatStateOf(0f) }
    var word by remember { mutableStateOf<WordFrame?>(null) }
    var exitStart by remember { mutableStateOf<Float?>(null) }

    if (cast != null) {
        LaunchedEffect(Unit) {
            val start = withFrameNanos { it }
            val scale = ForgeMotion.durationScale.coerceAtLeast(0.05f)
            while (true) withFrameNanos { clock = (it - start) / 1_000_000_000f / scale }
        }
    }

    LaunchedEffect(Unit) {
        val intro = this
        val sequence = launch {
            if (lead > 0) delay(ForgeMotion.scaledDuration(lead).toLong())
            if (!reduceMotion) {
                reveal.animateTo(1f, ForgeMotion.enterTween(ForgeMotion.DurationEmphasized))
            }
            val choreographedExit = !reduceMotion && wordmarkExitChoreographed(icon)
            // Holds are raw delays Compose does not clock, so they take the animator scale here to stay
            // in step with the tweens (which Compose scales itself); the reduced-motion hold is a fixed
            // still beat, not motion, and is left alone.
            delay(
                if (reduceMotion) 450L
                else ForgeMotion.scaledDuration((if (choreographedExit) 570 else 700) + mascotHold).toLong()
            )
            if (choreographedExit) {
                // The death starts first; the plate fade joins in later so the destruction reads before
                // everything dims together. Molten melts DECELERATING (material gives way fast, then
                // drips slowly) and gets a longer window; the others accelerate away. Launched on the
                // intro's scope so the fade below does not wait for it.
                val melt = icon.family == IconFamily.Molten
                exitStart = clock
                intro.launch {
                    exit.animateTo(
                        1f,
                        tween(
                            ForgeMotion.nominalDuration(if (melt) 650 else 480),
                            easing = if (melt) ForgeMotion.Decelerate else ForgeMotion.Accelerate
                        )
                    )
                }
                delay(ForgeMotion.scaledDuration((if (melt) 320 else 230) + mascotLinger).toLong())
            }
        }
        val skipper = launch {
            snapshotFlow { skipped }.first { it }
            sequence.cancel()
        }
        sequence.join()
        skipper.cancel()
        if (!reduceMotion) {
            plateAlpha.animateTo(0f, tween(ForgeMotion.nominalDuration(ForgeMotion.DurationStandard), easing = ForgeMotion.Accelerate))
        }
        onDone()
    }

    val clockRead = { clock }
    val onBg = MaterialTheme.colorScheme.onBackground
    val palette = remember(icon, onBg) { icon.launchPalette?.map { Color(it) } ?: listOf(onBg, onBg, onBg) }
    val drop = rememberMascotWordDrop(cast, icon.family, clockRead)
    Box(
        modifier = Modifier
            .fillMaxSize()
            .graphicsLayer { alpha = plateAlpha.value }
            .background(Brush.verticalGradient(listOf(gradTop, gradBottom)))
            // The plate covers the first screen, so it takes the taps: one skips to the reveal
            // instead of landing on whatever is underneath.
            .pointerInput(Unit) { detectTapGestures { skipped = true } },
        contentAlignment = Alignment.Center
    ) {
        // Pass the animation values as deferred reads so this composable (and the plain wordmark path)
        // don't recompose every frame — AvexWordmark reads them in its draw/graphicsLayer phase.
        AvexWordmark(
            icon,
            reveal = { reveal.value },
            exit = { exit.value },
            reduceMotion = reduceMotion,
            clockDelay = leadSeconds,
            drop = drop,
            onFrame = if (cast != null) { frame -> word = frame } else null,
        )
        if (cast != null) {
            MascotStage(
                mascot = cast,
                family = icon.family,
                palette = palette,
                clock = clockRead,
                exit = { exit.value },
                exitStart = { exitStart },
                word = { word },
                modifier = Modifier.fillMaxSize(),
            )
        }
    }
}

/** Extra hold (ms) when a mascot is on stage, so its routine finishes before the name leaves. */
private const val MASCOT_EXTRA_HOLD = 450

/** Extra time (ms) the plate stays up through a choreographed death while a mascot reacts to it. */
private const val MASCOT_EXIT_LINGER = 380
