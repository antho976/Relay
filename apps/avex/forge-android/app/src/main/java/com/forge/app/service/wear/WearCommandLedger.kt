package com.forge.app.service.wear

import android.content.Context
import androidx.room.withTransaction
import com.forge.app.core.time.Clock
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.entities.WearCommand
import com.forge.shared.protocol.CmdAckDto
import com.forge.shared.protocol.WearCodec
import dagger.hilt.android.qualifiers.ApplicationContext
import java.io.File
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.serialization.json.*

/** Room serializes the effect and outcome together; publication happens only after commit. */
@Singleton
class WearCommandLedger(
    private val db: ForgeDatabase,
    private val file: File,
    private val clock: Clock
) {
    @Inject constructor(db: ForgeDatabase, @ApplicationContext context: Context, clock: Clock) :
        this(db, File(context.filesDir, FILE_NAME), clock)

    suspend fun run(
        commandId: String,
        publish: suspend (CmdAckDto) -> Unit,
        afterCommit: suspend (CmdAckDto) -> Unit = {},
        effect: suspend () -> CmdAckDto
    ) {
        val (ack, executed) = db.withTransaction {
            val stored = db.wearCommandDao().get(commandId)
            if (stored != null) return@withTransaction decodeAck(stored.ackJson) to false
            val legacy = legacyAck(commandId)
            val outcome = legacy ?: effect()
            db.wearCommandDao().insert(WearCommand(commandId,
                WearCodec.encode(outcome).decodeToString(), clock.nowMs()))
            outcome to (legacy == null)
        }
        // Timer/transport failures cannot roll back the committed mutation or make it retryable.
        try {
            if (executed) afterCommit(ack)
        } finally {
            publish(ack)
        }
    }

    private fun decodeAck(json: String): CmdAckDto =
        when (val decoded = WearCodec.decode<CmdAckDto>(json.encodeToByteArray())) {
            is WearCodec.DecodeResult.Ok -> decoded.value
            else -> error("Stored watch outcome cannot be decoded")
        }

    private fun legacyAck(id: String): CmdAckDto? = legacyAcks[id]

    /**
     * The pre-Room ledger's outcomes, read once per process. Nothing writes that file any more, so
     * reading and re-parsing all of it inside every command's transaction bought nothing.
     */
    private val legacyAcks: Map<String, CmdAckDto> by lazy {
        if (!file.isFile) return@lazy emptyMap()
        runCatching {
            val acks = LinkedHashMap<String, CmdAckDto>()
            WearCodec.json.parseToJsonElement(file.readText()).jsonObject["entries"]?.jsonArray?.forEach { entry ->
                runCatching {
                    val id = entry.jsonObject["commandId"]?.jsonPrimitive?.contentOrNull
                    val ack = entry.jsonObject["ack"]?.let { decodeAck(it.toString()) }
                    if (id != null && ack != null) acks.putIfAbsent(id, ack)
                }
            }
            acks
        }.getOrDefault(emptyMap())
    }

    companion object { const val FILE_NAME = "wear_command_ledger.json" }
}
