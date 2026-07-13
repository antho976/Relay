package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.rank.RankInfo
import com.forge.app.domain.rank.RankLadder
import com.forge.app.domain.rank.StandingEngine
import com.forge.app.domain.rank.StandingMetric
import com.forge.app.domain.rank.StandingSnapshot
import com.forge.app.domain.rank.XpBreakdown
import com.forge.app.domain.rank.XpEngine
import com.forge.app.domain.rank.XpSnapshot
import com.forge.app.domain.trophy.TrophyEvaluator
import com.forge.app.domain.trophy.TrophyStatsSnapshot
import com.forge.app.program.Trophy
import com.forge.app.program.TrophyIcon
import com.forge.app.program.Trophies
import com.forge.app.ui.overview.state.OnThisDayMemory
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withContext
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.IsoFields
import java.util.Locale
import javax.inject.Inject
import javax.inject.Singleton

/**
 * One trophy in the profile's trophy-case grid. [progress] is 0f..1f (1f when unlocked).
 * [name] / [description] / [progressLabel] feed the tap-to-inspect popup; [progressLabel] is the
 * "3 / 10 days"-style hint for locked cells (null when unlocked).
 */
data class TrophyCell(
    val icon: TrophyIcon,
    val unlocked: Boolean,
    val progress: Float,
    val name: String,
    val description: String,
    val progressLabel: String?,
    /** Tier-pip count so same-icon cells stay visually distinct (see [Trophies.variantFor]). */
    val variant: Int = 0,
    /** When this trophy was earned (epoch ms); null for locked cells. Surfaced in the inspect popup. */
    val unlockedAt: Long? = null
)

/** Everything the profile screen needs, assembled off the main thread in one fan-out. */
data class ProfileData(
    val rank: RankInfo,
    val xp: XpBreakdown,
    val standings: List<StandingMetric>,
    val totalSessions: Int,
    val totalVolumeLb: Double,
    val totalPrs: Int,
    val streakDays: Int,
    val longestStreakDays: Int,
    val sinceLabel: String,
    val trophyUnlocked: Int,
    val trophyTotal: Int,
    val trophyGrid: List<TrophyCell>,
    val closestTrophy: String?,
    val memory: OnThisDayMemory?,
    /** Lifetime total sets logged — an All-Time tile alongside workouts/volume/PRs. */
    val totalSets: Int = 0,
    /** This-week vs last-week tallies (ISO weeks) — power the All-Time tiles' "vs last week" arrows. */
    val workoutsThisWeek: Int = 0,
    val workoutsLastWeek: Int = 0,
    val setsThisWeek: Int = 0,
    val setsLastWeek: Int = 0,
    val prsThisWeek: Int = 0,
    val prsLastWeek: Int = 0,
    /** Cumulative lifted volume (lb), one point per finished session, oldest → newest — drives the
     *  All-Time graph. Single-point until a second lifting workout exists. */
    val lifetimeVolumeSeriesLb: List<Double> = emptyList(),
    /** How many times you trained each calendar day THIS YEAR (gym sessions + non-rest cardio),
     *  keyed by epoch-day — feeds the THIS YEAR consistency grid. Empty ⇒ no activity yet this year. */
    val activityByDay: Map<Long, Int> = emptyMap()
)

/**
 * Assembles the profile "You" hub: lifetime XP + rank (via [XpEngine] / [RankLadder]), the
 * offline standing estimate ([StandingEngine]) and the trophy-case grid —
 * all derived from existing finished-session data, so there is no new schema.
 * Pure engines stay in `domain/rank`; this layer only does I/O + assembly (the AdaptationRepository
 * pattern). Loaded once per profile open.
 */
