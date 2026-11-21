package com.forge.app.domain.volume

import com.forge.app.data.db.entities.LoggedSet

/**
 * Volume = sum of (weight_lb × reps) across sets. Sets without a numeric weight
 * (bodyweight, unparseable) contribute zero — this matches every other lifting app's
 * convention. If we ever want bodyweight to contribute to volume we'd multiply by
 * the user's bodyweight, which we don't track today.
 *
 * Timed holds are excluded, as `LoggedSet.durationSeconds` says they are from "every weight×reps
 * aggregate (volume, e1RM, PR)" and as every LoggedSetDao aggregate already does. Their `reps` is a
 * duration, not a count, so a 90-second weighted plank at 45 lb was adding 4050 lb to the session
 * total it was stamped with.
 */
object VolumeCalculator {

    fun sessionVolumeLb(sets: List<LoggedSet>): Double = sets.sumOf { it.volumeLb() }

    /**
     * One set's contribution to volume, for the surfaces that hold their own set type rather than
     * a [LoggedSet] — the day card's live comparison, the session-detail per-set readout.
     *
     * They each open-coded `(weightLb ?: 0.0) * reps`, which is the expression this object exists
     * to stop anyone writing: for a timed hold `reps` is a DURATION, so a 90-second weighted plank
     * at 45 lb contributed 4,050 lb. The session total was fixed; the four places that recompute
     * volume for display were not, so the same workout read differently depending on which screen
     * you were standing on.
     *
     * New phone logs happen to store timed reps as zero, which hides most of this — but
     * [LoggedSet.durationSeconds] documents reps as MEANINGLESS for a hold, not as guaranteed zero,
     * and imported and legacy rows are under no such obligation.
     */
    fun setVolumeLb(weightLb: Double?, reps: Int, durationSeconds: Int?): Double =
        if (durationSeconds != null) 0.0 else (weightLb ?: 0.0) * reps
}

/** This set's contribution to volume — zero for a timed hold. See [VolumeCalculator.setVolumeLb]. */
fun LoggedSet.volumeLb(): Double =
    VolumeCalculator.setVolumeLb(weightLb, reps, durationSeconds)
