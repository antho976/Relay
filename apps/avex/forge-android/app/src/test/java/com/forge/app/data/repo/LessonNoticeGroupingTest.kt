package com.forge.app.data.repo

import org.junit.Assert.assertEquals
import org.junit.Test

class LessonNoticeGroupingTest {

    private fun lesson(id: String, title: String) = AppNotice(
        id = "lesson.$id",
        kind = NoticeKind.ACADEMY,
        glyph = NoticeGlyph.ACADEMY,
        eyebrow = "NEW LESSON",
        title = title,
        action = NoticeAction.OpenLesson(id)
    )

    @Test
    fun oneLessonKeepsItsOwnRow() {
        val one = listOf(lesson("a", "A"))
        assertEquals(one, NotificationFeed.groupLessons(one))
    }

    @Test
    fun severalLessonsFoldIntoOneRowThatOpensTheAcademy() {
        val rows = NotificationFeed.groupLessons(listOf(lesson("a", "First"), lesson("b", "Second"), lesson("c", "Third")))
        assertEquals(1, rows.size)
        assertEquals("3 new lessons", rows[0].title)
        assertEquals("First, Second and 1 more", rows[0].detail)
        assertEquals(NoticeAction.OpenAcademy, rows[0].action)
    }

    @Test
    fun theAcademyBadgeStillCountsEveryLesson() {
        val grouped = NotificationFeed.groupLessons(listOf(lesson("a", "A"), lesson("b", "B")))
        assertEquals(2, NotificationFeed.newLessonCount(grouped))
        assertEquals(1, NotificationFeed.newLessonCount(listOf(lesson("a", "A"))))
    }
}
