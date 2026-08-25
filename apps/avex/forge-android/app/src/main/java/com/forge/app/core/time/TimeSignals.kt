package com.forge.app.core.time

import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.onStart
import java.time.Instant
import java.time.ZoneId
import javax.inject.Inject
import javax.inject.Singleton

/**
 * "Which day is it now" as a Flow.
 *
 * Every day and week bucket in the app is derived at read time from `ZoneId.systemDefault()`, which
 * is the right storage model — but nothing invalidated a flow when the day, the zone or the clock
 * actually changed. Two consequences, both reported:
 *
 * - A week anchor computed once when a Flow is BUILT, while "today" is recomputed inside its
 *   combine, describes two different weeks the moment midnight passes. Leave the Overview open at
 *   23:50 on a Sunday and Monday's session lights Monday of *last* week's dot row, while "next up"
 *   has already rolled over. Nothing recovered until the screen was left long enough to unsubscribe.
 * - Fly Auckland → London and the process survives the zone change, so the dots, the streak and the
 *   "next up" day stay bucketed in the old zone until the app is killed.
 *
 * Collectors take [dayStarts] and rebuild whatever they anchored on. Emissions are deduplicated, so
 * the poll below costs a comparison and nothing else on a day that doesn't change.
 */
@Singleton
class TimeSignals @Inject constructor(private val clock: Clock) {

    private val external = MutableSharedFlow<Unit>(
        replay = 0,
        extraBufferCapacity = 1,
        onBufferOverflow = BufferOverflow.DROP_OLDEST
    )

    /** Called by [com.forge.app.service.TimeChangeReceiver] on a zone / clock / date change. */
    fun onSystemTimeChanged() {
        external.tryEmit(Unit)
    }

    /** Epoch-ms of local midnight today. Emits at once, at each midnight, and on any system time or
     *  timezone change. Repeats of the same day are filtered out. */
    @OptIn(ExperimentalCoroutinesApi::class)
    fun dayStarts(): Flow<Long> =
        external.onStart { emit(Unit) }
            .flatMapLatest { ticks() }
            .map { dayStartMs(clock.nowMs()) }
            .distinctUntilChanged()

    /**
     * Fires now, then again at the next local midnight — but never sleeps longer than
     * [MAX_TICK_DELAY_MS], so a device that dozed through the boundary, or one whose clock was wrong
     * when the flow started, converges within the hour instead of staying a day behind. Repeats are
     * dropped by the `distinctUntilChanged` above, so an early wake costs nothing.
     */
    private fun ticks(): Flow<Unit> = flow {
        while (true) {
            emit(Unit)
            val now = clock.nowMs()
            val zone = ZoneId.systemDefault()
            val nextMidnight = Instant.ofEpochMilli(now).atZone(zone).toLocalDate()
                .plusDays(1).atStartOfDay(zone).toInstant().toEpochMilli()
            delay((nextMidnight - now).coerceIn(MIN_TICK_DELAY_MS, MAX_TICK_DELAY_MS))
        }
    }

    private companion object {
        const val MIN_TICK_DELAY_MS = 1_000L
        const val MAX_TICK_DELAY_MS = 60L * 60 * 1000 // 1 h
    }
}

/** Epoch-ms of local midnight on the day containing [nowMs]. Mirrors [mondayStartMs] for a day. */
fun dayStartMs(nowMs: Long, zone: ZoneId = ZoneId.systemDefault()): Long =
    Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate()
        .atStartOfDay(zone).toInstant().toEpochMilli()
