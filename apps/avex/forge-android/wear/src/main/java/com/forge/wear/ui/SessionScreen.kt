package com.forge.wear.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
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
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.rotary.onRotaryScrollEvent
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.wear.compose.material.Text
import com.forge.shared.protocol.CmdAckDto
import com.forge.shared.protocol.SessionLiveDto
import com.forge.wear.data.WearDataRepository
import kotlinx.coroutines.delay

/**
 * The ~2 seconds around each set (W2): target big, one-tap log, adjust by bezel OR the ± steppers
 * (§16: rotary primary, touch capsules always present), ticks. Everything shown arrives from
 * /session/live; a log is pending until its ack lands (never optimistic). THE FIGURE IS THE
 * ADJUST TARGET: weight big by default, tap the small reps line to bring reps up (and back) —
 * whatever is big is what the bezel and steppers change. Bodyweight slots pin reps big (no
 * weight to adjust). A phone-flagged big jump answers with a Confirm capsule, one more tap logs
 * it. Log-success haptics + the PR flash live in WearRoot — the rest timer usually replaces this
 * screen before the ack lands.
 */
@Composable
fun SetView(
    session: SessionLiveDto,
    repo: WearDataRepository,
    onRpe: (Long) -> Unit,
    modifier: Modifier = Modifier
) {
    val colors = LocalWearColors.current
    val lastAck by repo.lastAck.collectAsStateWithLifecycle()
    val lastLog by repo.lastLog.collectAsStateWithLifecycle()

    // Adjusted values, reseeded whenever the mirror advances to a new set/exercise.
    val seedKey = "${session.exerciseId}:${session.setIndex}:${session.targetWeightText}"
    var weightValue by remember(seedKey) {
        mutableStateOf(session.targetWeightText?.toDoubleOrNull())
    }
    var reps by remember(seedKey) {
        mutableStateOf(session.targetRepsText?.takeWhile { it.isDigit() }?.toIntOrNull())
    }
    // What the bezel/steppers change = what renders big. Bodyweight pins reps.
    var adjustReps by remember(seedKey) { mutableStateOf(session.isBodyweight) }
    var rotaryAccum by remember { mutableStateOf(0f) }

    // Pending command lifecycle: LOG → pending until the matching ack (or a quiet timeout line).
    var pendingId by remember { mutableStateOf<String?>(null) }
    // A command that timed out without an ack. It may still have landed, so a re-tap must RESEND
    // it under the same id rather than mint a new one — the phone's deduper keys on the id, and a
    // fresh UUID is a second set. Held with the exact payload it was sent for: if the user adjusts
    // the weight or reps before tapping again, that is a different set and earns a new id.
    // Keyed on seedKey: if the mirror advances to the next set, the command DID land, so any
    // remembered retry is stale and must not be reused against a different set.
    var timedOutId by remember(seedKey) { mutableStateOf<String?>(null) }
    var timedOutPayload by remember(seedKey) { mutableStateOf<String?>(null) }
    var statusLine by remember { mutableStateOf<String?>(null) }
    var confirmJump by remember(seedKey) { mutableStateOf(false) }
    var nowMs by remember { mutableLongStateOf(System.currentTimeMillis()) }

    LaunchedEffect(Unit) {
        while (true) { nowMs = System.currentTimeMillis(); delay(250) }
    }
    // Ack resolution — confirmation is the mirror updating; success feedback fires in WearRoot.
    LaunchedEffect(lastAck, pendingId) {
        val ack: CmdAckDto = lastAck ?: return@LaunchedEffect
        if (ack.commandId != pendingId) return@LaunchedEffect
        pendingId = null
        // Resolved either way, so there is nothing left to resend under this id.
        timedOutId = null
        timedOutPayload = null
        when {
            ack.ok -> statusLine = null
            ack.needsConfirm -> { confirmJump = true; statusLine = "Big jump, tap to confirm" }
            else -> statusLine = ack.reason ?: "Not logged"
        }
    }
    // Pending timeout → quiet reconnect line. The command may still land, which is why the id is
    // kept above: retry is only safe because the resend carries it, not because a retry is harmless.
    LaunchedEffect(pendingId) {
        if (pendingId == null) return@LaunchedEffect
        delay(4_000)
        if (pendingId != null) {
            statusLine = "Not logged · reconnecting"
            timedOutId = pendingId
            pendingId = null
        }
    }

    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }

    val weightStep = session.weightStep
    val elapsedMin = ((nowMs - session.startedAtMs) / 60_000L).coerceAtLeast(0)

    // ONE step path for bezel detents and ± taps — the two inputs may never drift. Stepping a
    // weightless slot builds a weight up from zero; touching a value voids a pending confirm.
    val step: (Int) -> Unit = { detents ->
        confirmJump = false
        statusLine = null
        if (adjustReps) {
            reps = ((reps ?: 0) + detents).coerceIn(1, 99)
        } else {
            weightValue = ((weightValue ?: 0.0) + detents * weightStep).coerceAtLeast(0.0)
        }
    }

    Box(
        modifier = modifier
            .fillMaxSize()
            .onRotaryScrollEvent { ev ->
                rotaryAccum += ev.verticalScrollPixels
                val detents = (rotaryAccum / ROTARY_PX_PER_STEP).toInt()
                if (detents != 0) {
                    rotaryAccum -= detents * ROTARY_PX_PER_STEP
                    step(detents)
                }
                true
            }
            .focusRequester(focus)
            .focusable(),
        contentAlignment = Alignment.Center
    ) {
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

            // THE serif figure = the value being adjusted, flanked by its ± steppers (§16).
            val weightText = weightValue?.let { formatAdjusted(it) } ?: session.targetWeightText
            val repsText = reps?.toString() ?: session.targetRepsText ?: "—"
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                WristStepper("−", onClick = { step(-1) })
                Text(
                    if (adjustReps) repsText else (weightText ?: "—"),
                    style = WearType.figure,
                    color = colors.onBg
                )
                WristStepper("+", onClick = { step(+1) })
            }
            Spacer(Modifier.height(2.dp))
            // The OTHER value, one small line — tap it to swap what's big (pinned on bodyweight).
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (adjustReps) {
                    Text("REPS", style = WearType.labelSmall, color = colors.muted)
                    Spacer(Modifier.width(8.dp))
                    val other = if (session.isBodyweight) "BW"
                    else "${weightText ?: "—"} ${unitLabel(session).uppercase()}"
                    Text(
                        other,
                        style = WearType.label,
                        color = if (session.isBodyweight) colors.muted else colors.onBg,
                        modifier = Modifier
                            .clickable(enabled = !session.isBodyweight) { adjustReps = false }
                            .padding(4.dp)
                    )
                } else {
                    Text(unitLabel(session).uppercase(), style = WearType.labelSmall, color = colors.muted)
                    Spacer(Modifier.width(8.dp))
                    Text(
                        "× $repsText",
                        style = WearType.label,
                        color = colors.onBg,
                        modifier = Modifier
                            .clickable { adjustReps = true }
                            .padding(4.dp)
                    )
                }
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
            Spacer(Modifier.height(8.dp))

            // Status ABOVE the capsule — the column is center-anchored, so a line down here stays
            // on the round screen where one below the capsule clipped off its bottom edge.
            val line = statusLine
            if (line != null && pendingId == null) {
                Text(line, style = WearType.labelSmall, color = colors.muted, textAlign = TextAlign.Center)
                Spacer(Modifier.height(4.dp))
            }

            if (pendingId != null) {
                Text("LOGGING…", style = WearType.label, color = colors.muted)
            } else {
                WristCapsule(
                    label = if (confirmJump) "Confirm" else "Log set",
                    filled = true,
                    onClick = {
                        statusLine = null
                        val text = weightValue?.let { formatAdjusted(it) } ?: session.targetWeightText
                        val payload = "$text|$reps|$confirmJump"
                        // Same set, tapped again after a timeout → resend the SAME command id so
                        // the phone can recognise it as a replay. Anything edited since makes it a
                        // genuinely different set, which gets a fresh id.
                        val reuseId = timedOutId?.takeIf { timedOutPayload == payload }
                        pendingId = repo.sendLogSet(
                            sessionId = session.sessionId,
                            exerciseId = session.exerciseId,
                            weightText = text,
                            reps = reps,
                            confirmedJump = confirmJump,
                            commandId = reuseId
                        )
                        timedOutId = null
                        timedOutPayload = payload
                        confirmJump = false
                    }
                )
            }

            // The just-logged set's window (repo-tracked, shared with the rest screen): undo + rate.
            val log = lastLog
            if (pendingId == null && log != null && !log.rpeSent && nowMs - log.atLocalMs < LAST_LOG_WINDOW_MS) {
                Spacer(Modifier.height(2.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        "undo",
                        style = WearType.label, color = colors.accent,
                        modifier = Modifier
                            .clickable { pendingId = repo.sendUndoSet(session.sessionId) }
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

/**
 * Rate the set the log ack named (§16: one decision per screen). Bezel or steppers move the
 * figure in the phone's half-step 6–10 scale; Save sends the targeted /cmd/set-rpe.
 */
@Composable
fun RpeScreen(
    setId: Long,
    repo: WearDataRepository,
    onDone: () -> Unit,
    modifier: Modifier = Modifier
) {
    val colors = LocalWearColors.current
    var rpe by remember { mutableStateOf(8.0) }
    var rotaryAccum by remember { mutableStateOf(0f) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }

    fun stepRpe(detents: Int) {
        rpe = (rpe + detents * 0.5).coerceIn(6.0, 10.0)
    }

    Box(
        modifier = modifier
            .fillMaxSize()
            .onRotaryScrollEvent { ev ->
                rotaryAccum += ev.verticalScrollPixels
                val detents = (rotaryAccum / ROTARY_PX_PER_STEP).toInt()
                if (detents != 0) {
                    rotaryAccum -= detents * ROTARY_PX_PER_STEP
                    stepRpe(detents)
                }
                true
            }
            .focusRequester(focus)
            .focusable(),
        contentAlignment = Alignment.Center
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier.padding(horizontal = 20.dp)
        ) {
            Text("RPE · HOW HARD", style = WearType.labelSmall, color = colors.muted)
            Spacer(Modifier.height(6.dp))
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                WristStepper("−", onClick = { stepRpe(-1) })
                Text(formatAdjusted(rpe), style = WearType.figure, color = colors.onBg)
                WristStepper("+", onClick = { stepRpe(+1) })
            }
            Spacer(Modifier.height(2.dp))
            Text("${formatAdjusted(10.0 - rpe)} RIR", style = WearType.labelSmall, color = colors.muted)
            Spacer(Modifier.height(10.dp))
            WristCapsule(
                label = "Save",
                filled = true,
                onClick = { repo.sendSetRpe(setId, rpe); onDone() }
            )
            Spacer(Modifier.height(2.dp))
            Text(
                "close",
                style = WearType.labelSmall, color = colors.muted,
                modifier = Modifier
                    .clickable(onClick = onDone)
                    .padding(6.dp)
            )
        }
    }
}

/** "187.5" → "187.5", "185.0" → "185" — adjusted values stay clean input text. */
internal fun formatAdjusted(value: Double): String =
    if (value % 1.0 == 0.0) value.toInt().toString() else value.toString()

private fun unitLabel(session: SessionLiveDto): String =
    if (session.isPlates) "plates" else when (session.unit) {
        com.forge.shared.weight.ProtocolWeightUnit.KG -> "kg"
        com.forge.shared.weight.ProtocolWeightUnit.ST -> "st"
        else -> "lb"
    }

internal const val ROTARY_PX_PER_STEP = 40f

/** How long after a logged set the wrist offers undo + rate (matches the phone's slack). */
internal const val LAST_LOG_WINDOW_MS = 12_000L
