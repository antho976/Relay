package com.forge.app.domain.session

import java.time.LocalDate

/**
 * The longest run of training days in [days], where a gap made only of [bridged] days (a declared
 * holiday, a rest day the weekday schedule planned) neither breaks the run nor adds to it. The
 * live streak bridges the same days (StatsRepository.streakBridge), so every "longest streak" the
 * app shows is measured the way the current one is: Profile's BEST, the trophies and the year
 * recap used to disagree, the recap counting only back-to-back calendar days.
 */
fun longestBridgedStreak(days: Collection<LocalDate>, bridged: (LocalDate) -> Boolean): Int {
    if (days.isEmpty()) return 0
    val sorted = days.toSortedSet()
    var best = 1
    var run = 1
    var prev = sorted.first()
    for (day in sorted.drop(1)) {
        run = if (allBridged(prev, day, bridged)) run + 1 else 1
        if (run > best) best = run
        prev = day
    }
    return best
}

/** Every day strictly between [prev] and [next] is bridged (a one-day gap has none, so it bridges). */
private fun allBridged(prev: LocalDate, next: LocalDate, bridged: (LocalDate) -> Boolean): Boolean {
    var d = prev.plusDays(1)
    while (d.isBefore(next)) {
        if (!bridged(d)) return false
        d = d.plusDays(1)
    }
    return true
}
