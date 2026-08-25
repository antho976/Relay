package com.forge.app.domain.adapt

import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.types.EffortRating
import com.forge.app.domain.coach.WeightPhase
import com.forge.app.program.ExerciseUnit
import kotlin.math.roundToInt

/**
 * The session's intensity intent (Session.intensity, #123), typed for the engine.
 * Readiness autoregulation (System 6) will later *suggest* this value; the user's explicit
 * pick always wins.
 */
enum class IntensityIntent {
    LIGHT, NORMAL, HARD;

    companion object {
        fun fromCode(code: String?): IntensityIntent = when (code) {
            "light" -> LIGHT
            "hard" -> HARD
            else -> NORMAL
        }
    }
}

/** Numeric rep range parsed from a plan's display string ("8-10" → 8..10, "15" → 15..15). */
data class RepRange(val min: Int, val max: Int) {
    companion object {
        /** Null when the text holds no digits (pure-AMRAP/timed entries stay suggestion-free). */
        fun parse(text: String): RepRange? {
            val nums = Regex("\\d+").findAll(text).map { it.value.toInt() }.toList()
            if (nums.isEmpty()) return null
            return RepRange(nums.min(), nums.max())
        }
    }
}

/**
 * System 1 of the adaptation engine: double progression + plateau detection. Pure —
 * no DAO/Android/clock dependencies; both entry points are deterministic functions of
 * their inputs (mirrors [com.forge.app.domain.trophy.TrophyEvaluator]).
 *
 * Two entry points:
 *  - [suggestNextLoad] — the in-session suggestion chip: next load for one exercise from
 *    its previous bout. Cheap; safe on the day-screen build path.
 *  - [evaluate] — the snapshot pass: plateau detection over the whole program, with an
 *    escalation ladder (micro-load / reset → rep-range shift → variation swap).
 *
 * Double progression: reps climb within the slot's range first; weight rises only when
 * every top-weight set reached the top of the range at acceptable effort. Per-set RPE,
 * when logged, outranks the coarse per-exercise rating.
 */
object ProgressionAdvisor {

    // ── Chip path ──────────────────────────────────────────────────────────────

