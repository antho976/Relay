package com.forge.app.ui.common

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.graphicsLayer
import com.forge.app.appicon.AppIcon
import com.forge.app.appicon.IconFamily
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.forgeBackgroundGradient
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * One-shot launch wordmark. The serif app name settles in on the brand gradient, holds a beat, then
 * the whole plate fades to reveal the first screen composed beneath it. Shown once per cold launch
 * (MainActivity gates it). Honors the system "Remove animations" setting: no motion, just a short
 * still hold before it hands off.
 *
 * The launch is themed to the user's chosen app icon ([iconKey]) through the WORDMARK ITSELF — each
 * family gives the name one subtle verb in the icon's palette ([AvexWordmark]). The full-screen
 * scenes (LaunchScenes.kt) are deliberately UNWIRED, kept for re-wiring: compose
 * `IconLaunchScene(icon, reduceMotion, Modifier.fillMaxSize())` behind the wordmark and restore the
 * longer (950ms) themed hold.
 *
 * [themed] is the user's "Custom startup animation" setting (Appearance). Off resolves the intro to
 * [AppIcon.Default] — no palette — so the shared plain path in [AvexWordmark] plays the black-and-
 * white Avex settle instead of the icon-family effect (and no choreographed family exit fires).
 */
@Composable
fun AvexIntro(iconKey: String, themed: Boolean = true, onDone: () -> Unit) {
    val (gradTop, gradBottom) = forgeBackgroundGradient(LocalForgeSettings.current.amoledMode)
    val reduceMotion = ForgeMotion.durationScale <= 0f
    val icon = if (themed) AppIcon.fromKey(iconKey) else AppIcon.Default

    // Text settles: alpha 0→1 with a hair of upward scale. Plate: the whole overlay fades out to
    // reveal. Exit: families with a choreographed wordmark death (melt, black hole, CRT-off, wipe)
    // play it over a shortened hold so the total stays in the stock envelope.
    val reveal = remember { Animatable(if (reduceMotion) 1f else 0f) }
    val plateAlpha = remember { Animatable(1f) }
    val exit = remember { Animatable(0f) }

    LaunchedEffect(Unit) {
        if (!reduceMotion) {
            reveal.animateTo(1f, ForgeMotion.enterTween(ForgeMotion.DurationEmphasized))
        }
        val choreographedExit = !reduceMotion && wordmarkExitChoreographed(icon)
        delay(if (reduceMotion) 450 else if (choreographedExit) 570 else 700)
        if (choreographedExit) {
            // The death starts first; the plate fade joins in later so the destruction reads before
            // everything dims together. Molten melts DECELERATING (material gives way fast, then
            // drips slowly) and gets a longer window; the others accelerate away.
            val melt = icon.family == IconFamily.Molten
            launch {
                exit.animateTo(
                    1f,
                    tween(
                        ForgeMotion.scaledDuration(if (melt) 650 else 480),
                        easing = if (melt) ForgeMotion.Decelerate else ForgeMotion.Accelerate
                    )
                )
            }
            delay(if (melt) 320 else 230)
        }
        if (!reduceMotion) {
            plateAlpha.animateTo(0f, tween(ForgeMotion.scaledDuration(ForgeMotion.DurationStandard), easing = ForgeMotion.Accelerate))
        }
        onDone()
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .graphicsLayer { alpha = plateAlpha.value }
            .background(Brush.verticalGradient(listOf(gradTop, gradBottom))),
        contentAlignment = Alignment.Center
    ) {
        // Pass the animation values as deferred reads so this composable (and the plain wordmark path)
        // don't recompose every frame — AvexWordmark reads them in its draw/graphicsLayer phase.
        AvexWordmark(icon, reveal = { reveal.value }, exit = { exit.value }, reduceMotion = reduceMotion)
    }
}
