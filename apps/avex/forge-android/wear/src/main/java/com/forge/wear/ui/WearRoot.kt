package com.forge.wear.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.wear.compose.material.Text
import androidx.wear.compose.material.TimeText
import com.forge.shared.protocol.GlanceTodayDto
import com.forge.wear.data.WearDataRepository
import com.forge.wear.data.WristHaptics

/**
 * The watch app's whole surface (W1): pure black ground, one screen at a time, driven entirely by
 * the mirrored DataItems. No session → the idle glance; session + running timer → the countdown;
 * session otherwise → the set view. A newer phone protocol → the one honest update screen.
 */
@Composable
fun WearRoot(repo: WearDataRepository, haptics: WristHaptics) {
    val session by repo.session.collectAsStateWithLifecycle()
    val timer by repo.timer.collectAsStateWithLifecycle()
    val config by repo.config.collectAsStateWithLifecycle()
    val glance by repo.glance.collectAsStateWithLifecycle()
    val newerVersion by repo.newerVersion.collectAsStateWithLifecycle()

    CompositionLocalProvider(LocalWearColors provides wearColors(config)) {
        Box(Modifier.fillMaxSize().background(Color.Black)) {
            when {
                newerVersion -> UpdateScreen()
                session != null -> {
                    val t = timer
                    // A finished/stopped timer (paused at 0) hands back to the set view.
                    val timerLive = t != null && (!t.paused || t.pausedRemainingSeconds > 0)
                    if (timerLive) TimerView(t!!, repo, haptics) else SetView(session!!, repo, haptics)
                }
                else -> IdleScreen(glance)
            }
            TimeText()
        }
    }
}

/** Idle home — the glance-lite (W1): readiness figure, next day, week count, stamped with age. */
@Composable
private fun IdleScreen(glance: GlanceTodayDto?) {
    val colors = LocalWearColors.current
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier.padding(horizontal = 24.dp)
        ) {
            Text("AVEX", style = WearType.labelSmall, color = colors.muted)
            Spacer(Modifier.height(6.dp))
            when {
                glance == null -> {
                    Text("—", style = WearType.figure, color = colors.muted)
                    Spacer(Modifier.height(4.dp))
                    Text("Open Avex on your phone", style = WearType.body, color = colors.muted, textAlign = TextAlign.Center)
                }
                glance.readinessPercent != null -> {
                    Text("${glance.readinessPercent}", style = WearType.figure, color = colors.onBg)
                    Text("READY", style = WearType.labelSmall, color = colors.muted)
                    GlanceLines(glance)
                }
                else -> {
                    // Below the readiness data gates: degrade to the next day, never blank.
                    Text(glance.nextDayTitle ?: "Rest", style = WearType.figureSmall, color = colors.onBg, textAlign = TextAlign.Center)
                    Text("NEXT", style = WearType.labelSmall, color = colors.muted)
                    GlanceLines(glance, showNext = false)
                }
            }
        }
    }
}

@Composable
private fun GlanceLines(glance: GlanceTodayDto, showNext: Boolean = true) {
    val colors = LocalWearColors.current
    Spacer(Modifier.height(8.dp))
    if (showNext && glance.nextDayTitle != null) {
        Text("NEXT · ${glance.nextDayTitle}".uppercase(), style = WearType.labelSmall, color = colors.onBg)
        Spacer(Modifier.height(3.dp))
    }
    Text(
        buildString {
            append("WEEK · ${glance.weekSessionsDone}")
            glance.weekSessionsPlanned?.let { append(" OF $it") }
            glance.weekVolumeText?.let { append(" · ${it.uppercase()}") }
        },
        style = WearType.labelSmall, color = colors.muted
    )
    Spacer(Modifier.height(6.dp))
    Text(dataAge(glance.computedAtMs), style = WearType.labelSmall, color = colors.muted.copy(alpha = 0.7f))
}

/** "2 MIN AGO" / "3H AGO" — every glance is stamped with its data age (never wrong-confident). */
private fun dataAge(computedAtMs: Long): String {
    val min = ((System.currentTimeMillis() - computedAtMs) / 60_000L).coerceAtLeast(0)
    return when {
        min < 1 -> "JUST NOW"
        min < 60 -> "$min MIN AGO"
        else -> "${min / 60}H AGO"
    }
}

/** The phone speaks a newer protocol — one honest line, never a crash or silently-wrong data. */
@Composable
private fun UpdateScreen() {
    val colors = LocalWearColors.current
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.padding(24.dp)) {
            Text("Update", style = WearType.figureSmall, color = colors.onBg)
            Spacer(Modifier.height(4.dp))
            Text(
                "Update Avex on your watch to match your phone",
                style = WearType.body, color = colors.muted, textAlign = TextAlign.Center
            )
        }
    }
}