    /**
     * Suggested next load from the previous bout on this exercise, or null when there is
     * no defensible suggestion (cold start, bodyweight, mid-range progress, HARD-but-not-
     * brutal effort — the silent cases are deliberate).
     *
     * Unit-aware: DUMBBELL moves in [AdaptThresholds.dumbbellStepLb] steps (scaled);
     * PLATES moves in whole plates and ignores fine scaling (a 15 lb plate is too coarse —
     * any down-scale just suppresses increases); BODYWEIGHT never gets a weight suggestion.
     *
     * Scaling: an explicit pre-session [intensity] pick always wins (user intent is not
     * second-guessed); on a NORMAL day, [readiness] (System 6) fills in — a bounded ±few-%
     * autoregulation nudge whose reason is carried into the chip.
     */
    fun suggestNextLoad(
        exerciseId: String,
        exerciseName: String,
        prevSets: List<LoggedSet>,
        prevEffort: EffortRating?,
        repsText: String,
        unit: ExerciseUnit,
        plateLb: Double,
        intensity: IntensityIntent = IntensityIntent.NORMAL,
        readiness: Recommendation.ReadinessScale? = null,
        /** Heaviest dumbbell owned (lb) — DB progress targets are capped here; null = no ceiling. */
        dbMaxLb: Double? = null,
        /** Calibrated: this user beats DB suggestions and succeeds — double the step (Phase 2). */
        fastStep: Boolean = false,
        /** Calibrated: taken jumps keep failing — hold the weight instead of progressing (Phase 2). */
        consolidate: Boolean = false,
        t: AdaptThresholds = AdaptThresholds()
    ): Recommendation.WeightChange? {
        if (unit == ExerciseUnit.BODYWEIGHT) return null
        val working = prevSets.filter { !it.isAssisted && it.weightLb != null }
        val prevMax = working.maxOfOrNull { it.weightLb!! } ?: return null

        val scale: Double
        val scaleNote: String?
        when {
            intensity != IntensityIntent.NORMAL -> {
                scale = intensityScale(intensity, t)
                scaleNote = "intensity adjusted"
            }
            readiness != null -> {
                scale = 1 + readiness.percent / 100.0
                scaleNote = if (readiness.percent < 0) "eased ${-readiness.percent}% — readiness low"
                else "+${readiness.percent}% — readiness high"
            }
            else -> {
                scale = 1.0
                scaleNote = null
            }
        }

        // How hard was that? One model, all the logged evidence (A1) — per-set RPE first, then
        // failure/technique flags, then the difficulty tag, then the coarse exercise rating.
        val effort = EffortModel.read(working, prevEffort, t)
        val backOff = effort.backOff
        val backOffReason = effort.backOffReason
        val okToProgress = effort.roomToProgress

        val range = RepRange.parse(repsText)
        val topSets = working.filter { it.weightLb == prevMax }
        val hitTopOnAllTopSets = range != null && topSets.all { it.reps >= range.max }
        // Both same-weight cues (consolidate, keep-going) render prevMax identically — derive once.
        val sameWeightInput = if (unit == ExerciseUnit.PLATES) inputTextFor(prevMax, unit, plateLb) else trim(prevMax)

        return when {
            backOff -> backOffSuggestion(exerciseId, exerciseName, prevMax, unit, plateLb, scale, scaleNote, backOffReason, t)
            // Calibration says recent taken jumps failed — earn this weight before the next one.
            hitTopOnAllTopSets && okToProgress && consolidate ->
                weightChange(
                    exerciseId, exerciseName, prevMax, prevMax,
                    inputText = sameWeightInput,
                    reason = "calibrated to you — recent jumps haven't stuck, consolidate this weight first"
                )
            hitTopOnAllTopSets && okToProgress ->
                progressSuggestion(exerciseId, exerciseName, prevMax, unit, plateLb, scale, scaleNote, dbMaxLb, fastStep, t)
            // Worked at this weight last time but hasn't filled the rep range yet — surface an explicit
            // same-weight "keep going" cue instead of vanishing, so the missing chip never reads as
            // broken. This teaches double-progression (earn the reps here before the weight moves) and
            // distinguishes "still working up to this weight" from the plateau ladder's "stuck for N
            // sessions" (that read lives on the coach/snapshot path). BODYWEIGHT already returned above;
            // a null range (AMRAP / timed holds) has no top to reach, so it stays silent.
            range != null && !hitTopOnAllTopSets ->
                weightChange(
                    exerciseId, exerciseName, prevMax, prevMax,
                    inputText = sameWeightInput,
                    reason = "keep this weight — reach ${range.max} reps on every set before adding load"
                )
            else -> null
        }
    }

    /**
     * Bodyweight rep double-progression for the in-session chip (CO5). When every working set cleared
     * the top of the rep range at acceptable effort, aim for one more rep next time. Stays silent
     * mid-range (fill the range first) and when the last set was brutal — mirroring [suggestNextLoad]'s
     * deliberate silences. This is the only progression cue a pure-bodyweight movement ever gets, since
     * [suggestNextLoad] has no weight to suggest for BODYWEIGHT.
     */
    fun suggestNextReps(
        exerciseId: String,
        exerciseName: String,
        prevSets: List<LoggedSet>,
        prevEffort: EffortRating?,
        repsText: String,
        t: AdaptThresholds = AdaptThresholds()
    ): Recommendation.RepProgression? {
        // Unassisted working sets only — an assisted (band) set is an easier variation, not the target.
        // No fallback to assisted sets: with no unassisted history there's no defensible target, so stay
        // silent (mirrors suggestNextLoad, which returns null when every set is assisted).
        val working = prevSets.filterNot { it.isAssisted }
        if (working.isEmpty()) return null
        val prevMaxReps = working.maxOf { it.reps }
        if (prevMaxReps <= 0) return null
        val range = RepRange.parse(repsText) ?: return null

        // The same effort gate as the weighted chip (A1: EffortModel owns it).
        val okToProgress = EffortModel.read(working, prevEffort, t).roomToProgress

        // Only nudge once every working set has cleared the top of the range at acceptable effort.
        if (!working.all { it.reps >= range.max } || !okToProgress) return null

        // Always one more than the best set just done — correct whether they're at the range top or
        // already past it (pure bodyweight has no weight to add, so reps keep climbing). The reason
        // stays honest in both cases (it would read oddly to say "hit the top" when they're well above).
        val target = prevMaxReps + 1
        return Recommendation.RepProgression(
            exerciseId = exerciseId,
            exerciseName = exerciseName,
            fromReps = repsText,
            targetReps = target,
            reason = "cleared every set — add a rep",
            confidence = Confidence.MEDIUM
        )
    }

