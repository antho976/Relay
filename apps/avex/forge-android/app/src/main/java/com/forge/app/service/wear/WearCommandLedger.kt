package com.forge.app.service.wear

import android.content.Context
import com.forge.app.core.time.Clock
import com.forge.shared.protocol.CmdAckDto
import com.forge.shared.protocol.WearCodec
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.longOrNull
import kotlinx.serialization.json.put
import java.io.File
import java.io.FileOutputStream
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Command idempotency (W1 protocol rule), made durable: every wrist command carries a UUID, and a
 * re-send after a BT flap, a double-tap, or a phone process death must never run twice — AND must
 * always be answered, so the wrist can learn what happened to the first attempt.
 *
 * The previous deduper was an in-memory set that recorded the id BEFORE the command ran. Two
 * failure modes followed: the process dying between the write and the ack forgot the id, so the
 * watch's same-id retry logged the set a second time; and an ack put that failed (or an exception
 * mid-command) left the id marked, so every retry was dropped in silence and the wrist sat at
 * "Not logged" for a set that was in the database.
 *
 * This ledger is keyed by command id and holds each command's status and, once it has run, the
 * exact ack it produced:
 *
 *  - [run] marks the id in flight, runs the effect, records `done + ack` ONLY after the effect
 *    returned, then publishes. A crash or exception before the record leaves nothing that blocks a
 *    retry; a crash after it makes the retry a replay of the recorded ack rather than a re-run.
 *  - A duplicate whose entry is done never re-executes: its recorded ack is published again,
 *    byte-for-byte, so an ack the watch missed is delivered on the next attempt.
 *  - A duplicate delivered while the SAME process is still executing the id is dropped; the
 *    original delivery's ack covers it.
 *
 * Persistence is a small JSON file in the app's files dir — not Room, because a schema change needs
 * the Room compiler to regenerate the schema hash (see docs/AUDIT_DEFERRED.md). Every write goes
 * to a sibling `.tmp` and is renamed in, so a half-written ledger can never be read back; a file
 * that cannot be parsed starts the ledger empty rather than failing. The in-memory map is the fast
 * path and is loaded from the file once, lazily.
 *
 * What this does NOT close: the record is written after the Room mutation commits, not inside its
 * transaction, so a process death in the few milliseconds between the commit and the ledger write
 * still re-runs the command on retry. Closing that window needs the outcome in a `wear_command`
 * table written in the same transaction — a migration — and is left for a pass with a compiler.
 */
