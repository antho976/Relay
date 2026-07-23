package com.forge.wear.data

import android.annotation.SuppressLint
import android.content.Context
import com.forge.shared.protocol.CmdAckDto
import com.forge.shared.protocol.ConfigDto
import com.forge.shared.protocol.GlanceTodayDto
import com.forge.shared.protocol.HapticAckDto
import com.forge.shared.protocol.LogSetCommand
import com.forge.shared.protocol.SessionLiveDto
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
                    for (item in buffer) applyItem(item.freeze(), deleted = false)
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

    private fun applyItem(item: DataItem, deleted: Boolean) {
        val bytes = item.data
        when (item.uri.path) {
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
            WearProtocol.PATH_CMD_ACK ->
                if (!deleted) decodeInto<CmdAckDto>(bytes) { _lastAck.value = it }
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

    fun sendLogSet(sessionId: Long, exerciseId: String?, weightText: String?, reps: Int?): String =
        send(
            WearProtocol.PATH_CMD_LOG_SET,
            LogSetCommand(
                commandId = newId(), sessionId = sessionId,
                exerciseId = exerciseId, weightText = weightText, reps = reps
            )
        )

    fun sendUndoSet(sessionId: Long): String =
        send(WearProtocol.PATH_CMD_UNDO_SET, UndoSetCommand(commandId = newId(), sessionId = sessionId))

    fun sendHrBatch(sessionId: Long, samples: List<com.forge.shared.protocol.HrBatchDto.Sample>, totalKcal: Double?) {
        sendBytes(
            WearProtocol.PATH_HR_BATCH,
            WearCodec.encode(
                com.forge.shared.protocol.HrBatchDto(sessionId = sessionId, samples = samples, totalKcal = totalKcal)
            )
        )
    }

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
            else -> ""
        }
    }

    private fun sendBytes(path: String, bytes: ByteArray) {
        scope.launch {
            runCatching {
                val nodes = Wearable.getNodeClient(appContext).connectedNodes.await()
                nodes.forEach { node ->
                    Wearable.getMessageClient(appContext).sendMessage(node.id, path, bytes).await()
                }
            }
        }
    }

    private fun newId(): String = UUID.randomUUID().toString()

    companion object {
        @SuppressLint("StaticFieldLeak") // application context only
        @Volatile private var INSTANCE: WearDataRepository? = null

        fun instance(context: Context): WearDataRepository =
            INSTANCE ?: synchronized(this) {
                INSTANCE ?: WearDataRepository(context).also { it.start(); INSTANCE = it }
            }
    }
}