    private fun progressSuggestion(
        exerciseId: String,
        exerciseName: String,
        prevMax: Double,
        unit: ExerciseUnit,
        plateLb: Double,
        scale: Double,
        scaleNote: String?,
        dbMaxLb: Double?,
        fastStep: Boolean,
        t: AdaptThresholds
    ): Recommendation.WeightChange? = when (unit) {
        ExerciseUnit.DUMBBELL, ExerciseUnit.WEIGHT -> {
            val step = if (fastStep) t.dumbbellStepLb * 2 else t.dumbbellStepLb
            val target = floorToGrid((prevMax + step) * scale, t.dumbbellStepLb)
            // The heaviest-dumbbell ceiling applies ONLY to dumbbells (a barbell/machine keeps
            // loading). Trust it only while history doesn't contradict it — a heavier logged set
            // means the setting is stale, and progress shouldn't be capped on bad data.
            val ceiling = dbMaxLb?.takeIf { unit == ExerciseUnit.DUMBBELL && prevMax <= it }
            when {
                target <= 0.0 -> null
                ceiling != null && target > ceiling -> {
                    val capped = floorToGrid(ceiling, t.dumbbellStepLb)
                    if (capped > prevMax) weightChange(
                        exerciseId, exerciseName, prevMax, capped,
                        inputText = trim(capped),
                        reason = withNote("hit top of range — capped at your heaviest dumbbell", scaleNote)
                    ) else weightChange(
                        // The user owns nothing heavier: anchor the weight, progress reps instead.
                        exerciseId, exerciseName, prevMax, prevMax,
                        inputText = trim(prevMax),
                        reason = "at your heaviest dumbbell — add reps past the range instead"
                    )
                }
                else -> weightChange(
                    exerciseId, exerciseName, prevMax, target,
                    inputText = trim(target),
                    reason = withNote(
                        if (fastStep) "hit top of range · calibrated to you (bigger steps)"
                        else "hit top of range",
                        scaleNote
                    )
                )
            }
        }
        ExerciseUnit.PLATES ->
            // A whole plate is too big a step up on a deliberately light / low-readiness day.
            if (scale < 1.0) null
            else plateChange(exerciseId, exerciseName, prevMax, prevMax + plateLb, plateLb, "hit top of range")
        ExerciseUnit.BODYWEIGHT -> null
    }

    private fun backOffSuggestion(
        exerciseId: String,
        exerciseName: String,
        prevMax: Double,
        unit: ExerciseUnit,
        plateLb: Double,
        scale: Double,
        scaleNote: String?,
        reason: String,
        t: AdaptThresholds
    ): Recommendation.WeightChange? = when (unit) {
        ExerciseUnit.DUMBBELL, ExerciseUnit.WEIGHT -> {
            val target = floorToGrid((prevMax - t.dumbbellStepLb) * scale, t.dumbbellStepLb)
            if (target <= 0.0) null
            else weightChange(
                exerciseId, exerciseName, prevMax, target,
                inputText = trim(target),
                reason = withNote(reason, scaleNote)
            )
        }
        ExerciseUnit.PLATES -> {
            val target = prevMax - plateLb
            // Can't drop below one plate — stay silent rather than suggest the impossible.
            if (target < plateLb) null
            else plateChange(exerciseId, exerciseName, prevMax, target, plateLb, reason)
        }
        ExerciseUnit.BODYWEIGHT -> null
    }

    // ── Snapshot path: plateau ladder ──────────────────────────────────────────

