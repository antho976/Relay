package com.tally.app.ui.shell

import com.tally.app.data.sync.Connect
import com.tally.app.data.sync.PairedPc
import com.tally.app.data.sync.SyncPrefs
import com.tally.core.Copy
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

/** What the status bar says about sync, in each state the connect and the syncs can be in. */
class SyncTextTest {

    private val now = 1_800_000_000_000L
    private val pc = PairedPc("vm", "h1", "stable", listOf("wss://bot.example/relay/join/r"), "d1", "t1", now - 86_400_000L)

    @Test fun waitingOnDiscordSaysTheCode() {
        val line = syncLine(SyncPrefs(), false, Connect.Waiting("7F3A", notified = true, expiresAt = now + 60_000, server = "https://bot.example"), now)
        assertEquals("Allow on Discord · 7F3A", line.text)
        assertEquals(Tone.WAITING, line.tone)
        val dashboard = syncLine(SyncPrefs(), false, Connect.Waiting("7F3A", notified = false, expiresAt = now + 60_000, server = "https://bot.example"), now)
        assertEquals("Allow on your dashboard · 7F3A", dashboard.text)
    }

    @Test fun aRefusalIsHeldAndSaysHowToAskAgain() {
        assertEquals(Tone.HELD, syncLine(SyncPrefs(), false, Connect.Denied, now).tone)
        assertEquals(Tone.HELD, syncLine(SyncPrefs(), false, Connect.Expired, now).tone)
        val off = syncLine(SyncPrefs(), false, Connect.Trouble("Relay is not running on your server yet", cloudOff = true), now)
        assertEquals("Relay is not running on your server yet", off.text)
        assertEquals(Tone.HELD, off.tone)
        assertEquals(Tone.WAITING, syncLine(SyncPrefs(), false, Connect.Trouble("Your server could not be reached"), now).tone)
    }

    @Test fun aConnectedPhoneSaysWhenItLastSynced() {
        val synced = SyncPrefs(pc = pc, replaced = true, lastAttemptAt = now - 120_000, lastSuccessAt = now - 120_000)
        assertEquals("Synced · 2m ago", syncLine(synced, false, Connect.Connected, now).text)
        assertEquals(Tone.LIVE, syncLine(synced, false, Connect.Connected, now).tone)
        assertEquals("Syncing", syncLine(synced, true, Connect.Connected, now).text)
    }

    @Test fun aFailedSyncAfterGoodOnesIsWaitingNotBroken() {
        val offline = SyncPrefs(pc = pc, replaced = true, lastAttemptAt = now - 10_000, lastSuccessAt = now - 3_600_000, lastError = "Your server could not be reached")
        val line = syncLine(offline, false, Connect.Connected, now)
        assertEquals("Offline · synced 1h ago", line.text)
        assertEquals(Tone.WAITING, line.tone)
        assertEquals("Your server could not be reached", line.detail)
    }

    @Test fun phoneOnlyIsOff() {
        assertEquals(Tone.OFF, syncLine(SyncPrefs(phoneOnly = true), false, Connect.PhoneOnly, now).tone)
    }

    @Test fun theLinesKeepTheVoice() {
        val lines = listOf(
            syncLine(SyncPrefs(), false, Connect.Reaching, now),
            syncLine(SyncPrefs(), false, Connect.Denied, now),
            syncLine(SyncPrefs(), false, Connect.Expired, now),
            syncLine(SyncPrefs(phoneOnly = true), false, Connect.PhoneOnly, now),
        ).flatMap { listOfNotNull(it.text, it.detail) }
        lines.forEach { line -> Copy.banned.forEach { bad -> assertFalse("\"$line\" has \"$bad\"", line.lowercase().contains(bad)) } }
    }
}
