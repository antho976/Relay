package com.forge.app.data.repo

import com.forge.app.data.db.dao.ProgramDao
import com.forge.app.data.db.entities.ProgramDay
import com.forge.app.data.db.entities.ProgramSlot
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.program.DayPlan
import com.forge.app.program.Difficulty
import com.forge.app.program.Equipment
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.ExercisePlan
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.GenerationParams
import com.forge.app.program.MuscleGroup
import com.forge.app.program.ProblemArea
import com.forge.app.program.Program
import com.forge.app.program.ProgramGenerator
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Bridges the active program between the DB (`program_day`/`program_slot`) and the [Program]
 * facade (program-unlock plan, Phase 1). On first run it seeds the DB from the hard-coded split;
 * on every launch it loads the DB program into [Program.setActive], so the rest of the app — which
 * still reads `Program.days` synchronously — is transparently data-driven. Generation (Phase 2)
 * will write a new program through the same `replaceProgram` path and re-load the facade.
 */
@Singleton
class ProgramRepository @Inject constructor(
    private val dao: ProgramDao,
    private val settings: SettingsRepository
) {
    private val _revision = MutableStateFlow(0L)
    /** Bumps whenever the active program changes (load/generate) so program-display VMs can refresh. */
    val revision: StateFlow<Long> = _revision.asStateFlow()

    /** Seed-if-empty, then load the DB program into the [Program] facade. Safe to call at startup. */
    suspend fun ensureLoaded() {
        if (dao.dayCount() == 0) seedFromDefault()
        loadIntoFacade()
    }

    /** Generate a fresh program from [params] + equipment/like/dislike, persist it, and load it (Phase 2). */
    suspend fun generate(
        params: GenerationParams,
        available: Set<Equipment>,
        liked: Set<String>,
        disliked: Set<String>,
        recent: Set<String> = emptySet(),
        seed: Long = System.nanoTime()
    ) {
        val generated = ProgramGenerator.generate(params, available, liked, disliked, recent, seed)
        val days = ArrayList<ProgramDay>()
        val slots = ArrayList<ProgramSlot>()
        generated.forEachIndexed { i, gd ->
            days += ProgramDay(gd.key, i, gd.name, gd.word, gd.accentHex, gd.archetype)
            gd.exercises.forEachIndexed { j, ge ->
                slots += ProgramSlot("${gd.key}-$j", gd.key, j, ge.libId, ge.sets, ge.reps)
            }
        }
        dao.replaceProgram(days, slots)
        loadIntoFacade()
    }

    /** Re-roll the active program's exercise picks, keeping the same split structure (rotation). */
    suspend fun reroll(
        params: GenerationParams,
        available: Set<Equipment>,
        liked: Set<String>,
        disliked: Set<String>
    ) {
        val recent = Program.days.flatMap { it.exercises }.map { it.id }.toSet()
        generate(params, available, liked, disliked, recent, System.nanoTime())
    }

    /** Current generation inputs read from prefs — the single source for per-day re-roll (Phase 6). */
    private suspend fun currentParams(): GenerationParams = GenerationParams(
        daysPerWeek = settings.daysPerWeek.first(),
        emphasis = settings.programEmphasis.first(),
        cardioDays = settings.cardioDaysPerWeek.first(),
        goal = settings.userGoal.first().ifBlank { "build_muscle" },
        experience = settings.programExperience.first(),
        problemAreas = settings.problemAreas.first().mapNotNull { ProblemArea.fromCode(it) }.toSet(),
        priorityMuscles = settings.priorityMuscles.first()
            .mapNotNull { runCatching { MuscleGroup.fromCode(it) }.getOrNull() }.toSet(),
        pinned = settings.pinnedExercises.first()
    )

    private suspend fun currentEquipment(): Set<Equipment> = settings.availableEquipment.first()
        .mapNotNull { runCatching { Equipment.valueOf(it) }.getOrNull() }.toSet()

    /** Re-roll just one day's exercises (anti-repeating its current picks), keeping the rest intact. */
    suspend fun rerollDay(dayKey: String) {
        val recent = Program.day(dayKey).exercises.map { it.id }.toSet()
        val fresh = ProgramGenerator.generate(
            currentParams(), currentEquipment(),
            settings.likedExercises.first(), settings.dislikedExercises.first(),
            recent, System.nanoTime()
        )
        val day = fresh.firstOrNull { it.key == dayKey } ?: return
        val slots = day.exercises.mapIndexed { j, ge -> ProgramSlot("$dayKey-$j", dayKey, j, ge.libId, ge.sets, ge.reps) }
        dao.replaceDaySlots(dayKey, slots)
        loadIntoFacade()
    }

    /** First-run seed: mirror the hard-coded split into the tables, mapping each movement to its library id. */
    private suspend fun seedFromDefault() {
        val days = ArrayList<ProgramDay>()
        val slots = ArrayList<ProgramSlot>()
        Program.seedDays.forEachIndexed { i, dp ->
            days += ProgramDay(
                id = dp.key,
                position = i,
                name = dp.defaultName,
                word = dp.word,
                accentHex = dp.accentHex,
                // Phase 1: archetype mirrors the seed day key; real archetypes arrive with the generator.
                archetype = dp.key
            )
            dp.exercises.forEachIndexed { j, ex ->
                slots += ProgramSlot(
                    id = "${dp.key}-$j",
                    dayId = dp.key,
                    position = j,
                    exerciseLibId = ExerciseLibrary.byName(ex.name)?.id ?: ex.id,
                    sets = ex.sets,
                    reps = ex.reps
                )
            }
        }
        dao.replaceProgram(days, slots)
    }

    /** Build [DayPlan]s from the stored rows and push them into the facade. */
    private suspend fun loadIntoFacade() {
        val dbDays = dao.days()
        if (dbDays.isEmpty()) return
        val plans = dbDays.map { pd ->
            // Subtitle/warmup aren't stored (decided: derive, don't store). Seeded days reuse the
            // original split's copy for parity; generated days fall back to the archetype label.
            val seed = Program.seedDays.firstOrNull { it.key == pd.id }
            DayPlan(
                key = pd.id,
                defaultName = pd.name,
                subtitle = seed?.subtitle ?: pd.archetype,
                word = pd.word,
                accentHex = pd.accentHex,
                warmup = seed?.warmup ?: emptyList(),
                exercises = dao.slotsForDay(pd.id).map { slot -> slot.toPlan() }
            )
        }
        Program.setActive(plans)
        _revision.value += 1
    }

    private fun ProgramSlot.toPlan(): ExercisePlan {
        val def = ExerciseLibrary.byId(exerciseLibId)
        return if (def != null) {
            ExercisePlan(
                id = def.id,
                name = def.name,
                sets = sets,
                reps = reps,
                unit = def.unit,
                muscle = def.muscle,
                difficulty = def.difficulty,
                note = def.note,
                tags = def.tags,
                formCue = def.formCue,
                equipment = def.equipment
            )
        } else {
            // Unknown library id (shouldn't happen) — render a placeholder rather than drop the slot.
            ExercisePlan(
                id = exerciseLibId,
                name = exerciseLibId,
                sets = sets,
                reps = reps,
                unit = ExerciseUnit.DUMBBELL,
                muscle = MuscleGroup.CHEST,
                difficulty = Difficulty.BEGINNER,
                note = ""
            )
        }
    }
}