    /**
     * Plateau detection over every program slot with enough history. The stall length is
     * the number of bouts since the last meaningful e1RM improvement, so a recent PR
     * naturally resets it. Emits at most ONE recommendation per exercise — the highest
     * rung of the ladder the stall has reached:
     *
     *   stall ≥ [AdaptThresholds.plateauMinBouts]        → weight nudge (micro-load when
     *       effort has been low — likely sandbagging — or a ~10% reset when effort is high)
     *   stall ≥ [AdaptThresholds.repShiftAfterStalledBouts] → rep-range shift
     *   stall ≥ [AdaptThresholds.swapAfterStalledBouts]     → variation swap
     */
    fun evaluate(
        s: AdaptationSnapshot,
        t: AdaptThresholds = AdaptThresholds(),
        /**
         * The live block's phase (Coach v3 C), or null when no block is running. A deload week
         * suppresses escalation entirely: the whole point of the week is to ask for less, and a
         * coach that proposes resets and swaps during it is arguing with its own plan.
         */
        phase: com.forge.app.domain.coach.BlockPhase? = null
    ): List<Recommendation> {
        if (phase == com.forge.app.domain.coach.BlockPhase.DELOAD) return emptyList()
        val out = mutableListOf<Recommendation>()
        val seen = HashSet<String>()
        // Reads UNKNOWN until the weigh-ins support a claim, and UNKNOWN changes nothing (A2).
        val weightPhase = WeightPhase.of(s.bodyweight)
        for (day in s.program) {
            for (slot in day.slots) {
                if (!seen.add(slot.exerciseId)) continue
                if (slot.unit == ExerciseUnit.BODYWEIGHT) continue
                val bouts = (s.exerciseHistory[slot.exerciseId] ?: continue)
                    // A1: test / technique / first-back bouts are not ordinary training — a top
                    // single on a test day would anchor the weight, and a deliberately light
                    // technique day reads as a stall. Filtering here removes them from the e1RM
                    // series, the stall counter AND the prevMax anchor in one place.
                    .filter { b -> b.countsForProgression }
                    .filter { b -> !b.skipped && b.sets.any { it.weightLb != null && !it.isAssisted } }
                    // A swap changes what the series MEASURES. Rotating a barbell row (e1RM ~160) to
                    // a dumbbell row (e1RM ~62) left `best` pinned to the pre-swap lift, so no bout on
                    // the new exercise could ever beat it and the stall counter grew by one every
                    // session — forever. The coach then re-proposed the same rotation every week, and
                    // the outcome watcher, which judges swaps on attendance alone, walked it to
                    // autopilot. The series restarts at the boundary instead.
                    .let { history -> sinceLastSwap(history) }
                if (bouts.size <= t.plateauMinBouts) continue

                val e1rms = bouts.map { bestE1rm(it.sets) }
                var best = e1rms.first()
                var lastImprovedIdx = 0
                for (i in 1 until e1rms.size) {
                    if (e1rms[i] > best * (1 + t.stallTolerance)) {
                        best = e1rms[i]
                        lastImprovedIdx = i
                    }
                }
                val stall = e1rms.lastIndex - lastImprovedIdx
                if (stall < t.plateauMinBouts) continue

                // A2: holding strength in a deficit is the expected — and good — outcome, so a flat
                // line while cutting is not a plateau to fix. Escalating it would tell a lifter who
                // is doing everything right to reset their weights. Regression still speaks: this
                // only suppresses a HOLD, never a decline (`e1rms.last() >= best`).
                if (weightPhase == WeightPhase.CUT && e1rms.last() >= best * (1 - t.stallTolerance)) continue

                val confidence = if (stall >= t.highConfidenceStall) Confidence.HIGH else Confidence.MEDIUM
                out += plateauSuggestion(slot, bouts, stall, confidence, s.prefs.plateLb, s.prefs.maxDbLb, t)
            }
        }
        return out
    }

    /**
     * Exercise ids whose stall [evaluate] is deliberately NOT escalating because the athlete is in
     * a deficit (A2). Same rule, exposed as a reading: the Coach Lab can say what it held and why,
     * and the Academy can unlock `coach.strength_on_a_cut` the first time it happens — the coach's
     * most counterintuitive behavior should never be invisible.
     */
    fun cutSuppressedStalls(s: AdaptationSnapshot, t: AdaptThresholds = AdaptThresholds()): List<String> {
        if (WeightPhase.of(s.bodyweight) != WeightPhase.CUT) return emptyList()
        val out = mutableListOf<String>()
        val seen = HashSet<String>()
        for (day in s.program) {
            for (slot in day.slots) {
                if (!seen.add(slot.exerciseId)) continue
                if (slot.unit == ExerciseUnit.BODYWEIGHT) continue
                val bouts = (s.exerciseHistory[slot.exerciseId] ?: continue)
                    .filter { b -> b.countsForProgression }
                    .filter { b -> !b.skipped && b.sets.any { it.weightLb != null && !it.isAssisted } }
                if (bouts.size <= t.plateauMinBouts) continue
                val e1rms = bouts.map { bestE1rm(it.sets) }
                var best = e1rms.first()
                var lastImprovedIdx = 0
                for (i in 1 until e1rms.size) {
                    if (e1rms[i] > best * (1 + t.stallTolerance)) {
                        best = e1rms[i]
                        lastImprovedIdx = i
                    }
                }
                val stall = e1rms.lastIndex - lastImprovedIdx
                if (stall < t.plateauMinBouts) continue
                if (e1rms.last() >= best * (1 - t.stallTolerance)) out += slot.exerciseId
            }
        }
        return out
    }

