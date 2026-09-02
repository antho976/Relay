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
    private val ledger: WearCommandLedger,
    private val publisher: WearStatePublisher,
    private val setLog: com.forge.app.domain.session.SetLogUseCase,
    private val clock: Clock
) {
    /**
     * Every mutating command goes through the ledger: it runs [effect] once per command id, records
     * the ack it produced, and answers any same-id retry by publishing that recorded ack again —
     * so a set logged just before the phone process died is neither logged twice nor left
     * unacknowledged on the wrist.
     */
    private suspend fun runOnce(commandId: String, effect: suspend () -> CmdAckDto) =
        ledger.run(commandId, publish = { publisher.publishAck(it) }, effect = effect)

    suspend fun handleLogSet(bytes: ByteArray) {
        val cmd = when (val d = WearCodec.decode<LogSetCommand>(bytes)) {
            is WearCodec.DecodeResult.Ok -> d.value
            WearCodec.DecodeResult.NewerVersion -> return refuseNewerVersion(bytes)
            else -> return // Invalid: corrupt bytes, nothing to ack against.
        }
        runOnce(cmd.commandId) {
            val result = setLog.logFromWatch(cmd)
            CmdAckDto(
                commandId = cmd.commandId,
                ok = result.ok,
                reason = result.reason,
                pr = result.wasPr,
                needsConfirm = result.needsConfirm,
                setId = result.setId,
                atMs = clock.nowMs(),
                kind = CmdAckDto.KIND_LOG_SET
            )
        }
    }

    suspend fun handleSetRpe(bytes: ByteArray) {
        val cmd = when (val d = WearCodec.decode<SetRpeCommand>(bytes)) {
            is WearCodec.DecodeResult.Ok -> d.value
            WearCodec.DecodeResult.NewerVersion -> return refuseNewerVersion(bytes)
            else -> return
        }
        runOnce(cmd.commandId) {
            val result = setLog.rpeFromWatch(cmd.setId, cmd.rpe)
            CmdAckDto(
                commandId = cmd.commandId,
                ok = result.ok,
                reason = result.reason,
                setId = result.setId,
                atMs = clock.nowMs(),
                // Named explicitly: this ack's setId is the set that was RATED, not one just
                // logged. Without the kind the wrist could only see "an ok ack with a setId" and
                // re-armed its undo/rate row on the strength of it.
                kind = CmdAckDto.KIND_SET_RPE
            )
        }
    }

    /**
     * Tell the wrist its command was understood-but-refused because THIS side is out of date.
     *
     * Version handling used to be one-directional: the watch has an UpdateScreen for a newer phone,
     * but the phone dropped a newer watch's command with a bare `return` — before publishing any
     * ack. Wear apps update on their own Play schedule and routinely land ahead of the phone build,
     * so the wrist sat at "LOGGING…", timed out to "Not logged · reconnecting", and invited a
     * re-tap into the duplicate loop — every attempt guaranteed to fail, with no stated cause and no
     * path to a fix.
     *
     * The body can't be decoded, but [WearCodec.probeCommandId] reads the id out of the raw JSON,
     * which is all an ack needs.
     */
    private suspend fun refuseNewerVersion(bytes: ByteArray) {
        val commandId = WearCodec.probeCommandId(bytes) ?: return
        publisher.publishAck(
            CmdAckDto(
                commandId = commandId,
                ok = false,
                reason = "Update Avex on your phone",
                atMs = clock.nowMs()
            )
        )
    }

    suspend fun handleUndoSet(bytes: ByteArray) {
        val cmd = when (val d = WearCodec.decode<UndoSetCommand>(bytes)) {
            is WearCodec.DecodeResult.Ok -> d.value
            WearCodec.DecodeResult.NewerVersion -> return refuseNewerVersion(bytes)
            else -> return
        }
        runOnce(cmd.commandId) {
            val result = setLog.undoLastFromWatch(cmd.sessionId, cmd.setId)
            CmdAckDto(
                commandId = cmd.commandId,
                ok = result.ok,
                reason = result.reason,
                atMs = clock.nowMs(),
                kind = CmdAckDto.KIND_UNDO_SET
            )
        }
    }
}
