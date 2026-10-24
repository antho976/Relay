package com.quietsoftware.relay.service

import android.content.Context
import android.content.Intent
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** A hold's notification key hands its answer to expedited work, with all it needs to check the hold. */
@RunWith(RobolectricTestRunner::class)
class HoldWorkerTest {
    @Test fun `allow once becomes expedited work carrying the hold it was shown`() {
        val intent = Intent(ApplicationProvider.getApplicationContext<Context>(), ActionReceiver::class.java)
            .setAction(ActionReceiver.ACTION_ALLOW)
            .putExtra(ActionReceiver.EXTRA_HOLD, 42L)
            .putExtra(ActionReceiver.EXTRA_OP, "git.push")
            .putExtra(ActionReceiver.EXTRA_CREATED, "2026-10-24T10:00:00.000000000Z")
            .putExtra(ActionReceiver.EXTRA_NOTIFICATION, 10_042)
        val work = HoldWorker.request(intent).workSpec
        assertTrue(work.expedited)
        assertEquals(ActionReceiver.ACTION_ALLOW, work.input.getString("action"))
        assertEquals(42L, work.input.getLong(ActionReceiver.EXTRA_HOLD, -1))
        assertEquals("git.push", work.input.getString(ActionReceiver.EXTRA_OP))
        assertEquals("2026-10-24T10:00:00.000000000Z", work.input.getString(ActionReceiver.EXTRA_CREATED))
        assertEquals(10_042, work.input.getInt(ActionReceiver.EXTRA_NOTIFICATION, 0))
    }
}
