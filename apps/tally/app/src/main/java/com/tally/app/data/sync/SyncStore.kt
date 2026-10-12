package com.tally.app.data.sync

import android.content.Context
import androidx.compose.runtime.Immutable
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.MutablePreferences
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.tally.link.Enrollment
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import java.io.IOException
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The Relay door this phone is paired with (the cloud copy on the owner's server), and the
 * credential it gave. Named for the PC it once was; the wire is the same.
 */
@Immutable
data class PairedPc(
    val name: String,
    val hostId: String,
    val instance: String,
    /** Where to reach it, in the order tried: direct addresses, then its rendezvous. */
    val routes: List<String>,
    val deviceId: String,
    /** The phone's token. Never shown and never sent again: each connection sends a proof. */
    val token: String,
    val pairedAt: Long,
)

/** Where sync with the server stands. Everything but [pc] and the person's choices is about the last run. */
@Immutable
data class SyncPrefs(
    val pc: PairedPc? = null,
    /** The server's cursor from the last sync, sent back as `since`. */
    val cursor: Long = 0L,
    /** Which of the server's ledgers that cursor counts in; it changes when the server's is replaced. */
    val generation: String? = null,
    /** When the last successful sync started: rows stamped from then on are sent next time. */
    val watermark: Long = 0L,
    /** The first sync, which merges this phone's ledger with the server's, has been done. */
    val replaced: Boolean = false,
    val lastAttemptAt: Long = 0L,
    val lastSuccessAt: Long = 0L,
    /** Why the last run failed, in words; null when it worked. */
    val lastError: String? = null,
    val lastSent: Int = 0,
    val lastReceived: Int = 0,
    /** News from the last syncs worth a line of its own (the phone took the server's ledger). */
    val note: String? = null,
    /**
     * The tables this phone could read when it last synced ([SyncStore.LEDGER_VERSION] then).
     * Below the current one, the next sync asks the PC for everything (`since` 0), so the rows the
     * PC sent while this phone could not read them arrive.
     */
    val ledgerVersion: Int = 0,
    /** The server's address when the person changed it; null is the one this build was made for. */
    val server: String? = null,
    /** A request to connect that is waiting on the owner's Allow, kept so a restart resumes it. */
    val pending: Enrollment? = null,
    /** The person chose to keep this phone's ledger on this phone only. */
    val phoneOnly: Boolean = false,
    /** The next sync replaces the server's ledger with this phone's, as the person asked. */
    val replaceNext: Boolean = false,
    /** The server said it no longer knows this phone: it has to connect again. */
    val unpaired: Boolean = false,
) {
    val paired: Boolean get() = pc != null

    /** The next sync asks for every row the PC has, not only those since [cursor]. */
    val catchingUp: Boolean get() = replaced && ledgerVersion < SyncStore.LEDGER_VERSION
}

/**
 * Kept in its own private DataStore file, apart from the settings, and left out of every backup
 * (res/xml): the token is this phone's key to the server and stays on this phone.
 */
private val Context.syncStore: DataStore<Preferences> by preferencesDataStore(name = "relay_sync")

@Singleton
class SyncStore @Inject constructor(@ApplicationContext context: Context) {

    private val store = context.syncStore

    private object Keys {
        val name = stringPreferencesKey("pc_name")
        val hostId = stringPreferencesKey("pc_host_id")
        val instance = stringPreferencesKey("pc_instance")
        val routes = stringPreferencesKey("pc_routes")
        val deviceId = stringPreferencesKey("device_id")
        val token = stringPreferencesKey("device_token")
        val pairedAt = longPreferencesKey("paired_at")
        val cursor = longPreferencesKey("cursor")
        val generation = stringPreferencesKey("generation")
        val note = stringPreferencesKey("note")
        val watermark = longPreferencesKey("watermark")
        val replaced = booleanPreferencesKey("replaced")
        val lastAttemptAt = longPreferencesKey("last_attempt_at")
        val lastSuccessAt = longPreferencesKey("last_success_at")
        val lastError = stringPreferencesKey("last_error")
        val lastSent = intPreferencesKey("last_sent")
        val lastReceived = intPreferencesKey("last_received")
        val ledgerVersion = intPreferencesKey("ledger_version")
        val server = stringPreferencesKey("server")
        val phoneOnly = booleanPreferencesKey("phone_only")
        val replaceNext = booleanPreferencesKey("replace_next")
        val unpaired = booleanPreferencesKey("unpaired")
        val pendingServer = stringPreferencesKey("pending_server")
        val pendingId = stringPreferencesKey("pending_id")
        val pendingVerifier = stringPreferencesKey("pending_verifier")
        val pendingCode = stringPreferencesKey("pending_code")
        val pendingExpires = longPreferencesKey("pending_expires")
        val pendingNotified = booleanPreferencesKey("pending_notified")
    }

    /** What survives a new pairing and a forgotten one: where the server is, and the person's choice. */
    private val keep: Set<String> = setOf(Keys.server.name, Keys.phoneOnly.name)

    val prefs: Flow<SyncPrefs> = store.data
        .catch { if (it is IOException) emit(emptyPreferences()) else throw it }
        .map { p ->
            val token = p[Keys.token]
            val device = p[Keys.deviceId]
            SyncPrefs(
                pc = if (token != null && device != null) {
                    PairedPc(
                        name = p[Keys.name] ?: "Relay PC",
                        hostId = p[Keys.hostId].orEmpty(),
                        instance = p[Keys.instance].orEmpty(),
                        routes = p[Keys.routes].orEmpty().split('\n').filter { it.isNotBlank() },
                        deviceId = device,
                        token = token,
                        pairedAt = p[Keys.pairedAt] ?: 0L,
                    )
                } else {
                    null
                },
                cursor = p[Keys.cursor] ?: 0L,
                generation = p[Keys.generation],
                note = p[Keys.note],
                watermark = p[Keys.watermark] ?: 0L,
                replaced = p[Keys.replaced] ?: false,
                lastAttemptAt = p[Keys.lastAttemptAt] ?: 0L,
                lastSuccessAt = p[Keys.lastSuccessAt] ?: 0L,
                lastError = p[Keys.lastError],
                lastSent = p[Keys.lastSent] ?: 0,
                lastReceived = p[Keys.lastReceived] ?: 0,
                ledgerVersion = p[Keys.ledgerVersion] ?: 0,
                server = p[Keys.server],
                pending = run {
                    val id = p[Keys.pendingId]
                    val verifier = p[Keys.pendingVerifier]
                    val server = p[Keys.pendingServer]
                    if (id != null && verifier != null && server != null) {
                        Enrollment(
                            server = server,
                            id = id,
                            verifier = verifier,
                            code = p[Keys.pendingCode].orEmpty(),
                            expiresAt = p[Keys.pendingExpires] ?: 0L,
                            notified = p[Keys.pendingNotified] ?: false,
                        )
                    } else {
                        null
                    }
                },
                phoneOnly = p[Keys.phoneOnly] ?: false,
                replaceNext = p[Keys.replaceNext] ?: false,
                unpaired = p[Keys.unpaired] ?: false,
            )
        }
        .distinctUntilChanged()

    suspend fun current(): SyncPrefs = prefs.first()

    /** A new pairing starts from nothing: cursor 0, and the first sync still to come. */
    suspend fun pair(pc: PairedPc) = store.edit {
        clearAllBut(it)
        it[Keys.name] = pc.name
        it[Keys.hostId] = pc.hostId
        it[Keys.instance] = pc.instance
        it[Keys.routes] = pc.routes.joinToString("\n")
        it[Keys.deviceId] = pc.deviceId
        it[Keys.token] = pc.token
        it[Keys.pairedAt] = pc.pairedAt
    }

    /** The server's name as it greeted last. */
    suspend fun rename(name: String) = store.edit { if (it[Keys.token] != null) it[Keys.name] = name }

    /**
     * A sync that worked. [note] is news to keep showing; a later sync that moves anything
     * either way retires it, one that moves nothing leaves it standing. Every sync that works has
     * caught up with the PC's ledger in every table this build reads (RelaySync asks for everything
     * when [SyncPrefs.catchingUp]), so it records [LEDGER_VERSION].
     */
    suspend fun recordSuccess(at: Long, cursor: Long, generation: String?, watermark: Long, sent: Int, received: Int, note: String? = null) = store.edit {
        if (it[Keys.token] == null) return@edit
        it[Keys.cursor] = cursor
        if (generation != null) it[Keys.generation] = generation else it.remove(Keys.generation)
        when {
            note != null -> it[Keys.note] = note
            sent + received > 0 -> it.remove(Keys.note)
        }
        it[Keys.watermark] = watermark
        it[Keys.replaced] = true
        it[Keys.lastAttemptAt] = at
        it[Keys.lastSuccessAt] = at
        it.remove(Keys.lastError)
        it[Keys.lastSent] = sent
        it[Keys.lastReceived] = received
        it[Keys.ledgerVersion] = LEDGER_VERSION
        it.remove(Keys.replaceNext)
        it.remove(Keys.unpaired)
    }

    suspend fun recordFailure(at: Long, reason: String, unpaired: Boolean = false) = store.edit {
        it[Keys.lastAttemptAt] = at
        it[Keys.lastError] = reason
        if (unpaired) it[Keys.unpaired] = true
    }

    /** Forget this server: its name, its routes, the token and where sync stood with it. */
    suspend fun forget() = store.edit { clearAllBut(it) }

    private fun clearAllBut(p: MutablePreferences) {
        val kept = p.asMap().filterKeys { it.name in keep }
        p.clear()
        for ((key, value) in kept) {
            @Suppress("UNCHECKED_CAST")
            p[key as Preferences.Key<Any>] = value
        }
    }

    /** The address the person typed for their server; null goes back to this build's own. */
    suspend fun setServer(url: String?) = store.edit {
        if (url.isNullOrBlank()) it.remove(Keys.server) else it[Keys.server] = url.trim()
    }

    suspend fun setPhoneOnly(on: Boolean) = store.edit { it[Keys.phoneOnly] = on }

    /** The next sync makes the server's ledger this phone's (the person asked, in Settings). */
    suspend fun requestReplace(on: Boolean = true) = store.edit {
        if (on) it[Keys.replaceNext] = true else it.remove(Keys.replaceNext)
    }

    suspend fun savePending(e: Enrollment) = store.edit {
        it[Keys.pendingServer] = e.server
        it[Keys.pendingId] = e.id
        it[Keys.pendingVerifier] = e.verifier
        it[Keys.pendingCode] = e.code
        it[Keys.pendingExpires] = e.expiresAt
        it[Keys.pendingNotified] = e.notified
    }

    suspend fun clearPending() = store.edit {
        it.remove(Keys.pendingServer)
        it.remove(Keys.pendingId)
        it.remove(Keys.pendingVerifier)
        it.remove(Keys.pendingCode)
        it.remove(Keys.pendingExpires)
        it.remove(Keys.pendingNotified)
    }

    /** Undo for [forget]: everything back as it was, the token and the cursor included. */
    suspend fun restore(prefs: SyncPrefs) {
        val pc = prefs.pc ?: return
        pair(pc)
        store.edit {
            it[Keys.cursor] = prefs.cursor
            prefs.generation?.let { g -> it[Keys.generation] = g }
            prefs.note?.let { n -> it[Keys.note] = n }
            it[Keys.watermark] = prefs.watermark
            it[Keys.replaced] = prefs.replaced
            it[Keys.lastAttemptAt] = prefs.lastAttemptAt
            it[Keys.lastSuccessAt] = prefs.lastSuccessAt
            prefs.lastError?.let { e -> it[Keys.lastError] = e }
            it[Keys.lastSent] = prefs.lastSent
            it[Keys.lastReceived] = prefs.lastReceived
            it[Keys.ledgerVersion] = prefs.ledgerVersion
        }
    }

    companion object {
        /**
         * The synced tables this build reads: the Room version that last added one (4, the
         * investments of docs/INVESTMENTS.md). Raise it with the next table.
         */
        const val LEDGER_VERSION = 4
    }
}
