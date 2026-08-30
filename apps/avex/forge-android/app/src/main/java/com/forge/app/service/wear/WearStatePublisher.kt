package com.forge.app.service.wear

import android.content.Context
import com.forge.app.core.time.Clock
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.AdaptationRepository
import com.forge.app.data.repo.StatsRepository
import com.forge.app.domain.timer.RestTimerState
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.program.Program
import com.forge.shared.protocol.CmdAckDto
import com.forge.shared.protocol.ConfigDto
import com.forge.shared.protocol.GlanceTodayDto
import com.forge.shared.protocol.TimerStateDto
import com.forge.shared.protocol.WearCodec
import com.forge.shared.protocol.WearProtocol
import com.google.android.gms.wearable.DataClient
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

    /**
     * `endAtMs` of the running timer as last published to the wrist, or 0 when none is running.
     *
     * [com.forge.app.service.WorkoutSessionService] compares the wrist's haptic ack against this so
     * an ack for a finished rest cannot silence the next one.
     */
    @Volatile
    var lastPublishedTimerEndAtMs: Long = 0L
        private set

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
                    // The identity the wrist will quote back in its haptic ack. Held here because
                    // this is the only place that knows what was actually PUBLISHED — the
                    // controller's own state is recomputed every tick and would not match.
                    lastPublishedTimerEndAtMs = if (dto == null || dto.paused) 0L else dto.endAtMs
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
        val unit = settingsRepo.weightUnit.first()
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
            // The phone's own compact formatter, in the user's unit. A private lb-only helper here
            // meant a kg user's Week tile read "12.4k LB" while the phone beside it read "5.6k kg".
            weekVolumeText = week?.volumeLb?.takeIf { it > 0 }?.let { formatVolumeCompact(it, unit) },
            computedAtMs = clock.nowMs()
        )
        putItem(WearProtocol.PATH_GLANCE_TODAY, WearCodec.encode(dto))
    }

    /**
     * Ack a wrist command, at that command's OWN path so a later ack can never supersede an
     * unsynced earlier one (see [WearProtocol.PATH_CMD_ACK]).
     *
     * Per-path items would otherwise accumulate in the Data Layer forever, so the oldest is dropped
     * once [ACK_HISTORY] newer ones exist — long past any plausible sync delay, and never the item
     * just written.
     */
    suspend fun publishAck(ack: CmdAckDto) {
        val path = WearProtocol.ackPath(ack.commandId)
        putItem(path, WearCodec.encode(ack))
        pruneAcks(keep = path)
    }

    /**
     * Delete every ack DataItem beyond the newest [ACK_HISTORY], reading the live set rather than a
     * list of what THIS process wrote.
     *
     * The bound used to be an in-memory queue of recently written paths, which meant it only ever
     * bounded one process lifetime. DataItems outlive the process by design — that is why acks live
     * on the Data Layer at all — so every restart began remembering nothing and the items from
     * previous runs were never deleted by anyone. They accumulate one per wrist command, for the
     * life of the install, and the watch's tiles and complications each read the FULL item set on
     * every render (WearGlanceStore has no path-scoped query), so the cost lands on the surface
     * that has to be fastest.
     *
     * Ordered by the ack's own [CmdAckDto.atMs] rather than the item's
     * URI, which carries no time. An item that cannot be decoded sorts oldest: it is unreadable to
     * the watch too, so it has nothing to lose by going first.
     */
    private suspend fun pruneAcks(keep: String) {
        runCatching {
            val prefix = android.net.Uri.Builder()
                .scheme(PutDataRequest.WEAR_URI_SCHEME)
                .path(WearProtocol.PATH_CMD_ACK)
                .build()
            val buffer = dataClient.getDataItems(prefix, DataClient.FILTER_PREFIX).await()
            val stale = try {
                buffer
                    .map { it.freeze() }
                    .filter { it.uri.path != keep }
                    .sortedByDescending { ackTimeOf(it.data) }
                    .drop(ACK_HISTORY - 1) // -1: `keep` was just written and holds one of the slots.
                    .mapNotNull { it.uri.path }
            } finally {
                buffer.release()
            }
            stale.forEach { deleteItem(it) }
        }
    }

    /** When an ack DataItem's payload says it was written, or [Long.MIN_VALUE] if it can't say. */
    private fun ackTimeOf(bytes: ByteArray?): Long {
        if (bytes == null) return Long.MIN_VALUE
        return when (val decoded = WearCodec.decode<CmdAckDto>(bytes)) {
            is WearCodec.DecodeResult.Ok -> decoded.value.atMs
            else -> Long.MIN_VALUE
        }
    }

    private fun RestTimerState.toDto(): TimerStateDto {
        val now = clock.nowMs()
        return TimerStateDto(
            endAtMs = if (isPaused) 0L else now + secondsRemaining * 1000L,
            totalSeconds = totalSeconds,
            paused = isPaused,
            pausedRemainingSeconds = if (isPaused) secondsRemaining else 0,
            // Stamped from the SAME reading as endAtMs, so the watch can turn an absolute instant on
            // this phone's clock into a duration and cancel the skew between the two devices.
            publishedAtMs = now
        )
    }

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

    private companion object {
        /** endAtMs jitter tolerated between tick-derived recomputes before it counts as a restart. */
        const val TIMER_REPUBLISH_SLOP_MS = 1_500L
        /** How many per-command acks stay live before the oldest is deleted. */
        const val ACK_HISTORY = 10
    }
}
