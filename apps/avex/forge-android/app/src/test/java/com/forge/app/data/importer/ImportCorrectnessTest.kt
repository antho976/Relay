package com.forge.app.data.importer

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.Instant
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.ZoneId
import java.util.Locale

/** Audit 2026-09-26, step 4: the parsing half of import correctness. */
class ImportCorrectnessTest {

    private val originalLocale: Locale = Locale.getDefault()

    @After
    fun restoreLocale() = Locale.setDefault(originalLocale)

    private fun day(millis: Long?): LocalDate? =
        millis?.let { Instant.ofEpochMilli(it).atZone(ZoneId.systemDefault()).toLocalDate() }

    // ── Hevy units (P1 #9) ──────────────────────────────────────────────────────

    @Test
    fun hevyImperialExportKeepsItsPoundWeights() {
        val csv = """
            title,start_time,end_time,description,exercise_title,superset_id,exercise_notes,set_index,set_type,weight_lbs,reps,distance_miles,duration_seconds,rpe
            Push,2024-06-01 18:00:00,2024-06-01 19:00:00,,Bench Press (Barbell),,,0,normal,225,5,,,
        """.trimIndent()

        val set = HevyImporter().parse(csv, assumeKg = true).single().exercises.single().sets.single()

        assertEquals("pounds stay pounds, whatever the app unit", 225.0, set.weightLb!!, 0.001)
        assertEquals(5, set.reps)
    }

    @Test
    fun hevyMetricExportStillConvertsKilograms() {
        val csv = """
            title,start_time,end_time,exercise_title,set_index,set_type,weight_kg,reps
            Push,2024-06-01 18:00:00,2024-06-01 19:00:00,Bench Press (Barbell),0,normal,100,5
        """.trimIndent()

        val set = HevyImporter().parse(csv, assumeKg = false).single().exercises.single().sets.single()

        assertEquals(220.5, set.weightLb!!, 0.1)
    }

    @Test
    fun anImperialHevyHeaderIsStillRecognised() {
        assertTrue(HevyImporter().canParse("title,exercise_title,weight_lbs,reps\n"))
    }

    // ── Cardio rows (09 P2, regression of bug-scan 02) ─────────────────────────

    private val strongHeader =
        "Date,Workout Name,Duration,Exercise Name,Set Order,Weight,Weight Unit,Reps,RPE,Distance,Distance Unit,Seconds,Notes,Workout Notes"

    @Test
    fun strongRunIsCardioNotAPhantomHold() {
        // The prior audit's exact row.
        val csv = "$strongHeader\n2024-06-02 07:15:00,Morning Cardio,45m,Running,1,0,kg,0,,5,km,1800,,"

        val importer = StrongImporter()
        assertTrue("no strength session", importer.parse(csv, assumeKg = true).isEmpty())
        val run = importer.parseExtras(csv, assumeKg = true).cardio.single()
        assertEquals("run", run.type)
        assertEquals(30, run.durationMin)
        assertEquals(5.0, run.distanceKm!!, 0.0001)
    }

    @Test
    fun strongCardioInMilesIsStoredInKilometres() {
        val csv = "$strongHeader\n2024-06-02 07:15:00,Cardio,30m,Cycling,1,0,lbs,0,,10,mi,1800,,"

        val ride = StrongImporter().parseExtras(csv, assumeKg = false).cardio.single()

        assertEquals("cycle", ride.type)
        assertEquals(16.093, ride.distanceKm!!, 0.001)
    }

    @Test
    fun aDurationOnlyCardioRowIsCardioButAPlankIsStillAHold() {
        val csv = strongHeader + "\n" +
            "2024-06-02 07:15:00,Mixed,45m,Treadmill,1,0,kg,0,,,,1200,,\n" +
            "2024-06-02 07:15:00,Mixed,45m,Plank,1,0,kg,0,,,,90,,"

        val importer = StrongImporter()
        val treadmill = importer.parseExtras(csv, assumeKg = true).cardio.single()
        val plank = importer.parse(csv, assumeKg = true).single().exercises.single()

        assertEquals("treadmill", treadmill.type)
        assertEquals(20, treadmill.durationMin)
        assertEquals("Plank", plank.name)
        assertEquals(90, plank.sets.single().durationSeconds)
    }

