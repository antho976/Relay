package com.forge.app.data.repo

import android.content.Context
import androidx.room.withTransaction
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.dao.ExerciseCustomizationDao
import com.forge.app.data.db.dao.ProgramCustomizationDao
import com.forge.app.data.db.dao.ProgramDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.entities.CoachDecision
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
import com.forge.app.widget.refreshForgeWidgets
import com.forge.app.domain.program.resolvePendingGeneration
import com.forge.app.domain.program.programSignature
import com.forge.app.domain.program.ProgramSignatureSlot
import com.forge.app.domain.program.ProgramSignatureDay
import com.forge.app.domain.program.ProgramGenerationIntent
import com.forge.app.domain.program.PendingGeneration
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
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
    private val database: ForgeDatabase,
    private val dao: ProgramDao,
    private val customizationDao: ProgramCustomizationDao,
    private val programCustomizationRepo: ProgramCustomizationRepository,
    private val exerciseCustomizationDao: ExerciseCustomizationDao,
    private val sessionDao: SessionDao,
    private val coachDao: com.forge.app.data.db.dao.CoachDao,
    private val settings: SettingsRepository,
    // Lazy on purpose: AdaptationRepository depends on this repository, so a direct injection would
    // be a construction cycle. The caps read happens long after both exist.
    private val adaptationRepositoryLazy: dagger.Lazy<AdaptationRepository>,
    private val clock: com.forge.app.core.time.Clock,
    @ApplicationContext private val context: Context
) {
    private val _revision = MutableStateFlow(0L)
    /** Bumps whenever the active program changes (load/generate) so program-display VMs can refresh. */
    val revision: StateFlow<Long> = _revision.asStateFlow()

    /** Serializes [ensureLoaded] so two concurrent callers (e.g. app startup + a ViewModel awaiting the
     *  load before resolving orphan sessions) can't both seed an empty DB or double-load the facade. */
    private val loadMutex = Mutex()

    /** Seed-if-empty, then load the DB program into the [Program] facade. Safe to call at startup,
     *  idempotent, and safe to call concurrently — on return [Program.isLoaded] is guaranteed true. */
    suspend fun ensureLoaded() = loadMutex.withLock {
        // A load that cannot complete is REPORTED, not silent (M-30): callers awaiting readiness —
        // the widget deep-link, which cannot judge a day key against a program that has not loaded —
        // would otherwise wait for something that is never coming.
        try {
            // Seed the default split only ONCE, on a genuine first run before onboarding. The persisted
            // PROGRAM_SEEDED flag (set the first time we seed) — not just dayCount==0 — is what gates this,
            // so a deliberately EMPTY plan (build-your-own / cleared) can never be silently re-seeded: not
            // on relaunch, and not in the brief window where onboarding has cleared the program but not yet
            // flipped ONBOARDING_DONE (a concurrent widget refresh used to re-seed the default there).
            if (dao.dayCount() == 0 && !settings.programSeeded.first() && !settings.onboardingDone.first()) {
                seedFromDefault()
                settings.setProgramSeeded(true)
            }
            loadIntoFacade()
        } catch (c: kotlinx.coroutines.CancellationException) {
            throw c
        } catch (t: Throwable) {
            Program.markLoadFailed()
            throw t
        }
    }

    /**
     * Persist a hand-built program (the builder) and make it live. The base rows become the single
     * source of truth, so the customization overlay + coach swaps are wiped (a manual rebuild is the
     * user taking the wheel) and any in-progress session is discarded. Empty lists = no plan.
     */
    suspend fun saveCustomProgram(days: List<ProgramDay>, slots: List<ProgramSlot>) {
        database.withTransaction {
            dao.replaceProgram(days, slots)
            customizationDao.deleteAll()
            exerciseCustomizationDao.clearCoachSwaps()
            coachDao.foldAllAppliedDeltas()
            discardActiveSession()
        }
        loadIntoFacade(refreshWidget = true)
    }

    /** Clear the program entirely (no plan) — used when the user opts to build their own from scratch. */
    suspend fun clearProgram() = saveCustomProgram(emptyList(), emptyList())

    /** Raw current day rows (ordered) — the builder loads these to edit the existing plan. */
    suspend fun currentDayRows(): List<ProgramDay> = dao.days()

    /** Raw current slot rows for a day (ordered) — the builder resolves names via [ExerciseLibrary]. */
    suspend fun slotRowsForDay(dayId: String): List<ProgramSlot> = dao.slotsForDay(dayId)

    /**
     * What every (re)generation learns from the coach's record (auto-coach): applied volume
     * changes become baseline bias, proven swap replacements are boosted like likes, failed ones
     * softly avoided. Re-derived from the same rows on every call — idempotent, never compounds.
     * Empty history → exactly the un-coached behavior.
     */
    private suspend fun coachBias(): com.forge.app.domain.coach.GenBias =
        runCatching { com.forge.app.domain.coach.CoachGenBias.from(coachDao.allDecisions()) }
            .getOrDefault(com.forge.app.domain.coach.GenBias.NEUTRAL)

    /**
     * Mark the volume decisions the weekly cap wouldn't let through as NOT FOLLOWED, so they drop
     * out of [com.forge.app.domain.coach.CoachGenBias] — and with it out of the planner's ±2 drift
     * cap and the Coach Lab's "carried forward" line.
     *
     * "Not followed" is the honest verdict: the change was applied but the program never carried
     * it, so there is nothing for the watcher to judge and nothing for generation to learn.
     */
    private suspend fun retireTrimmedVolumeBias(
        intended: Map<MuscleGroup, Int>,
        effective: Map<MuscleGroup, Int>
    ) {
        if (intended.isEmpty()) return
        val trimmedByMuscle = intended.mapValues { (muscle, want) ->
            val got = effective[muscle] ?: 0
            // Only a shortfall in the SAME direction counts; a sign flip retires the lot.
            val delivered = if (want > 0) got.coerceAtLeast(0) else (-got).coerceAtLeast(0)
            (kotlin.math.abs(want) - delivered).coerceAtLeast(0)
        }.filterValues { it > 0 }
        if (trimmedByMuscle.isEmpty()) return

        val candidates = coachDao.allDecisions()
            .filter {
                (it.status == CoachRepository.STATUS_APPLIED || it.status == CoachRepository.STATUS_FOLDED) &&
                    it.type.startsWith("volume") &&
                    it.outcome != CoachDecision.OUTCOME_NOT_FOLLOWED && it.outcome != "failed"
            }
            .sortedByDescending { it.id } // newest first: the most recent credit is the one that bounced
        trimmedByMuscle.forEach { (muscle, count) ->
            val wantUp = (intended[muscle] ?: 0) > 0
            candidates
                .filter {
                    ExerciseLibrary.byId(it.targetKey)?.muscle == muscle &&
                        (it.type == "volume_up") == wantUp
                }
                .take(count)
                .forEach { coachDao.setOutcome(it.id, CoachDecision.OUTCOME_NOT_FOLLOWED) }
        }
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
        // The regenerate is about to clear the customization overlay (reconcile below) — fold the
        // coach's learned adjustments into the new BASELINE so they survive the refresh. volumeBias
        // and prefer/avoid feed selection inside the generator; repBias is applied to the slot reps
        // below (a coach rep_shift re-binds to its exercise wherever it lands).
        val bias = coachBias()
        val genParams = params.copy(volumeBias = bias.volumeBias, avoid = params.avoid + bias.avoid)
        val generated = ProgramGenerator.generate(
            genParams, available, liked + bias.prefer, disliked, recent, seed
        )
        val days = ArrayList<ProgramDay>()
        val slots = ArrayList<ProgramSlot>()
        generated.forEachIndexed { i, gd ->
            days += ProgramDay(gd.key, i, gd.name, gd.word, gd.accentHex, gd.archetype)
            gd.exercises.forEachIndexed { j, ge ->
                slots += ProgramSlot("${gd.key}-$j", gd.key, j, ge.libId, ge.sets, bias.repBias[ge.libId] ?: ge.reps)
            }
        }
        // Recorded BEFORE the transaction, carrying the signature of the program it replaces, so a
        // process death anywhere in the next few lines leaves enough behind to finish the job
        // (M-06). See [reconcilePendingGeneration].
        val intent = ProgramGenerationIntent(
            deload = params.deload,
            atMs = clock.nowMs(),
            beforeSignature = currentProgramSignature()
        )
        settings.setProgramGenerationIntent(intent)
        // One transaction so a crash can't leave the fresh program bound to stale overlays (seam
        // finding 13): replace the program; drop the now-invalid customization overlay AND the
        // coach-applied swaps (their learning is already folded into the baseline above — see
        // [reconcileCustomizations]); retire the cleared coach deltas to 'folded' so per-change undo
        // can't replay stale state onto the new program (findings 0/1); discard the in-progress session.
        database.withTransaction {
            dao.replaceProgram(days, slots)
            reconcileCustomizations(days)
            // The weekly cap can shave off sets the coach's bias just added. Retire the decisions
            // that didn't survive BEFORE folding, so the ledger stops claiming volume the program
            // never received.
            retireTrimmedVolumeBias(bias.volumeBias, ProgramGenerator.effectiveVolumeBias(genParams))
            coachDao.foldAllAppliedDeltas()
            discardActiveSession()
        }
        // The deload marker is settled HERE, after the rows are committed, and by this method alone
        // (M-06). It used to be written by AdaptationRepository BEFORE calling in, so a generate
        // that threw left "you are in a deload week" standing over the untouched full-volume plan,
        // permanently; and Settings' own "Generate deload week" set no marker at all, so its
        // lighter plan was never recognised as one. The intent recorded above is what closes the
        // remaining window: a crash between this transaction and this write is finished on the
        // next boot by [reconcilePendingGeneration].
        settings.setDeloadWeekStartMs(if (params.deload) intent.atMs else 0L)
        settings.clearProgramGenerationIntent()
        loadIntoFacade(refreshWidget = true)
    }

    /**
     * Finish a regeneration that was interrupted between its program transaction and its deload
     * marker (M-06). Called once at startup, after [ensureLoaded].
     *
     * The recorded intent carries the signature of the program it was replacing, so "did the
     * transaction commit?" is answered by looking rather than guessing: an unchanged program means
     * it did not, and the attempt is dropped with the marker untouched; a changed one means the
     * rows landed and the marker is brought into agreement with them now.
     */
    suspend fun reconcilePendingGeneration() {
        val intent = settings.programGenerationIntent.first() ?: return
        when (val pending = resolvePendingGeneration(intent, currentProgramSignature())) {
            is PendingGeneration.Apply -> settings.setDeloadWeekStartMs(pending.deloadStartMs)
            PendingGeneration.Discard, PendingGeneration.None -> Unit
        }
        settings.clearProgramGenerationIntent()
    }

    /** [programSignature] of the program currently on disk. */
    private suspend fun currentProgramSignature(): String = programSignature(
        dao.days().map { day ->
            ProgramSignatureDay(
                id = day.id,
                archetype = day.archetype,
                slots = dao.slotsForDay(day.id).map {
                    ProgramSignatureSlot(it.exerciseLibId, it.sets, it.reps)
                }
            )
        }
    )

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
        dbMaxLb = settings.maxDbWeightLb.first(),
        frozenIds = settings.frozenExerciseIds.first(),
        // D: personal volume ceilings where the athlete's own history has earned them; empty
        // otherwise, which is exactly the pre-D behavior.
        personalCaps = personalCaps()
    )

    /**
     * The athlete's measured per-muscle caps (Coach v3 D). Never throws: a failed read degrades to
     * the population defaults rather than blocking a regenerate.
     */
    private suspend fun personalCaps(): Map<MuscleGroup, Int> = runCatching {
        com.forge.app.domain.coach.PersonalProfile.build(adaptationRepositoryLazy.get().snapshotOrEmpty()).volumeCaps
    }.getOrDefault(emptyMap())

    private suspend fun currentEquipment(): Set<Equipment> = settings.availableEquipment.first()
        .mapNotNull { runCatching { Equipment.valueOf(it) }.getOrNull() }.toSet()

    /**
     * Re-roll the WHOLE program using the user's full saved generation profile (goal, experience,
     * emphasis, problem areas, priority muscles, pinned). Used by automatic rotation, so a rotated
     * program reflects the same inputs a manual re-roll would — including avoiding flagged injuries.
     */
    /**
     * Return the program to full volume once a deload week has run its course, keeping the same
     * exercises.
     *
     * Deliberately NOT [rerollAll]: that passes the current picks as `recent` so the generator
     * anti-repeats them, which would charge the user a whole new program as the price of ending a
     * deload. Here `recent` is empty and the params are unchanged, so the generator is free to
     * re-select what they were already doing — at full volume, because [currentParams] leaves
     * `deload` unset and [generate] clears the deload marker for any non-deload regenerate.
     */
    suspend fun restoreAfterDeload() {
        generate(
            currentParams(), currentEquipment(),
            settings.likedExercises.first(), settings.dislikedExercises.first()
        )
    }

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
        // Volume-stable rotation: a single-day re-roll changes WHICH exercises, not how many sets, so
        // it keeps this day's current EFFECTIVE per-position set counts (baseline + any override) and
        // never re-plans volume. That removes the cross-path volume oscillation a week-scoped re-derive
        // caused (seam finding 17); coach volume re-materializes only on a full generate. No volumeBias.
        val effectiveSets = programCustomizationRepo.effectivePlanForDay(dayKey).map { it.sets }
        val fresh = ProgramGenerator.generate(
            currentParams().let { it.copy(avoid = it.avoid + bias.avoid) },
            currentEquipment(),
            settings.likedExercises.first() + bias.prefer, settings.dislikedExercises.first(),
            recent, System.nanoTime()
        )
        val day = fresh.firstOrNull { it.key == dayKey } ?: return
        val slots = day.exercises.mapIndexed { j, ge ->
            ProgramSlot(
                "$dayKey-$j", dayKey, j, ge.libId,
                effectiveSets.getOrElse(j) { ge.sets }, bias.repBias[ge.libId] ?: ge.reps
            )
        }
        // One transaction (seam finding 13). Only THIS day's overlays were cleared, so fold only this
        // day's coach deltas; coach swaps are global and survive a single-day re-roll.
        database.withTransaction {
            dao.replaceDaySlots(dayKey, slots)
            reconcileDayCustomizations(dayKey)
            coachDao.foldAppliedDeltasForDay(dayKey)
            // If a workout is in progress ON THIS day, its exercises just changed — discard it.
            sessionDao.getActiveSession()?.takeIf { it.dayKey == dayKey }?.let { sessionDao.delete(it) }
        }
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
        if (dbDays.isEmpty()) {
            // Empty is a real state now (build-your-own / no plan) — reflect it in the facade instead
            // of leaving a stale plan loaded, and still notify display VMs.
            Program.setActive(emptyList())
            _revision.value += 1
            return
        }
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
        if (refreshWidget) refreshForgeWidgets(context)
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
     *
     * Coach-applied SWAP overlays are also cleared here (they live in a separate, globally-keyed
     * table): the prefer/avoid bias already carried that learning into the new baseline, so leaving
     * the overlay would double-apply it and rebind the old swap to a fresh slot (seam fix, finding 3 —
     * previously NO regenerate path touched exercise_customization). User swaps are left untouched.
     * Caller wraps this in the same transaction as replaceProgram (finding 13).
     */
    private suspend fun reconcileCustomizations(days: List<ProgramDay>) {
        val validKeys = days.map { it.id }.toSet()
        customizationDao.all().forEach { c ->
            val keep = c.exerciseId.startsWith("custom_") && c.dayKey in validKeys
            if (!keep) customizationDao.delete(c.dayKey, c.exerciseId)
        }
        exerciseCustomizationDao.clearCoachSwaps()
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
            "Jumping jacks — 20 reps",
            "Arm circles — 10 forward, 10 back",
            "Shoulder rolls — 10 each direction",
            "Push-ups — 10 slow reps"
        )
        val WARMUP_PULL = listOf(
            "Jumping jacks — 20 reps",
            "Arm circles — 10 forward, 10 back",
            "Shoulder rolls — 10 each direction",
            "Standing torso twists — 10 each side"
        )
        val WARMUP_LEGS = listOf(
            "Jumping jacks — 20 reps",
            "Bodyweight squats — 15 reps slow",
            "Walking lunges — 10 steps",
            "Leg swings — 10 each leg"
        )
        val WARMUP_FULL = listOf(
            "Jumping jacks — 20 reps",
            "Bodyweight squats — 15 reps",
            "Push-ups — 10 reps",
            "Arm circles — 10 forward, 10 back"
        )
        val WARMUP_ARMS = listOf(
            "Jumping jacks — 20 reps",
            "Arm circles — 10 forward, 10 back",
            "Shoulder rolls — 10 each direction",
            "Wrist circles — 10 each direction"
        )
    }
}
