package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.SessionDao
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
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.program.Program
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
import java.time.LocalTime
import java.time.YearMonth
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle
import java.time.temporal.IsoFields
import java.util.Locale
import javax.inject.Inject
import javax.inject.Singleton

/** A single signature lift (heaviest set ever). */
data class SignatureLift(val name: String, val weightLb: Double)

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
    val variant: Int = 0
)

/** One "On the record" row (a month- or year-in-review summary). */
data class RecapRowData(val title: String, val subtitle: String, val isYear: Boolean)

/** Everything the profile screen needs, assembled off the main thread in one fan-out. */
data class ProfileData(
    val rank: RankInfo,
    val xp: XpBreakdown,
    val standings: List<StandingMetric>,
    val totalSessions: Int,
    val totalVolumeLb: Double,
    val totalPrs: Int,
    val streakDays: Int,
    val sinceLabel: String,
    val topLift: SignatureLift?,
    val mostLoggedDay: String?,
    val usualHour: String?,
    val trophyUnlocked: Int,
    val trophyTotal: Int,
    val trophyGrid: List<TrophyCell>,
    val closestTrophy: String?,
    val memory: OnThisDayMemory?,
    val recaps: List<RecapRowData>
)

/**
 * Assembles the profile "You" hub: lifetime XP + rank (via [XpEngine] / [RankLadder]), the
 * offline standing estimate ([StandingEngine]), the signature lifts, the trophy-case grid and the
 * month/year recaps — all derived from existing finished-session data, so there is no new schema.
 * Pure engines stay in `domain/rank`; this layer only does I/O + assembly (the AdaptationRepository
 * pattern). Loaded once per profile open.
 */
