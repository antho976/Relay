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

    /**
     * Whether ANY node carrying the Avex wear app is paired — reachable or not (P-02).
     *
     * Deliberately not [reachableWearNodeId]: a DataItem written for a watch that is currently out
     * of range syncs when it comes back, so a paired-but-away watch must still be published to. The
     * question here is only "is there a consumer at all", and on a phone with no watch — which is
     * most phones — the answer lets the caller skip building a payload nothing will read.
     *
     * Fail-soft like everything else on this seam: no Play services, no wear app, or a thrown
     * lookup all read as "no watch", which costs a stale tile at worst and never a crash.
     */
    suspend fun hasPairedWearApp(): Boolean = try {
        Wearable.getCapabilityClient(context)
            .getCapability(WearProtocol.CAPABILITY_WEAR_APP, CapabilityClient.FILTER_ALL)
            .await()
            .nodes.isNotEmpty()
    } catch (t: Throwable) {
        false
    }

    // ── Timer-haptic handoff (the no-silent-timer rule, DESIGN.md §16) ───────
    // The watch acks its timer-done buzz over PATH_HAPTIC_ACK; the phone waits a short grace and
    // buzzes itself only when no ack arrived — one buzz, on the body part that feels it, and a
    // BT drop at the boundary degrades to a LATE phone buzz, never silence.

    @Volatile private var lastHapticAckAtMs: Long = 0L
    @Volatile private var lastHapticAckTimerEndAtMs: Long = 0L

    /**
     * Record that the wrist buzzed for the timer whose published `endAtMs` is [timerEndAtMs].
     *
     * The identity matters as much as the time. Recency alone was the whole test, over an
     * eight-second window sized for Bluetooth lag — so an ack for a rest that had just ended
     * silenced the phone for ANY timer starting within those eight seconds. Skipping a finished
     * rest and immediately starting the next set is exactly that sequence, and the outcome was
     * silence on both devices: the wrist had already buzzed for the old timer and would not buzz
     * again, and the phone believed it had.
     */
    fun recordHapticAck(timerEndAtMs: Long, atMs: Long) {
        if (atMs > lastHapticAckAtMs) {
            lastHapticAckAtMs = atMs
            lastHapticAckTimerEndAtMs = timerEndAtMs
        }
    }

    /**
     * True when the watch confirmed a buzz for the timer published as [timerEndAtMs], within
     * [windowMs] of [nowMs].
     *
     * A zero [timerEndAtMs] on either side means "no identity available" — an ack from a watch
     * build that predates the field, or a phone that has not published a running timer — and never
     * matches. That degrades to the documented safe direction: the phone buzzes late rather than
     * nobody buzzing.
     */
    fun hapticAckedFor(timerEndAtMs: Long, windowMs: Long, nowMs: Long): Boolean =
        timerEndAtMs != 0L &&
            timerEndAtMs == lastHapticAckTimerEndAtMs &&
            nowMs - lastHapticAckAtMs <= windowMs
}
