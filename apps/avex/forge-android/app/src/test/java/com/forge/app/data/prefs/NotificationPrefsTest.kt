package com.forge.app.data.prefs

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.data.repo.NoticeKind
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Every write the notifications feed's UNDO lambdas make, round-tripped against a real DataStore.
 *
 * `NotificationFeed.dismiss()` hands the caller back an operation that restores what it cleared, and
 * the snackbar runs that operation up to four seconds later. The feed itself needs Health Connect, a
 * coach and a workout repo to construct, so this covers the half that actually carries the undo
 * semantics: for each source, does writing the dismissal and then writing the captured previous
 * value land back where it started.
 *
 * Prompted by a device test that looked like a broken undo and was really a snackbar that had timed
 * out before the tap — the kind of thing worth pinning down mechanically rather than by hand.
 */
@RunWith(RobolectricTestRunner::class)
class NotificationPrefsTest {

    private lateinit var repo: SettingsRepository

    @Before
    fun setUp() {
        val context: Context = ApplicationProvider.getApplicationContext()
        repo = SettingsRepository(context, Clock { 0L })
    }

    @Test
    fun wearableInviteDismissalIsReversible() = runBlocking {
        assertFalse("starts undismissed", repo.cardioWearableHintDismissed.first())

        repo.setCardioWearableHintDismissed(true)
        assertTrue("dismissing hides the invite", repo.cardioWearableHintDismissed.first())

        // The undo lambda's only write.
        repo.setCardioWearableHintDismissed(false)
        assertFalse("undo brings the invite back", repo.cardioWearableHintDismissed.first())
    }

    @Test
    fun notificationInviteDismissalIsReversible() = runBlocking {
        repo.setNotifPermAsked(true)
        assertTrue(repo.notifPermAsked.first())

        repo.setNotifPermAsked(false)
        assertFalse("undo brings the turn-on-notifications row back", repo.notifPermAsked.first())
    }

    @Test
    fun clearingAMilestoneNeverLetsItFireAgain() = runBlocking {
        repo.markMilestoneShown("sessions_100")
        assertTrue("firing queues it unread", "sessions_100" in repo.unreadMilestones.first())
        assertTrue("and marks it fired", "sessions_100" in repo.shownMilestones.first())

        repo.setMilestoneUnread("sessions_100", unread = false)
        assertFalse("clearing takes it out of the feed", "sessions_100" in repo.unreadMilestones.first())
        assertTrue(
            "but it stays FIRED, so it can never re-fire",
            "sessions_100" in repo.shownMilestones.first()
        )

        repo.setMilestoneUnread("sessions_100", unread = true)
        assertTrue("undo re-queues it", "sessions_100" in repo.unreadMilestones.first())
    }

    @Test
    fun systemNoticeRoundTripsAndSupersedesItsOwnId() = runBlocking {
        repo.addSystemNotice("import", "Brought in 12 sessions.")
        assertEquals(listOf("Brought in 12 sessions."), repo.systemNotices.first().map { it.text })

        // A second import replaces the first rather than stacking a second row.
        repo.addSystemNotice("import", "Brought in 3 sessions.")
        val afterSecond = repo.systemNotices.first()
        assertEquals(1, afterSecond.size)
        assertEquals("Brought in 3 sessions.", afterSecond.single().text)

        // A different id coexists.
        repo.addSystemNotice("restore", "Your backup was restored.")
        assertEquals(2, repo.systemNotices.first().size)

        val captured = repo.systemNotices.first().single { it.id == "import" }
        repo.removeSystemNotice("import")
        assertFalse(repo.systemNotices.first().any { it.id == "import" })

        repo.addSystemNotice(captured.id, captured.text)
        assertEquals(
            "undo restores the exact line",
            "Brought in 3 sessions.",
            repo.systemNotices.first().single { it.id == "import" }.text
        )
    }

    @Test
    fun everyNoticeKindDefaultsOnAndTogglesBothWays() = runBlocking {
        assertTrue("no kind is disabled out of the box", repo.disabledNoticeKinds.first().isEmpty())

        NoticeKind.entries.forEach { kind ->
            repo.setNoticeKindEnabled(kind.key, enabled = false)
            assertTrue("${kind.key} switches off", kind.key in repo.disabledNoticeKinds.first())
            repo.setNoticeKindEnabled(kind.key, enabled = true)
            assertFalse("${kind.key} switches back on", kind.key in repo.disabledNoticeKinds.first())
        }
    }

    /**
     * The two setup invites are dismissed FOR GOOD and have no un-dismiss control of their own, so
     * resetting the Notifications section is the only way back. It has to actually cover them.
     */
    @Test
    fun resettingNotificationsRestoresEverySwitchOnThatPage() = runBlocking {
        repo.setCardioWearableHintDismissed(true)
        repo.setNotifPermAsked(true)
        repo.setNoticeKindEnabled(NoticeKind.MILESTONE.key, enabled = false)

        repo.resetSection(SettingsSection.NOTIFICATIONS)

        assertFalse("the watch invite comes back", repo.cardioWearableHintDismissed.first())
        assertFalse("the notifications invite comes back", repo.notifPermAsked.first())
        assertTrue("every kind is on again", repo.disabledNoticeKinds.first().isEmpty())
    }
}
