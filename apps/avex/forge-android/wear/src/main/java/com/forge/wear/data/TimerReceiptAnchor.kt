package com.forge.wear.data

import com.forge.shared.protocol.TimerStateDto
import com.forge.shared.protocol.WearCodec

/** Receipt belongs to a payload version, so a new composition cannot restart its duration. */
internal class TimerReceiptAnchor(
    initialKey: String? = null,
    initialReceivedAtMs: Long = 0L,
    private val persist: (String, Long) -> Unit = { _, _ -> }
) {
    private var key = initialKey
    private var receivedAtMs = initialReceivedAtMs

    @Synchronized fun receivedAt(timer: TimerStateDto, nowMs: Long): Long {
        val version = WearCodec.encode(timer).decodeToString()
        if (key != version) {
            key = version
            receivedAtMs = nowMs
            persist(version, receivedAtMs)
        }
        return receivedAtMs
    }
}
