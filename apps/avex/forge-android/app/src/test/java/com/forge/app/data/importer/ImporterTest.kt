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

    @Test fun neverInfersEquipmentTheSourceNameDidNotState() {
        // Each of these used to fuzzy-match a dumbbell/machine/cable variant at exactly 2/3, filing
        // history under equipment the export never claimed. Unmatched keeps the user's own label.
        assertNull(ExerciseNameMatcher.match("Reverse Fly (Dumbbell)"))  // matched DB Fly — CHEST
        assertNull(ExerciseNameMatcher.match("Bicep Curl"))              // matched a machine curl
        assertNull(ExerciseNameMatcher.match("Good Morning"))            // matched the bodyweight one
        assertNull(ExerciseNameMatcher.match("Crunch"))                  // matched the cable crunch
        assertNull(ExerciseNameMatcher.match("Shoulder Press"))          // matched the barbell press
    }

    @Test fun bareBenchPressNamesAreTheBarbellLifts() {
        assertEquals("barbell-bench-press", ExerciseNameMatcher.match("Bench Press"))
        assertEquals("incline-barbell-bench", ExerciseNameMatcher.match("Incline Bench Press"))
    }

    @Test fun anEquipmentQualifierOnTheSourceNameStillMatches() {
        // The source may be MORE specific than the library name — that invents nothing.
        assertEquals("lat-pulldown", ExerciseNameMatcher.match("Lat Pulldown (Cable)"))
    }

    @Test fun neverResolvesToAnOwnerOnlyPlateCountStation() {
        // leg-extension / leg-curl are ExerciseUnit.PLATES stations; an imported lb value filed under
        // one reads back as a plate count. They stay unmatched and import under their own label.
        assertNull(ExerciseNameMatcher.match("Leg Extension (Machine)"))
        assertNull(ExerciseNameMatcher.match("Leg Curl"))
        assertNull(ExerciseNameMatcher.match("Straight Arm Pulldown"))
        // The exact library name still reaches the WEIGHT-unit entry, not the plate-count leg-curl.
        assertEquals("seated-leg-curl", ExerciseNameMatcher.match("Seated Leg Curl"))
    }

    @Test fun aMovementChangingModifierIsNotDroppedByTheFuzzyPass() {
        assertNull(ExerciseNameMatcher.match("Decline Bench Press (Barbell)"))
        assertNull(ExerciseNameMatcher.match("Decline Bench Press (Dumbbell)"))
        // Scored 0.75 against the flat barbell bench before the guard.
        assertNull(ExerciseNameMatcher.match("Paused Bench Press (Barbell)"))
        // The plain names still match.
        assertEquals("barbell-bench-press", ExerciseNameMatcher.match("Bench Press (Barbell)"))
        assertEquals("db-bench-press", ExerciseNameMatcher.match("Bench Press (Dumbbell)"))
    }

    @Test fun legacyMatchReproducesWhatEarlierBuildsStoredNamesUnder() {
        // Identity only: the duplicate guard must still recognise rows written under these ids.
        assertEquals("barbell-bench-press", ExerciseNameMatcher.legacyMatch("Decline Bench Press (Barbell)"))
        assertEquals("barbell-bench-press", ExerciseNameMatcher.legacyMatch("Paused Bench Press (Barbell)"))
        assertEquals("leg-extension", ExerciseNameMatcher.legacyMatch("Leg Extension (Machine)"))
        assertEquals("leg-curl", ExerciseNameMatcher.legacyMatch("Seated Leg Curl"))
        // Where nothing narrowed, it agrees with today's matcher.
        assertEquals(ExerciseNameMatcher.match("Lat Pulldown (Cable)"), ExerciseNameMatcher.legacyMatch("Lat Pulldown (Cable)"))
        assertNull(ExerciseNameMatcher.legacyMatch("Zercher Carry"))
    }

    // ── Weight parsing ──────────────────────────────────────────────────────────
    @Test fun parseWeightTellsThousandsSeparatorsFromDecimalCommas() {
        // A lone comma with a 1-2 digit tail is a European decimal point...
        assertEquals(82.5, ImportParsing.parseWeight("82,5")!!, 0.001)
        assertEquals(100.5, ImportParsing.parseWeight("100,5")!!, 0.001)
        // ...anything else is a thousands separator. "1,250" used to parse as 1.25.
        assertEquals(1250.0, ImportParsing.parseWeight("1,250")!!, 0.001)
        assertEquals(12345.0, ImportParsing.parseWeight("12,345")!!, 0.001)
        assertEquals(1234567.0, ImportParsing.parseWeight("1,234,567")!!, 0.001)
        // With both separators, the last one is the decimal point.
        assertEquals(1234.5, ImportParsing.parseWeight("1,234.5")!!, 0.001)
        assertEquals(1250.75, ImportParsing.parseWeight("1.250,75")!!, 0.001)
        assertEquals(45.5, ImportParsing.parseWeight("45.5 kg")!!, 0.001)
        assertNull(ImportParsing.parseWeight(""))
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

    @Test fun aUnitWrittenInTheWeightCellOverridesTheAssumedUnit() {
        val lbCsv = "Date,Exercise,Weight,Reps\n2024-11-05,Back Squat,225 lb,5"
        val kgAssumed = GenericCsvImporter().parse(lbCsv, assumeKg = true)
        assertEquals(225.0, kgAssumed.single().exercises[0].sets[0].weightLb!!, 0.001)
        val kgCsv = "Date,Exercise,Weight,Reps\n2024-11-05,Back Squat,100 kg,5"
        val lbAssumed = GenericCsvImporter().parse(kgCsv, assumeKg = false)
        assertEquals(220.5, lbAssumed.single().exercises[0].sets[0].weightLb!!, 0.05)
    }

    @Test fun avexPrListIsNotAWorkoutCsv() {
        val csv = "exercise,muscle,bestWeightLb,reps,date\nBack Squat,Quads,315.0,5,2024-11-05\n"
        assertTrue(ImportParsing.isAvexPrListHeader(ImportParsing.firstLine(csv)))
        assertTrue(!GenericCsvImporter().canParse(csv))
    }

    @Test fun fitNotesSkipsBodyweightAndUnitColumnsAheadOfWeight() {
        val csv = "Date,Exercise,Category,Bodyweight,Weight Unit,Weight,Reps\n" +
            "2024-11-05,Squat,Legs,80,lbs,225,5"
        val importer = FitNotesImporter()
        assertTrue(importer.canParse(csv))
        val set = importer.parse(csv, assumeKg = false).single().exercises[0].sets[0]
        assertEquals(225.0, set.weightLb!!, 0.001)
    }

    @Test fun genericCsvRejectsNonWorkoutFile() {
        assertTrue(!GenericCsvImporter().canParse("name,email\nAda,ada@x.com"))
    }
}
