package com.forge.shared.protocol

import com.forge.shared.weight.ProtocolWeightUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class WearCodecTest {

    private val session = SessionLiveDto(
        sessionId = 42L,
        dayTitle = "Pull B",
        exerciseId = "row_barbell",
        exerciseName = "Barbell Row",
        exerciseIndex = 2,
        exerciseCount = 5,
        setIndex = 3,
        setTotal = 4,
        targetWeightText = "185 lb",
        targetRepsText = "8-12",
        loggedSets = 2,
        lastSetWasPr = true,
        startedAtMs = 1_000_000L,
        unit = ProtocolWeightUnit.KG,
        weightStep = 2.5,
        isPlates = false
    )

    @Test
    fun `every dto round-trips`() {
        assertEquals(session, roundTrip(session))
        val timer = TimerStateDto(endAtMs = 99_000L, totalSeconds = 150, paused = true, pausedRemainingSeconds = 42)
        assertEquals(timer, roundTrip(timer))
        val config = ConfigDto(accentHex = "#3D4F73", accentEnabled = false, unit = ProtocolWeightUnit.ST)
        assertEquals(config, roundTrip(config))
        val glance = GlanceTodayDto(readinessPercent = 82, nextDayTitle = "Push A", weekSessionsDone = 2, weekSessionsPlanned = 4, weekVolumeText = "12.4k lb", computedAtMs = 5L)
        assertEquals(glance, roundTrip(glance))
        val log = LogSetCommand(commandId = "c-1", sessionId = 42L, exerciseId = "row_barbell", weightText = "187.5", reps = 9)
        assertEquals(log, roundTrip(log))
        val timerCmd = TimerCommand(commandId = "c-2", action = TimerCommand.Action.ADD_30)
        assertEquals(timerCmd, roundTrip(timerCmd))
        val undo = UndoSetCommand(commandId = "c-3", sessionId = 42L)
        assertEquals(undo, roundTrip(undo))
        val hr = HrBatchDto(sessionId = 42L, samples = listOf(HrBatchDto.Sample(1L, 120), HrBatchDto.Sample(6L, 131)))
        assertEquals(hr, roundTrip(hr))
        val ack = CmdAckDto(commandId = "c-1", ok = false, reason = "stale session", atMs = 9L)
        assertEquals(ack, roundTrip(ack))
        val haptic = HapticAckDto(timerEndAtMs = 99_000L, atMs = 10L)
        assertEquals(haptic, roundTrip(haptic))
    }

    private inline fun <reified T> roundTrip(dto: T): T {
        val result = WearCodec.decode<T>(WearCodec.encode(dto))
        return (result as WearCodec.DecodeResult.Ok<T>).value
    }

    @Test
    fun `enum constants have fixed wire strings on both ends of the protocol`() {
        // The phone APK and the watch APK are minified by two independent R8 runs, and an enum
        // travels as its constant name. If one end renamed a constant, decode would report Invalid
        // and the payload would be dropped in silence — release-only, with no crash and no log.
        // Golden strings, so a rename becomes a red test instead of a field report.
        assertEquals(
            """{"v":1,"commandId":"c-2","action":"ADD_30"}""",
            WearCodec.encode(TimerCommand(commandId = "c-2", action = TimerCommand.Action.ADD_30))
                .decodeToString()
        )
        assertEquals(
            """{"v":1,"accentHex":"#3D4F73","accentEnabled":false,"unit":"ST"}""",
            WearCodec.encode(ConfigDto(accentHex = "#3D4F73", accentEnabled = false, unit = ProtocolWeightUnit.ST))
                .decodeToString()
        )
    }

    @Test
    fun `a newer protocol version is dropped as NewerVersion, not a crash`() {
        val newer = WearCodec.encode(session.copy(v = WearProtocol.VERSION + 1))
        val result = WearCodec.decode<SessionLiveDto>(newer)
        assertTrue(result is WearCodec.DecodeResult.NewerVersion)
    }

    @Test
    fun `unknown extra fields are ignored - additive evolution is compatible`() {
        // A v1 payload from a future build that ADDED a field still decodes on this build.
        val withExtra = """{"v":1,"commandId":"c-9","sessionId":7,"futureField":"x"}"""
        val result = WearCodec.decode<UndoSetCommand>(withExtra.encodeToByteArray())
        assertEquals(UndoSetCommand(commandId = "c-9", sessionId = 7L), (result as WearCodec.DecodeResult.Ok).value)
    }

    @Test
    fun `corrupt payloads decode to Invalid`() {
        assertTrue(WearCodec.decode<SessionLiveDto>("not json".encodeToByteArray()) is WearCodec.DecodeResult.Invalid)
        assertTrue(WearCodec.decode<SessionLiveDto>("""{"no":"stamp"}""".encodeToByteArray()) is WearCodec.DecodeResult.Invalid)
        assertTrue(WearCodec.decode<SessionLiveDto>(byteArrayOf(0, -1, 4)) is WearCodec.DecodeResult.Invalid)
    }

    @Test
    fun `wrong-shape payload with a valid stamp decodes to Invalid`() {
        val timerBytes = WearCodec.encode(TimerStateDto(endAtMs = 1L, totalSeconds = 60))
        assertTrue(WearCodec.decode<LogSetCommand>(timerBytes) is WearCodec.DecodeResult.Invalid)
    }
}
