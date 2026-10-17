package com.forge.app.program

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ExerciseSearchTest {

    private fun names(query: String) = ExerciseSearch.filter(ExerciseLibrary.all, query).map { it.name }

    @Test
    fun theLongWayFindsTheShorthand() {
        assertTrue(names("dumbbell bench").contains("DB Bench Press"))
        assertTrue(names("romanian").contains("Barbell RDL"))
        assertTrue(names("kettlebell swing").contains("KB Swing"))
    }

    @Test
    fun theShorthandFindsTheLongWay() {
        assertTrue(names("db rdl").contains("DB Romanian Deadlift"))
    }

    @Test
    fun everyWordHasToMatchInAnyOrder() {
        val hits = names("incline bench")
        assertTrue(hits.containsAll(listOf("Incline Barbell Bench", "Incline DB Bench Press")))
        assertTrue(hits.none { it == "Barbell Bench Press" })
        assertTrue(names("press bench").contains("DB Bench Press"))
    }

    @Test
    fun spellingsOfTheSameMoveMeet() {
        assertTrue(names("skull crusher").contains("EZ-Bar Skullcrusher"))
        assertTrue(names("triceps extension").contains("DB Overhead Tricep Ext."))
        assertTrue(names("pullup").contains("Pull-Up"))
        assertTrue(names("pull up").contains("Pull-Up"))
    }

    @Test
    fun anImplementFindsItsMovesAfterTheNamedOnes() {
        val hits = names("barbell")
        assertTrue(hits.contains("Back Squat"))
        // Names that say "barbell" come first.
        assertTrue(hits.indexOf("Barbell Row") < hits.indexOf("Back Squat"))
    }

    @Test
    fun aBlankQueryKeepsTheGivenOrder() {
        assertEquals(ExerciseLibrary.all.map { it.name }, names("  "))
    }
}
