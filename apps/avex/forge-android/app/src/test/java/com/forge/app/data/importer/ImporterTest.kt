package com.forge.app.data.importer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Parser + name-matching coverage for the gym-app import feature (#GYMAP-17). These stay off Android
 * types (no org.json) so they run as plain JVM unit tests; the Forge-JSON path and DB insert are
 * exercised in the app.
 */
class ImporterTest {

    // ── CSV parsing ────────────────────────────────────────────────────────────
    @Test fun csvHandlesQuotedCommasAndEscapedQuotes() {
        val rows = CsvParser.parse("a,b,c\n\"x,y\",\"he said \"\"hi\"\"\",z")
        assertEquals(2, rows.size)
        assertEquals(listOf("x,y", "he said \"hi\"", "z"), rows[1])
    }

    @Test fun csvDetectsSemicolonDelimiter() {
        val rows = CsvParser.parse("a;b;c\n1;2;3")
        assertEquals(listOf("1", "2", "3"), rows[1])
    }

    @Test fun csvStripsBomAndBlankRows() {
        val rows = CsvParser.parse("﻿a,b\n\n1,2\n")
        assertEquals(2, rows.size)
        assertEquals(listOf("a", "b"), rows[0])
    }

    // ── Exercise name matching ──────────────────────────────────────────────────
    @Test fun matchesEquipmentParentheticalToLibraryId() {
        assertEquals("barbell-bench-press", ExerciseNameMatcher.match("Bench Press (Barbell)"))
        assertEquals("db-bench-press", ExerciseNameMatcher.match("Bench Press (Dumbbell)"))
        assertEquals("incline-db-bench-press", ExerciseNameMatcher.match("Incline Bench Press (Dumbbell)"))
    }

    @Test fun matchesCuratedAndAbbreviatedNames() {
        assertEquals("back-squat", ExerciseNameMatcher.match("Squat (Barbell)"))
        assertEquals("conventional-deadlift", ExerciseNameMatcher.match("Deadlift"))
        assertEquals("barbell-rdl", ExerciseNameMatcher.match("Romanian Deadlift (Barbell)"))
        assertEquals("lat-pulldown", ExerciseNameMatcher.match("Lat Pulldown (Cable)"))
    }

    @Test fun unknownExerciseReturnsNull() {
        assertNull(ExerciseNameMatcher.match("Zercher Carry"))
        assertNull(ExerciseNameMatcher.match(""))
    }

    // ── Strong ──────────────────────────────────────────────────────────────────
    private val strongCsv = """
        Date,Workout Name,Duration,Exercise Name,Set Order,Weight,Weight Unit,Reps,RPE
        2024-11-05 18:00:00,Push,1h 2m,Bench Press (Barbell),1,100,kg,5,8
        2024-11-05 18:00:00,Push,1h 2m,Bench Press (Barbell),2,100,kg,5,
        2024-11-05 18:00:00,Push,1h 2m,Overhead Press (Barbell),1,60,kg,8,
    """.trimIndent()

    @Test fun strongGroupsSetsIntoOneWorkout() {
        val importer = StrongImporter()
        assertTrue(importer.canParse(strongCsv))
        val sessions = importer.parse(strongCsv, assumeKg = false)
        assertEquals(1, sessions.size)
        val s = sessions.first()
        assertEquals(2, s.exercises.size)
        assertEquals(2, s.exercises[0].sets.size)
        // 100 kg → 220.5 lb (rounded to 0.1)
        assertEquals(220.5, s.exercises[0].sets[0].weightLb!!, 0.05)
        assertEquals(8.0, s.exercises[0].sets[0].rpe!!, 0.001)
        // 1h 2m duration is applied to the finish time.
        assertEquals(s.startedAtMs + 3_720_000L, s.finishedAtMs)
    }

    @Test fun strongWithoutUnitColumnUsesAssumedUnit() {
        val csv = "Date,Workout Name,Exercise Name,Set Order,Weight,Reps\n" +
            "2024-11-05,Legs,Back Squat,1,225,5"
        val lb = StrongImporter().parse(csv, assumeKg = false).first().exercises[0].sets[0].weightLb!!
        assertEquals(225.0, lb, 0.001)
    }

    // ── Hevy ──────────────────────────────────────────────────────────────────
    @Test fun hevyConvertsKgAndGroupsByStartTime() {
        val csv = "title,start_time,end_time,description,exercise_title,superset_id,exercise_notes," +
            "set_index,set_type,weight_kg,reps,distance_km,duration_seconds,rpe\n" +
            "Push,2024-11-05 18:30:00,2024-11-05 19:15:00,,Bench Press (Barbell),,,0,normal,100,5,,,8\n" +
            "Push,2024-11-05 18:30:00,2024-11-05 19:15:00,,Bench Press (Barbell),,,1,warmup,60,10,,,"
        val importer = HevyImporter()
        assertTrue(importer.canParse(csv))
        val sessions = importer.parse(csv, assumeKg = false)
        assertEquals(1, sessions.size)
        assertEquals(2, sessions.first().exercises[0].sets.size)
        assertEquals(220.5, sessions.first().exercises[0].sets[0].weightLb!!, 0.05)
        assertTrue(sessions.first().exercises[0].sets[1].isWarmup)
    }

    // ── FitNotes ──────────────────────────────────────────────────────────────
    @Test fun fitNotesReadsUnitFromWeightHeaderAndGroupsByDate() {
        val csv = "Date,Exercise,Category,Weight (kgs),Reps,Distance,Distance Unit,Time,Comment\n" +
            "2024-11-05,Barbell Bench Press,Chest,100,5,,,,\n" +
            "2024-11-05,Squat,Legs,140,3,,,,"
        val importer = FitNotesImporter()
        assertTrue(importer.canParse(csv))
        val sessions = importer.parse(csv, assumeKg = false)
        assertEquals(1, sessions.size) // both rows share a date → one session
        assertEquals(2, sessions.first().exercises.size)
        assertEquals(220.5, sessions.first().exercises[0].sets[0].weightLb!!, 0.05)
    }

    // ── Generic CSV fallback ────────────────────────────────────────────────────
    @Test fun genericCsvMatchesFuzzyHeaders() {
        val csv = "Date,Workout,Exercise,Weight,Reps\n" +
            "2024-11-05,Leg Day,Back Squat,225,5\n" +
            "2024-11-05,Leg Day,Back Squat,225,5"
        val importer = GenericCsvImporter()
        assertTrue(importer.canParse(csv))
        val sessions = importer.parse(csv, assumeKg = false)
        assertEquals(1, sessions.size)
        assertEquals(2, sessions.first().exercises[0].sets.size)
        assertEquals(225.0, sessions.first().exercises[0].sets[0].weightLb!!, 0.001)
    }

    @Test fun genericCsvRejectsNonWorkoutFile() {
        assertTrue(!GenericCsvImporter().canParse("name,email\nAda,ada@x.com"))
    }
}
