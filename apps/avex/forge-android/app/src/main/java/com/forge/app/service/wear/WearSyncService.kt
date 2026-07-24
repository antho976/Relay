package com.forge.app.service.wear

import com.forge.app.core.time.Clock
import com.forge.shared.protocol.CmdAckDto
import com.forge.shared.protocol.HapticAckDto
import com.forge.shared.protocol.TimerCommand
import com.forge.shared.protocol.WearCodec
import com.forge.shared.protocol.WearProtocol
import com.google.android.gms.wearable.MessageEvent
import com.google.android.gms.wearable.WearableListenerService
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.runBlocking
import javax.inject.Inject

/**
 * The phone's ear for wrist commands (W1). A WearableListenerService wakes the process even when
 * the app was killed — ForgeApp.onCreate has already re-seeded the Program facade by the time a
 * message lands, so commands act on real state. Every command is commandId-deduped and answered
 * with a /cmd/ack DataItem; an unknown-newer payload is dropped (the wrist shows "update").
 *
 * W1 handles the timer commands + the haptic handoff; /cmd/log-set and /cmd/undo-set route
 * through SetLogUseCase (W2).
 */
@AndroidEntryPoint
class WearSyncService : WearableListenerService() {

    @Inject lateinit var timerHolder: SessionTimerHolder
    @Inject lateinit var wearConnection: WearConnection
    @Inject lateinit var deduper: CommandDeduper
    @Inject lateinit var publisher: WearStatePublisher
    @Inject lateinit var commandHandler: WearCommandHandler
    @Inject lateinit var hrIngest: WearHrIngest
    @Inject lateinit var clock: Clock

    override fun onMessageReceived(event: MessageEvent) {
        // The Data Layer delivers on a binder thread and keeps the service alive for the handler's
        // duration; the work here is a few Room writes / a DataItem put, so a bounded runBlocking
        // is the honest shape (returning early would let the process die mid-write).
        when (event.path) {
            WearProtocol.PATH_HAPTIC_ACK -> {
                val ack = WearCodec.decode<HapticAckDto>(event.data)
                if (ack is WearCodec.DecodeResult.Ok) wearConnection.recordHapticAck(clock.nowMs())
            }
            WearProtocol.PATH_CMD_TIMER -> runBlocking {
                when (val cmd = WearCodec.decode<TimerCommand>(event.data)) {
                    is WearCodec.DecodeResult.Ok -> handleTimer(cmd.value)
                    else -> Unit // NewerVersion/Invalid: dropped, the wrist surfaces its own state.
                }
            }
            WearProtocol.PATH_CMD_LOG_SET -> runBlocking { commandHandler.handleLogSet(event.data) }
            WearProtocol.PATH_CMD_UNDO_SET -> runBlocking { commandHandler.handleUndoSet(event.data) }
            WearProtocol.PATH_CMD_SET_RPE -> runBlocking { commandHandler.handleSetRpe(event.data) }
            WearProtocol.PATH_HR_BATCH -> runBlocking { hrIngest.handleBatch(event.data) }
            else -> super.onMessageReceived(event)
        }
    }

    private suspend fun handleTimer(cmd: TimerCommand) {
        if (!deduper.isNew(cmd.commandId)) return
        when (cmd.action) {
            TimerCommand.Action.SKIP -> timerHolder.controller.stop()
            TimerCommand.Action.ADD_30 -> timerHolder.controller.addSeconds(30)
            TimerCommand.Action.START -> timerHolder.controller.start()
        }
        publisher.publishAck(CmdAckDto(commandId = cmd.commandId, ok = true, atMs = clock.nowMs()))
    }
}
