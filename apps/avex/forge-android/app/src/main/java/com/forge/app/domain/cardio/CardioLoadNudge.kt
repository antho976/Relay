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
     * The concrete recent-cardio block behind the nudge — the facts the Overview card surfaces so it
     * reads "Run · 63 min · yesterday" instead of a vague "you did some cardio". [activityLabel] is the
     * single activity's name when the block was all one type, else "Cardio".
     */
    data class Load(
        val totalMinutes: Int,
        val activityLabel: String,
        /** Most recent active session in the block — drives the "today / yesterday" recency label. */
        val lastSessionMs: Long
    )

    /**
     * The active (non-rest) cardio block in the last [WINDOW_HOURS] hours before [nowMs], or null when
     * it totals under [HEAVY_MINUTES] — i.e. not big enough to compete with lifting recovery. Non-null
     * exactly when [recentlyHeavy] is true, and carries the detail the nudge needs to be specific.
     */
    fun recentLoad(entries: List<CardioEntry>, nowMs: Long): Load? {
        val since = nowMs - WINDOW_HOURS * HOUR_MS
        val active = entries.filter { it.date >= since && it.type != CardioType.REST.code }
        val minutes = active.sumOf { it.durationMin }
        if (minutes < HEAVY_MINUTES) return null
        val distinctTypes = active.map { it.type }.distinct()
        val label = if (distinctTypes.size == 1) CardioType.fromCode(distinctTypes.first()).displayName else "Cardio"
        return Load(totalMinutes = minutes, activityLabel = label, lastSessionMs = active.maxOf { it.date })
    }

    /**
     * True when active (non-rest) cardio minutes logged in the last [WINDOW_HOURS] hours before
     * [nowMs] total at least [HEAVY_MINUTES] — a recent block big enough to compete with lifting
     * recovery.
     */
    fun recentlyHeavy(entries: List<CardioEntry>, nowMs: Long): Boolean = recentLoad(entries, nowMs) != null
}
