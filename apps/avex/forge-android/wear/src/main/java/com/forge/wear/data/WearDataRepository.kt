package com.forge.wear.data

import android.annotation.SuppressLint
import android.content.Context
import com.forge.shared.protocol.CmdAckDto
import com.forge.shared.protocol.ConfigDto
import com.forge.shared.protocol.GlanceTodayDto
import com.forge.shared.protocol.HapticAckDto
import com.forge.shared.protocol.LogSetCommand
import com.forge.shared.protocol.SessionLiveDto
import com.forge.shared.protocol.SetRpeCommand
import com.forge.shared.protocol.TimerCommand
import com.forge.shared.protocol.TimerStateDto
import com.forge.shared.protocol.UndoSetCommand
import com.forge.shared.protocol.WearCodec
import com.forge.shared.protocol.WearProtocol
import com.google.android.gms.wearable.DataClient
import com.google.android.gms.wearable.DataEvent
import com.google.android.gms.wearable.DataEventBuffer
import com.google.android.gms.wearable.DataItem
import com.google.android.gms.wearable.Wearable
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.tasks.await
import java.util.UUID

/**
 * The watch's single source of phone state (W1): mirrors the /session/live, /timer/state,
 * /config, /glance/today and /cmd/ack DataItems into StateFlows, and sends commands back as
 * Messages. The phone is the brain — this class NEVER invents state; a disconnect just leaves
 * the last-known DataItems (and the UI stamps their age).
 *
 * Process-wide singleton (see [instance]): the activity, the background listener service and the
 * tiles all read the same flows.
 */
class WearDataRepository private constructor(context: Context) : DataClient.OnDataChangedListener {

    private val appContext = context.applicationContext
    private val dataClient by lazy { Wearable.getDataClient(appContext) }
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    private val _session = MutableStateFlow<SessionLiveDto?>(null)
    val session: StateFlow<SessionLiveDto?> = _session

    private val _timer = MutableStateFlow<TimerStateDto?>(null)
    val timer: StateFlow<TimerStateDto?> = _timer

    private val _config = MutableStateFlow(ConfigDto())
    val config: StateFlow<ConfigDto> = _config

    private val _glance = MutableStateFlow<GlanceTodayDto?>(null)
    val glance: StateFlow<GlanceTodayDto?> = _glance

    private val _lastAck = MutableStateFlow<CmdAckDto?>(null)
    val lastAck: StateFlow<CmdAckDto?> = _lastAck

    /** The last successfully logged set (from its ack), stamped with LOCAL receive time — drives
     *  the rest screen's transient undo/RPE row. [rpeSent] hides the row once a rating went out. */
    data class LastLog(val setId: Long, val atLocalMs: Long, val rpeSent: Boolean = false)

    private val _lastLog = MutableStateFlow<LastLog?>(null)
    val lastLog: StateFlow<LastLog?> = _lastLog

    /** The phone speaks a newer protocol — the UI shows "update Avex on your phone". */
    private val _newerVersion = MutableStateFlow(false)
    val newerVersion: StateFlow<Boolean> = _newerVersion

    fun start() {
        dataClient.addListener(this)
        // Seed from whatever DataItems already exist — state survives both apps restarting.
        scope.launch {
            runCatching {
                val buffer = dataClient.dataItems.await()
                try {
                    // seeded = these DataItems were already on the node when we started. State items
                    // (session, timer, config, glance) are exactly what we want back; a COMMAND ACK
                    // is not. /cmd/ack is never deleted, so the last ack ever published — possibly
                    // days old — was re-applied here and stamped with the local clock, arming the
                    // 12-second undo + rate row from app launch rather than from the set. Wear OS
                    // reclaims background processes readily, so a wrist raised mid-session offered
                    // "undo · rate" for a set two exercises back, and the RPE window is ten minutes
                    // wide: the rating landed on it.
                    for (item in buffer) applyItem(item.freeze(), deleted = false, seeded = true)
                } finally {
                    buffer.release()
                }
            }
        }
    }

    override fun onDataChanged(events: DataEventBuffer) {
        for (event in events) {
            applyItem(event.dataItem.freeze(), deleted = event.type == DataEvent.TYPE_DELETED)
        }
    }

