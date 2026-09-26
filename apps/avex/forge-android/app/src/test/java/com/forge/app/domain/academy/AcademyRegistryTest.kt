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

    private val cut = "training.protein"

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
            assertTrue("${l.id} needs an unlock label", l.unlock.label.isNotBlank())
            assertTrue("${l.id} needs an unlock detail", l.unlock.detail.isNotBlank())
            // The detail earns its line only by adding something the label didn't say.
            assertTrue(
                "${l.id}: unlock detail just restates its label",
                !l.unlock.detail.equals(l.unlock.label, ignoreCase = true)
            )
            // byYou promises the reader can act: those read as instructions, not as observations.
            if (l.unlock.byYou) assertTrue(
                "${l.id}: a by-you unlock must be an instruction, not a 'When ...' moment",
                !l.unlock.label.startsWith("When ", ignoreCase = true)
            ) else assertTrue(
                "${l.id}: a coach-side unlock must name the moment, so it can't read as a task",
                l.unlock.label.startsWith("When ", ignoreCase = true) ||
                    l.unlock.label.startsWith("The first time", ignoreCase = true)
            )
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
                add(lesson.title); add(lesson.summary)
                add(lesson.unlock.label); add(lesson.unlock.detail)
                lesson.blocks.forEach { b ->
                    when (b) {
                        is LessonBlock.Heading -> add(b.text)
                        is LessonBlock.Paragraph -> add(b.text)
                        is LessonBlock.Callout -> add(b.text)
                        is LessonBlock.Bullets -> addAll(b.items)
                        is LessonBlock.Example -> { add(b.label); add(b.fallback) }
                        is LessonBlock.Figure -> add(b.caption)
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
    fun theProteinLessonIsWiredToTheCutSuppressionMoment() {
        assertEquals(
            AcademyRegistry.UNLOCK_CUT_STALL_SUPPRESSED,
            AcademyRegistry.unlockKeyFor(cut)
        )
        assertNotNull(AcademyRegistry.lesson(cut))
    }

    // ── The 2026-09-26 cut: 35 pieces to 12 ────────────────────────────────────

    @Test
    fun theAcademyShipsTwelveLessonsInThreeChapters() {
        assertEquals(12, AcademyRegistry.lessons.size)
        assertEquals(7, AcademyRegistry.byTrack(LessonTrack.TRAINING).size)
        assertEquals(3, AcademyRegistry.byTrack(LessonTrack.COACH).size)
        assertEquals(2, AcademyRegistry.byTrack(LessonTrack.CARDIO).size)
    }

    @Test
    fun idsCarryTheirChapterAndStayRouteSafe() {
        AcademyRegistry.lessons.forEach {
            assertTrue("${it.id} must start with ${it.track.code}.", it.id.startsWith("${it.track.code}."))
            assertTrue("${it.id} must be lowercase, dots and underscores", it.id.matches(Regex("[a-z0-9_.]+")))
        }
    }

    @Test
    fun everyRetiredIdResolvesToAShippedLesson() {
        val shipped = AcademyRegistry.lessons.map { it.id }.toSet()
        AcademyRegistry.aliases.forEach { (old, new) ->
            assertTrue("$old points at $new, which does not ship", new in shipped)
            assertFalse("$old is both retired and shipped", old in shipped)
            assertEquals(new, AcademyRegistry.lesson(old)?.id)
        }
        // The ids the coach's own reasons still carry.
        listOf(
            "coach.readiness_built_from", "fundamentals.how_the_coach_works", "coach.why_goals_fight",
            "signals.stress_hrv", "coach.strength_on_a_cut", "programming.imbalances",
            "coach.what_a_project_is", "programming.what_a_block_is", "engine.what_zone2_is",
            "engine.intervals"
        ).forEach { assertNotNull("$it no longer resolves", AcademyRegistry.lesson(it)) }
    }

    @Test
    fun aReadUnderARetiredIdCountsForTheLessonThatAbsorbedIt() {
        val events = listOf(
            event("fundamentals.warmups", LessonEventKind.UNLOCKED, 10),
            event("fundamentals.warmups", LessonEventKind.OPENED, 20)
        )
        val form = AcademyRegistry.stateOf("training.form", events)!!
        assertTrue(form.unlocked)
        assertTrue(form.opened)
        assertFalse("a lesson read under its old id is not new again", form.isNew)
    }

    @Test
    fun lessonsThatCiteResearchNameEverySource() {
        AcademyRegistry.lessons.flatMap { l -> l.sources.map { l.id to it } }.forEach { (id, src) ->
            assertTrue("$id: source needs authors", src.authors.isNotBlank())
            assertTrue("$id: source needs a title", src.title.isNotBlank())
            assertTrue("$id: implausible year ${src.year}", src.year in 1950..2030)
        }
    }

    // ── The Training chapter is the cold-start curriculum (B3) ─────────────────

    @Test
    fun theColdStartTrackIsTraining_inReadingOrder() {
        assertEquals(7, AcademyRegistry.coldStartTrack.size)
        assertTrue(AcademyRegistry.coldStartTrack.all { it.track == LessonTrack.TRAINING })
        assertEquals("training.getting_stronger", AcademyRegistry.coldStartTrack.first().id)
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
    fun summariesAreShortEnoughToRead() {
        // A summary is a line in the contents, not a paragraph: if it needs more, it belongs in the body.
        AcademyRegistry.lessons.forEach {
            assertTrue("${it.id} summary is too long", it.summary.length <= 140)
        }
    }
}
