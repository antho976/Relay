package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.BodyFatEntry
import com.forge.app.data.db.entities.BodyMeasurementEntry
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.db.entities.LeanMassEntry
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.measurement.BodyMeasurementType
import com.forge.app.domain.units.WeightUnit
import com.forge.app.ui.profile.state.ProfileUiState
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.ProfileColors
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.ZoneId

/**
 * A lifter like the one the Profile design was drawn for: training since March 2025 on a
 * Mon/Tue/Wed/Fri/Sat plan, a deload week mid-October, a cut from 186 toward a 178 goal, and a
 * year and a half of Front/Side/Back photos. "Today" is pinned so the month never moves under a
 * golden.
 */
internal object ProfileFixtures {
    val TODAY: LocalDate = LocalDate.of(2026, 10, 21)
    private val zone: ZoneId = ZoneId.systemDefault()
    private const val DAY = 86_400_000L
    fun ms(d: LocalDate, hour: Int = 9): Long = d.atTime(hour, 0).atZone(zone).toInstant().toEpochMilli()
    val NOW: Long = ms(TODAY, 12)

    /** The deload week of Oct 12: three light days, the rest of it planned off. */
    private val deloadWeek = (12..18).map { LocalDate.of(2026, 10, it) }
    private val deloadTrained = setOf(12, 14, 16).map { LocalDate.of(2026, 10, it) }

    val planOn: (LocalDate) -> Boolean? = { d ->
        when {
            d.isBefore(LocalDate.of(2025, 3, 10)) -> null
            d in deloadWeek -> d !in deloadTrained
            else -> d.dayOfWeek == DayOfWeek.THURSDAY || d.dayOfWeek == DayOfWeek.SUNDAY
        }
    }
    val onVacation: (LocalDate) -> Boolean = { false }

    /** Every planned day trained since March 2025, and today not yet. */
    val trainedDays: List<LocalDate> = generateSequence(LocalDate.of(2025, 3, 10)) { it.plusDays(1) }
        .takeWhile { it.isBefore(TODAY) }
        .filter { planOn(it) == false }
        .toList()

    fun state(): ProfileUiState {
        val sessions = trainedDays.takeLast(212)
        val perSession = 2_900_000.0 / sessions.size
        return ProfileUiState(
            loading = false,
            name = "Antho",
            sinceLabel = "MAR 2025",
            totalSessions = 212,
            totalVolumeLb = 2_900_000.0,
            totalPrs = 64,
            totalSets = 4830,
            workoutsThisWeek = 3,
            setsThisWeek = 52,
            prsThisMonth = 2,
            photos = photos(),
            lifetimeVolumeSeriesLb = sessions.indices.map { (it + 1) * perSession },
            lifetimeVolumeTimesMs = sessions.map { ms(it) },
            activityByDay = trainedDays.associate { it.toEpochDay() to 1 },
            deloadDays = deloadTrained.map { it.toEpochDay() }.toSet()
        )
    }

    fun body(): ProfileBody {
        val weights = (0 until 30).map { i ->
            val d = TODAY.minusDays(87L - i * 3)
            val wave = if (i % 4 == 1) 0.4 else if (i % 4 == 3) -0.3 else 0.0
            BodyweightEntry(id = i.toLong(), dateKey = d.toString(), weightLb = 186.1 - i * 0.13 + wave, recordedAt = ms(d, 7))
        }
        val fat = (0 until 6).map { i ->
            val d = TODAY.minusDays(30L - i * 6)
            BodyFatEntry(id = i.toLong(), dateKey = d.toString(), percent = listOf(17.4, 17.3, 17.1, 17.2, 16.9, 16.8)[i], recordedAt = ms(d, 7))
        }
        val lean = (0 until 6).map { i ->
            val d = TODAY.minusDays(30L - i * 6)
            LeanMassEntry(id = i.toLong(), dateKey = d.toString(), weightLb = listOf(147.4, 147.5, 147.5, 147.8, 148.0, 148.2)[i], recordedAt = ms(d, 7))
        }
        val waist = (0 until 4).map { i ->
            val d = TODAY.minusDays(28L - i * 9)
            BodyMeasurementEntry(id = i.toLong(), type = "waist", dateKey = d.toString(), valueCm = listOf(34.0, 33.9, 33.6, 33.5)[i] * 2.54, recordedAt = ms(d, 7))
        }
        return ProfileBody(
            weight = weights,
            goalLb = 178.0,
            bodyFat = fat,
            leanMass = lean,
            leanMassConnected = true,
            measurements = BodyMeasurementsUiState(
                series = BodyMeasurementType.entries.map {
                    MeasurementSeries(it, if (it == BodyMeasurementType.WAIST) waist else emptyList())
                },
                useCm = false
            )
        )
    }

    /** 38 photos: a Front, Side and Back most months from March 2025, Front first and heaviest. */
    fun photos(): List<ProgressPhoto> {
        val out = mutableListOf<ProgressPhoto>()
        var month = java.time.YearMonth.of(2025, 3)
        var i = 0
        val months = generateSequence(month) { it.plusMonths(1) }.takeWhile { !it.isAfter(java.time.YearMonth.of(2026, 10)) }.toList()
        months.forEachIndexed { mi, m ->
            if (mi in setOf(2, 4, 6, 9, 11, 13, 15, 17)) return@forEachIndexed
            val day = if (m == java.time.YearMonth.of(2026, 10)) 19 else 12
            val base = 191.2 - mi * (191.2 - 182.4) / (months.size - 1)
            listOf("FRONT" to 0.0, "SIDE" to 0.2, "BACK" to 0.7).forEach { (pose, off) ->
                out += ProgressPhoto("p${i++}.jpg", ms(m.atDay(day)), pose = pose, weightLb = if (pose == "FRONT") "%.1f".format(java.util.Locale.US, base).toDouble() else base + off)
            }
        }
        month = java.time.YearMonth.of(2026, 9)
        out += ProgressPhoto("p${i++}.jpg", ms(month.atDay(26)), weightLb = 183.0)
        out += ProgressPhoto("p${i}.jpg", ms(month.atDay(28)), weightLb = 182.8)
        return out
    }

    val unit = WeightUnit.LB

    /** A stand-in for a decoded photo: the frame's ground and a figure, as the design draws it. */
    val photo: @Composable (ProgressPhoto, Modifier) -> Unit = { _, m ->
        Box(m.background(ProfileColors.photoGround)) {
            Icon(OpenIcons.User, contentDescription = null, tint = ProfileColors.PhotoGlyph, modifier = Modifier.align(Alignment.Center).size(36.dp))
        }
    }
}