    private fun applyItem(item: DataItem, deleted: Boolean, seeded: Boolean = false) {
        val bytes = item.data
        val path = item.uri.path
        // Acks live at "$PATH_CMD_ACK/$commandId" — one path each, so a second command's ack can't
        // supersede an unsynced first one — hence a prefix match rather than equality. A replayed
        // ack is not an event: it answers a command from a previous run of this app.
        if (path != null && path.startsWith(WearProtocol.PATH_CMD_ACK)) {
            if (!deleted && !seeded) decodeInto<CmdAckDto>(bytes) { ack ->
                _lastAck.value = ack
                // A successful log names its set — remember it locally for undo/RPE.
                if (ack.ok && ack.setId != null) {
                    _lastLog.value = LastLog(ack.setId!!, System.currentTimeMillis())
                }
            }
            return
        }
        when (path) {
            WearProtocol.PATH_SESSION_LIVE ->
                if (deleted) _session.value = null
                else decodeInto<SessionLiveDto>(bytes) { _session.value = it }
            WearProtocol.PATH_TIMER_STATE ->
                if (deleted) _timer.value = null
                else decodeInto<TimerStateDto>(bytes) { _timer.value = it }
            WearProtocol.PATH_CONFIG ->
                if (!deleted) decodeInto<ConfigDto>(bytes) { _config.value = it }
            WearProtocol.PATH_GLANCE_TODAY ->
                if (!deleted) decodeInto<GlanceTodayDto>(bytes) { _glance.value = it }
        }
    }

    private inline fun <reified T> decodeInto(bytes: ByteArray?, apply: (T) -> Unit) {
        when (val result = WearCodec.decode<T>(bytes ?: return)) {
            is WearCodec.DecodeResult.Ok -> apply(result.value)
            is WearCodec.DecodeResult.NewerVersion -> _newerVersion.value = true
            is WearCodec.DecodeResult.Invalid -> Unit
        }
    }

    // ── Commands (fire-once Messages; the ack DataItem confirms) ─────────────

    fun sendTimerCommand(action: TimerCommand.Action): String =
        send(WearProtocol.PATH_CMD_TIMER, TimerCommand(commandId = newId(), action = action))

    /**
     * [commandId] lets the caller REUSE an id rather than mint one. The phone dedupes by command
     * id, so re-sending a command that may already have landed is only safe when it carries the
     * same id — a fresh UUID reads as a second, unrelated set. See SetView's retry handling.
     */
    fun sendLogSet(
        sessionId: Long,
        exerciseId: String?,
        weightText: String?,
        reps: Int?,
        confirmedJump: Boolean = false,
        commandId: String? = null
    ): String =
        send(
            WearProtocol.PATH_CMD_LOG_SET,
            LogSetCommand(
                commandId = commandId ?: newId(), sessionId = sessionId,
                exerciseId = exerciseId, weightText = weightText, reps = reps,
                confirmedJump = confirmedJump
            )
        )

    /**
     * Undo [setId] — the set the caller's row is offering to undo, not "whatever was logged last".
     * Callers pass the id from [lastLog]; the phone falls back to its own resolution when it is null.
     */
    fun sendUndoSet(sessionId: Long, setId: Long?): String {
        _lastLog.value = null // The row acted; don't offer to rate an undone set.
        return send(
            WearProtocol.PATH_CMD_UNDO_SET,
            UndoSetCommand(commandId = newId(), sessionId = sessionId, setId = setId)
        )
    }

    fun sendSetRpe(setId: Long, rpe: Double): String {
        _lastLog.value = _lastLog.value?.takeIf { it.setId == setId }?.copy(rpeSent = true)
        return send(WearProtocol.PATH_CMD_SET_RPE, SetRpeCommand(commandId = newId(), setId = setId, rpe = rpe))
    }

    /**
     * Deliver a batch of HR samples. Returns true only once a connected node has accepted it, so the
     * caller can keep the samples buffered instead of dropping them into a failed send.
     *
     * `nowMs` rides along so the phone can measure the skew between the two devices' clocks: the
     * samples are stamped on the WATCH's clock and used to be filtered against a PHONE-clock session
     * start, which silently ate the first seconds of every trace whenever the watch ran behind.
     */
    suspend fun sendHrBatchAwait(
        sessionId: Long,
        samples: List<com.forge.shared.protocol.HrBatchDto.Sample>,
        totalKcal: Double?
    ): Boolean = sendWithRetry(
        WearProtocol.PATH_HR_BATCH,
        WearCodec.encode(
            com.forge.shared.protocol.HrBatchDto(
                sessionId = sessionId, samples = samples, totalKcal = totalKcal,
                sentAtMs = System.currentTimeMillis()
            )
        )
    )

