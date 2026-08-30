package com.forge.wear.data

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The trust rule for a measured phone/watch clock offset.
 *
 * The rest-timer complication renders a countdown from an instant stamped on the PHONE's clock,
 * against the watch's own — so any skew between them was error, silently, and in the opposite
 * direction from the same countdown inside the app. Correcting for it is only safe while the
 * measurement is still current, because Wear's periodic time sync moves the watch clock underneath
 * any earlier reading.
 */
class WearClockSkewTest {

    private val now = 1_700_000_000_000L

    @Test
    fun `a fresh measurement is applied`() {
        assertEquals(
            -4_000L,
            WearClockSkew.usableOffset(offsetMs = -4_000L, measuredAtMs = now - 60_000L, watchNowMs = now)
        )
    }

    @Test
    fun `never measured means no correction`() {
        assertEquals(0L, WearClockSkew.usableOffset(offsetMs = 9_999L, measuredAtMs = 0L, watchNowMs = now))
    }

    @Test
    fun `a stale measurement is discarded rather than trusted`() {
        val tooOld = now - WearClockSkew.MAX_AGE_MS - 1
        assertEquals(0L, WearClockSkew.usableOffset(offsetMs = 5_000L, measuredAtMs = tooOld, watchNowMs = now))
    }

    @Test
    fun `a measurement at exactly the age limit still counts`() {
        val edge = now - WearClockSkew.MAX_AGE_MS
        assertEquals(5_000L, WearClockSkew.usableOffset(offsetMs = 5_000L, measuredAtMs = edge, watchNowMs = now))
    }

    @Test
    fun `a measurement stamped in the future is discarded`() {
        // The watch clock moved BACKWARDS since the reading, which is the one case where the stored
        // offset is certainly wrong rather than merely old.
        assertEquals(
            0L,
            WearClockSkew.usableOffset(offsetMs = 5_000L, measuredAtMs = now + 1_000L, watchNowMs = now)
        )
    }
}