@Singleton
class ProfileRepository @Inject constructor(
    private val sessionDao: SessionDao,
    private val loggedSetDao: LoggedSetDao,
    private val trophyRepo: TrophyRepository,
    private val statsRepo: StatsRepository,
    private val settingsRepo: SettingsRepository,
    private val clock: Clock
) {
    /** Last assembled data — lets the ViewModel paint instantly on profile re-entry (P3); always refreshed. */
    @Volatile private var lastData: ProfileData? = null
    /** The cached profile data, or null before the first load. */
    fun cached(): ProfileData? = lastData

    suspend fun load(): ProfileData = withContext(Dispatchers.IO) {
        val useKg = settingsRepo.useKg.first()
        val zone = ZoneId.systemDefault()
        val nowMs = clock.nowMs()
        coroutineScope {
        // Fire the independent DAO reads concurrently — the trophy snapshot's 13+ queries are the
        // long pole, so overlapping it with the rest is the main win for profile-open latency (#8).
        val sessionsD = async { sessionDao.allFinished().filter { !it.isUntracked } }
        val unlockedD = async { trophyRepo.unlockedIds() }
        val snapshotD = async { runCatching { trophyRepo.snapshot() }.getOrNull() }
        val topLiftD = async { loggedSetDao.topLift() }
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

        val sessions = sessionsD.await()
        val unlockedIds = unlockedD.await()
        val trophyPoints = Trophies.all.filter { it.id in unlockedIds }.sumOf { it.tier.points }
        // Hoisted once — these full-list sums feed both the XP snapshot and the ProfileData below.
        val totalVolumeLb = sessions.sumOf { it.totalVolumeLb ?: 0.0 }
        val totalPrs = sessions.sumOf { it.prCount }

        // ── XP + rank ───────────────────────────────────────────────────────────
        val xp = XpEngine.compute(
            XpSnapshot(
                finishedSessions = sessions.size,
                totalSets = sessions.sumOf { it.setCount },
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

        // ── Signature ─────────────────────────────────────────────────────────────
        val topLift = topLiftD.await()?.let { row ->
            // Humanized (incl. seed split) so a signature lift never drops out or shows a raw id (C3).
            SignatureLift(Program.exerciseDisplayName(row.exerciseId), row.weightLb)
        }
        val mostLoggedDay = sessions.groupingBy { it.dayKey }.eachCount().maxByOrNull { it.value }?.key
            // dayOrNull (not day) so a since-deleted history key shows no name rather than the wrong
            // day's name — Program.day would now substitute a real (but unrelated) day for a stale key.
            ?.let { key -> Program.dayOrNull(key)?.defaultName }
        val usualHour = sessions
            .groupingBy { Instant.ofEpochMilli(it.startedAt).atZone(zone).hour }
            .eachCount().maxByOrNull { it.value }?.key
            ?.let { LocalTime.of(it, 0).format(HOUR_FMT) }

        // ── Trophy case ───────────────────────────────────────────────────────────
        val snapshot = snapshotD.await()
        val trophyGrid = curatedTrophyGrid(unlockedIds, snapshot, useKg)
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
            sinceLabel = sessions.minOfOrNull { it.startedAt }
                ?.let { Instant.ofEpochMilli(it).atZone(zone).format(SINCE_FMT).uppercase() } ?: "",
            topLift = topLift,
            mostLoggedDay = mostLoggedDay,
            usualHour = usualHour,
            trophyUnlocked = unlockedIds.size,
            trophyTotal = Trophies.all.size,
            trophyGrid = trophyGrid,
            closestTrophy = closestTrophy,
            memory = memoryD.await(),
            recaps = buildRecaps(sessions, zone, nowMs, useKg)
        )
        }.also { lastData = it }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────────

    private fun buildRecaps(sessions: List<Session>, zone: ZoneId, nowMs: Long, useKg: Boolean): List<RecapRowData> {
        if (sessions.isEmpty()) return emptyList()
        val now = Instant.ofEpochMilli(nowMs).atZone(zone)
        val thisMonth = YearMonth.from(now)
        return buildList {
            var m = thisMonth.minusMonths(1)
            var added = 0
            while (added < 2 && m.isAfter(thisMonth.minusMonths(7))) {
                val inMonth = sessions.filter { YearMonth.from(Instant.ofEpochMilli(it.startedAt).atZone(zone)) == m }
                if (inMonth.isNotEmpty()) {
                    add(
                        RecapRowData(
                            title = "${m.month.getDisplayName(TextStyle.FULL, Locale.getDefault())} in review",
                            subtitle = recapLine(inMonth, useKg),
                            isYear = false
                        )
                    )
                    added++
                }
                m = m.minusMonths(1)
            }
            val inYear = sessions.filter { Instant.ofEpochMilli(it.startedAt).atZone(zone).year == now.year }
            if (inYear.isNotEmpty()) add(RecapRowData("${now.year}, so far", recapLine(inYear, useKg), isYear = true))
        }
    }

    private fun recapLine(s: List<Session>, useKg: Boolean): String {
        val vol = s.sumOf { it.totalVolumeLb ?: 0.0 }
        val prs = s.sumOf { it.prCount }
        return "${s.size} sessions · ${formatVolumeCompact(vol, useKg)} · $prs PRs"
    }

    /**
     * The profile's curated trophy-case highlight (NOT the full catalog — that's one tap away):
     * the [HARDEST_DONE] hardest unlocked trophies (by tier points), then locked trophies ranked by
     * progress (almost-complete first, 0%-progress fillers after) up to [TROPHY_HIGHLIGHTS] cells.
     */
    private fun curatedTrophyGrid(unlockedIds: Set<String>, snapshot: TrophyStatsSnapshot?, useKg: Boolean): List<TrophyCell> {
        val hardestDone = Trophies.all
            .filter { it.id in unlockedIds }
            .sortedWith(compareByDescending<Trophy> { it.tier.points }.thenBy { it.name })
            .take(HARDEST_DONE)
            .map { TrophyCell(it.icon, unlocked = true, progress = 1f, name = it.name, description = it.description, progressLabel = null, variant = Trophies.variantFor(it.id)) }

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
        val HOUR_FMT: DateTimeFormatter = DateTimeFormatter.ofPattern("h a", Locale.US)
    }
}