    fun sendHapticAck(timerEndAtMs: Long) {
        sendBytes(
            WearProtocol.PATH_HAPTIC_ACK,
            WearCodec.encode(HapticAckDto(timerEndAtMs = timerEndAtMs, atMs = System.currentTimeMillis()))
        )
    }

    private inline fun <reified T> send(path: String, dto: T): String where T : Any {
        sendBytes(path, WearCodec.encode(dto))
        // The commandId rides inside the dto; recover it for pending-state tracking.
        return when (dto) {
            is TimerCommand -> dto.commandId
            is LogSetCommand -> dto.commandId
            is UndoSetCommand -> dto.commandId
            is SetRpeCommand -> dto.commandId
            else -> ""
        }
    }

    private fun sendBytes(path: String, bytes: ByteArray) {
        scope.launch { sendWithRetry(path, bytes) }
    }

    /**
     * Deliver [bytes], retrying a few times with backoff.
     *
     * MessageClient is fire-and-forget and requires a LIVE connection, so a send attempted while
     * the phone is out of Bluetooth range simply does not happen. The previous implementation wrote
     * that off in three separate ways: a single `runCatching` swallowed every failure, an EMPTY
     * connected-node list made `forEach` a no-op that returned normally — so "nothing was sent"
     * was indistinguishable from success — and a throw on the first node skipped every node after
     * it. A set logged at the far end of the gym was gone, with nothing retried and nothing said.
     *
     * Retrying covers the case that actually dominates: a transient flap, or the user walking back
     * into range within a few seconds. Combined with SetView reusing the command id on a re-tap,
     * a redelivery that races a landed command is dropped by the phone's deduper rather than
     * logging twice.
     *
     * This does NOT survive process death — the queue is in memory. Genuine offline durability
     * needs the commands moved onto DataClient, whose items persist and sync on reconnect (the
     * /cmd/ack path already uses it), and that is a transport change on both sides rather than a
     * fix here.
     */
    private suspend fun sendWithRetry(path: String, bytes: ByteArray): Boolean {
        var wait = SEND_RETRY_INITIAL_MS
        repeat(SEND_ATTEMPTS) { attempt ->
            if (deliverOnce(path, bytes)) return true
            if (attempt < SEND_ATTEMPTS - 1) {
                delay(wait)
                wait = (wait * 2).coerceAtMost(SEND_RETRY_MAX_MS)
            }
        }
        return false
    }

    /** One delivery pass. True when at least one connected node accepted the payload. */
    private suspend fun deliverOnce(path: String, bytes: ByteArray): Boolean {
        val nodes = runCatching {
            Wearable.getNodeClient(appContext).connectedNodes.await()
        }.getOrNull() ?: return false
        // No connected node means nothing was delivered. Reporting that as success is what let an
        // out-of-range log disappear silently.
        if (nodes.isEmpty()) return false
        var delivered = false
        for (node in nodes) {
            // Per node, so one unreachable node cannot skip the rest.
            runCatching {
                Wearable.getMessageClient(appContext).sendMessage(node.id, path, bytes).await()
            }.onSuccess { delivered = true }
        }
        return delivered
    }

    private fun newId(): String = UUID.randomUUID().toString()

    companion object {
        /** Total delivery attempts, including the first. Backoff runs 1s, 2s, 4s, 8s. */
        private const val SEND_ATTEMPTS = 5
        private const val SEND_RETRY_INITIAL_MS = 1_000L
        private const val SEND_RETRY_MAX_MS = 8_000L

        @SuppressLint("StaticFieldLeak") // application context only
        @Volatile private var INSTANCE: WearDataRepository? = null

        fun instance(context: Context): WearDataRepository =
            INSTANCE ?: synchronized(this) {
                INSTANCE ?: WearDataRepository(context).also { it.start(); INSTANCE = it }
            }
    }
}
