package com.tally.app.ui.shell

import com.tally.app.data.sync.Connect
import com.tally.app.data.sync.SyncPrefs

/** How a status reads: Relay's lamps. */
enum class Tone { LIVE, WAITING, HELD, OFF }

/** One line about sync, as the status bar and Settings say it. */
data class SyncLine(val text: String, val tone: Tone, val detail: String? = null)

/** "just now", "4m ago", "2h ago", "3d ago". */
internal fun agoText(epochMs: Long, now: Long): String {
    val s = (now - epochMs) / 1000
    return when {
        s < 45 -> "just now"
        s < 3600 -> "${s / 60}m ago"
        s < 86400 -> "${s / 3600}h ago"
        else -> "${s / 86400}d ago"
    }
}

/**
 * Where this phone stands with the owner's server, in the fewest words: connecting first, then the
 * last sync. A sync that failed after earlier ones worked reads as waiting, not broken: it goes on
 * its own when the network comes back.
 */
internal fun syncLine(prefs: SyncPrefs, busy: Boolean, connect: Connect, now: Long): SyncLine {
    if (prefs.phoneOnly || connect == Connect.PhoneOnly) return SyncLine("On this phone only", Tone.OFF, "Sync is off. Turn it on in Settings, Sync.")
    if (!prefs.paired) {
        return when (connect) {
            is Connect.Waiting -> if (connect.notified) {
                SyncLine("Allow on Discord · ${connect.code}", Tone.WAITING, "Your bot sent you a message. Tap Allow if it shows ${connect.code}.")
            } else {
                SyncLine("Allow on your dashboard · ${connect.code}", Tone.WAITING, "Open your bot's dashboard, Tally sync, and allow the request that shows ${connect.code}.")
            }
            Connect.Reaching -> SyncLine("Reaching your server", Tone.WAITING)
            Connect.Pairing -> SyncLine("Connecting", Tone.WAITING)
            Connect.Denied -> SyncLine("Your server declined this phone", Tone.HELD, "Ask again from Settings, Sync.")
            Connect.Expired -> SyncLine("Request ran out · ask again", Tone.HELD, "Nobody answered in 15 minutes. Ask again from Settings, Sync.")
            is Connect.Trouble -> SyncLine(connect.reason, if (connect.cloudOff) Tone.HELD else Tone.WAITING)
            Connect.Connected, Connect.Idle, Connect.PhoneOnly -> SyncLine("Not connected yet", Tone.OFF)
        }
    }
    if (busy) return SyncLine("Syncing", Tone.WAITING)
    val last = prefs.lastSuccessAt
    val error = prefs.lastError
    return when {
        error != null && prefs.lastAttemptAt >= last ->
            SyncLine(if (last > 0) "Offline · synced ${agoText(last, now)}" else "Waiting for your server", Tone.WAITING, error)
        last > 0 -> SyncLine("Synced · ${agoText(last, now)}", Tone.LIVE, prefs.note)
        else -> SyncLine("Connected", Tone.LIVE)
    }
}
