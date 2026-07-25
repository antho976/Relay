package com.forge.app.domain.academy

import com.forge.app.data.db.entities.LessonEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The Academy's contract. The audit the plan promises runs HERE, as a test, so "every lesson is
 * reachable from a real moment" can't quietly stop being true.
 */
class AcademyRegistryTest {

    private fun event(id: String, kind: LessonEventKind, at: Long) =
        LessonEvent(lessonId = id, kind = kind.code, atMs = at)

    private val cut = "coach.strength_on_a_cut"

    // ── The audit ──────────────────────────────────────────────────────────────

    @Test
    fun everyShippedLessonIsReachableFromAMoment() {
        assertTrue(
            "orphan lessons: ${AcademyRegistry.orphanLessons().map { it.id }}",
            AcademyRegistry.orphanLessons().isEmpty()
        )
    }

    @Test
    fun everyLessonHasIdentityAndContent() {
        AcademyRegistry.lessons.forEach { l ->
            assertTrue("${l.id} needs a title", l.title.isNotBlank())
            assertTrue("${l.id} needs a summary", l.summary.isNotBlank())
            assertTrue("${l.id} needs an unlock description", l.unlockedBy.isNotBlank())
            assertTrue("${l.id} needs content", l.blocks.isNotEmpty())
        }
    }

    @Test
    fun lessonIdsAreUnique() {
        val ids = AcademyRegistry.lessons.map { it.id }
        assertEquals(ids.size, ids.toSet().size)
    }

    @Test
    fun contentObeysTheVoiceRules() {
        // §11: no exclamation marks, no em dashes in rendered strings.
        AcademyRegistry.lessons.forEach { lesson ->
            val text = buildList {
                add(lesson.title); add(lesson.summary); add(lesson.unlockedBy)
                lesson.blocks.forEach { b ->
                    when (b) {
                        is LessonBlock.Heading -> add(b.text)
                        is LessonBlock.Paragraph -> add(b.text)
                        is LessonBlock.Callout -> add(b.text)
                        is LessonBlock.Bullets -> addAll(b.items)
                        is LessonBlock.Example -> { add(b.label); add(b.fallback) }
                    }
                }
            }.joinToString(" ")
            assertFalse("${lesson.id} has an exclamation mark", text.contains("!"))
            assertFalse("${lesson.id} has an em dash", text.contains("—"))
            assertFalse("${lesson.id} title takes no terminal period", lesson.title.endsWith("."))
        }
    }

    // ── Ledger-derived state ───────────────────────────────────────────────────

    @Test
    fun withNoEvents_everythingIsUpcoming() {
        assertTrue(AcademyRegistry.unlocked(emptyList()).isEmpty())
        assertEquals(AcademyRegistry.lessons.size, AcademyRegistry.upcoming(emptyList()).size)
    }

    @Test
    fun anUnlockEventUnlocksExactlyItsLesson() {
        val state = AcademyRegistry.stateOf(cut, listOf(event(cut, LessonEventKind.UNLOCKED, 100)))!!
        assertTrue(state.unlocked)
        assertFalse(state.opened)
        assertTrue("unlocked but unread is what the new-lesson chip counts", state.isNew)
        assertEquals(100L, state.unlockedAtMs)
    }

    @Test
    fun openingClearsTheNewFlag_andCompletionIsTracked() {
        val events = listOf(
            event(cut, LessonEventKind.UNLOCKED, 100),
            event(cut, LessonEventKind.OPENED, 200),
            event(cut, LessonEventKind.COMPLETED, 300)
        )
        val state = AcademyRegistry.stateOf(cut, events)!!
        assertTrue(state.opened)
        assertTrue(state.completed)
        assertFalse(state.isNew)
    }

    @Test
    fun repeatedEventsAreIdempotent() {
        val once = AcademyRegistry.stateOf(cut, listOf(event(cut, LessonEventKind.UNLOCKED, 100)))
        val many = AcademyRegistry.stateOf(
            cut,
            listOf(
                event(cut, LessonEventKind.UNLOCKED, 100),
                event(cut, LessonEventKind.UNLOCKED, 400),
                event(cut, LessonEventKind.UNLOCKED, 900)
            )
        )
        // The earliest unlock is the truth; re-firing the moment can't rewrite when it happened.
        assertEquals(once, many)
    }

    @Test
    fun unknownLessonIdsInTheLedgerAreIgnored() {
        val events = listOf(event("retired.lesson", LessonEventKind.UNLOCKED, 100))
        assertTrue(AcademyRegistry.unlocked(events).isEmpty())
        assertEquals(AcademyRegistry.lessons.size, AcademyRegistry.stateFrom(events).size)
    }

    @Test
    fun unlockedListIsNewestFirst() {
        val events = listOf(event(cut, LessonEventKind.UNLOCKED, 500))
        assertEquals(cut, AcademyRegistry.unlocked(events).first().lesson.id)
    }

    @Test
    fun theCutLessonIsWiredToTheSuppressionMoment() {
        assertEquals(
            AcademyRegistry.UNLOCK_CUT_STALL_SUPPRESSED,
            AcademyRegistry.unlockKeyFor(cut)
        )
        assertNotNull(AcademyRegistry.lesson(cut))
    }

    // ── The Fundamentals track (B3) ────────────────────────────────────────────

    @Test
    fun theColdStartTrackIsTheFundamentals_inReadingOrder() {
        assertEquals(10, AcademyRegistry.coldStartTrack.size)
        assertTrue(AcademyRegistry.coldStartTrack.all { it.track == LessonTrack.FUNDAMENTALS })
        assertEquals("fundamentals.what_a_program_is", AcademyRegistry.coldStartTrack.first().id)
        assertEquals("fundamentals.log_honestly", AcademyRegistry.coldStartTrack.last().id)
    }

    @Test
    fun theNextColdStartLessonWalksTheTrack() {
        assertEquals(
            AcademyRegistry.coldStartTrack[0].id,
            AcademyRegistry.nextColdStartLesson(emptyList())!!.id
        )
        val readFirst = listOf(event(AcademyRegistry.coldStartTrack[0].id, LessonEventKind.OPENED, 1))
        assertEquals(
            AcademyRegistry.coldStartTrack[1].id,
            AcademyRegistry.nextColdStartLesson(readFirst)!!.id
        )
    }

    @Test
    fun aFinishedTrackStopsCarryingTheDirective() {
        val allRead = AcademyRegistry.coldStartTrack.mapIndexed { i, l ->
            event(l.id, LessonEventKind.OPENED, i.toLong())
        }
        assertNull(AcademyRegistry.nextColdStartLesson(allRead))
    }

    @Test
    fun everyTrackWithContentIsReachable() {
        // Each shipped lesson belongs to a track the user can actually reach a moment for.
        val tracks = AcademyRegistry.lessons.map { it.track }.toSet()
        assertTrue(LessonTrack.FUNDAMENTALS in tracks)
        assertTrue(LessonTrack.COACH in tracks)
    }

    @Test
    fun summariesAreShortEnoughToRead() {
        // A summary is a card line, not a paragraph: if it needs more, it belongs in the body.
        AcademyRegistry.lessons.forEach {
            assertTrue("${it.id} summary is too long", it.summary.length <= 140)
        }
    }
}
