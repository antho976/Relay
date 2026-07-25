package com.forge.app.domain.notify

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class PrMilestoneTest {

    @Test fun `fires when crossing a milestone this session`() {
        // 8 → 11 crosses 10.
        val n = PrMilestone.check(lifetimePrCount = 11, sessionPrCount = 3)
        assertTrue(n != null && n.title.contains("10"))
    }

    @Test fun `fires when landing exactly on a milestone`() {
        val n = PrMilestone.check(lifetimePrCount = 50, sessionPrCount = 1)
        assertTrue(n != null && n.title.contains("50"))
    }

    @Test fun `quiet when no milestone in the crossed range`() {
        assertNull(PrMilestone.check(lifetimePrCount = 13, sessionPrCount = 2))
    }

    @Test fun `quiet when the session set no PRs`() {
        assertNull(PrMilestone.check(lifetimePrCount = 50, sessionPrCount = 0))
    }

    @Test fun `picks the highest milestone when several are crossed`() {
        // 8 → 60 clears 10, 25 and 50; the notification celebrates 50.
        val n = PrMilestone.check(lifetimePrCount = 60, sessionPrCount = 52)
        // No exclamation mark: DESIGN §11 bans them in every rendered string, notifications included.
        assertEquals("50 personal records", n?.title)
    }
}