    private fun plateauSuggestion(
        slot: ProgramSlotSnap,
        bouts: List<ExerciseBout>,
        stall: Int,
        confidence: Confidence,
        plateLb: Double,
        maxDbLb: Double?,
        t: AdaptThresholds
    ): Recommendation {
        val prevMax = bouts.last().sets
            .filter { it.weightLb != null && !it.isAssisted }
            .maxOf { it.weightLb!! }

        if (stall >= t.swapAfterStalledBouts && slot.swapCandidateIds.isNotEmpty()) {
            return Recommendation.VariationSwap(
                exerciseId = slot.exerciseId,
                exerciseName = slot.name,
                candidateIds = slot.swapCandidateIds.take(t.swapCandidateCount),
                reason = "No estimated-1RM gain on ${slot.name} in $stall sessions — a fresh variation can restart progress",
                confidence = confidence
            )
        }
        if (stall >= t.repShiftAfterStalledBouts) {
            val range = RepRange.parse(slot.repsText)
            val to = if (range != null && range.max >= 12) "8-10" else "12-15"
            return Recommendation.RepRangeShift(
                exerciseId = slot.exerciseId,
                exerciseName = slot.name,
                fromReps = slot.repsText,
                toReps = to,
                reason = "${slot.name} has stalled for $stall sessions — changing the rep range changes the stimulus",
                confidence = confidence
            )
        }

        // Effort signature over the recent window decides reset vs micro-load. A1: the reading now
        // also counts sets logged to failure and past-failure techniques (EffortModel), so a lifter
        // who logs the checkbox instead of an RPE gets the same "stalled while grinding" verdict.
        val window = bouts.takeLast(minOf(stall, t.plateauMinBouts))
        val highEffortCount = window.count { b -> EffortModel.read(b, t).highEffort }
        // The reset target, floored to the grid and never below one grid step. The PLATES branch was
        // always bounded (`coerceAtLeast(plateLb)`); the free-weight one wasn't, and floorToGrid
        // returns 0.0 for anything at or under about 2.8 lb — so a rehab lifter stalling on 2.5 lb
        // front raises was told, in Stats and in the Overview coach feed, to drop to 0.
        val resetTarget = when (slot.unit) {
            ExerciseUnit.PLATES -> (prevMax - plateLb).coerceAtLeast(plateLb)
            else -> floorToGrid(prevMax * (1 - t.resetFraction), t.dumbbellStepLb)
                .coerceAtLeast(t.dumbbellStepLb)
        }
        // Coercing can land the "reset" at or above where the lifter already is on a light load.
        // That isn't a reset, so it falls through to the micro-load branch rather than proposing a
        // change of nothing under a "drop and build back up" sentence.
        return if (highEffortCount >= t.highEffortCountInWindow && resetTarget < prevMax) {
            // The percentage is measured off the target actually prescribed, not off the threshold
            // that motivated it: on a light load the grid makes the real cut much bigger than the
            // configured fraction, and the reason used to quote the fraction regardless.
            val dropPct = ((1 - resetTarget / prevMax) * 100).roundToInt()
            weightChange(
                slot.exerciseId, slot.name, prevMax, resetTarget,
                inputText = inputTextFor(resetTarget, slot.unit, plateLb),
                reason = "${slot.name} stalled $stall sessions at high effort — drop ~$dropPct% and build back up",
                confidence = confidence
            )
        } else {
            val raw = when (slot.unit) {
                ExerciseUnit.PLATES -> prevMax + plateLb
                else -> floorToGrid(prevMax + t.dumbbellStepLb, t.dumbbellStepLb)
            }
            // A DB target can't exceed the heaviest dumbbell the user owns (ceiling trusted only
            // while history doesn't contradict it). At the ceiling, anchor the weight and push
            // reps instead — the next ladder rung shifts the rep range anyway.
            val ceiling = maxDbLb?.takeIf { slot.unit == ExerciseUnit.DUMBBELL && prevMax <= it }
            val target = if (ceiling != null) minOf(raw, floorToGrid(ceiling, t.dumbbellStepLb)) else raw
            if (target <= prevMax) weightChange(
                slot.exerciseId, slot.name, prevMax, prevMax,
                inputText = inputTextFor(prevMax, slot.unit, plateLb),
                reason = "${slot.name} hasn't gained in $stall sessions and you're at your heaviest dumbbell — add reps",
                confidence = confidence
            ) else weightChange(
                slot.exerciseId, slot.name, prevMax, target,
                inputText = inputTextFor(target, slot.unit, plateLb),
                reason = "${slot.name} hasn't gained in $stall sessions but effort has stayed low — push a small increase",
                confidence = confidence
            )
        }
    }

