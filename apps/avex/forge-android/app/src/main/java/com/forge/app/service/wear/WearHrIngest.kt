package com.forge.app.service.wear

import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.dao.SessionHrSampleDao
import com.forge.app.data.db.entities.SessionHrSample
import com.forge.shared.protocol.HrBatchDto
import com.forge.shared.protocol.WearCodec
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Receives the watch's live HR batches (W3): validates the batch belongs to the ACTIVE session
 * (a stale batch after a session ended is dropped — the watch never invents state the phone
 * accepts blindly), bounds the values, and writes to Room. The (session, at_ms) primary key makes
 * a re-sent batch after a BT flap idempotent. Also keeps the watch's cumulative measured calories
 * per session (in-memory — at finish they replace the MET estimate when present).
 */
@Singleton
class WearHrIngest @Inject constructor(
    private val sessionDao: SessionDao,
    private val hrDao: SessionHrSampleDao,
    private val clock: com.forge.app.core.time.Clock
) {
    private val watchKcalBySession = java.util.concurrent.ConcurrentHashMap<Long, Double>()

    suspend fun handleBatch(bytes: ByteArray) {
        val batch = when (val d = WearCodec.decode<HrBatchDto>(bytes)) {
            is WearCodec.DecodeResult.Ok -> d.value
            else -> return
        }
        val active = sessionDao.getActiveSession() ?: return
        if (active.id != batch.sessionId) return
        if (hrDao.countForSession(active.id) >= MAX_SAMPLES_PER_SESSION) return
        // Compare like with like. The samples carry WATCH-clock timestamps; `active.startedAt` is a
        // PHONE-clock instant. Filtering one against the other dropped the opening seconds of every
        // trace whenever the watch ran behind — routine, since Wear time sync is periodic. When the
        // batch reports the watch's own clock we measure the offset directly; otherwise (an older
        // watch build) we fall back to a fixed tolerance, which the per-session sample cap already
        // bounds the damage of.
        val skewMs = if (batch.sentAtMs > 0L) batch.sentAtMs - clock.nowMs() else 0L
        val floorMs = active.startedAt + skewMs - CLOCK_SKEW_TOLERANCE_MS
        val rows = batch.samples.asSequence()
            .filter { it.bpm in MIN_BPM..MAX_BPM && it.atMs >= floorMs }
            .take(MAX_SAMPLES_PER_BATCH)
            .map { SessionHrSample(sessionId = active.id, atMs = it.atMs, bpm = it.bpm) }
            .toList()
        if (rows.isNotEmpty()) hrDao.insertAll(rows)
        batch.totalKcal?.takeIf { it > 0 }?.let { watchKcalBySession[active.id] = it }
    }

    /** The watch's measured calories for [sessionId], if any arrived this process lifetime. */
    fun watchKcal(sessionId: Long): Double? = watchKcalBySession[sessionId]

    private companion object {
        /** Slack on the session-start boundary for an unmeasurable clock offset between the two
         *  devices. Wide enough for real Wear drift, far inside MAX_SAMPLES_PER_SESSION. */
        const val CLOCK_SKEW_TOLERANCE_MS = 60_000L

        const val MIN_BPM = 25
        const val MAX_BPM = 240
        const val MAX_SAMPLES_PER_BATCH = 64
        /** ~4h at 1 Hz — a runaway stream can't grow a session's trace unbounded. */
        const val MAX_SAMPLES_PER_SESSION = 15_000
    }
}
