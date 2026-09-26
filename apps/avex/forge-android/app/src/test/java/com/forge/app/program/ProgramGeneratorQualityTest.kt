package com.forge.app.program

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Regression suite for the 2026-09-21 plan-generation review: overlapping movements, the same
 * muscle back-to-back, and a weaker movement chosen when a better one was available. Each test
 * pins one finding from `docs/PLAN_GENERATION_REVIEW_2026-09-21.md`. Statistical checks run many
 * seeds and assert on a rate the review measured, with headroom.
 */
class ProgramGeneratorQualityTest {

    private val full = emptySet<Equipment>()
    private val commercial = setOf(
        Equipment.DUMBBELLS, Equipment.BARBELL, Equipment.SQUAT_RACK, Equipment.BENCH, Equipment.INCLINE_BENCH,
        Equipment.CABLE, Equipment.MACHINE, Equipment.PULL_UP_BAR, Equipment.EZ_BAR
    )
    private val mwm = setOf(Equipment.DUMBBELLS, Equipment.BENCH, Equipment.CABLE, Equipment.MACHINE)

    private fun defs(day: GeneratedDay) = day.exercises.map { ExerciseLibrary.byId(it.libId)!! }
    private fun weeks(days: Int, eq: Set<Equipment>, n: Int, params: GenerationParams = GenerationParams(days)) =
        (0 until n).map { ProgramGenerator.generate(params.copy(daysPerWeek = days), eq, emptySet(), emptySet(), seed = it.toLong()) }

    @Test
    fun noMuscleIsTrainedBackToBackWithinADay() {
        // Finding 7: push ended triceps/triceps and pull opened back/back/back — 40% of 5-day days.
        // Templates are now interleaved; the generator never reorders, so this is a template check
        // for every day-count and a generated check for equipment that can drop slots.
        (1..7).forEach { d ->
            SplitTemplates.forDays(d).forEach { day ->
                val adjacent = day.targets.zipWithNext().count { (a, b) -> a.muscle == b.muscle }
                assertEquals("${day.key} ($d-day) has back-to-back slots of one muscle", 0, adjacent)
            }
        }
        listOf(full, commercial, mwm).forEach { eq ->
            weeks(6, eq, 50).flatten().forEach { day ->
                val adjacent = defs(day).zipWithNext().count { (a, b) -> a.muscle == b.muscle }
                assertEquals("${day.key} generated with a same-muscle pair", 0, adjacent)
            }
        }
    }

    @Test
    fun secondHamstringSlotIsRarelyASecondHinge() {
        // Finding 1: 38% of full-gym leg days ran two deadlift variants. The pattern-repeat penalty
        // is now shared by the whole hinge group, so one fresh leg curl out-weighs five stale hinges.
        var legDays = 0; var twoHinges = 0
        weeks(5, commercial, 400).flatten().filter { it.key == "legs" }.forEach { day ->
            legDays++
            if (defs(day).count { ExerciseLibrary.patternOf(it) == MovementPattern.HINGE } >= 2) twoHinges++
        }
        assertTrue("two hinges on $twoHinges/$legDays leg days", twoHinges <= legDays / 10)
    }

    @Test
    fun heavyQuadSlotLeadsWithASquatPattern() {
        // Finding 3: a trap-bar deadlift could open the leg day and force the hamstring lead into
        // a second hinge. The quad STRENGTH slot now prefers the SQUAT pattern.
        var lead = 0; var squat = 0
        weeks(5, full, 400).flatten().filter { it.key == "legs" || it.key == "lower" }.forEach { day ->
            lead++
            if (ExerciseLibrary.patternOf(defs(day).first()) == MovementPattern.SQUAT) squat++
        }
        assertTrue("squat-pattern leads $squat/$lead", squat >= lead * 95 / 100)
    }

