package com.forge.app.domain.cardio

import com.forge.app.domain.engine.ConditioningPlanner
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Carries today's cardio prescription from Home to the Cardio tab's log sheet.
 *
 * Home's button on a cardio day switches to the Cardio tab, which is another screen with another
 * view model. Without something between them the tab opened on its overview and the session Home
 * had just named had to be typed in again from memory. Home leaves the prescription here, the Cardio
 * tab takes it once and opens its log already filled in, so finishing the session is one tap.
 *
 * One slot, taken once: a prescription nobody collected is replaced by the next, never queued.
 */
@Singleton
class CardioHandOff @Inject constructor() {

    private val slot = MutableStateFlow<ConditioningPlanner.Prescription?>(null)

    /** The prescription waiting to be logged, or null. */
    val pending: StateFlow<ConditioningPlanner.Prescription?> = slot

    fun offer(prescription: ConditioningPlanner.Prescription) {
        slot.value = prescription
    }

    /** Take the waiting prescription, leaving the slot empty. */
    fun take(): ConditioningPlanner.Prescription? {
        val p = slot.value
        slot.value = null
        return p
    }
}
