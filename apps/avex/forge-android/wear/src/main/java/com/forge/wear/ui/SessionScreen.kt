package com.forge.wear.ui

import androidx.compose.foundation.background
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
import androidx.compose.foundation.focusable
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.rotary.onRotaryScrollEvent
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.clickable
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.wear.compose.material.Text
import com.forge.shared.protocol.CmdAckDto
import com.forge.shared.protocol.SessionLiveDto
import com.forge.wear.data.WearDataRepository
import com.forge.wear.data.WristHaptics
import kotlinx.coroutines.delay

/**
 * The ~2 seconds around each set (W2): target big, one-tap log, bezel adjust, ticks, PR gold.
 * Everything shown arrives from /session/live; a log is pending until its ack lands (never
 * optimistic). The bezel adjusts WEIGHT by the shared step table; tapping the reps line flips
 * the bezel to reps.
 */
@Composable
fun SetView(
    session: SessionLiveDto,
    repo: WearDataRepository,
    haptics: WristHaptics,
    modifier: Modifier = Modifier
) {
    val colors = LocalWearColors.current
    val lastAck by repo.lastAck.collectAsStateWithLifecycle()

    // Bezel-adjusted values, reseeded whenever the mirror advances to a new set/exercise.
    val seedKey = "${session.exerciseId}:${session.setIndex}:${session.targetWeightText}"
    var weightValue by remember(seedKey) {
        mutableStateOf(session.targetWeightText?.toDoubleOrNull())
    }
    var reps by remember(seedKey) {
        mutableStateOf(session.targetRepsText?.takeWhile { it.isDigit() }?.toIntOrNull())
    }
    var bezelOnReps by remember(seedKey) { mutableStateOf(false) }
    var rotaryAccum by remember { mutableStateOf(0f) }

    // Pending command lifecycle: LOG → pending until the matching ack (or a quiet timeout line).
    var pendingId by remember { mutableStateOf<String?>(null) }
    var pendingSinceMs by remember { mutableLongStateOf(0L) }
    var statusLine by remember { mutableStateOf<String?>(null) }
    var prFlashUntilMs by remember { mutableLongStateOf(0L) }
    var undoUntilMs by remember { mutableLongStateOf(0L) }
    var nowMs by remember { mutableLongStateOf(System.currentTimeMillis()) }

    LaunchedEffect(Unit) {
        while (true) { nowMs = System.currentTimeMillis(); delay(250) }
    }
    // Ack resolution — confirmation is the mirror updating; the ack drives haptics + errors.
    LaunchedEffect(lastAck, pendingId) {
        val ack: CmdAckDto = lastAck ?: return@LaunchedEffect
        if (ack.commandId != pendingId) return@LaunchedEffect
        pendingId = null
        if (ack.ok) {
            if (ack.pr) { haptics.pr(); prFlashUntilMs = System.currentTimeMillis() + 1200 } else haptics.setLogged()
            undoUntilMs = System.currentTimeMillis() + 10_000
            statusLine = null
        } else {
            statusLine = ack.reason ?: "Not logged"
        }
    }
    // Pending timeout → quiet reconnect line (the command may still land; dedup makes retry safe).
    LaunchedEffect(pendingId) {
        if (pendingId == null) return@LaunchedEffect
        delay(4_000)
        if (pendingId != null) { statusLine = "Not logged · reconnecting"; pendingId = null }
    }

    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }

    val weightStep = session.weightStep
    val elapsedMin = ((nowMs - session.startedAtMs) / 60_000L).coerceAtLeast(0)
    val prFlash = nowMs < prFlashUntilMs

    Box(
        modifier = modifier
            .fillMaxSize()
            .onRotaryScrollEvent { ev ->
                rotaryAccum += ev.verticalScrollPixels
                val detents = (rotaryAccum / ROTARY_PX_PER_STEP).toInt()
                if (detents != 0) {
                    rotaryAccum -= detents * ROTARY_PX_PER_STEP
                    if (bezelOnReps) {
                        reps = ((reps ?: 0) + detents).coerceIn(1, 99)
                    } else if (weightValue != null) {
                        weightValue = ((weightValue ?: 0.0) + detents * weightStep).coerceAtLeast(0.0)
                    }
                }
                true
            }
            .focusRequester(focus)
            .focusable(),
        contentAlignment = Alignment.Center
    ) {
        if (prFlash) {
            Box(Modifier.fillMaxSize().background(colors.prGold.copy(alpha = 0.25f)))
        }
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier.padding(horizontal = 20.dp)
        ) {
            // Eyebrow: day + elapsed + position — the glance context, one mono line.
            Text(
                "${session.dayTitle} · ${elapsedMin} MIN".uppercase(),
                style = WearType.labelSmall, color = colors.muted, textAlign = TextAlign.Center
            )
            Spacer(Modifier.height(2.dp))
            Text(
                (session.exerciseName ?: "").uppercase(),
                style = WearType.label, color = colors.onBg, textAlign = TextAlign.Center, maxLines = 2
            )
            Spacer(Modifier.height(8.dp))

            // THE serif figure: the weight the tap will log (bezel-adjusted), or the rep target
            // for bodyweight/unseeded slots.
            val weightText = weightValue?.let { formatAdjusted(it) } ?: session.targetWeightText
            Text(
                weightText ?: "—",
                style = WearType.figure,
                color = if (prFlash) colors.prGold else colors.onBg
            )
            Spacer(Modifier.height(2.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    unitLabel(session).uppercase(),
                    style = WearType.labelSmall, color = colors.muted
                )
                Spacer(Modifier.width(8.dp))
                // Tapping the reps readout flips the bezel target; the accent marks the live one.
                Text(
                    "× ${reps ?: (session.targetRepsText ?: "—")}",
                    style = WearType.label,
                    color = if (bezelOnReps) colors.accent else colors.onBg,
                    modifier = Modifier
                        .clickable { bezelOnReps = !bezelOnReps }
                        .padding(4.dp)
                )
            }
            Spacer(Modifier.height(6.dp))
            val liveBpm by com.forge.wear.service.wearLiveBpm.collectAsStateWithLifecycle()
            Text(
                buildString {
                    append("SET ${session.setIndex} OF ${session.setTotal}")
                    liveBpm?.let { append(" · $it BPM") }
                },
                style = WearType.labelSmall, color = colors.muted
            )
            Spacer(Modifier.height(4.dp))
            SetTicks(done = session.loggedSets, total = session.setTotal)
            Spacer(Modifier.height(10.dp))

            if (pendingId != null) {
                Text("LOGGING…", style = WearType.label, color = colors.muted)
            } else {
                WristCapsule(
                    label = "Log set",
                    filled = true,
                    onClick = {
                        statusLine = null
                        pendingSinceMs = System.currentTimeMillis()
                        pendingId = repo.sendLogSet(
                            sessionId = session.sessionId,
                            exerciseId = session.exerciseId,
                            weightText = weightValue?.let { formatAdjusted(it) } ?: session.targetWeightText,
                            reps = reps
                        )
                    }
                )
            }

            val line = statusLine
            if (line != null) {
                Spacer(Modifier.height(4.dp))
                Text(line, style = WearType.labelSmall, color = colors.muted, textAlign = TextAlign.Center)
            } else if (nowMs < undoUntilMs) {
                Spacer(Modifier.height(4.dp))
                Text(
                    "undo",
                    style = WearType.label, color = colors.accent,
                    modifier = Modifier
                        .clickable {
                            undoUntilMs = 0
                            pendingId = repo.sendUndoSet(session.sessionId)
                        }
                        .padding(6.dp)
                )
            }
        }
    }
}

/** "187.5" → "187.5", "185.0" → "185" — adjusted weights stay clean input text. */
private fun formatAdjusted(value: Double): String =
    if (value % 1.0 == 0.0) value.toInt().toString() else value.toString()

private fun unitLabel(session: SessionLiveDto): String =
    if (session.isPlates) "plates" else when (session.unit) {
        com.forge.shared.weight.ProtocolWeightUnit.KG -> "kg"
        com.forge.shared.weight.ProtocolWeightUnit.ST -> "st"
        else -> "lb"
    }

private const val ROTARY_PX_PER_STEP = 40f
