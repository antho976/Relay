package com.forge.wear.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.wear.compose.material.Text
import com.forge.shared.protocol.TimerCommand
import com.forge.shared.protocol.TimerStateDto
import com.forge.wear.data.WearDataRepository
import kotlinx.coroutines.delay

/**
 * The rest countdown (W1): ONE serif figure inside the ring, skip / +30 beneath. The countdown is
 * derived locally from the DataItem's wall-clock endAtMs — no per-second sync. Hitting zero fires
 * the wrist's strong buzz ONCE per timer instance and acks it so the phone stays silent.
 * The rest that follows a set is the natural rating moment, and this screen replaces SetView the
 * instant a log lands — so the just-logged set's undo + rate affordances live here too.
 */
@Composable
fun TimerView(
    timer: TimerStateDto,
    repo: WearDataRepository,
    onRpe: (Long) -> Unit,
    modifier: Modifier = Modifier
) {
    val colors = LocalWearColors.current
    val lastLog by repo.lastLog.collectAsStateWithLifecycle()
    val session by repo.session.collectAsStateWithLifecycle()
    var nowMs by remember { mutableLongStateOf(System.currentTimeMillis()) }
    // When THIS watch first saw this timer instance. endAtMs is an absolute instant on the PHONE's
    // clock; rendering it against the watch's own made every millisecond of skew between the two
    // devices a millisecond of error in the countdown. With publishedAtMs the payload carries a
    // DURATION we can measure locally instead. A phone too old to send it leaves it 0 and we fall
    // back to the raw instant, exactly as before.
    val receivedAtMs = remember(timer.endAtMs, timer.publishedAtMs) { System.currentTimeMillis() }
    // One undo per logged set: reset when a new set's ack arrives.
    var undoSent by remember(lastLog?.setId) { mutableStateOf(false) }

    LaunchedEffect(Unit) {
        while (true) { nowMs = System.currentTimeMillis(); delay(200) }
    }

    val remainingMs = when {
        timer.publishedAtMs > 0L -> (timer.endAtMs - timer.publishedAtMs) - (nowMs - receivedAtMs)
        else -> timer.endAtMs - nowMs
    }
    val remainingSec = if (timer.paused) timer.pausedRemainingSeconds
    else ((remainingMs + 999) / 1000).toInt().coerceAtLeast(0)

    // The rest-done buzz is NOT fired here — see WearRoot. This composable is unmounted the moment
    // the phone republishes the timer as paused-at-zero, which usually beats the local tick to
    // zero, so a buzz owned by this lifetime was a race the wrist mostly lost.

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
            // The just-logged set's window: undo it or rate it, right where the rest is happening.
            val log = lastLog
            if (log != null && !log.rpeSent && nowMs - log.atLocalMs < LAST_LOG_WINDOW_MS) {
                Spacer(Modifier.height(6.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        "undo",
                        style = WearType.label, color = colors.accent,
                        modifier = Modifier
                            // Gated like SetView's undo: clearing lastLog only removes this row on
                            // the NEXT recomposition, so two taps inside one frame both fired, each
                            // with its own command id — two undos the deduper couldn't tell apart.
                            .clickable(enabled = !undoSent) {
                                session?.let {
                                    undoSent = true
                                    repo.sendUndoSet(it.sessionId, log.setId)
                                }
                            }
                            .padding(6.dp)
                    )
                    Spacer(Modifier.width(10.dp))
                    Text(
                        "rate →",
                        style = WearType.label, color = colors.accent,
                        modifier = Modifier
                            .clickable { onRpe(log.setId) }
                            .padding(6.dp)
                    )
                }
            }
        }
    }
}

private fun formatMmSs(totalSeconds: Int): String =
    "%d:%02d".format(totalSeconds / 60, totalSeconds % 60)
