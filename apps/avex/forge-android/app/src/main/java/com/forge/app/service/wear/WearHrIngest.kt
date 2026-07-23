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
    private val hrDao: SessionHrSampleDao
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
        val rows = batch.samples.asSequence()
            .filter { it.bpm in MIN_BPM..MAX_BPM && it.atMs >= active.startedAt }
            .take(MAX_SAMPLES_PER_BATCH)
            .map { SessionHrSample(sessionId = active.id, atMs = it.atMs, bpm = it.bpm) }
            .toList()
        if (rows.isNotEmpty()) hrDao.insertAll(rows)
        batch.totalKcal?.takeIf { it > 0 }?.let { watchKcalBySession[active.id] = it }
    }

    /** The watch's measured calories for [sessionId], if any arrived this process lifetime. */
    fun watchKcal(sessionId: Long): Double? = watchKcalBySession[sessionId]

    private companion object {
        const val MIN_BPM = 25
        const val MAX_BPM = 240
        const val MAX_SAMPLES_PER_BATCH = 64
        /** ~4h at 1 Hz — a runaway stream can't grow a session's trace unbounded. */
        const val MAX_SAMPLES_PER_SESSION = 15_000
    }
}
