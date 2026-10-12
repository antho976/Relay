package com.tally.app.data.sync

import android.os.Build
import com.tally.app.data.prefs.SettingsRepository
import com.tally.link.PairLink
import com.tally.link.RelayClient
import com.tally.link.RelayFailure
import com.tally.link.RelaySession
import com.tally.link.RelayWire
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.longOrNull
import kotlinx.serialization.json.put
import javax.inject.Inject
import javax.inject.Singleton

/** How a pairing or a sync ended. */
sealed interface SyncOutcome {
    data object NotPaired : SyncOutcome
    data class Done(val sent: Int, val received: Int) : SyncOutcome
    data class Failed(val reason: String) : SyncOutcome
}

/**
 * This phone and Relay holding one ledger (RelayV4 docs/MONEY.md, "Sync"). Tally syncs with the
 * cloud copy of Relay on the owner's server, which the PC's own Relay syncs with in turn, so the
 * phone, the server and the PC converge wherever the phone is. [CloudConnect] gets this phone paired
 * without a shell; each sync after that is one connection proving the phone's key, one `money.sync`
 * request carrying what changed here, and the server's changes merged in.
 *
 * The first sync MERGES: it sends everything here and takes everything there, newest edit winning
 * per row, so a new install never empties the server's ledger and a phone with entries never loses
 * them. Only the person replaces the server's copy with this phone's ([SyncStore.requestReplace]),
 * or takes the server's whole ([takeServers]).
 *
 * One sync at a time. Failure is quiet: it is recorded for the status line, never thrown.
 */