@Singleton
class ProfileRepository @Inject constructor(
    private val sessionDao: SessionDao,
    private val loggedSetDao: LoggedSetDao,
    private val trophyRepo: TrophyRepository,
    private val statsRepo: StatsRepository,
    private val cardioRepo: CardioRepository,
    private val settingsRepo: SettingsRepository,
    private val clock: Clock
) {
    /** Last assembled data — lets the ViewModel paint instantly on profile re-entry (P3); always refreshed. */
    @Volatile private var lastData: ProfileData? = null
    /** The cached profile data, or null before the first load. */
    fun cached(): ProfileData? = lastData

    suspend fun load(): ProfileData = withContext(Dispatchers.IO) {
        val useKg = settingsRepo.useKg.first()
        val memberSinceMs = settingsRepo.memberSinceMs.first()
        val zone = ZoneId.systemDefault()
        val nowMs = clock.nowMs()
        // Today (system zone) — drives both the calendar-year bounds for the THIS YEAR consistency
        // grid's cardio read and the this-week/last-week tallies further down.
        val today = Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate()
        val yearStartMs = today.withDayOfYear(1).atStartOfDay(zone).toInstant().toEpochMilli()
        val yearEndMs = today.withDayOfYear(1).plusYears(1).atStartOfDay(zone).toInstant().toEpochMilli()
        coroutineScope {
        // Fire the independent DAO reads concurrently — the trophy snapshot's 13+ queries are the
        // long pole, so overlapping it with the rest is the main win for profile-open latency (#8).
        val sessionsD = async { sessionDao.allFinished().filter { !it.isUntracked } }
        val unlockedDatesD = async { trophyRepo.unlockedDatesById() }
        val snapshotD = async { runCatching { trophyRepo.snapshot() }.getOrNull() }
        val memoryD = async { runCatching { statsRepo.findOnThisDayMemory() }.getOrNull() }
        // Focused streak read (two queries) rather than subscribing the whole weekly fan-out just
        // to pull one Int — see StatsRepository.currentStreakDays.
        val streakD = async { runCatching { statsRepo.currentStreakDays() }.getOrDefault(0) }
        // Best e1RM over the 90-day window — one focused query rather than subscribing the full
        // stats Flow. Null when no weighted sets exist yet (pre-baseline guard in StandingEngine).
        val bestE1rmD = async {
            runCatching {
                val since90 = nowMs - 90L * 24 * 3600 * 1000
                loggedSetDao.bestE1rmLbSince(since90)
            }.getOrNull()
        }
        // This year's non-rest cardio, for the consistency grid (a "showed up" day is gym OR cardio).
        val cardioYearD = async { runCatching { cardioRepo.entriesInRange(yearStartMs, yearEndMs) }.getOrDefault(emptyList()) }
        val sessions = sessionsD.await()
        val unlockedDates = unlockedDatesD.await()
        val unlockedIds = unlockedDates.keys
        val trophyPoints = Trophies.all.filter { it.id in unlockedIds }.sumOf { it.tier.points }
        // Hoisted once — these full-list sums feed both the XP snapshot and the ProfileData below.
        val totalVolumeLb = sessions.sumOf { it.totalVolumeLb ?: 0.0 }
        val totalPrs = sessions.sumOf { it.prCount }
        val totalSets = sessions.sumOf { it.setCount }

        // ── This-week vs last-week tallies (ISO weeks) — the tiles' "vs last week" arrows ─────────
        val thisWeekKey = weekKeyOf(today)
        val lastWeekKey = weekKeyOf(today.minusWeeks(1))
        var workoutsThisWeek = 0; var workoutsLastWeek = 0
        var setsThisWeek = 0; var setsLastWeek = 0
        var prsThisWeek = 0; var prsLastWeek = 0
        sessions.forEach { s ->
            when (weekKey(s.startedAt, zone)) {
                thisWeekKey -> { workoutsThisWeek++; setsThisWeek += s.setCount; prsThisWeek += s.prCount }
                lastWeekKey -> { workoutsLastWeek++; setsLastWeek += s.setCount; prsLastWeek += s.prCount }
            }
        }

        // ── XP + rank ───────────────────────────────────────────────────────────
        val xp = XpEngine.compute(
            XpSnapshot(
                finishedSessions = sessions.size,
                totalSets = totalSets,
                totalPrs = totalPrs,
                totalVolumeLb = totalVolumeLb,
                activeWeeks = sessions.mapTo(mutableSetOf()) { weekKey(it.startedAt, zone) }.size,
                trophyPoints = trophyPoints
            ),
            useKg
        )
        val rank = RankLadder.rankFor(xp.total)

        // ── Standing (90-day window) ──────────────────────────────────────────────
        val since90 = nowMs - 90L * 24 * 3600 * 1000
        val recent = sessions.filter { it.startedAt >= since90 }
        val weeks90 = 90.0 / 7.0
        val standings = StandingEngine.standings(
            StandingSnapshot(
                sessionsPerWeek = recent.size / weeks90,
                streakWeeks = currentStreakWeeks(sessions, zone, nowMs),
                weeklyVolumeLb = recent.sumOf { it.totalVolumeLb ?: 0.0 } / weeks90,
                bestE1rmLb = bestE1rmD.await()
            ),
            useKg
        )

        // ── Trophy case ───────────────────────────────────────────────────────────
        val snapshot = snapshotD.await()
        val trophyGrid = curatedTrophyGrid(unlockedIds, unlockedDates, snapshot, useKg)
        val closestTrophy = snapshot?.let { snap ->
            Trophies.all.filter { it.id !in unlockedIds }
                .mapNotNull { t -> TrophyEvaluator.progressFraction(t.unlock, snap)?.let { t to it } }
                .filter { it.second > 0f }
                .maxByOrNull { it.second }
                ?.let { (t, _) -> TrophyEvaluator.progressRemaining(t.unlock, snap, useKg)?.let { "$it away from ${t.name}" } }
        }

        ProfileData(
            rank = rank,
            xp = xp,
            standings = standings,
            totalSessions = sessions.size,
            totalVolumeLb = totalVolumeLb,
            totalPrs = totalPrs,
            streakDays = streakD.await(),
            longestStreakDays = snapshot?.maxStreakEver ?: 0,
            sinceLabel = sinceMs(memberSinceMs, sessions)
                ?.let { Instant.ofEpochMilli(it).atZone(zone).format(SINCE_FMT).uppercase() } ?: "",
            trophyUnlocked = unlockedIds.size,
            trophyTotal = Trophies.all.size,
            trophyGrid = trophyGrid,
            closestTrophy = closestTrophy,
            memory = memoryD.await(),
            totalSets = totalSets,
            workoutsThisWeek = workoutsThisWeek,
            workoutsLastWeek = workoutsLastWeek,
            setsThisWeek = setsThisWeek,
            setsLastWeek = setsLastWeek,
            prsThisWeek = prsThisWeek,
            prsLastWeek = prsLastWeek,
            lifetimeVolumeSeriesLb = cumulativeSessionVolumeLb(sessions),
            activityByDay = buildYearActivity(sessions, cardioYearD.await(), zone, today.year)
        )
        }.also { lastData = it }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────────

    /**
     * The "member since" instant (epoch ms) for the profile's SINCE line, or null when there's nothing
     * to anchor it to yet. Prefers the onboarding date ([memberSinceMs], stamped when setup finished) so
     * the line shows from day one — even with zero workouts — instead of only after the first session.
     * When a logged session predates onboarding (imported history), the earlier of the two wins so
     * "since" reflects when the user actually started. Pre-onboarding-stamp users (memberSinceMs == 0)
     * fall back to their earliest session, preserving the old behaviour for them.
     */
    private fun sinceMs(memberSinceMs: Long, sessions: List<Session>): Long? {
        val earliestSession = sessions.minOfOrNull { it.startedAt }
        return when {
            memberSinceMs > 0L -> minOf(memberSinceMs, earliestSession ?: Long.MAX_VALUE)
            else -> earliestSession
        }
    }

    /**
     * The profile's curated trophy-case highlight (NOT the full catalog — that's one tap away):
     * the [HARDEST_DONE] hardest unlocked trophies (by tier points), then locked trophies ranked by
     * progress (almost-complete first, 0%-progress fillers after) up to [TROPHY_HIGHLIGHTS] cells.
     */
    private fun curatedTrophyGrid(unlockedIds: Set<String>, unlockedDates: Map<String, Long>, snapshot: TrophyStatsSnapshot?, useKg: Boolean): List<TrophyCell> {
        val hardestDone = Trophies.all
            .filter { it.id in unlockedIds }
            .sortedWith(compareByDescending<Trophy> { it.tier.points }.thenBy { it.name })
            .take(HARDEST_DONE)
            .map { TrophyCell(it.icon, unlocked = true, progress = 1f, name = it.name, description = it.description, progressLabel = null, variant = Trophies.variantFor(it.id), unlockedAt = unlockedDates[it.id]) }

        val locked = Trophies.all
            .filter { it.id !in unlockedIds }
            .map { t -> t to (snapshot?.let { TrophyEvaluator.progressFraction(t.unlock, it) }?.coerceIn(0f, 1f) ?: 0f) }
            .sortedWith(compareByDescending<Pair<Trophy, Float>> { it.second }.thenBy { it.first.name })
            .take(TROPHY_HIGHLIGHTS - hardestDone.size)
            .map { (t, p) ->
                TrophyCell(
                    t.icon, unlocked = false, progress = p, name = t.name, description = t.description,
                    progressLabel = snapshot?.let { TrophyEvaluator.progressHint(t.unlock, it, useKg) },
                    variant = Trophies.variantFor(t.id)
                )
            }

        return hardestDone + locked
    }

    /** Consecutive ISO weeks (ending this week or last) with ≥1 session — the streak in weeks. */
    private fun currentStreakWeeks(sessions: List<Session>, zone: ZoneId, nowMs: Long): Int {
        if (sessions.isEmpty()) return 0
        val weeks = sessions.mapTo(mutableSetOf()) { weekKey(it.startedAt, zone) }
        val today = Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate()
        var cursor = when {
            weekKeyOf(today) in weeks -> today
            weekKeyOf(today.minusWeeks(1)) in weeks -> today.minusWeeks(1)
            else -> return 0
        }
        var count = 0
        while (weekKeyOf(cursor) in weeks) {
            count++
            cursor = cursor.minusWeeks(1)
        }
        return count
    }

    private fun weekKey(ms: Long, zone: ZoneId): Int =
        weekKeyOf(Instant.ofEpochMilli(ms).atZone(zone).toLocalDate())

    private fun weekKeyOf(d: LocalDate): Int =
        d.get(IsoFields.WEEK_BASED_YEAR) * 100 + d.get(IsoFields.WEEK_OF_WEEK_BASED_YEAR)

    private companion object {
        const val HARDEST_DONE = 3        // unlocked trophies shown (hardest first)
        const val TROPHY_HIGHLIGHTS = 9   // total cells (done + almost-complete + 0% fillers)
        val SINCE_FMT: DateTimeFormatter = DateTimeFormatter.ofPattern("MMM yyyy", Locale.getDefault())
    }
}

/**
 * Cumulative lifted volume (lb) — one point per finished session, oldest → newest, as a running
 * total (a null/0-volume session just carries the total flat). Feeds the All-Time graph, which needs
 * ≥2 points to draw a line, so the curve appears as soon as there are two logged lifting workouts.
 * Pure (no DAO/DI), mirroring the StatsVolumeAggregations helper pattern.
 */
internal fun cumulativeSessionVolumeLb(sessions: List<Session>): List<Double> {
    if (sessions.isEmpty()) return emptyList()
    var cumulative = 0.0
    return sessions.sortedBy { it.startedAt }.map { cumulative += it.totalVolumeLb ?: 0.0; cumulative }
}

/**
 * Per-day training count for [year] — how many times you trained each calendar day (finished gym
 * sessions + non-rest cardio), keyed by epoch-day (system zone). Only days with activity are present,
 * so the map's size is the year's active-day count. Feeds the profile's THIS YEAR consistency grid.
 * Pure (no DAO/DI), mirroring [cumulativeSessionVolumeLb].
 */
internal fun buildYearActivity(
    sessions: List<Session>,
    cardio: List<CardioEntry>,
    zone: ZoneId,
    year: Int
): Map<Long, Int> {
    val counts = HashMap<Long, Int>()
    fun bump(ms: Long) {
        val date = Instant.ofEpochMilli(ms).atZone(zone).toLocalDate()
        if (date.year == year) {
            val key = date.toEpochDay()
            counts[key] = (counts[key] ?: 0) + 1
        }
    }
    sessions.forEach { bump(it.startedAt) }
    cardio.forEach { bump(it.date) }
    return counts
}

