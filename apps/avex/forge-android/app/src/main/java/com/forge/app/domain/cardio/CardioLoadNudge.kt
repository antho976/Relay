package com.forge.app.domain.cardio

import com.forge.app.data.db.entities.CardioEntry

/**
 * Pure check for "you did a big cardio block recently" — drives the Overview nudge to keep the next
 * session's heavy work in check (or fuel for it). A deliberately simple, testable rule rather than a
 * full readiness model (ReadinessAdvisor already scales the in-session chip for acute cardio load);
 * this is the gentle, plain-language heads-up on the home screen.
 */
object CardioLoadNudge {

    private const val HOUR_MS = 60L * 60 * 1000

    /** How far back "recently" reaches. */
    const val WINDOW_HOURS = 36

    /** Active cardio minutes within the window at/above which the nudge fires. */
    const val HEAVY_MINUTES = 45

    /**
     * True when active (non-rest) cardio minutes logged in the last [WINDOW_HOURS] hours before
     * [nowMs] total at least [HEAVY_MINUTES] — a recent block big enough to compete with lifting
     * recovery.
     */
    fun recentlyHeavy(entries: List<CardioEntry>, nowMs: Long): Boolean {
        val since = nowMs - WINDOW_HOURS * HOUR_MS
        val minutes = entries
            .filter { it.date >= since && it.type != CardioType.REST.code }
            .sumOf { it.durationMin }
        return minutes >= HEAVY_MINUTES
    }
}