    @Test
    fun nearDuplicateIsolationsRarelyShareADay() {
        // Finding 2: DB + cable lateral raise on 25% of push days; barbell + EZ curl, two skull
        // crushers, fly + pec deck likewise. Isolation families now count as pattern repeats.
        var pushDays = 0; var bothLaterals = 0; var days = 0; var familyDupes = 0
        weeks(5, commercial, 400).flatten().forEach { day ->
            days++
            // Two biceps slots on a pull day are both curls by design; every other family repeat is avoidable.
            val patterns = defs(day).map { ExerciseLibrary.patternOf(it) }
                .filter { it.isIsolation && it != MovementPattern.ISOLATION && it != MovementPattern.CURL }
            if (patterns.size != patterns.toSet().size) familyDupes++
            if (day.key == "push") {
                pushDays++
                val ids = day.exercises.map { it.libId }
                if ("db-lateral-raise" in ids && "cable-lateral-raise" in ids) bothLaterals++
            }
        }
        assertTrue("both lateral raises on $bothLaterals/$pushDays push days", bothLaterals <= pushDays / 20)
        assertTrue("isolation family repeated on $familyDupes/$days days", familyDupes <= days / 12)
    }

    @Test
    fun heavySlotsPreferLoadableMovementsWhenABarIsAvailable() {
        // Finding 4: barbell row, suspension row, chest-supported DB row and lat pulldown were
        // equally likely heavy leads. Loadability tiers now steer the STRENGTH slot.
        var strength = 0; var bodyweight = 0; var handheld = 0
        weeks(5, full, 300).forEach { week ->
            val template = SplitTemplates.forDays(5)
            week.forEachIndexed { di, day ->
                val ds = defs(day)
                if (ds.size != template[di].targets.size) return@forEachIndexed
                template[di].targets.forEachIndexed { si, slot ->
                    if (slot.scheme != RepScheme.STRENGTH) return@forEachIndexed
                    strength++
                    if (ds[si].unit == ExerciseUnit.BODYWEIGHT) bodyweight++
                    if (ds[si].unit == ExerciseUnit.DUMBBELL || Equipment.KETTLEBELL in ds[si].equipment) handheld++
                }
            }
        }
        assertTrue("bodyweight-led heavy slots $bodyweight/$strength", bodyweight <= strength * 6 / 100)
        assertTrue("handheld-led heavy slots $handheld/$strength", handheld <= strength * 32 / 100)
    }

    @Test
    fun pinnedIsolationNeverTakesTheHeavySlotAndTheDayKeepsItsPress() {
        // Finding 5: a pinned DB fly led push AND upper at 6-10 and the push day lost its press.
        (0 until 40).forEach { s ->
            val week = ProgramGenerator.generate(GenerationParams(5, pinned = setOf("db-fly")), full, emptySet(), emptySet(), seed = s.toLong())
            val push = week.first { it.key == "push" }
            assertEquals("seed $s: fly should be the second chest movement", "db-fly", push.exercises[2].libId)
            assertTrue("seed $s: push must still open with a press", ExerciseTag.COMPOUND in defs(push).first().tags)
            assertTrue("seed $s: fly must never carry strength reps",
                week.flatMap { it.exercises }.filter { it.libId == "db-fly" }.none { it.reps == "6-10" })
            val upper = week.first { it.key == "upper" }
            assertTrue("seed $s: upper's only chest slot is heavy — the pin is not placed there",
                upper.exercises.none { it.libId == "db-fly" })
        }
    }

    @Test
    fun aPinIsNeverPlacedTwiceInOneDay() {
        // Audit 2026-09-26: an isolation pin reserves the muscle's last PUMP slot, but earlier slots
        // could still draw it, and the pin was then placed again — ~37% of dumbbell-only weeks with
        // a pinned hammer curl. Both copies share one plan id, which breaks per-exercise logging.
        val dbBench = setOf(Equipment.DUMBBELLS, Equipment.BENCH)
        listOf(dbBench to "db-hammer-curl", full to "db-lateral-raise").forEach { (eq, pin) ->
            (0 until 300).forEach { s ->
                ProgramGenerator.generate(GenerationParams(5, pinned = setOf(pin)), eq, emptySet(), emptySet(), seed = s.toLong())
                    .forEach { day ->
                        val ids = day.exercises.map { it.libId }
                        assertEquals("seed $s, $pin, ${day.key}: $ids", ids.size, ids.toSet().size)
                    }
            }
        }
    }

    @Test
    fun pinnedCompoundStillTakesTheHeavySlot() {
        (0 until 20).forEach { s ->
            val legs = ProgramGenerator.generate(GenerationParams(5, pinned = setOf("back-squat")), full, emptySet(), emptySet(), seed = s.toLong())
                .first { it.key == "legs" }
            assertEquals("seed $s", "back-squat", legs.exercises.first().libId)
        }
    }

