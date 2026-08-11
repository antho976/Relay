package com.forge.app.domain.notify

import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatVolumeCompact

/**
 * The one-shot training milestones (#56).
 *
 * Each is a stable id, and its line is DERIVED from that id rather than stored beside it: a milestone
 * is queued unread in the notifications feed and may sit there across a unit change, so the volume
 * line has to re-read in whatever unit is current when it's finally shown.
 */
object Milestones {
    const val SESSIONS_100 = "sessions_100"
    const val VOLUME_10K_WEEK = "volume_10k_week"
    const val FIRST_FULL_MONTH = "first_full_month"

    /** Weekly-volume threshold, kept in the stored unit (lb) so it triggers the same for everyone. */
    const val VOLUME_THRESHOLD_LB = 10_000.0

    /**
     * The milestone's one line, or null for an id this build no longer knows — a queued unread id can
     * outlive the milestone that wrote it (a downgrade, a renamed rule), and an unknown id is dropped
     * from the feed rather than rendered as a raw key (DESIGN §11: machine identifiers never render).
     */
    fun messageFor(id: String, weightUnit: WeightUnit): String? = when (id) {
        SESSIONS_100 -> "100 workouts complete. You've earned this."
        // The threshold is fixed in lb; the label honours the user's unit ("10k lb" / "4.5k kg").
        VOLUME_10K_WEEK -> "${formatVolumeCompact(VOLUME_THRESHOLD_LB, weightUnit)} moved in one week, a first."
        FIRST_FULL_MONTH -> "First full month of training. You're building something real."
        else -> null
    }
}
