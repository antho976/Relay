package com.forge.app.service.wear

/**
 * The ONE policy for "which slot is the wrist on" — used by both [WatchSessionMirror] (display)
 * and SetLogUseCase (write validation) so what the wrist shows is exactly what a log targets.
 *
 * Policy: follow the lifter, not the plan order. The slot of the most recently logged set wins
 * while it is still short of its planned sets; once it fills (or nothing is logged yet), the next
 * incomplete slot AFTER it (wrapping) is current — skipping slots the user declared done early,
 * so an abandoned exercise can't pin the wrist to the past. A fully-done day stays on the last
 * logged slot ("3 of 3" beats a blank).
 */
object CurrentSlotResolver {

    /**
     * @param plannedSets planned set count per slot, in plan order.
     * @param doneSets logged set count per slot, same order.
     * @param lastLoggedIdx slot of the most recently logged set; null when nothing is logged.
     * @param earlyDoneIdx slots the user finished early (skipped when scanning forward).
     * @return the current slot index; -1 only when there are no slots at all.
     */
    fun resolve(
        plannedSets: List<Int>,
        doneSets: List<Int>,
        lastLoggedIdx: Int?,
        earlyDoneIdx: Set<Int> = emptySet()
    ): Int {
        if (plannedSets.isEmpty()) return -1
        fun incomplete(i: Int) = doneSets[i] < plannedSets[i] && i !in earlyDoneIdx

        // Fresh activity trumps everything — even an early-done mark (logging a set on a slot
        // un-abandons it for as long as it stays the latest).
        if (lastLoggedIdx != null && doneSets[lastLoggedIdx] < plannedSets[lastLoggedIdx]) {
            return lastLoggedIdx
        }
        val start = (lastLoggedIdx ?: -1) + 1
        for (offset in plannedSets.indices) {
            val i = (start + offset) % plannedSets.size
            if (incomplete(i)) return i
        }
        return lastLoggedIdx ?: plannedSets.lastIndex
    }
}
