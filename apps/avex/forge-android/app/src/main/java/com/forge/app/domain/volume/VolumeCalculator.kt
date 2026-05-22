package com.forge.app.domain.volume

import com.forge.app.data.db.entities.LoggedSet

/**
 * Volume = sum of (weight_lb × reps) across sets. Sets without a numeric weight
 * (bodyweight, unparseable) contribute zero — this matches every other lifting app's
 * convention. If we ever want bodyweight to contribute to volume we'd multiply by
 * the user's bodyweight, which we don't track today.
 */
object VolumeCalculator {

    fun sessionVolumeLb(sets: List<LoggedSet>): Double =
        sets.sumOf { (it.weightLb ?: 0.0) * it.reps }
}
