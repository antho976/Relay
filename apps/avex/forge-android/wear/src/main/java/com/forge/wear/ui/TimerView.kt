package com.forge.wear.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.wear.compose.material.Text
import com.forge.shared.protocol.TimerCommand
import com.forge.shared.protocol.TimerStateDto
import com.forge.wear.data.WearDataRepository
import com.forge.wear.data.WristHaptics
import kotlinx.coroutines.delay

/**
 * The rest countdown (W1): ONE serif figure inside the ring, skip / +30 beneath. The countdown is
 * derived locally from the DataItem's wall-clock endAtMs — no per-second sync. Hitting zero fires
 * the wrist's strong buzz ONCE per timer instance and acks it so the phone stays silent.
 */
@Composable
fun TimerView(
    timer: TimerStateDto,
    repo: WearDataRepository,
    haptics: WristHaptics,
    modifier: Modifier = Modifier
) {
    val colors = LocalWearColors.current
    var nowMs by remember { mutableLongStateOf(System.currentTimeMillis()) }
    var buzzedForEndAt by remember { mutableLongStateOf(0L) }

    LaunchedEffect(Unit) {
        while (true) { nowMs = System.currentTimeMillis(); delay(200) }
    }

    val remainingSec = if (timer.paused) timer.pausedRemainingSeconds
    else (((timer.endAtMs - nowMs) + 999) / 1000).toInt().coerceAtLeast(0)

    // The one strong buzz, once per timer instance (endAtMs identifies it), acked to the phone.
    LaunchedEffect(remainingSec, timer.endAtMs) {
        if (!timer.paused && remainingSec == 0 && buzzedForEndAt != timer.endAtMs) {
            buzzedForEndAt = timer.endAtMs
            haptics.timerDone()
            repo.sendHapticAck(timer.endAtMs)
        }
    }

    val progress = if (timer.totalSeconds <= 0) 0f else remainingSec.toFloat() / timer.totalSeconds

    Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        TimerRing(progress = progress, modifier = Modifier.fillMaxSize().padding(6.dp))
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text("REST", style = WearType.labelSmall, color = colors.muted)
            Spacer(Modifier.height(2.dp))
            Text(formatMmSs(remainingSec), style = WearType.figure, color = colors.onBg)
            Spacer(Modifier.height(10.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                WristCapsule(label = "+30", onClick = { repo.sendTimerCommand(TimerCommand.Action.ADD_30) })
                WristCapsule(label = "Skip", onClick = { repo.sendTimerCommand(TimerCommand.Action.SKIP) })
            }
        }
    }
}

private fun formatMmSs(totalSeconds: Int): String =
    "%d:%02d".format(totalSeconds / 60, totalSeconds % 60)
