package com.forge.app.ui.checkin

import com.forge.app.data.repo.AppNotice
import com.forge.app.data.repo.NoticeAction
import com.forge.app.data.repo.NoticeGlyph
import com.forge.app.data.repo.NoticeKind
import com.forge.app.ui.notifications.NoticeGroup
import com.forge.app.ui.notifications.actionVerb
import com.forge.app.ui.notifications.groupNotices
import com.forge.app.ui.notifications.noticesMetaLine
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class CheckinEffectTest {

    @Test
    fun neutralAnswersLeaveTodayAsPlanned() {
        val p = checkinEffectPercent(sleep = 4, soreness = 2, stress = 2, drive = 4, sick = false)
        assertEquals(0, p)
        assertEquals("Today: targets as planned", checkinEffectLine(p, false))
    }

    @Test
    fun theAnswersMoveTodayTheWayReadinessDoes() {
        // Slept badly (−2) and sore (−2): targets ease 4%.
        assertEquals(-4, checkinEffectPercent(2, 4, null, null, false))
        assertEquals("Today: targets ease 4%", checkinEffectLine(-4, false))
        // Great sleep and eager: up 2%.
        assertEquals(2, checkinEffectPercent(5, null, null, 5, false))
        assertEquals("Today: targets up 2%", checkinEffectLine(2, false))
    }

    @Test
    fun unwellIsBoundedLikeTheScale() {
        assertEquals(-5, checkinEffectPercent(1, 5, 5, 1, sick = true))
    }

    @Test
    fun eachAnswerIsNamed() {
        assertEquals("Good", CheckinQuestion.SLEEP.answers[3])
        assertEquals("A bit", CheckinQuestion.SORENESS.answers[1])
        assertEquals("Keen", CheckinQuestion.DRIVE.answers[3])
    }

    @Test
    fun theFeedGroupsDecisionsFirstThenNewsThenSetup() {
        fun n(id: String, kind: NoticeKind, action: NoticeAction = NoticeAction.None) =
            AppNotice(id, kind, NoticeGlyph.BELL, "", id, action = action)
        val rows = listOf(
            n("setup", NoticeKind.SETUP, NoticeAction.EnableNotifications),
            n("milestone", NoticeKind.MILESTONE),
            n("checkin", NoticeKind.CHECK_IN, NoticeAction.OpenCheckin),
        )
        val groups = groupNotices(rows)
        assertEquals(listOf(NoticeGroup.NEEDS_YOU, NoticeGroup.NEW, NoticeGroup.SET_UP), groups.map { it.first })
        assertEquals("Check in", actionVerb(NoticeAction.OpenCheckin))
        assertEquals("Allow", actionVerb(NoticeAction.EnableNotifications))
        assertNull(actionVerb(NoticeAction.OpenCoachBrief))
        assertEquals("3 waiting · swipe a row to clear it", noticesMetaLine(rows))
        assertEquals("0 waiting", noticesMetaLine(emptyList()))
    }
}
