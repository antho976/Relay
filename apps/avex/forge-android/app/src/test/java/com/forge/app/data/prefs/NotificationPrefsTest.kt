package com.forge.app.data.prefs

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.data.repo.NoticeKind
import com.forge.app.ui.DesignDoctrine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
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
    fun wearableInviteDismissalIsReversible() = runTest {
        assertFalse("starts undismissed", repo.cardioWearableHintDismissed.first())

        repo.setCardioWearableHintDismissed(true)
        assertTrue("dismissing hides the invite", repo.cardioWearableHintDismissed.first())

        // The undo lambda's only write.
        repo.setCardioWearableHintDismissed(false)
        assertFalse("undo brings the invite back", repo.cardioWearableHintDismissed.first())
    }

    @Test
    fun notificationInviteDismissalIsReversible() = runTest {
        repo.setNotifPermAsked(true)
        assertTrue(repo.notifPermAsked.first())

        repo.setNotifPermAsked(false)
        assertFalse("undo brings the turn-on-notifications row back", repo.notifPermAsked.first())
    }

    @Test
    fun clearingAMilestoneNeverLetsItFireAgain() = runTest {
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
    fun systemNoticeRoundTripsAndSupersedesItsOwnId() = runTest {
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
    fun everyNoticeKindDefaultsOnAndTogglesBothWays() = runTest {
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
    fun resettingNotificationsRestoresEverySwitchOnThatPage() = runTest {
        repo.setCardioWearableHintDismissed(true)
        repo.setNotifPermAsked(true)
        repo.setNoticeKindEnabled(NoticeKind.MILESTONE.key, enabled = false)
        repo.setTrainingReminderEnabled(true)
        repo.setWeeklyRecapEnabled(false)
        repo.setRestTimerAlertEnabled(false)
        repo.setQuietHoursEnabled(true)

        repo.resetSection(SettingsSection.NOTIFICATIONS)

        assertFalse("the watch invite comes back", repo.cardioWearableHintDismissed.first())
        assertFalse("the notifications invite comes back", repo.notifPermAsked.first())
        assertTrue("every kind is on again", repo.disabledNoticeKinds.first().isEmpty())
        // Each of these goes back to its DEFAULT, which is not the same as "on": training reminders
        // and quiet hours default off, the other two default on.
        assertFalse("training reminders go back off", repo.trainingReminderEnabled.first())
        assertTrue("weekly recap comes back on", repo.weeklyRecapEnabled.first())
        assertTrue("rest timer alerts come back on", repo.restTimerAlertEnabled.first())
        assertFalse("quiet hours goes back off", repo.quietHoursEnabled.first())
    }

    /**
     * The case above is an enumeration, and an enumeration goes stale in silence.
     *
     * It was named "every switch on that page" while checking three of them, and two toggles —
     * weekly recap and rest timer alerts — were added to the page without ever being added to the
     * section. Resetting left them exactly as they were, and the test that would have caught it was
     * green because its name was the only place the word "every" appeared.
     *
     * So derive the list instead of writing it down: read `NotificationsPage`'s own source, take
     * every `state.x` it renders, and require each one to be accounted for here AND covered by the
     * section. Add a toggle to that page and this fails until both are true.
     */
    @Test
    fun everyToggleOnTheNotificationsPageIsCoveredByItsSectionReset() {
        val source = DesignDoctrine.appSource("ui/settings/SettingsSubPages.kt")
        assertTrue("SettingsSubPages.kt not found at ${source.path}", source.isFile)
        val text = source.readText()

        val start = text.indexOf("internal fun NotificationsPage(")
        assertTrue("NotificationsPage not found — did it move or get renamed?", start >= 0)
        val after = text.indexOf("\n@Composable", start)
        val body = if (after < 0) text.substring(start) else text.substring(start, after)

        val rendered = Regex("""\bstate\.([A-Za-z][A-Za-z0-9]*)""").findAll(body)
            .map { it.groupValues[1] }.toSortedSet()
        assertTrue("no state fields found — the scan is broken, not the page", rendered.isNotEmpty())

        // Each control on the page and the preference key behind it. `trainingReminderHour` is the
        // reminder's companion picker rather than a switch of its own; it still has to reset.
        val backingKey = mapOf(
            "trainingReminderEnabled" to PreferenceKeys.TRAINING_REMINDER_ENABLED,
            "trainingReminderHour" to PreferenceKeys.TRAINING_REMINDER_HOUR,
            "weeklyRecapEnabled" to PreferenceKeys.WEEKLY_RECAP_ENABLED,
            "restTimerAlertEnabled" to PreferenceKeys.REST_TIMER_ALERT_ENABLED,
            "quietHoursEnabled" to PreferenceKeys.QUIET_HOURS_ENABLED,
            "disabledNoticeKinds" to PreferenceKeys.DISABLED_NOTICE_KINDS,
        )

        val unaccounted = rendered - backingKey.keys
        assertTrue(
            "\n\nNotificationsPage renders these, and this test does not know what preference " +
                "they are stored under: $unaccounted\nAdd each to `backingKey` above, and to " +
                "SettingsSection.NOTIFICATIONS, so \"Reset this section\" keeps its promise.\n",
            unaccounted.isEmpty()
        )

        val notInSection = rendered.mapNotNull { backingKey[it] }
            .filterNot { it in SettingsSection.NOTIFICATIONS.keys }
        assertTrue(
            "\n\nThese are switches on the Notifications page that resetting the section leaves " +
                "untouched: $notInSection\n",
            notInSection.isEmpty()
        )
    }

    // ── Academy: dismissal and announcement are separate records ───────────────

    @Test
    fun lessonNoticeDismissalIsReversible() = runTest {
        assertTrue("nothing dismissed to start", repo.dismissedLessonNotices.first().isEmpty())

        repo.setLessonNoticeDismissed("coach.strength_on_a_cut", dismissed = true)
        assertTrue(
            "clearing the row hides it",
            "coach.strength_on_a_cut" in repo.dismissedLessonNotices.first()
        )

        repo.setLessonNoticeDismissed("coach.strength_on_a_cut", dismissed = false)
        assertTrue("undo brings it back", repo.dismissedLessonNotices.first().isEmpty())
    }

    /**
     * The split that keeps the banner from nagging. A lesson stays UNREAD until it is opened,
     * possibly for weeks; it is UNANNOUNCED only until its banner has played once. Sharing one flag
     * would re-announce everything still unread on every app open, which is the exact behaviour the
     * whole arrival mechanism exists to avoid.
     */
    @Test
    fun announcingIsOneWayAndIndependentOfDismissal() = runTest {
        assertTrue("nothing announced to start", repo.announcedLessonNotices.first().isEmpty())

        repo.markLessonNoticesAnnounced(setOf("fundamentals.warmups", "engine.intervals"))
        assertEquals(
            setOf("fundamentals.warmups", "engine.intervals"),
            repo.announcedLessonNotices.first()
        )

        // Additive: a later arrival must not wipe the earlier announcements.
        repo.markLessonNoticesAnnounced(setOf("signals.stress_hrv"))
        assertEquals(3, repo.announcedLessonNotices.first().size)

        // Dismissing a row does not un-announce it, and vice versa: two separate records.
        repo.setLessonNoticeDismissed("fundamentals.warmups", dismissed = true)
        assertTrue(
            "still announced after dismissal",
            "fundamentals.warmups" in repo.announcedLessonNotices.first()
        )
        repo.setLessonNoticeDismissed("fundamentals.warmups", dismissed = false)
        assertTrue(
            "un-dismissing does not replay the banner",
            "fundamentals.warmups" in repo.announcedLessonNotices.first()
        )
    }

    @Test
    fun announcingNothingIsANoOp() = runTest {
        repo.markLessonNoticesAnnounced(emptySet())
        assertTrue(repo.announcedLessonNotices.first().isEmpty())
    }

    @Test
    fun checkinArrivalIsRememberedByDate() = runTest {
        assertTrue(repo.announcedCheckinDates.first().isEmpty())

        repo.markCheckinDatesAnnounced(setOf("2026-09-01"))
        repo.markCheckinDatesAnnounced(setOf("2026-09-02"))

        assertEquals(setOf("2026-09-01", "2026-09-02"), repo.announcedCheckinDates.first())
    }
}
