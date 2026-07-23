package com.forge.app.service.wear

import android.content.Context
import com.google.android.gms.wearable.CapabilityClient
import com.google.android.gms.wearable.Wearable
import com.forge.shared.protocol.WearProtocol
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.tasks.await
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Phone-side view of the watch link (W1): capability-based reachability + the timer-haptic
 * handoff ledger. Fail-soft throughout — no Play services / no watch just reads as unreachable.
 */
@Singleton
class WearConnection @Inject constructor(
    @ApplicationContext private val context: Context
) {
    /**
     * The node id of a reachable Avex wear app, or null. Checks the capability the wear APK
     * declares (res/values/wear.xml) — reachable-only, nearby preferred.
     */
    suspend fun reachableWearNodeId(): String? = try {
        val info = Wearable.getCapabilityClient(context)
            .getCapability(WearProtocol.CAPABILITY_WEAR_APP, CapabilityClient.FILTER_REACHABLE)
            .await()
        (info.nodes.firstOrNull { it.isNearby } ?: info.nodes.firstOrNull())?.id
    } catch (t: Throwable) {
        null
    }

    // ── Timer-haptic handoff (the no-silent-timer rule, DESIGN.md §16) ───────
    // The watch acks its timer-done buzz over PATH_HAPTIC_ACK; the phone waits a short grace and
    // buzzes itself only when no ack arrived — one buzz, on the body part that feels it, and a
    // BT drop at the boundary degrades to a LATE phone buzz, never silence.

    @Volatile private var lastHapticAckAtMs: Long = 0L

    fun recordHapticAck(atMs: Long) {
        if (atMs > lastHapticAckAtMs) lastHapticAckAtMs = atMs
    }

    /** True when the watch confirmed a timer-done buzz within [windowMs] of [nowMs]. */
    fun hapticAckedWithin(windowMs: Long, nowMs: Long): Boolean =
        nowMs - lastHapticAckAtMs <= windowMs
}
