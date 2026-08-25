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

    fun sessionVolumeLb(sets: List<LoggedSet>): Double =
        sets.filter { it.durationSeconds == null }
            .sumOf { (it.weightLb ?: 0.0) * it.reps }
}
