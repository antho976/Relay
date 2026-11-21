package com.forge.app.service.wear

import android.app.Application
import com.forge.app.core.time.Clock
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.db.session
import com.forge.shared.protocol.*
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class)
class WearHrBatchTest {
    private val db = inMemoryForgeDb()
    private val ingest = WearHrIngest(db.sessionDao(), db.sessionHrSampleDao(), Clock { 300_000 })
    @After fun close() = db.close()

    @Test fun `legacy 200-sample buffer persists fully and chunk retries are idempotent`() = runBlocking {
        val sid = db.sessionDao().insert(session(startedAt = 1000, finishedAt = null))
        val samples = (1..200).map { HrBatchDto.Sample(atMs = 1000L + it * 1000, bpm = 100) }
        ingest.handleBatch(WearCodec.encode(HrBatchDto(sessionId = sid, samples = samples, sentAtMs = 300_000)))
        assertEquals(200, db.sessionHrSampleDao().countForSession(sid))
        for ((i, chunk) in samples.chunked(WearProtocol.HR_SEND_BATCH_SIZE).withIndex()) {
            val payload = WearCodec.encode(HrBatchDto(sessionId = sid, samples = chunk, batchId = "chunk-$i", sentAtMs = 300_000))
            repeat(2) {
                val ack = ingest.handleBatch(payload)
                assertEquals("chunk-$i", ack!!.batchId)
                assertEquals(200, db.sessionHrSampleDao().countForSession(sid))
            }
        }
    }

    @Test fun `persistence failure cannot produce a success acknowledgement`() = runBlocking {
        val sid = db.sessionDao().insert(session(startedAt = 1000, finishedAt = null))
        val failing = object : com.forge.app.data.db.dao.SessionHrSampleDao by db.sessionHrSampleDao() {
            override suspend fun insertAll(samples: List<com.forge.app.data.db.entities.SessionHrSample>) {
                error("disk failure")
            }
        }
        val ingest = WearHrIngest(db.sessionDao(), failing, Clock { 300_000 })
        try {
            ingest.handleBatch(WearCodec.encode(HrBatchDto(sessionId = sid,
                samples = listOf(HrBatchDto.Sample(2000, 100)), batchId = "retry")))
            fail("must not acknowledge failed persistence")
        } catch (_: IllegalStateException) { }
        assertEquals(0, db.sessionHrSampleDao().countForSession(sid))
    }
}
