package com.forge.app.domain.goal

/**
 * How a pinned goal is named, and how many fit (L-06).
 *
 * These lived beside the Goals ViewModel, which made the data layer unable to reach them: deleting
 * a goal from the workout screen went straight to `GoalRepository.clearGoal` and left the pin
 * behind, because only ONE of the two delete paths knew the key format. A key with no goal behind
 * it is invisible on Home — Home resolves keys against live goals — but still counted as one of the
 * three, so the next pin evicted a LIVE one to make room and Home showed two goals in three slots,
 * the dropped one gone with nothing saying so.
 *
 * Here, where both a repository and a ViewModel can use the same definition.
 */

/** How many goals Home shows, and so how many pins are kept. */
const val HOME_PIN_SLOTS = 3

/** Pin key for a lift target. Namespaced so it cannot collide with a custom goal's row id. */
fun liftPinKey(exerciseId: String): String = "lift:$exerciseId"

/** Pin key for a custom goal. Keyed on the row ID, never the label — a rename must not drop the pin. */
fun customPinKey(id: Long): String = "custom:$id"
