package com.forge.shared.protocol

import com.forge.shared.weight.ProtocolWeightUnit
import kotlinx.serialization.Serializable

/**
 * The wear protocol payloads (v1). Every DTO carries its protocol version as `v`; decode through
 * [WearCodec] so an unknown-newer version is dropped with a surface, never a crash. All fields
 * the watch renders arrive AUTHORITATIVE from the phone (display-name strings, preformatted
 * weight text) — the watch never resolves names or units itself.
 */

/** The active-session mirror ([WearProtocol.PATH_SESSION_LIVE]). Absent DataItem = no session. */
@Serializable
data class SessionLiveDto(
    val v: Int = WearProtocol.VERSION,
    val sessionId: Long,
    val dayTitle: String,
    /** Current exercise (the one the next set belongs to); null between exercises / session done. */
    val exerciseId: String? = null,
    val exerciseName: String? = null,
    /** 1-based position of the current exercise among the session's exercises. */
    val exerciseIndex: Int = 0,
    val exerciseCount: Int = 0,
    /** Next set number (1-based) and the planned set count for the current exercise. */
    val setIndex: Int = 0,
    val setTotal: Int = 0,
    /** Prescribed target, preformatted by the phone ("185 lb", "3 plates"); null = none. */
    val targetWeightText: String? = null,
    /** Rep target text (a range like "8-12" or a number); null = none. */
    val targetRepsText: String? = null,
    /** Sets already logged for the current exercise — the ticks. */
    val loggedSets: Int = 0,
    /** The most recently logged set was a PR — the watch's gold moment. */
    val lastSetWasPr: Boolean = false,
    /** Wall-clock anchor the watch derives session-elapsed from locally (never a ticking stream). */
    val startedAtMs: Long,
    /** Bezel-adjust metadata: unit + step in display unit + plate mode (shared step table). */
    val unit: ProtocolWeightUnit = ProtocolWeightUnit.LB,
    val weightStep: Double = 5.0,
    val isPlates: Boolean = false
)

/** Rest-timer state ([WearProtocol.PATH_TIMER_STATE]). The watch renders the countdown locally
 *  from [endAtMs] wall clock; no per-second sync. Absent DataItem = no timer. */
@Serializable
data class TimerStateDto(
    val v: Int = WearProtocol.VERSION,
    /** Wall-clock instant the countdown reaches zero. Authoritative while running. */
    val endAtMs: Long,
    val totalSeconds: Int,
    val paused: Boolean = false,
    /** Remaining seconds AT PAUSE TIME (meaningful only while [paused]). */
    val pausedRemainingSeconds: Int = 0
)

/** Theme + units ([WearProtocol.PATH_CONFIG]) — the user's single accent, delivered so the watch
 *  can obey the design doctrine's ladder; monochrome mode maps accent onto near-white. */
@Serializable
data class ConfigDto(
    val v: Int = WearProtocol.VERSION,
    /** "#RRGGBB" accent hex; empty = the app default accent. */
    val accentHex: String = "",
    val accentEnabled: Boolean = true,
    val unit: ProtocolWeightUnit = ProtocolWeightUnit.LB
)

/** The tile/idle-home payload ([WearProtocol.PATH_GLANCE_TODAY]), stamped with its data age. */
@Serializable
data class GlanceTodayDto(
    val v: Int = WearProtocol.VERSION,
    /** Readiness scale percent, null below the coach's data gates (degrade, never blank). */
    val readinessPercent: Int? = null,
    /** Next planned day's display name; null in freestyle / no-plan mode. */
    val nextDayTitle: String? = null,
    val weekSessionsDone: Int = 0,
    /** Weekly session target; null when no real plan. */
    val weekSessionsPlanned: Int? = null,
    /** Preformatted week volume ("12.4k lb"); null until a set lands. */
    val weekVolumeText: String? = null,
    /** When the phone computed this — the tile's data-age stamp. */
    val computedAtMs: Long
)

/** Log the current set ([WearProtocol.PATH_CMD_LOG_SET]). As-prescribed taps send no overrides;
 *  bezel-adjusted sets send the final values the wrist showed. */
@Serializable
data class LogSetCommand(
    val v: Int = WearProtocol.VERSION,
    /** UUID — the phone dedups on it; a re-send after a BT flap can never double-log. */
    val commandId: String,
    /** The session the watch believes is active — the phone drops a stale-session command. */
    val sessionId: Long,
    /** The exercise the watch showed; the phone validates it's still current. */
    val exerciseId: String? = null,
    /** Weight as the display text the wrist showed ("187.5", plate count on plate exercises);
     *  null = log as prescribed. */
    val weightText: String? = null,
    /** Reps override; null = as prescribed. */
    val reps: Int? = null
)

/** Rest-timer control ([WearProtocol.PATH_CMD_TIMER]). */
@Serializable
data class TimerCommand(
    val v: Int = WearProtocol.VERSION,
    val commandId: String,
    val action: Action
) {
    @Serializable
    enum class Action { SKIP, ADD_30, START }
}

/** Undo the last set ([WearProtocol.PATH_CMD_UNDO_SET]). */
@Serializable
data class UndoSetCommand(
    val v: Int = WearProtocol.VERSION,
    val commandId: String,
    val sessionId: Long
)

/** A batch of live HR samples during an active session ([WearProtocol.PATH_HR_BATCH], W3). */
@Serializable
data class HrBatchDto(
    val v: Int = WearProtocol.VERSION,
    val sessionId: Long,
    val samples: List<Sample>,
    /** The watch's cumulative measured calories for this exercise so far (Health Services);
     *  null when the watch isn't reporting them. Replaces the phone's MET estimate at finish. */
    val totalKcal: Double? = null
) {
    @Serializable
    data class Sample(val atMs: Long, val bpm: Int)
}

/** Command acknowledgement ([WearProtocol.PATH_CMD_ACK]) — a DataItem so it survives a flap. */
@Serializable
data class CmdAckDto(
    val v: Int = WearProtocol.VERSION,
    val commandId: String,
    val ok: Boolean,
    /** Why a command was refused, for the wrist's quiet error line ("stale session"). */
    val reason: String? = null,
    /** The acked set was a PR — the wrist's gold flash + double-tick rides the confirmation. */
    val pr: Boolean = false,
    val atMs: Long
)

/** The watch felt the timer-done buzz ([WearProtocol.PATH_HAPTIC_ACK]) — phone stays silent. */
@Serializable
data class HapticAckDto(
    val v: Int = WearProtocol.VERSION,
    /** The endAtMs of the timer instance that buzzed, so a late ack can't silence a NEW timer. */
    val timerEndAtMs: Long,
    val atMs: Long
)
