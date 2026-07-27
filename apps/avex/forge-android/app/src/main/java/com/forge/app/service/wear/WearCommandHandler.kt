package com.forge.app.service.wear

import com.forge.app.core.time.Clock
import com.forge.shared.protocol.CmdAckDto
import com.forge.shared.protocol.LogSetCommand
import com.forge.shared.protocol.SetRpeCommand
import com.forge.shared.protocol.UndoSetCommand
import com.forge.shared.protocol.WearCodec
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Routes wrist write commands through the phone's real write paths (W2). Split out of
 * [WearSyncService] so the command handling is a plain injectable unit the tests can drive with
 * fake message bytes — the single-write-path invariant is asserted here, not in a service.
 */
@Singleton
class WearCommandHandler @Inject constructor(
    private val deduper: CommandDeduper,
    private val publisher: WearStatePublisher,
    private val setLog: com.forge.app.domain.session.SetLogUseCase,
    private val clock: Clock
) {
    suspend fun handleLogSet(bytes: ByteArray) {
        val cmd = when (val d = WearCodec.decode<LogSetCommand>(bytes)) {
            is WearCodec.DecodeResult.Ok -> d.value
            else -> return // NewerVersion/Invalid: dropped; the wrist surfaces its own state.
        }
        if (!deduper.isNew(cmd.commandId)) return
        val result = setLog.logFromWatch(cmd)
        publisher.publishAck(
            CmdAckDto(
                commandId = cmd.commandId,
                ok = result.ok,
                reason = result.reason,
                pr = result.wasPr,
                needsConfirm = result.needsConfirm,
                setId = result.setId,
                atMs = clock.nowMs()
            )
        )
    }

    suspend fun handleSetRpe(bytes: ByteArray) {
        val cmd = when (val d = WearCodec.decode<SetRpeCommand>(bytes)) {
            is WearCodec.DecodeResult.Ok -> d.value
            else -> return
        }
        if (!deduper.isNew(cmd.commandId)) return
        val result = setLog.rpeFromWatch(cmd.setId, cmd.rpe)
        publisher.publishAck(
            CmdAckDto(
                commandId = cmd.commandId,
                ok = result.ok,
                reason = result.reason,
                setId = result.setId,
                atMs = clock.nowMs()
            )
        )
    }

    suspend fun handleUndoSet(bytes: ByteArray) {
        val cmd = when (val d = WearCodec.decode<UndoSetCommand>(bytes)) {
            is WearCodec.DecodeResult.Ok -> d.value
            else -> return
        }
        if (!deduper.isNew(cmd.commandId)) return
        val result = setLog.undoLastFromWatch(cmd.sessionId)
        publisher.publishAck(
            CmdAckDto(
                commandId = cmd.commandId,
                ok = result.ok,
                reason = result.reason,
                atMs = clock.nowMs()
            )
        )
    }
}
