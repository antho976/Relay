package com.forge.app.service.wear

import android.content.Context
import com.forge.app.core.time.Clock
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.AdaptationRepository
import com.forge.app.data.repo.StatsRepository
import com.forge.app.domain.timer.RestTimerState
import com.forge.app.domain.units.WeightUnit
import com.forge.app.program.Program
import com.forge.shared.protocol.ConfigDto
import com.forge.shared.protocol.GlanceTodayDto
import com.forge.shared.protocol.TimerStateDto
import com.forge.shared.protocol.WearCodec
import com.forge.shared.protocol.WearProtocol
import com.google.android.gms.wearable.PutDataRequest
import com.google.android.gms.wearable.Wearable
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.tasks.await
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.math.abs

/**
 * Publishes phone state to the wrist as DataItems (W1): /session/live off the repository-side
 * [WatchSessionMirror], /timer/state off the app-scoped [SessionTimerHolder], /config off the
 * settings flows, and /glance/today at the existing surface points (app open, session finish) —
 * never a hot path, never a ticking stream. All writes are fail-soft: no watch or no Play
 * services just means the DataItems sit unread.
 */
@Singleton
class WearStatePublisher @Inject constructor(
    @ApplicationContext private val context: Context,
    private val mirror: WatchSessionMirror,
    private val timerHolder: SessionTimerHolder,
    private val settingsRepo: SettingsRepository,
    private val adaptationRepo: AdaptationRepository,
    private val directiveRepo: com.forge.app.data.repo.DirectiveRepository,
    private val statsRepo: StatsRepository,
    private val clock: Clock
) {
    private val dataClient by lazy { Wearable.getDataClient(context) }

    fun start(scope: CoroutineScope) {
        scope.launch {
            mirror.sessionLive.distinctUntilChanged().collect { dto ->
                if (dto == null) {
                    deleteItem(WearProtocol.PATH_SESSION_LIVE)
                    // A session just ended (or none exists) — refresh the idle glance so the wrist's
                    // week count includes the workout that just finished.
                    publishGlanceNow()
                } else {
                    putItem(WearProtocol.PATH_SESSION_LIVE, WearCodec.encode(dto))
                }
            }
        }
        scope.launch {
            var last: TimerStateDto? = null
            timerHolder.controller.state.collect { state ->
                val dto = state?.toDto()
                val prev = last
                // The controller re-emits every tick; the wrist derives its own countdown from
                // endAtMs, so only STRUCTURAL changes republish (start/pause/resume/±30/stop —
                // detected as a shifted end instant or a mode flip), never the 1 Hz decrement.
                val structural = when {
                    (dto == null) != (prev == null) -> true
                    dto == null || prev == null -> false
                    dto.paused != prev.paused || dto.totalSeconds != prev.totalSeconds -> true
                    !dto.paused && abs(dto.endAtMs - prev.endAtMs) > TIMER_REPUBLISH_SLOP_MS -> true
                    dto.paused && dto.pausedRemainingSeconds != prev.pausedRemainingSeconds -> true
                    else -> false
                }
                if (structural) {
                    last = dto
                    if (dto == null) deleteItem(WearProtocol.PATH_TIMER_STATE)
                    else putItem(WearProtocol.PATH_TIMER_STATE, WearCodec.encode(dto))
                }
            }
        }
        scope.launch {
            combine(
                settingsRepo.accentColorHex,
                settingsRepo.accentEnabled,
                settingsRepo.weightUnit
            ) { hex, enabled, unit: WeightUnit ->
                ConfigDto(accentHex = hex, accentEnabled = enabled, unit = unit.toProtocol())
            }.distinctUntilChanged().collect { putItem(WearProtocol.PATH_CONFIG, WearCodec.encode(it)) }
        }
        // App open = a glance surface point.
        scope.launch { publishGlanceNow() }
    }

    /** One-shot /glance/today refresh — cheap reads, stamped with its compute time. */
    suspend fun publishGlanceNow() {
        val readiness = runCatching { adaptationRepo.readinessScale() }.getOrNull()
        val week = runCatching { statsRepo.observeWeeklyStats().first() }.getOrNull()
        val freestyle = settingsRepo.freestyleMode.first()
        val nextDayTitle = week?.nextUpDayKey
            ?.takeIf { !freestyle && Program.days.isNotEmpty() }
            ?.let { Program.dayDisplayName(it) }
        // The wrist shows the SAME answer the phone does (B2) — the tile consumes the directive
        // verbatim rather than re-deriving "what now?" from next-up, which is how two surfaces
        // start disagreeing. Null-safe: an unavailable directive falls back to next-planned-day.
        val answer = runCatching { directiveRepo.today() }.getOrNull()
        val dto = GlanceTodayDto(
            readinessPercent = readiness?.percent,
            nextDayTitle = nextDayTitle,
            directiveHeadline = answer?.directive?.headline,
            directiveReason = answer?.directive?.reason,
            weekSessionsDone = week?.workouts ?: 0,
            // The weekly target only means something with a real plan — freestyle shows the bare count.
            weekSessionsPlanned = settingsRepo.daysPerWeek.first()
                .takeIf { it > 0 && !freestyle && Program.days.isNotEmpty() },
            weekVolumeText = week?.volumeLb?.takeIf { it > 0 }?.let { formatVolumeLb(it) },
            computedAtMs = clock.nowMs()
        )
        putItem(WearProtocol.PATH_GLANCE_TODAY, WearCodec.encode(dto))
    }

    /** Ack a wrist command (latest-wins DataItem; the watch matches on commandId). */
    suspend fun publishAck(ack: com.forge.shared.protocol.CmdAckDto) {
        putItem(WearProtocol.PATH_CMD_ACK, WearCodec.encode(ack))
    }

    private fun RestTimerState.toDto(): TimerStateDto = TimerStateDto(
        endAtMs = if (isPaused) 0L else clock.nowMs() + secondsRemaining * 1000L,
        totalSeconds = totalSeconds,
        paused = isPaused,
        pausedRemainingSeconds = if (isPaused) secondsRemaining else 0
    )

    private suspend fun putItem(path: String, bytes: ByteArray) {
        runCatching {
            val request = PutDataRequest.create(path).apply {
                data = bytes
                setUrgent()
            }
            dataClient.putDataItem(request).await()
        }
    }

    private suspend fun deleteItem(path: String) {
        runCatching {
            dataClient.deleteDataItems(
                android.net.Uri.parse("wear://*$path")
            ).await()
        }
    }

    private fun formatVolumeLb(volumeLb: Double): String =
        if (volumeLb >= 10_000) "%.1fk lb".format(volumeLb / 1000) else "${volumeLb.toInt()} lb"

    private companion object {
        /** endAtMs jitter tolerated between tick-derived recomputes before it counts as a restart. */
        const val TIMER_REPUBLISH_SLOP_MS = 1_500L
    }
}