    @Test
    fun fixedRepMovementsKeepTheirOwnRangeAndStrandedIsolationsNeverGetStrengthReps() {
        // Finding 6 + 13: a KB swing was prescribed 8-12 (or 6-8 under get_stronger); a wall sit or
        // leg curl left alone in a STRENGTH slot by a hip restriction was prescribed 6-10.
        weeks(5, full, 60, GenerationParams(5, goal = "get_stronger")).flatten().flatMap { it.exercises }.forEach { ex ->
            val def = ExerciseLibrary.byId(ex.libId)!!
            if (def.fixedReps) assertEquals(def.id, def.defaultReps, ex.reps)
        }
        weeks(5, full, 30, GenerationParams(5, problemAreas = setOf(ProblemArea.HIPS))).flatten().flatMap { it.exercises }.forEach { ex ->
            val def = ExerciseLibrary.byId(ex.libId)!!
            if (ExerciseTag.COMPOUND !in def.tags) {
                assertTrue("${def.id} got heavy reps ${ex.reps}", ex.reps != "6-10" && ex.reps != "4-6")
            }
        }
    }

    @Test
    fun repeatedDaySplitsUseComplementaryTemplates() {
        // Finding 8: 6/7-day splits ran identical push/pull/legs templates for A and B.
        val six = SplitTemplates.forDays(6)
        assertEquals(MuscleGroup.SHOULDERS, six.first { it.key == "push-b" }.targets.first().muscle)
        assertEquals(MovementPattern.HINGE, six.first { it.key == "legs-b" }.targets.first().pattern)
        assertEquals(MovementPattern.HORIZONTAL_PULL, six.first { it.key == "pull-b" }.targets.first().pattern)
        assertEquals(MovementPattern.VERTICAL_PULL, six.first { it.key == "pull-a" }.targets.first().pattern)
        // Finding 9: the 3-day split ran full-body A twice.
        val three = SplitTemplates.forDays(3)
        assertTrue(three.map { it.targets }.toSet().size == 3)
        val weekly = VolumeTargets.plannedWeeklySetsByMuscle(
            ProgramGenerator.generate(GenerationParams(3), full, emptySet(), emptySet(), seed = 2L).map { d ->
                DayPlan(d.key, d.name, "", d.word, d.accentHex, emptyList(), d.exercises.map { ExerciseLibrary.byId(it.libId)!!.toPlan().copy(sets = it.sets) })
            }
        )
        listOf(MuscleGroup.GLUTES, MuscleGroup.CALVES).forEach { m ->
            assertTrue("3-day $m should be trained twice a week (${weekly[m]} sets)", (weekly[m] ?: 0) >= 4)
        }
    }

    @Test
    fun volumeTrimsAccessoriesBeforeTheHeavyCompound() {
        // Finding 10: an advanced leg day ran 3 sets of squats next to 4 of leg extensions because
        // the session cap shaved the largest slot and the compound lost the tie.
        val days = SplitTemplates.forDays(5)
        val sets = VolumeModel.allocate(days, volumeFactor = GoalProfiles.volumeFactor("advanced"))
        days.forEachIndexed { di, day ->
            val strength = day.targets.indices.filter { day.targets[it].scheme == RepScheme.STRENGTH }.map { sets[di][it] }
            val rest = day.targets.indices.filter { day.targets[it].scheme != RepScheme.STRENGTH }.map { sets[di][it] }
            if (strength.isNotEmpty() && rest.isNotEmpty()) {
                assertTrue("${day.key}: strength $strength should not sit below accessories $rest",
                    strength.min() >= rest.max())
            }
        }
    }