@Singleton
class WearCommandLedger(
    private val file: File,
    private val clock: Clock
) {
    @Inject
    constructor(@ApplicationContext context: Context, clock: Clock) :
        this(File(context.filesDir, FILE_NAME), clock)

    enum class Status { IN_FLIGHT, DONE }

    private class Entry(
        val commandId: String,
        val startedAtMs: Long,
        val doneAtMs: Long?,
        /** Non-null exactly when the command has run to completion. */
        val ack: CmdAckDto?
    )

    private sealed interface Claim {
        /** Already done — publish this again and do nothing else. */
        class Replay(val ack: CmdAckDto) : Claim
        /** This process is executing the same id right now. */
        data object Busy : Claim
        /** First sight of the id (or a retry of one that never completed). */
        data object Run : Claim
    }

    private val lock = Any()
    private var loaded = false

    /** Insertion-ordered, oldest first; bounded by [MAX_ENTRIES] via [removeEldestEntry]. */
    private val entries = object : LinkedHashMap<String, Entry>(64, 0.75f, false) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, Entry>?): Boolean =
            size > MAX_ENTRIES
    }

    /** Ids this PROCESS is executing — never persisted, because a previous run's "in flight" says
     *  nothing about whether the effect landed and must not block the retry that finds out. */
    private val inFlight = HashSet<String>()

    /**
     * Execute [effect] once for [commandId] and publish the ack it returns; on any later call with
     * the same id, publish the recorded ack instead. An exception from [effect] propagates after
     * the in-flight mark is cleared, so the next same-id delivery runs it again.
     */
    suspend fun run(
        commandId: String,
        publish: suspend (CmdAckDto) -> Unit,
        effect: suspend () -> CmdAckDto
    ) {
        when (val claim = claim(commandId)) {
            is Claim.Replay -> { publish(claim.ack); return }
            Claim.Busy -> return
            Claim.Run -> Unit
        }
        val ack = try {
            effect()
        } catch (t: Throwable) {
            release(commandId)
            throw t
        }
        // Recorded BEFORE it is published: a crash between the two replays on retry, where the
        // other order would re-run the command.
        complete(commandId, ack)
        publish(ack)
    }

    /** What the ledger has recorded for [commandId], or null if it has never seen it. */
    fun status(commandId: String): Status? = synchronized(lock) {
        ensureLoaded()
        val entry = entries[commandId] ?: return null
        if (entry.ack != null) Status.DONE else Status.IN_FLIGHT
    }

    /** The ack recorded for a completed [commandId], or null if it has not completed. */
    fun recordedAck(commandId: String): CmdAckDto? = synchronized(lock) {
        ensureLoaded()
        entries[commandId]?.ack
    }

    private fun claim(commandId: String): Claim = synchronized(lock) {
        ensureLoaded()
        val recorded = entries[commandId]?.ack
        when {
            recorded != null -> Claim.Replay(recorded)
            commandId in inFlight -> Claim.Busy
            else -> {
                inFlight += commandId
                entries[commandId] = Entry(commandId, startedAtMs = clock.nowMs(), doneAtMs = null, ack = null)
                persistLocked()
                Claim.Run
            }
        }
    }

    private fun release(commandId: String) = synchronized(lock) {
        // The file keeps its in-flight row: it records that an attempt happened, and does not
        // block the retry, which is exactly the state this command is in.
        inFlight -= commandId
    }

    private fun complete(commandId: String, ack: CmdAckDto) = synchronized(lock) {
        inFlight -= commandId
        val startedAtMs = entries.remove(commandId)?.startedAtMs ?: clock.nowMs()
        // Re-inserted so a completed command is the NEWEST entry and outlives its in-flight slot.
        entries[commandId] = Entry(commandId, startedAtMs, doneAtMs = clock.nowMs(), ack = ack)
        persistLocked()
    }

    // ── Persistence ────────────────────────────────────────────────────────────────────────────

    private fun ensureLoaded() {
        if (loaded) return
        loaded = true
        runCatching {
            if (!file.exists()) return
            val root = WearCodec.json.parseToJsonElement(file.readText()).jsonObject
            val list = root[KEY_ENTRIES]?.jsonArray ?: return
            for (element in list) {
                val obj = element as? JsonObject ?: continue
                val id = obj[KEY_COMMAND_ID]?.jsonPrimitive?.contentOrNull ?: continue
                entries[id] = Entry(
                    commandId = id,
                    startedAtMs = obj[KEY_STARTED_AT]?.jsonPrimitive?.longOrNull ?: 0L,
                    doneAtMs = obj[KEY_DONE_AT]?.jsonPrimitive?.longOrNull,
                    ack = obj[KEY_ACK]?.let { decodeAck(it) }
                )
            }
        }
    }

    private fun decodeAck(element: JsonElement): CmdAckDto? =
        when (val decoded = WearCodec.decode<CmdAckDto>(element.toString().encodeToByteArray())) {
            is WearCodec.DecodeResult.Ok -> decoded.value
            else -> null
        }

    /**
     * Write the whole ledger: `{"v":1,"entries":[{commandId,status,startedAtMs,doneAtMs?,ack?}…]}`,
     * oldest first. Fail-soft — a write that cannot land leaves the in-memory ledger authoritative
     * for this process, which is no worse than before.
     */
    private fun persistLocked() {
        runCatching {
            val json = buildJsonObject {
                put(KEY_VERSION, FORMAT_VERSION)
                put(KEY_ENTRIES, buildJsonArray {
                    for (entry in entries.values) {
                        add(buildJsonObject {
                            val ack = entry.ack
                            put(KEY_COMMAND_ID, entry.commandId)
                            put(KEY_STATUS, if (ack != null) STATUS_DONE else STATUS_IN_FLIGHT)
                            put(KEY_STARTED_AT, entry.startedAtMs)
                            if (entry.doneAtMs != null) put(KEY_DONE_AT, entry.doneAtMs)
                            if (ack != null) {
                                put(KEY_ACK, WearCodec.json.parseToJsonElement(WearCodec.encode(ack).decodeToString()))
                            }
                        })
                    }
                })
            }
            file.parentFile?.mkdirs()
            val tmp = File(file.parentFile, file.name + TMP_SUFFIX)
            FileOutputStream(tmp).use { out ->
                out.write(json.toString().encodeToByteArray())
                out.fd.sync()
            }
            if (!tmp.renameTo(file)) {
                file.delete()
                if (!tmp.renameTo(file)) tmp.delete()
            }
        }
    }

    companion object {
        const val FILE_NAME = "wear_command_ledger.json"
        /** Completed commands remembered — far beyond any plausible retry window. */
        const val MAX_ENTRIES = 200

        private const val TMP_SUFFIX = ".tmp"
        private const val FORMAT_VERSION = 1
        private const val KEY_VERSION = "v"
        private const val KEY_ENTRIES = "entries"
        private const val KEY_COMMAND_ID = "commandId"
        private const val KEY_STATUS = "status"
        private const val KEY_STARTED_AT = "startedAtMs"
        private const val KEY_DONE_AT = "doneAtMs"
        private const val KEY_ACK = "ack"
        private const val STATUS_IN_FLIGHT = "in_flight"
        private const val STATUS_DONE = "done"
    }
}
