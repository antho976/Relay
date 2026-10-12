package com.forge.app.relay

import android.content.Context
import android.os.Build
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import org.json.JSONObject
import java.text.DateFormat
import java.util.Date

/**
 * PERSONAL BUILD ONLY. Where Avex sends its training history: the cloud copy of Relay this phone
 * paired with, and how the last send went. Kept in app-private preferences, which Avex never backs
 * up (allowBackup is off); the credential is good for that cloud copy alone, and is revoked there
 * with `relay remote revoke`.
 */
class RelayStore(context: Context) {
    private val prefs = context.applicationContext.getSharedPreferences("avex_relay", Context.MODE_PRIVATE)

    data class Paired(val via: String, val host: String, val device: String, val token: String)

    val paired: Paired?
        get() {
            val via = prefs.getString(VIA, null) ?: return null
            val device = prefs.getString(DEVICE, null) ?: return null
            val token = prefs.getString(TOKEN, null) ?: return null
            return Paired(via, prefs.getString(HOST, null) ?: "Relay", device, token)
        }

    fun pair(p: Paired) {
        prefs.edit().putString(VIA, p.via).putString(HOST, p.host).putString(DEVICE, p.device).putString(TOKEN, p.token)
            .remove(LAST_OK).remove(LAST_ERROR).apply()
        publish()
    }

    fun forget() {
        prefs.edit().clear().apply()
        publish()
    }

    fun sent(sessions: Int) {
        prefs.edit().putLong(LAST_OK, System.currentTimeMillis()).putInt(LAST_SESSIONS, sessions).remove(LAST_ERROR).apply()
        publish()
    }

    fun failed(reason: String) {
        prefs.edit().putString(LAST_ERROR, reason).apply()
        publish()
    }

    /** One line for the screen: where it goes and how the last send went. */
    fun status(): String {
        val p = paired ?: return "Not connected yet. Connect, then tap Allow on the Discord message your server sends."
        val ok = prefs.getLong(LAST_OK, 0L)
        val last = if (ok == 0L) "Nothing sent yet." else
            "Last sent ${DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT).format(Date(ok))}" +
                " (${prefs.getInt(LAST_SESSIONS, 0)} workouts)."
        val error = prefs.getString(LAST_ERROR, null)?.let { "\nLast try failed: $it" }.orEmpty()
        return "Sending to ${p.host}.\n$last$error"
    }

    private fun publish() {
        _changes.value = _changes.value + 1
    }

    companion object {
        private const val VIA = "via"
        private const val HOST = "host"
        private const val DEVICE = "device"
        private const val TOKEN = "token"
        private const val LAST_OK = "last_ok"
        private const val LAST_SESSIONS = "last_sessions"
        private const val LAST_ERROR = "last_error"

        private val _changes = MutableStateFlow(0)
        /** Ticks whenever the stored link or its status changes, for the screen to re-read it. */
        val changes: StateFlow<Int> = _changes
    }
}

/** PERSONAL BUILD ONLY. Pairs with Relay's cloud copy, and sends it the whole training history. */
object RelayUploader {

    /** The rendezvous takes at most 8 MiB from a device in one message. */
    private const val MAX_BYTES = 7 * 1024 * 1024

    private fun deviceName(): String = "Avex on ${Build.MODEL}"

    /** Presents the code of the link the server handed over (RelayConnect). */
    suspend fun pair(store: RelayStore, linkText: String) {
        val link = PairLink.parse(linkText)
            ?: throw RelayFailure("Your server's pairing link did not read. Ask again.")
        val (session, welcome) = RelaySession.open(link.via, RelaySession.PAIR_WELCOME_TIMEOUT_MS) {
            RelayWire.pairHello(link.code, deviceName())
        }
        session.close()
        val device = welcome.device ?: throw RelayFailure("Relay paired this phone without a credential")
        val token = welcome.token ?: throw RelayFailure("Relay paired this phone without a credential")
        store.pair(RelayStore.Paired(link.via, link.host, device, token))
    }

    /** Sends [json], Avex's `avex_export.json`, to replace the cloud copy's training history. */
    suspend fun upload(store: RelayStore, json: String): Int {
        val p = store.paired ?: throw RelayFailure("Not paired with Relay")
        if (json.toByteArray(Charsets.UTF_8).size > MAX_BYTES) {
            throw RelayFailure("The training history is too big to send in one piece")
        }
        val (session, _) = try {
            RelaySession.open(p.via) { g -> RelayWire.proofHello(p.device, g.challenge!!, p.token) }
        } catch (e: RelayFailure) {
            if (e.unpaired) store.forget()
            throw e
        }
        session.use {
            val result = it.request("gym.upload", JSONObject().put("json", json).put("name", "avex_export.json"))
            return result?.optInt("sessions", 0) ?: 0
        }
    }
}
