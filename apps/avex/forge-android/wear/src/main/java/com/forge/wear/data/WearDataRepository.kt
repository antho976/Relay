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

    /**
     * The phone speaks a newer protocol on a path the watch cannot work without — the UI shows
     * "update Avex on your phone".
     *
     * Latched PER PATH, and cleared when a later payload on that same path decodes cleanly. It used
     * to be one process-wide boolean that any path could set and nothing could clear, so a single
     * newer-version `/glance/today` — a read-only tile payload the watch can simply do without —
     * blocked session mirroring, set logging and the rest timer for the rest of the process
     * lifetime, with `/session/live` still decoding perfectly the whole time.
     */
    private val newerVersionPaths = mutableSetOf<String>()

    private val _newerVersion = MutableStateFlow(false)
    val newerVersion: StateFlow<Boolean> = _newerVersion

    /** Paths the watch genuinely cannot proceed on. `/glance/today` is deliberately not one. */
    private fun blocksTheUi(path: String) = path != WearProtocol.PATH_GLANCE_TODAY

    private fun markVersion(path: String, newer: Boolean) {
        synchronized(newerVersionPaths) {
            if (newer) newerVersionPaths.add(path) else newerVersionPaths.remove(path)
            _newerVersion.value = newerVersionPaths.any { blocksTheUi(it) }
        }
    }

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
        val path = item.uri.path ?: return
        // Acks live at "$PATH_CMD_ACK/$commandId" — one path each, so a second command's ack can't
        // supersede an unsynced first one — hence a prefix match rather than equality. A replayed
        // ack is not an event: it answers a command from a previous run of this app.
        if (path.startsWith(WearProtocol.PATH_CMD_ACK)) {
            // A DELETED item says nothing about what protocol the phone speaks, so it must not
            // leave this path latched as newer — see [clearLatchOnDelete].
            if (deleted) return clearLatchOnDelete(WearProtocol.PATH_CMD_ACK)
            if (!seeded) decodeInto<CmdAckDto>(bytes, WearProtocol.PATH_CMD_ACK) { ack ->
                _lastAck.value = ack
                // A successful LOG names its set — remember it locally for undo/RPE.
                //
                // "A successful ack with a setId" is not the same statement: an RPE ack carries the
                // id of the set that was just RATED, so saving a rating re-armed this very row. The
                // undo/rate prompt reopened for a set the user had just finished rating, and
                // WearRoot's set-logged haptic fired for it a second time. The ack now says which
                // command it answers; an older phone leaves that blank, and the fallback is the
                // wrist's own record of the ids it sent RPE for.
                if (ack.ok && ack.setId != null && isNewlyLoggedSet(ack)) {
                    _lastLog.value = LastLog(ack.setId!!, System.currentTimeMillis())
                }
            }
            return
        }
        // Same rule for every state path: deletion clears the latch. Without this, an incompatible
        // payload latched "update Avex on your phone" and the DELETE that followed cleared the
        // session and timer state while leaving the latch set — so the watch sat on the update
        // screen until some later payload happened to decode cleanly on that same path, which for
        // a path whose item had just been removed could be indefinitely.
        if (deleted) return clearLatchOnDelete(path)
        when (path) {
            WearProtocol.PATH_SESSION_LIVE -> decodeInto<SessionLiveDto>(bytes, path) { _session.value = it }
            WearProtocol.PATH_TIMER_STATE -> decodeInto<TimerStateDto>(bytes, path) { dto ->
                _timer.value = dto
                // Both clocks in one payload: the phone's at publish, ours now. The complication
                // renders the countdown without ever seeing an arrival time, so it needs this.
                if (dto.publishedAtMs > 0L) {
                    WearClockSkew.record(appContext, dto.publishedAtMs, System.currentTimeMillis())
                }
            }
            WearProtocol.PATH_CONFIG -> decodeInto<ConfigDto>(bytes, path) { _config.value = it }
            WearProtocol.PATH_GLANCE_TODAY -> decodeInto<GlanceTodayDto>(bytes, path) { _glance.value = it }
        }
    }

    /** Clear [path]'s state and its version latch. */
    private fun clearLatchOnDelete(path: String) {
        when (path) {
            WearProtocol.PATH_SESSION_LIVE -> _session.value = null
            WearProtocol.PATH_TIMER_STATE -> _timer.value = null
        }
        markVersion(path, newer = false)
    }

    /**
     * Whether [ack] announces a set that was just logged, as opposed to one that was rated.
     *
     * Public because WearRoot asks the same question for the set-logged haptic: an RPE ack used to
     * fire the wrist's "set logged" tick a second time, minutes after the set.
     *
     * Prefers what the phone said. A phone predating [CmdAckDto.KIND_LOG_SET] says nothing, and the
     * fallback is what this process sent: an ack answering one of our own RPE commands is never a
     * new log, whatever else it carries.
     */
    fun isNewlyLoggedSet(ack: CmdAckDto): Boolean = when {
        ack.kind == CmdAckDto.KIND_LOG_SET -> true
        ack.kind.isNotEmpty() -> false
        else -> !isOwnRpeCommand(ack.commandId)
    }

    private inline fun <reified T> decodeInto(bytes: ByteArray?, path: String, apply: (T) -> Unit) {
        when (val result = WearCodec.decode<T>(bytes ?: return)) {
            // A clean decode CLEARS this path's latch: the phone that was ahead of us has been
            // downgraded, or the payload that tripped it was a one-off.
            is WearCodec.DecodeResult.Ok -> { markVersion(path, newer = false); apply(result.value) }
            is WearCodec.DecodeResult.NewerVersion -> markVersion(path, newer = true)
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
        val commandId = newId()
        rememberRpeCommand(commandId)
        return send(WearProtocol.PATH_CMD_SET_RPE, SetRpeCommand(commandId = commandId, setId = setId, rpe = rpe))
    }

    /**
     * Command ids this process sent as RPE ratings — the fallback identity for acks from a phone
     * too old to name the command it is answering. A handful is plenty: an ack that has not arrived
     * within the next few commands is not going to re-arm anything the user still recognises.
     */
    private val rpeCommandIds = ArrayDeque<String>()

    private fun rememberRpeCommand(commandId: String) {
        synchronized(rpeCommandIds) {
            rpeCommandIds.addLast(commandId)
            while (rpeCommandIds.size > RPE_COMMAND_MEMORY) rpeCommandIds.removeFirst()
        }
    }

    private fun isOwnRpeCommand(commandId: String): Boolean =
        synchronized(rpeCommandIds) { commandId in rpeCommandIds }

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
        /** How many recently sent RPE command ids the wrist keeps, for pre-[CmdAckDto.kind] phones. */
        private const val RPE_COMMAND_MEMORY = 16

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