@Singleton
class RelaySync @Inject constructor(
    private val store: SyncStore,
    private val ledger: LedgerSync,
    private val settings: SettingsRepository,
    private val client: RelayClient,
) {
    private val mutex = Mutex()
    private val _busy = MutableStateFlow(false)

    /** A pairing or a sync is talking to the server right now. */
    val busy: StateFlow<Boolean> = _busy.asStateFlow()

    /** When the last run ended, in epoch millis: the ledger writes it made are not local changes. */
    @Volatile var lastFinishedAt: Long = 0L
        private set

    /** The name the server lists this phone under, and the device its syncs are recorded for. */
    val deviceName: String
        get() = "Tally on " + (Build.MODEL?.trim()?.takeIf { it.isNotEmpty() } ?: "a phone")

    /** Pairs with the door behind [link] and runs the first sync on the same connection. */
    suspend fun pair(link: PairLink): SyncOutcome = run {
        val session = try {
            client.connect(
                link.routes,
                hello = { RelayWire.pairHello(link.code, deviceName.take(64)) },
                welcomeTimeoutMs = RelayClient.PAIR_WELCOME_TIMEOUT_MS,
                oneShot = true,
            )
        } catch (e: RelayFailure) {
            return@run SyncOutcome.Failed(e.reason)
        }
        session.use {
            val token = it.welcome.token
            val device = it.welcome.device
            if (token.isNullOrBlank() || device.isNullOrBlank()) {
                return@run SyncOutcome.Failed("Your server let this phone in without a key. Connect again.")
            }
            store.pair(
                PairedPc(
                    name = it.greeting.host.ifBlank { link.host },
                    hostId = it.greeting.hostId.ifBlank { link.hostId },
                    instance = it.greeting.instance.ifBlank { link.instance },
                    routes = link.routes,
                    deviceId = device,
                    token = token,
                    pairedAt = System.currentTimeMillis(),
                )
            )
            exchange(it, store.current())
        }
    }

    /** Syncs with the server; [SyncOutcome.NotPaired] when this phone is not connected. */
    suspend fun syncNow(): SyncOutcome = run {
        val prefs = store.current()
        val pc = prefs.pc ?: return@run SyncOutcome.NotPaired
        val session = try {
            open(pc)
        } catch (e: RelayFailure) {
            store.recordFailure(System.currentTimeMillis(), e.reason, unpaired = e.unpaired)
            return@run SyncOutcome.Failed(e.reason)
        }
        session.use {
            if (it.greeting.host.isNotBlank() && it.greeting.host != pc.name) store.rename(it.greeting.host)
            exchange(it, prefs)
        }
    }

    /**
     * Replaces this phone's ledger with the server's whole one: what the person asks for when the
     * server's copy is the one to keep (a backup restored on the PC, a phone that went wrong).
     */
    suspend fun takeServers(): SyncOutcome = run {
        val pc = store.current().pc ?: return@run SyncOutcome.NotPaired
        val session = try {
            open(pc)
        } catch (e: RelayFailure) {
            store.recordFailure(System.currentTimeMillis(), e.reason, unpaired = e.unpaired)
            return@run SyncOutcome.Failed(e.reason)
        }
        session.use {
            try {
                takeWhole(it, TOOK_SERVER_LEDGER_ASKED)
            } catch (e: CancellationException) {
                throw e
            } catch (e: RelayFailure) {
                store.recordFailure(System.currentTimeMillis(), e.reason)
                SyncOutcome.Failed(e.reason)
            } catch (e: Exception) {
                store.recordFailure(System.currentTimeMillis(), NOT_FINISHED)
                SyncOutcome.Failed(NOT_FINISHED)
            }
        }
    }

    /**
     * Forgets the server and hands back what was kept, for Undo. Its copy of the ledger stays where
     * it is; this phone just stops syncing with it.
     */
    suspend fun forget(): SyncPrefs = mutex.withLock {
        val was = store.current()
        store.forget()
        was
    }

    suspend fun restore(prefs: SyncPrefs) = mutex.withLock { store.restore(prefs) }

    private suspend fun open(pc: PairedPc): RelaySession =
        client.connect(pc.routes, hello = { g -> RelayWire.proofHello(pc.deviceId, g.challenge, pc.token) })

    private suspend fun run(block: suspend () -> SyncOutcome): SyncOutcome = mutex.withLock {
        _busy.value = true
        try {
            block()
        } finally {
            lastFinishedAt = System.currentTimeMillis()
            _busy.value = false
        }
    }

    /**
     * One exchange on an admitted connection: send, receive, merge, remember where it stands.
     * The watermark is taken before anything is read, so an edit made while the sync runs is sent
     * next time rather than lost. The first sync, and the first after an update that reads new
     * tables, asks from cursor 0: it is the merge, newest stamp winning, that leaves every row
     * that is already the same as it is.
     */
    private suspend fun exchange(session: RelaySession, prefs: SyncPrefs): SyncOutcome {
        val started = System.currentTimeMillis()
        return try {
            val first = !prefs.replaced
            val replace = prefs.replaceNext
            val since = if (first || replace) null else prefs.watermark
            val rows = ledger.collect(since)
            val local = settingsChanges(since) + rows
            var cursor = if (first || replace || prefs.catchingUp) 0L else prefs.cursor
            // A merge or a replace starts a new relationship with the server's ledger: no generation.
            var generation = if (first || replace) null else prefs.generation
            val received = ArrayList<Change>()
            // Batched, so a ledger of years stays under the door's message size. Only the first
            // batch replaces; the rest add to what it put there. Tables stay in order across them.
            val batches = local.chunked(BATCH).ifEmpty { listOf(emptyList()) }
            for ((i, batch) in batches.withIndex()) {
                val result = try {
                    session.request("money.sync", payload(replace && i == 0, cursor, generation, batch))
                } catch (e: RelayFailure) {
                    // The server's ledger was restored, erased or replaced since: take it whole.
                    if (e.code == STALE) return takeWhole(session, TOOK_SERVER_LEDGER)
                    throw e
                }
                cursor = result.cursor()
                generation = result.generation() ?: generation
                (result["changes"] as? JsonArray)?.forEach { c -> Change.fromJson(c)?.let { received += it } }
            }
            val (ledgerRows, settingRows) = received.partition { it.table != SyncTables.SETTINGS }
            val applied = ledger.apply(ledgerRows)
            // A phone with nothing in it yet takes the server's settings whatever their stamps:
            // its own are only the defaults it was installed with.
            val taken = applySettings(settingRows, whole = first && rows.isEmpty())
            // The server has every tombstone from before this sync began.
            ledger.pruneTombstones(started)
            store.recordSuccess(System.currentTimeMillis(), cursor, generation, started, local.size, applied.applied + taken)
            SyncOutcome.Done(sent = local.size, received = applied.applied + taken)
        } catch (e: CancellationException) {
            throw e
        } catch (e: RelayFailure) {
            store.recordFailure(System.currentTimeMillis(), e.reason)
            SyncOutcome.Failed(e.reason)
        } catch (e: Exception) {
            store.recordFailure(System.currentTimeMillis(), NOT_FINISHED)
            SyncOutcome.Failed(NOT_FINISHED)
        }
    }

    /**
     * Takes the server's ledger whole, keeping its stamps, and merges from there: after the server
     * refused to merge (`money.sync_stale`: its ledger was restored, erased or replaced, or its file
     * was lost), or because the person asked. What changed here since the last sync is not sent:
     * the latest wholesale act wins.
     */
    private suspend fun takeWhole(session: RelaySession, note: String): SyncOutcome {
        val result = session.request("money.sync", payload(replace = false, since = 0L, generation = null, changes = emptyList()))
        val cursor = result.cursor()
        val generation = result.generation()
        val received = (result["changes"] as? JsonArray)?.mapNotNull { Change.fromJson(it) }.orEmpty()
        val (rows, settingRows) = received.partition { it.table != SyncTables.SETTINGS }
        val applied = ledger.replaceWith(rows)
        val taken = applySettings(settingRows, whole = true)
        val now = System.currentTimeMillis()
        store.recordSuccess(now, cursor, generation, now, 0, applied.applied + taken, note = note)
        return SyncOutcome.Done(sent = 0, received = applied.applied + taken)
    }

    private fun payload(replace: Boolean, since: Long, generation: String?, changes: List<Change>) = buildJsonObject {
        put("device", deviceName.take(64))
        put("replace", replace)
        put("since", since)
        if (generation != null) put("generation", generation)
        put("changes", JsonArray(changes.map { it.toJson() }))
    }

    private fun JsonElement?.cursor(): Long =
        ((this as? JsonObject)?.get("cursor") as? JsonPrimitive)?.longOrNull ?: throw RelayFailure("Your server's answer did not read")

    private fun JsonElement?.generation(): String? =
        ((this as? JsonObject)?.get("generation") as? JsonPrimitive)?.contentOrNull

    private operator fun JsonElement?.get(key: String): JsonElement? = (this as? JsonObject)?.get(key)

    /** The server's settings; [whole] takes them whatever their stamps. */
    private suspend fun applySettings(rows: List<Change>, whole: Boolean): Int = rows.count { c ->
        val value = (c.row["value"] as? JsonPrimitive)?.contentOrNull
        !c.deleted && value != null && settings.applySynced(c.uid, value, c.updatedAt, force = whole)
    }

    /** The synced settings changed since [since] (all of them on a first sync), as Relay keeps them. */
    private suspend fun settingsChanges(since: Long?): List<Change> =
        settings.synced().all
            .filter { since == null || it.updatedAt >= since }
            // A setting never changed here still has a value; the server needs a time above 0 to take it.
            .map { s -> Change(SyncTables.SETTINGS, s.key, maxOf(s.updatedAt, 1L), row = buildJsonObject { put("value", s.value) }) }

    companion object {
        /** Changes per request: a few hundred kilobytes, far under the door's 8 MiB message. */
        const val BATCH = 1_500

        /** The server's ledger is not the one this phone last merged with (docs/MONEY.md, "Generation"). */
        const val STALE = "money.sync_stale"

        const val NOT_FINISHED = "The sync did not finish. Nothing here was changed."

        /** The status note after the server's ledger replaced this phone's on its own. */
        const val TOOK_SERVER_LEDGER = "Your server's ledger had been replaced, so this phone took it"

        const val TOOK_SERVER_LEDGER_ASKED = "This phone now holds your server's ledger"
    }
}