    @Test
    fun sameSeedReproducesTheProgramAcrossDeloadAndRestore() {
        // Finding 11: deload and restore drew fresh seeds — 3 of 30 slots survived the cycle.
        // With the seed replayed, a deload changes sets only and the restore hands back the plan.
        val base = ProgramGenerator.generate(GenerationParams(5), commercial, emptySet(), emptySet(), seed = 77L)
        val deload = ProgramGenerator.generate(GenerationParams(5, deload = true), commercial, emptySet(), emptySet(), seed = 77L)
        val restored = ProgramGenerator.generate(GenerationParams(5), commercial, emptySet(), emptySet(), seed = 77L)
        assertEquals(base.map { d -> d.exercises.map { it.libId } }, deload.map { d -> d.exercises.map { it.libId } })
        assertEquals(base, restored)
        assertTrue(deload.flatMap { it.exercises }.sumOf { it.sets } < base.flatMap { it.exercises }.sumOf { it.sets })
    }

    @Test
    fun swapRankingProposesAFreshVariationNotTheLibraryDefault() {
        // Finding 12: a stalled back squat was told to rotate to a goblet squat (library order).
        val pool = ExerciseLibrary.swapCandidates(MuscleGroup.QUADS, commercial, emptySet())
        val ranked = ProgramGenerator.rankSwapCandidates(ExerciseLibrary.byId("back-squat")!!, pool).map { it.id }
        assertTrue("top 3 for a stalled back squat: ${ranked.take(3)}",
            ranked.take(3).all { ExerciseLibrary.patternOf(ExerciseLibrary.byId(it)!!) == MovementPattern.SQUAT })
        assertTrue("a loadable machine/bar squat should out-rank the goblet squat",
            ranked.indexOf("goblet-squat") > ranked.indexOf("hack-squat"))
        // A stalled fly must not be sent to the press the day already opens with.
        val chest = ExerciseLibrary.swapCandidates(MuscleGroup.CHEST, commercial, emptySet())
        val flySwaps = ProgramGenerator.rankSwapCandidates(ExerciseLibrary.byId("db-fly")!!, chest, setOf("db-bench-press")).map { it.id }
        assertTrue("db-bench-press" !in flySwaps)
        assertTrue("a fly should rotate to another fly first: ${flySwaps.take(2)}",
            flySwaps.take(2).all { ExerciseLibrary.patternOf(ExerciseLibrary.byId(it)!!) == MovementPattern.FLY })
    }

    @Test
    fun singleDayRerollAvoidsWhatTheOtherDaysUseAndLeavesThemEmpty() {
        // Finding 14: a one-day re-roll de-duplicated against a phantom week. With `onlyDay` the
        // other days are untouched (empty) and `usedElsewhere` seeds the week-repeat penalty.
        val week = ProgramGenerator.generate(GenerationParams(6), mwm, emptySet(), emptySet(), seed = 5L)
        val others = week.filter { it.key != "push-b" }.flatMap { it.exercises }.map { it.libId }.toSet()
        var repeats = 0; var placements = 0
        (0 until 100).forEach { s ->
            val fresh = ProgramGenerator.generate(
                GenerationParams(6), mwm, emptySet(), emptySet(), seed = s.toLong(),
                usedElsewhere = others, onlyDay = "push-b"
            )
            assertTrue(fresh.filter { it.key != "push-b" }.all { it.exercises.isEmpty() })
            val day = fresh.first { it.key == "push-b" }
            assertTrue(day.exercises.isNotEmpty())
            placements += day.exercises.size
            repeats += day.exercises.count { it.libId in others }
        }
        // The MWM chest pool has two presses, so some overlap is unavoidable; it must be the minority.
        assertTrue("re-rolled day reused $repeats/$placements movements from the other days", repeats < placements / 2)
    }

    @Test
    fun kettlebellsCountAsHandheldForTheLightLoadSteer() {
        // Finding 15: the light-dumbbell steer only penalised the DUMBBELL unit.
        val kbGym = setOf(Equipment.KETTLEBELL, Equipment.MACHINE, Equipment.BENCH, Equipment.DUMBBELLS)
        fun kbLeads(dbMaxLb: Double?): Int = (0 until 100).count { s ->
            val lead = ProgramGenerator.generate(GenerationParams(5, dbMaxLb = dbMaxLb), kbGym, emptySet(), emptySet(), seed = s.toLong())
                .first { it.key == "legs" }.exercises.first()
            Equipment.KETTLEBELL in ExerciseLibrary.byId(lead.libId)!!.equipment
        }
        assertTrue("light loads should steer the heavy quad slot off the kettlebell", kbLeads(25.0) < kbLeads(null))
    }
}