    @Test
    fun aLoadedCarryWithADistanceStaysASet() {
        val csv = "$strongHeader\n2024-06-02 07:15:00,Strongman,45m,Farmer's Walk,1,40,kg,0,,50,m,,,"

        val importer = StrongImporter()
        assertTrue(importer.parseExtras(csv, assumeKg = true).cardio.isEmpty())
        assertEquals(1, importer.parse(csv, assumeKg = true).single().exercises.size)
    }

    @Test
    fun hevyCardioRowIsCardio() {
        val csv = """
            title,start_time,end_time,exercise_title,set_index,set_type,weight_kg,reps,distance_km,duration_seconds
            Run,2024-06-01 07:00:00,2024-06-01 07:40:00,Running,0,normal,,,6.2,2400
        """.trimIndent()

        val importer = HevyImporter()
        assertTrue(importer.parse(csv, assumeKg = true).isEmpty())
        val run = importer.parseExtras(csv, assumeKg = true).cardio.single()
        assertEquals("run", run.type)
        assertEquals(40, run.durationMin)
        assertEquals(6.2, run.distanceKm!!, 0.0001)
    }

    // ── Generic CSV dates (09 P2) ───────────────────────────────────────────────

    @Test
    fun spreadsheetDatesThatUsedToBeDroppedNowParse() {
        Locale.setDefault(Locale.US)
        assertEquals(LocalDate.of(2024, 5, 4), day(ImportParsing.parseEpochMillis("5/4/2024")))
        assertEquals(LocalDate.of(2024, 5, 4), day(ImportParsing.parseEpochMillis("05/04/2024 18:30")))
        assertEquals(LocalDate.of(2024, 5, 4), day(ImportParsing.parseEpochMillis("5/4/2024 6:30 PM")))
        val local = LocalDateTime.of(2024, 5, 6, 18, 30).atZone(ZoneId.systemDefault()).toInstant().toEpochMilli()
        assertEquals(local, ImportParsing.parseEpochMillis("2024-05-06T18:30"))
        assertEquals(local + 123, ImportParsing.parseEpochMillis("2024-05-06T18:30:00.123"))
        assertEquals(local, ImportParsing.parseEpochMillis("2024-05-06 18:30"))
    }

    @Test
    fun theNamedFormatsStillParse() {
        assertNotNull(ImportParsing.parseEpochMillis("4 May 2024, 18:30"))
        assertNotNull(ImportParsing.parseEpochMillis("May 4, 2024"))
        assertNotNull(ImportParsing.parseEpochMillis("2024/05/04"))
        assertNull(ImportParsing.parseEpochMillis("13/13/2024"))
    }

    @Test
    fun aDayFirstFileIsReadDayFirstOnAMonthFirstPhone() {
        Locale.setDefault(Locale.US)
        val csv = "Date,Exercise,Weight,Reps\n13/05/2024,Squat,100,5\n04/05/2024,Squat,100,5\n"

        val days = GenericCsvImporter().parse(csv, assumeKg = true).map { day(it.startedAtMs) }

        assertEquals(listOf(LocalDate.of(2024, 5, 13), LocalDate.of(2024, 5, 4)), days)
    }

    @Test
    fun aMonthFirstFileIsReadMonthFirstOnADayFirstPhone() {
        Locale.setDefault(Locale.UK)
        val csv = "Date,Exercise,Weight,Reps\n5/4/2024,Squat,100,5\n5/13/2024,Squat,100,5\n"

        val days = GenericCsvImporter().parse(csv, assumeKg = true).map { day(it.startedAtMs) }

        assertEquals(listOf(LocalDate.of(2024, 5, 4), LocalDate.of(2024, 5, 13)), days)
    }

    @Test
    fun anAllAmbiguousFileFollowsThePhone() {
        Locale.setDefault(Locale.UK)
        assertEquals(false, ImportParsing.slashDatesMonthFirst(sequenceOf("04/05/2024", "06/07/2024")))
        Locale.setDefault(Locale.US)
        assertEquals(true, ImportParsing.slashDatesMonthFirst(sequenceOf("04/05/2024", "06/07/2024")))
    }

    // ── P3s: holds, rest timers, skipped rows, synthetic ids ────────────────────