    // Callers only reach this with at least one working set; the shared helper returns null only when
    // there are none, so !! preserves the original "must have a working set" contract.
    private fun bestE1rm(sets: List<LoggedSet>): Double = bestWorkingE1rm(sets)!!

    // ── Shared helpers ─────────────────────────────────────────────────────────

    private fun intensityScale(intensity: IntensityIntent, t: AdaptThresholds): Double = when (intensity) {
        IntensityIntent.LIGHT -> t.intensityLightScale
        IntensityIntent.HARD -> t.intensityHardScale
        IntensityIntent.NORMAL -> 1.0
    }

    private fun withNote(reason: String, note: String?): String =
        if (note != null) "$reason · $note" else reason

    private fun weightChange(
        exerciseId: String,
        exerciseName: String,
        prevMax: Double,
        target: Double,
        inputText: String,
        reason: String,
        confidence: Confidence = Confidence.MEDIUM
    ) = Recommendation.WeightChange(
        exerciseId = exerciseId,
        exerciseName = exerciseName,
        previousMaxLb = prevMax,
        targetWeightLb = target,
        deltaLb = target - prevMax,
        inputText = inputText,
        reason = reason,
        confidence = confidence
    )

    private fun plateChange(
        exerciseId: String,
        exerciseName: String,
        prevMax: Double,
        targetLb: Double,
        plateLb: Double,
        reason: String
    ) = weightChange(
        exerciseId, exerciseName, prevMax, targetLb,
        inputText = inputTextFor(targetLb, ExerciseUnit.PLATES, plateLb),
        reason = reason
    )

    /**
     * Text for the weight field, in the exercise's input unit and parseable by WeightParser:
     * a plate count on PLATES ("3 plates"), explicit lb when off the plate grid ("50 lb"),
     * plain lb otherwise.
     */
    private fun inputTextFor(targetLb: Double, unit: ExerciseUnit, plateLb: Double): String {
        if (unit != ExerciseUnit.PLATES) return trim(targetLb)
        val plates = targetLb / plateLb
        val whole = plates.roundToInt()
        return if (kotlin.math.abs(plates - whole) < 0.01) {
            if (whole == 1) "1 plate" else "$whole plates"
        } else {
            "${trim(targetLb)} lb"
        }
    }

    /**
     * The tail of [bouts] performed on the exercise the slot holds NOW — everything from the last
     * change of swap onward. A bout's swappedName is the lift it was actually performed on (null for
     * the base exercise), so a change between consecutive bouts is a swap boundary.
     */
    private fun sinceLastSwap(bouts: List<ExerciseBout>): List<ExerciseBout> {
        val boundary = (bouts.indices.reversed()
            .firstOrNull { i -> i > 0 && bouts[i].swappedName != bouts[i - 1].swappedName })
            ?: return bouts
        return bouts.subList(boundary, bouts.size)
    }

    /** Floor to the increment grid — matches the old suggestion's rounding exactly. */
    private fun floorToGrid(weight: Double, grid: Double): Double = (weight / grid).toInt() * grid

    /**
     * One decimal, never a raw Double. `"$v"` printed the full binary expansion, so a target
     * derived from an unrounded history rendered as "Suggested next → 220.46226218487757".
     * Locale.US because this string can seed the weight field, so it is read back by WeightParser.
     */
    private fun trim(v: Double): String =
        if (v % 1.0 == 0.0) "${v.toInt()}" else String.format(java.util.Locale.US, "%.1f", v)
}
