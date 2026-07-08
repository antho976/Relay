package com.forge.app.ui.common

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.graphicsLayer
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.forgeBackgroundGradient
import kotlinx.coroutines.delay

/**
 * One-shot launch wordmark. The serif app name settles in on the brand gradient, holds a beat, then
 * the whole plate fades to reveal the first screen composed beneath it. Shown once per cold launch
 * (MainActivity gates it). Honors the system "Remove animations" setting: no motion, just a short
 * still hold before it hands off.
 */
@Composable
fun AvexIntro(onDone: () -> Unit) {
    val (gradTop, gradBottom) = forgeBackgroundGradient(LocalForgeSettings.current.amoledMode)
    val reduceMotion = ForgeMotion.durationScale <= 0f

    // Text settles: alpha 0→1 with a hair of upward scale. Plate: the whole overlay fades out to reveal.
    val reveal = remember { Animatable(if (reduceMotion) 1f else 0f) }
    val plateAlpha = remember { Animatable(1f) }

    LaunchedEffect(Unit) {
        if (!reduceMotion) {
            reveal.animateTo(1f, ForgeMotion.enterTween(ForgeMotion.DurationEmphasized))
        }
        delay(if (reduceMotion) 450 else 700)
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
        Text(
            "Avex",
            style = MaterialTheme.typography.displayLarge,
            color = MaterialTheme.colorScheme.onBackground,
            modifier = Modifier.graphicsLayer {
                alpha = reveal.value
                val s = 0.94f + 0.06f * reveal.value
                scaleX = s
                scaleY = s
            }
        )
    }
}
