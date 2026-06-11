package com.forge.app.data.repo

import android.content.Context
import androidx.glance.appwidget.updateAll
import com.forge.app.data.db.dao.ProgramCustomizationDao
import com.forge.app.data.db.dao.ProgramDao
import com.forge.app.data.db.dao.SessionDao
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
import com.forge.app.widget.ForgeWidget
import dagger.hilt.android.qualifiers.ApplicationContext
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
    private val customizationDao: ProgramCustomizationDao,
    private val sessionDao: SessionDao,
    private val coachDao: com.forge.app.data.db.dao.CoachDao,
    private val settings: SettingsRepository,
    @ApplicationContext private val context: Context
) {
    private val _revision = MutableStateFlow(0L)
    /** Bumps whenever the active program changes (load/generate) so program-display VMs can refresh. */
    val revision: StateFlow<Long> = _revision.asStateFlow()

    /** Seed-if-empty, then load the DB program into the [Program] facade. Safe to call at startup. */
    suspend fun ensureLoaded() {
        if (dao.dayCount() == 0) seedFromDefault()
        loadIntoFacade()
    }

    /**
     * What every (re)generation learns from the coach's record (auto-coach): applied volume
     * changes become baseline bias, proven swap replacements are boosted like likes, failed ones
     * softly avoided. Re-derived from the same rows on every call — idempotent, never compounds.
     * Empty history → exactly the un-coached behavior.
     */
    private suspend fun coachBias(): com.forge.app.domain.coach.GenBias =
        runCatching { com.forge.app.domain.coach.CoachGenBias.from(coachDao.allDecisions()) }
            .getOrDefault(com.forge.app.domain.coach.GenBias.NEUTRAL)

    /** Generate a fresh program from [params] + equipment/like/dislike, persist it, and load it (Phase 2). */
    suspend fun generate(
        params: GenerationParams,
        available: Set<Equipment>,
        liked: Set<String>,
        disliked: Set<String>,
        recent: Set<String> = emptySet(),
        seed: Long = System.nanoTime()
    ) {
        // The regenerate is about to clear the customization overlay (reconcile below) — fold the
        // coach's learned adjustments into the new BASELINE so they survive the refresh.
        val bias = coachBias()
        val generated = ProgramGenerator.generate(
            params.copy(volumeBias = bias.volumeBias, avoid = params.avoid + bias.avoid),
            available, liked + bias.prefer, disliked, recent, seed
        )
        val days = ArrayList<ProgramDay>()
        val slots = ArrayList<ProgramSlot>()
        generated.forEachIndexed { i, gd ->
            days += ProgramDay(gd.key, i, gd.name, gd.word, gd.accentHex, gd.archetype)
            gd.exercises.forEachIndexed { j, ge ->
                slots += ProgramSlot("${gd.key}-$j", gd.key, j, ge.libId, ge.sets, ge.reps)
            }
        }
        dao.replaceProgram(days, slots)
        // A wholesale rebuild invalidates the old customization overlay — drop stale rep/set/removed
        // overrides so they can't silently re-bind to (or delete) a freshly generated exercise, and
        // drop user-added exercises whose day no longer exists. See [reconcileCustomizations].
        reconcileCustomizations(days)
        // The exercises just changed under any in-progress workout — discard it so the user isn't
        // shown a "resume" for a session whose exercises no longer match the program.
        discardActiveSession()
        loadIntoFacade(refreshWidget = true)
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
        goal = settings.userGoal.first().ifBlank { "build_muscle" },
        experience = settings.programExperience.first(),
        problemAreas = settings.problemAreas.first().mapNotNull { ProblemArea.fromCode(it) }.toSet(),
        priorityMuscles = settings.priorityMuscles.first()
            .mapNotNull { runCatching { MuscleGroup.fromCode(it) }.getOrNull() }.toSet(),
        pinned = settings.pinnedExercises.first(),
        dbMaxLb = settings.maxDbWeightLb.first()
    )

    private suspend fun currentEquipment(): Set<Equipment> = settings.availableEquipment.first()
        .mapNotNull { runCatching { Equipment.valueOf(it) }.getOrNull() }.toSet()

    /**
     * Re-roll the WHOLE program using the user's full saved generation profile (goal, experience,
     * emphasis, problem areas, priority muscles, pinned). Used by automatic rotation, so a rotated
     * program reflects the same inputs a manual re-roll would — including avoiding flagged injuries.
     */
    suspend fun rerollAll() {
        val recent = Program.days.flatMap { it.exercises }.map { it.id }.toSet()
        generate(
            currentParams(), currentEquipment(),
            settings.likedExercises.first(), settings.dislikedExercises.first(), recent
        )
    }

    /** Re-roll just one day's exercises (anti-repeating its current picks), keeping the rest intact. */
    suspend fun rerollDay(dayKey: String) {
        // Anti-repeat against the WHOLE current week, not just this day, so a single-day re-roll
        // doesn't hand back a movement another (unchanged) day already uses.
        val recent = Program.days.flatMap { it.exercises }.map { it.id }.toSet()
        val bias = coachBias()
        val fresh = ProgramGenerator.generate(
            currentParams().let { it.copy(volumeBias = bias.volumeBias, avoid = it.avoid + bias.avoid) },
            currentEquipment(),
            settings.likedExercises.first() + bias.prefer, settings.dislikedExercises.first(),
            recent, System.nanoTime()
        )
        val day = fresh.firstOrNull { it.key == dayKey } ?: return
        val slots = day.exercises.mapIndexed { j, ge -> ProgramSlot("$dayKey-$j", dayKey, j, ge.libId, ge.sets, ge.reps) }
        dao.replaceDaySlots(dayKey, slots)
        // This day's exercises just changed — drop its stale static overrides (keep user-added ones).
        reconcileDayCustomizations(dayKey)
        // If a workout is in progress ON THIS day, its exercises just changed — discard it.
        sessionDao.getActiveSession()?.takeIf { it.dayKey == dayKey }?.let { sessionDao.delete(it) }
        loadIntoFacade(refreshWidget = true)
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

    /**
     * Build [DayPlan]s from the stored rows and push them into the facade. Subtitle/warmup aren't
     * stored (decided: derive, don't store): the seeded 4-day keys reuse the original split's
     * hand-written copy; every other generated day derives a presentable subtitle + a generic warmup
     * from its archetype (via [archetypeMeta]) so non-4-day splits aren't warmup-less or shown a raw
     * lowercase key. [refreshWidget] pushes a Glance update after a real program change (NOT on the
     * widget's own ensureLoaded path, to avoid a render→update→render loop).
     */
    private suspend fun loadIntoFacade(refreshWidget: Boolean = false) {
        val dbDays = dao.days()
        if (dbDays.isEmpty()) return
        val plans = dbDays.map { pd ->
            val seed = Program.seedDays.firstOrNull { it.key == pd.id }
            val meta = archetypeMeta(pd.archetype)
            DayPlan(
                key = pd.id,
                defaultName = pd.name,
                subtitle = seed?.subtitle ?: meta.subtitle,
                word = pd.word,
                accentHex = pd.accentHex,
                warmup = seed?.warmup ?: meta.warmup,
                exercises = dao.slotsForDay(pd.id).map { slot -> slot.toPlan() }
            )
        }
        Program.setActive(plans)
        _revision.value += 1
        if (refreshWidget) runCatching { ForgeWidget().updateAll(context) }
    }

    /** Presentable subtitle + generic warmup for a generated day, derived from its archetype key. */
    private fun archetypeMeta(archetype: String): DayMeta = when (archetype.substringBefore('-').lowercase()) {
        "push" -> DayMeta("Push-leaning · chest, shoulders, triceps", WARMUP_PUSH)
        "pull" -> DayMeta("Pull-leaning · back & biceps", WARMUP_PULL)
        "legs" -> DayMeta("Legs · quads, hamstrings, glutes", WARMUP_LEGS)
        "upper" -> DayMeta("Upper body", WARMUP_PUSH)
        "lower" -> DayMeta("Lower body", WARMUP_LEGS)
        "fb" -> DayMeta("Full body", WARMUP_FULL)
        "arms" -> DayMeta("Arms & delts", WARMUP_ARMS)
        else -> DayMeta(archetype.replace('-', ' ').replaceFirstChar { it.uppercase() }, WARMUP_FULL)
    }

    private data class DayMeta(val subtitle: String, val warmup: List<String>)

    /**
     * Drop the customization overlay rows that a wholesale rebuild has invalidated: every static
     * rep/set/removed override (so it can't re-bind to or silently delete a freshly generated
     * exercise), and user-added exercises whose day no longer exists. User-added ("custom_") exercises
     * on days that survive are kept — re-attaching them is their whole point.
     */
    private suspend fun reconcileCustomizations(days: List<ProgramDay>) {
        val validKeys = days.map { it.id }.toSet()
        customizationDao.all().forEach { c ->
            val keep = c.exerciseId.startsWith("custom_") && c.dayKey in validKeys
            if (!keep) customizationDao.delete(c.dayKey, c.exerciseId)
        }
    }

    /** Single-day variant: drop this day's stale static overrides, keep its user-added exercises. */
    private suspend fun reconcileDayCustomizations(dayKey: String) {
        customizationDao.forDay(dayKey).forEach { c ->
            if (!c.exerciseId.startsWith("custom_")) customizationDao.delete(dayKey, c.exerciseId)
        }
    }

    /** Discard any in-progress (unfinished) session — its exercises no longer match a rebuilt program. */
    private suspend fun discardActiveSession() {
        sessionDao.getActiveSession()?.let { sessionDao.delete(it) }
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

    private companion object {
        // Generic per-archetype warm-ups for generated days that don't reuse a seed key (Cluster C2).
        val WARMUP_PUSH = listOf(
            "Arm circles — 10 forward, 10 back",
            "Push-ups — 10 slow reps",
            "Band/empty-hand shoulder press — 15 reps",
            "Scapular wall slides — 10 reps"
        )
        val WARMUP_PULL = listOf(
            "Dead hangs or scapular pulls — 20 seconds",
            "Cat-cow stretches — 10 reps",
            "Band pull-aparts — 15 reps",
            "Light face pulls — 15 reps"
        )
        val WARMUP_LEGS = listOf(
            "Bodyweight squats — 15 reps slow",
            "Leg swings — 10 each leg, front and side",
            "Walking lunges — 10 steps",
            "Glute bridges — 15 reps"
        )
        val WARMUP_FULL = listOf(
            "Bodyweight squats — 15 reps",
            "Push-ups — 10 reps",
            "Arm circles — 10 forward, 10 back",
            "Leg swings — 10 each leg"
        )
        val WARMUP_ARMS = listOf(
            "Arm circles — 10 forward, 10 back",
            "Band pull-aparts — 15 reps",
            "Light curls with empty hands — 15 reps",
            "Wrist circles — 10 each direction"
        )
    }
}
