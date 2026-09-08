package com.forge.wear.data

import com.forge.shared.protocol.TimerStateDto
import com.forge.wear.ui.RestCountdown
import org.junit.Assert.*
import org.junit.Test

class TimerReceiptAnchorTest {
    @Test fun `remount replay and repository recreation preserve original countdown`() {
        var savedKey: String? = null
        var savedTime = 0L
        val timer = TimerStateDto(endAtMs = 250_000L, totalSeconds = 150, publishedAtMs = 100_000L)
        val anchor = TimerReceiptAnchor { key, time -> savedKey = key; savedTime = time }
        val received = anchor.receivedAt(timer, 1000L)
        assertEquals(90, RestCountdown.remainingSeconds(timer, 61_000L, received))
        assertEquals(received, anchor.receivedAt(timer, 61_000L))
        val recreated = TimerReceiptAnchor(savedKey, savedTime)
        assertEquals(90, RestCountdown.remainingSeconds(timer, 61_000L, recreated.receivedAt(timer, 61_000L)))
        val extended = timer.copy(endAtMs = 280_000L, publishedAtMs = 160_000L)
        assertEquals(61_000L, recreated.receivedAt(extended, 61_000L))
        assertEquals(120, RestCountdown.remainingSeconds(extended, 61_000L, 61_000L))
    }
}