    private val fitNotesHeader = "Date,Exercise,Category,Weight,Weight Unit,Reps,Distance,Distance Unit,Time,Comment"

    @Test
    fun fitNotesReadsTheTimeColumnAsAHold() {
        val csv = "$fitNotesHeader\n2024-06-01,Plank,Abs,0,kgs,0,,,0:01:30,"

        val hold = FitNotesImporter().parse(csv, assumeKg = true).single().exercises.single().sets.single()

        assertEquals(90, hold.durationSeconds)
    }

    @Test
    fun fitNotesCardioIsCardio() {
        val csv = fitNotesHeader + "\n" +
            "2024-06-01,Running (Outdoor),Cardio,,,,5.0,km,0:28:00,\n" +
            "2024-06-01,Stair Climber,Cardio,,,,,,0:15:00,"

        val read = FitNotesImporter().read(csv, assumeKg = true)

        assertTrue(read.sessions.isEmpty())
        assertEquals(listOf("run" to 28, "other" to 15), read.extras.cardio.map { it.type to it.durationMin })
        assertEquals(0, read.skippedRows)
    }

    @Test
    fun genericCsvReadsOnlyASecondsHeaderAsAHold() {
        val withSeconds = "Date,Exercise,Weight,Reps,Seconds\n2024-06-01,Plank,0,0,60\n"
        assertEquals(60, GenericCsvImporter().parse(withSeconds, true).single().exercises.single().sets.single().durationSeconds)

        // A bare Duration is as likely the workout's length: every set is NOT a 45-minute hold.
        val withDuration = "Date,Exercise,Weight,Reps,Duration\n2024-06-01,Bench Press,100,5,45\n"
        assertNull(GenericCsvImporter().parse(withDuration, true).single().exercises.single().sets.single().durationSeconds)
    }

    @Test
    fun strongRestTimerRowsAreNeitherSetsNorSkipped() {
        val csv = strongHeader + "\n" +
            "2024-06-02 07:15:00,Push,45m,Bench Press (Barbell),1,100,kg,5,,,,,,\n" +
            "2024-06-02 07:15:00,Push,45m,Bench Press (Barbell),Rest Timer,0,kg,0,,,,120,,"

        val read = StrongImporter().read(csv, assumeKg = true)

        assertEquals(1, read.sessions.single().exercises.single().sets.size)
        assertEquals(0, read.skippedRows)
    }

    @Test
    fun unreadableRowsAreCounted() {
        val csv = strongHeader + "\n" +
            "2024-06-02 07:15:00,Push,45m,Bench Press (Barbell),1,100,kg,5,,,,,,\n" +
            "someday,Push,45m,Bench Press (Barbell),2,100,kg,5,,,,,,\n" +
            "2024-06-02 07:15:00,Push,45m,,3,100,kg,5,,,,,,\n" +
            "2024-06-02 07:15:00,Push,45m,Bench Press (Barbell),4,0,kg,0,,,,,,"

        assertEquals(3, StrongImporter().read(csv, assumeKg = true).skippedRows)
    }

    @Test
    fun theResultSaysHowManyRowsWereSkipped() {
        val success = ImportResult.Success(ImportSource.STRONG, 1, 1, 3, 1, 0, skippedRows = 2)
        assertTrue(success.userMessage(), success.userMessage().endsWith("2 rows couldn't be read and were skipped."))
        val none = ImportResult.NoReadableRows(ImportSource.GENERIC_CSV, 12).userMessage()
        assertTrue(none, none.startsWith("None of the 12 rows"))
    }

    @Test
    fun longUnmatchedNamesNoLongerShareAnId() {
        val a = WorkoutImportRepository.syntheticIdOf("Incline Dumbbell Bench Press (Neutral Grip)")
        val b = WorkoutImportRepository.syntheticIdOf("Incline Dumbbell Bench Press (Neutral Grip, Paused)")

        assertTrue("$a vs $b", a != b)
        assertEquals("a name that fits keeps its id", "ext-zercher-squat", WorkoutImportRepository.syntheticIdOf("Zercher Squat"))
        assertEquals(a, WorkoutImportRepository.syntheticIdOf("  incline dumbbell bench press (neutral grip) "))
    }
}
