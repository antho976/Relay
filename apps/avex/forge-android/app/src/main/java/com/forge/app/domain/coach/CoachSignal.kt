package com.forge.app.domain.coach

import com.forge.app.domain.adapt.AdaptationSnapshot

/**
 * The SignalRegistry (Coach v3 A2): every input the coach reads or intends to read, declared in
 * one place — including the ones that don't exist yet.
 *
 * Declaring the future is the point. A slot that renders as COMING_SOON tells the user what the
 * coach will grow into, and gives each later phase a contract to satisfy instead of an
 * architecture to change. Slots flip ACTIVE by wiring their reader; nothing about the registry
 * changes when they do.
 *
 * Renders inside Coach Lab's existing Signals lens — `RecoverySignal` stays the live-input row
 * type, this is the slot declaration beside it. Two "signal" concepts on one screen would be
 * incoherent, so the registry is that lens's "what I could read" section, never a new surface.
 */
object SignalRegistry {

    enum class Availability {
        /** Wired and reading real data now. */
        ACTIVE,

        /** Wired, but this user hasn't produced the data (no grant, no logs). */
        AWAITING_DATA,

        /** Declared, not built — the honest form of a roadmap. */
        COMING_SOON
    }

    /**
     * @param id stable slug, also the analytics/lesson key.
     * @param label short display name (sentence case; the UI uppercases mono labels itself).
     * @param reads what it would tell the coach, one dry line.
     * @param availability the STATIC ceiling — [statusFor] can only downgrade ACTIVE to
     *   AWAITING_DATA for a given snapshot, never promote a COMING_SOON slot.
     */
    data class Slot(
        val id: String,
        val label: String,
        val reads: String,
        val availability: Availability,
        val lessonId: String? = null
    )

    const val SLEEP = "sleep"
    const val RESTING_HR = "resting_hr"
    const val MOOD = "mood"
    const val BODYWEIGHT_GOAL = "bodyweight_goal"
    const val DAILY_STEPS = "daily_steps"
    const val STRESS_HRV = "stress_hrv"
    const val CONDITIONING = "conditioning"
    const val WATCH_HR = "watch_hr"
    const val PROTEIN_NUTRITION = "protein_nutrition"
    const val HYDRATION_SUPPLEMENTS = "hydration_supplements"
    const val CYCLE_READINESS = "cycle_readiness"

    /** Every slot, in the order the Coach Lab renders them: live first, forming next, future last. */
    val slots: List<Slot> = listOf(
        Slot(SLEEP, "Sleep", "How long you slept, against your own nightly average", Availability.ACTIVE),
        Slot(RESTING_HR, "Resting heart rate", "Your resting HR against your own baseline", Availability.ACTIVE),
        Slot(MOOD, "Session mood", "How your recent sessions actually felt", Availability.ACTIVE),
        Slot(
            BODYWEIGHT_GOAL, "Bodyweight", "Which way your weight is trending, so a flat lift reads right",
            Availability.ACTIVE, lessonId = "coach.strength_on_a_cut"
        ),
        Slot(DAILY_STEPS, "Daily movement", "Off-gym movement that spends the same recovery budget", Availability.ACTIVE),
        Slot(
            STRESS_HRV, "Heart-rate variability", "Overnight HRV against your own baseline",
            Availability.ACTIVE, lessonId = "signals.stress_hrv"
        ),
        Slot(
            CONDITIONING, "Conditioning load", "What your cardio costs your lifting",
            Availability.COMING_SOON
        ),
        Slot(WATCH_HR, "In-session heart rate", "Strain and recovery inside a session, from the watch", Availability.COMING_SOON),
        Slot(PROTEIN_NUTRITION, "Protein", "Whether under-fuelling explains a stall before your training does", Availability.COMING_SOON),
        Slot(HYDRATION_SUPPLEMENTS, "Hydration and supplements", "Consistency on the few things that measurably help", Availability.COMING_SOON),
        Slot(CYCLE_READINESS, "Cycle", "Cycle-aware readiness, when you choose to share it", Availability.COMING_SOON)
    )

    fun slot(id: String): Slot? = slots.firstOrNull { it.id == id }

    /**
     * This snapshot's live status for a slot: an ACTIVE slot with no data yet reports
     * AWAITING_DATA so the Coach Lab can draw a forming state instead of implying a reading it
     * doesn't have. COMING_SOON never changes.
     */
    fun statusFor(slot: Slot, s: AdaptationSnapshot): Availability {
        if (slot.availability != Availability.ACTIVE) return slot.availability
        val hasData = when (slot.id) {
            SLEEP -> s.health.sleepNights.isNotEmpty()
            RESTING_HR -> s.health.restingHr.isNotEmpty()
            MOOD -> s.moods.isNotEmpty()
            BODYWEIGHT_GOAL -> s.bodyweight.isNotEmpty()
            DAILY_STEPS -> s.health.dailySteps.isNotEmpty()
            STRESS_HRV -> s.health.hrv.isNotEmpty()
            else -> false
        }
        return if (hasData) Availability.ACTIVE else Availability.AWAITING_DATA
    }

    /** Every slot with its live status — the Coach Lab's whole data source for the section. */
    fun statuses(s: AdaptationSnapshot): List<Pair<Slot, Availability>> =
        slots.map { it to statusFor(it, s) }
}
