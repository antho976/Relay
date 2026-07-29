package com.forge.app.ui.cardio.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.cardio.CardioCondition
import com.forge.app.domain.cardio.CardioEffort
import com.forge.app.domain.cardio.CardioRestReason
import com.forge.app.domain.cardio.CardioActivity
import com.forge.app.domain.cardio.CardioSessionCompare
import com.forge.app.domain.cardio.CardioWearableDay
import com.forge.app.domain.cardio.RoutePoint
import com.forge.app.domain.cardio.cardioDetailParts
import com.forge.app.domain.cardio.compareCardioSession
import com.forge.app.domain.cardio.formatInclinePct
import com.forge.app.domain.cardio.formatPaceSec
import com.forge.app.domain.cardio.paceSecPerUnit
import com.forge.app.domain.cardio.pacePerUnit
import com.forge.app.domain.health.avgBpm
import com.forge.app.domain.health.maxBpm
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.formatDistance
import com.forge.app.domain.units.formatElevation
import com.forge.app.ui.common.EditorialHairline
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.clickableLabeled
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * The complete stats for ONE cardio session — opened by tapping a row in "What I did". Every field
 * the entry carries (duration, distance, pace, effort, HR zone, intervals, route, note) is laid out
 * as a labelled table, each reading carrying its compare meta — how this session stands against the
 * same activity's best pace / longest distance — plus the previous same-type session underneath.
 * This is the home for the full detail the list rows deliberately omit.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CardioSessionDetailSheet(
    entry: CardioEntry,
    /** Every logged entry — the same-type compare pool (best pace, longest, previous session). */
    allEntries: List<CardioEntry> = emptyList(),
    /** Distance/pace unit — true shows miles, false km. */
    useMiles: Boolean = false,
    /** GPS track (watch-only); non-null once the matching session's route is available to draw. */
    route: List<RoutePoint>? = null,
    /** Non-null when a matching watch session has a route that needs Health Connect consent first —
     *  shows a "Show GPS route" button that launches the consent flow. Ignored once [route] is set. */
    onShowRoute: (() -> Unit)? = null,
    /** Watch-derived steps for the session's day; null until loaded / when none. */
    wearable: CardioWearableDay? = null,
    /** Avex holds the steps grant — show the steps section (with a placeholder) even before data syncs. */
    wearableConnected: Boolean = false,
    /** Downsampled HR series of the matched watch workout (W5); non-null with ≥2 points draws the graph. */
    hr: List<com.forge.app.domain.health.HrPoint>? = null,
    /** The matched watch workout's measured stats (W5); drives the "watch measured" reading. */
    watchStats: com.forge.app.domain.health.WatchWorkout? = null,
    /** Adopt the watch's measured duration/distance onto this entry — offered only when they differ. */
    onAdoptWatchStats: (() -> Unit)? = null,
    onEdit: () -> Unit,
    onDelete: () -> Unit,
    onBack: () -> Unit
) {
    val activity = CardioActivity.resolve(entry.type, com.forge.app.ui.cardio.LocalCardioTypes.current)
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val accent = MaterialTheme.colorScheme.primary

    val dateLine = remember(entry.date) {
        SimpleDateFormat("EEE, MMM d, yyyy · h:mm a", Locale.getDefault()).format(Date(entry.date))
    }
    // How this session stands against its own activity type — null for rest days / a first session.
    val compare = remember(entry, allEntries) { compareCardioSession(entry, allEntries) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {},
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back", tint = muted)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(inner),
            contentPadding = PaddingValues(bottom = 48.dp)
        ) {
            item("hero") {
                Column(Modifier.padding(horizontal = 24.dp).padding(top = 8.dp)) {
                    Text(dateLine.uppercase(), style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
                    Spacer(Modifier.height(8.dp))
                    Text(activity.displayName, style = MaterialTheme.typography.displaySmall, color = onBg)
                    Spacer(Modifier.height(16.dp))
                }
            }

            item("stats") {
                Column(Modifier.padding(horizontal = 24.dp, vertical = 8.dp)) {
                    if (activity.isRest) {
                        StatRow("Rest", CardioRestReason.fromCode(entry.restReason)?.displayName ?: "Rest day", onBg, muted, outline)
                    } else {
                        val unit = distanceUnitLabel(useMiles)
                        StatRow("Duration", if (entry.durationMin > 0) "${entry.durationMin} min" else "0 min", onBg, muted, outline)
                        entry.distanceKm?.let { dist ->
                            val (meta, metaIsBest) = distanceCompareMeta(compare, useMiles)
                            StatRow("Distance", formatDistance(dist, useMiles), onBg, muted, outline, meta = meta, metaColor = if (metaIsBest) accent else muted)
                        }
                        pacePerUnit(entry.durationMin, entry.distanceKm, useMiles)?.let { pace ->
                            val (meta, metaIsBest) = paceCompareMeta(entry, compare, useMiles)
                            StatRow("Pace", "$pace /$unit", onBg, muted, outline, meta = meta, metaColor = if (metaIsBest) accent else muted)
                        }
                        // Per-type fields (GYMAP-38) — at most one applies to any given activity.
                        entry.laps?.takeIf { it > 0 }?.let { StatRow("Laps", "$it", onBg, muted, outline) }
                        entry.inclinePct?.takeIf { it > 0 }?.let { StatRow("Incline", formatInclinePct(it), onBg, muted, outline) }
                        entry.elevationM?.takeIf { it > 0 }?.let { StatRow("Elevation gain", formatElevation(it, useMiles), onBg, muted, outline) }
                        CardioEffort.fromCode(entry.effort)?.let { StatRow("Effort", it.displayName, onBg, muted, outline) }
                        entry.hrZone?.let { StatRow("HR zone", "Z$it", onBg, muted, outline) }
                        entry.intervalCount?.takeIf { it > 0 }?.let { StatRow("Intervals", "$it", onBg, muted, outline) }
                        // Weather tags (GYMAP-39), read-only here — the interactive chips live in the log sheet.
                        CardioCondition.decode(entry.conditions).takeIf { it.isNotEmpty() }?.let { tags ->
                            StatRow("Conditions", tags.joinToString(" · ") { it.displayName }, onBg, muted, outline)
                        }
                    }
                }
            }

            // The same activity's previous outing — the head-to-head the "am I improving" glance needs.
            compare?.previous?.let { prev ->
                item("previous") {
                    Column(Modifier.padding(horizontal = 24.dp, vertical = 8.dp)) {
                        EditorialHeader(label = "Previous ${activity.displayName}", muted = muted, accent = accent)
                        Spacer(Modifier.height(8.dp))
                        val prevDate = remember(prev.date) {
                            SimpleDateFormat("MMM d", Locale.getDefault()).format(Date(prev.date))
                        }
                        val parts = remember(prev, useMiles) {
                            cardioDetailParts(prev, useMiles = useMiles).joinToString(" · ")
                        }
                        Text(
                            if (parts.isBlank()) prevDate else "$prevDate · $parts",
                            style = MaterialTheme.typography.bodyMedium, color = onBg
                        )
                    }
                }
            }

            // The watch's HR series over this workout (W5) — the graph IS the section (§12); avg/max
            // ride the header line as its reading. Hidden entirely when no watch session matched.
            if (!activity.isRest && hr != null && hr.size >= 2) {
                item("heart-rate") {
                    HeartRateSection(
                        hr = hr,
                        watchStats = watchStats,
                        entry = entry,
                        useMiles = useMiles,
                        onAdoptWatchStats = onAdoptWatchStats,
                        onBg = onBg, muted = muted, accent = accent
                    )
                }
            }

            // Wearable steps — shown when a watch fed data, or as a quiet placeholder once connected
            // (so a connected user sees the section is live before that day's steps sync). Hidden
            // entirely on a rest day, and when nothing's connected (the banner carries the invite).
            if (!activity.isRest && (wearable?.hasData == true || wearableConnected)) {
                item("steps") {
                    StepsByHourSection(wearable = wearable, connected = wearableConnected, onBg = onBg, muted = muted, outline = outline, accent = accent)
                }
            }
            if (!activity.isRest && route != null && route.size >= 2) {
                item("route") {
                    Column(Modifier.padding(horizontal = 24.dp, vertical = 8.dp)) {
                        EditorialHeader(label = "Route", muted = muted, accent = accent)
                        Spacer(Modifier.height(10.dp))
                        Box(
                            modifier = Modifier.fillMaxWidth().height(160.dp)
                        ) {
                            RouteThumbnail(route = route, color = onBg, modifier = Modifier.fillMaxSize().padding(8.dp))
                        }
                    }
                }
            } else if (!activity.isRest && onShowRoute != null) {
                // A matching watch session has a route, but Health Connect needs per-route consent first.
                item("route-cta") {
                    Column(Modifier.padding(horizontal = 24.dp, vertical = 8.dp)) {
                        EditorialHeader(label = "Route", muted = muted, accent = accent)
                        Spacer(Modifier.height(10.dp))
                        ForgeOutlineCapsule(label = "Show GPS route", onClick = onShowRoute)
                    }
                }
            }

            entry.note?.takeIf { it.isNotBlank() }?.let { note ->
                item("note") {
                    Column(Modifier.padding(horizontal = 24.dp, vertical = 8.dp)) {
                        EditorialHeader(label = "Note", muted = muted, accent = accent)
                        Spacer(Modifier.height(8.dp))
                        Text(note, style = MaterialTheme.typography.bodyMedium, color = onBg, fontStyle = FontStyle.Italic)
                    }
                }
            }

            item("actions") {
                Spacer(Modifier.height(16.dp))
                Row(
                    modifier = Modifier.padding(horizontal = 24.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(20.dp)
                ) {
                    ForgeOutlineCapsule(label = "Edit", onClick = onEdit)
                    Text(
                        "Delete",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier
                            .clickableLabeled("Delete session", onClick = onDelete)
                            // Padding, not text size, carries the ≥48dp touch target (§8).
                            .padding(horizontal = 12.dp, vertical = 16.dp)
                    )
                }
            }
        }
    }
}

/**
 * The matched watch workout's heart rate over this session (W5): an open line chart (§10 — stroke
 * `primary`, no frame) with AVG · MAX as the header's reading, and — when the watch measured a
 * different duration/distance than the entry carries — one "watch measured" line with an explicit
 * adopt action (never a silent overwrite).
 */
@Composable
private fun HeartRateSection(
    hr: List<com.forge.app.domain.health.HrPoint>,
    watchStats: com.forge.app.domain.health.WatchWorkout?,
    entry: CardioEntry,
    useMiles: Boolean,
    onAdoptWatchStats: (() -> Unit)?,
    onBg: Color,
    muted: Color,
    accent: Color
) {
    val avg = hr.avgBpm()
    val max = hr.maxBpm()
    Column(Modifier.padding(horizontal = 24.dp, vertical = 8.dp)) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            EditorialHeader(label = "Heart rate", muted = muted, accent = accent)
            if (avg != null && max != null) {
                Text(
                    "AVG $avg · MAX $max BPM",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted, fontSize = 9.sp, letterSpacing = 0.5.sp
                )
            }
        }
        Spacer(Modifier.height(10.dp))
        com.forge.app.ui.gym.stats.components.LineChart(
            values = hr.map { it.bpm.toDouble() },
            lineColor = accent,
            modifier = Modifier.fillMaxWidth().height(96.dp)
        )
        // The watch's own reading of this workout, offered beside the logged values (§4.9). The
        // adopt link renders only when it would actually change something.
        val watchParts = watchStats?.let { w ->
            buildList {
                if (w.durationMin > 0 && w.durationMin != entry.durationMin) add("${w.durationMin} min")
                w.distanceKm?.takeIf { d -> entry.distanceKm == null || kotlin.math.abs(d - entry.distanceKm!!) > 0.05 }
                    ?.let { add(formatDistance(it, useMiles)) }
                w.kcal?.let { add("${it.toInt()} kcal") }
            }
        }.orEmpty()
        if (watchParts.isNotEmpty()) {
            Spacer(Modifier.height(8.dp))
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    "Watch measured ${watchParts.joinToString(" · ")}",
                    style = MaterialTheme.typography.bodySmall, color = muted
                )
                if (onAdoptWatchStats != null &&
                    watchParts.any { !it.endsWith("kcal") } // kcal alone isn't adoptable onto the entry
                ) {
                    Text(
                        "use watch stats →",
                        style = MaterialTheme.typography.labelMedium,
                        color = accent,
                        modifier = Modifier
                            .clickableLabeled("Use watch stats", onClick = onAdoptWatchStats)
                            .padding(vertical = 2.dp)
                    )
                }
            }
        }
    }
}

@Composable
private fun StatRow(
    label: String,
    value: String,
    onBg: Color,
    muted: Color,
    outline: Color,
    /** The reading's compare line ("best 5:32 /km · +0:09", "your fastest · prev 5:48") — §4.9,
     *  the deciding reading sits beside its value, never in a separate compare section. */
    meta: String? = null,
    metaColor: Color = muted
) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 12.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, color = muted)
        Column(horizontalAlignment = Alignment.End) {
            Text(value, style = MaterialTheme.typography.bodyLarge, color = onBg)
            if (meta != null) {
                Text(
                    meta.uppercase(),
                    style = MaterialTheme.typography.labelSmall,
                    color = metaColor, fontSize = 9.sp, letterSpacing = 0.5.sp
                )
            }
        }
    }
    // Table rule between stat rows — a line as data (§1), via the shared hairline.
    EditorialHairline(outline)
}

/** ("meta line", isPersonalBest) for the distance row, or (null, false) when there's no compare. */
private fun distanceCompareMeta(
    compare: CardioSessionCompare?,
    useMiles: Boolean
): Pair<String?, Boolean> {
    val best = compare?.bestOtherDistanceKm ?: return null to false
    return when {
        compare.isDistanceBest -> "your longest · prev ${formatDistance(best, useMiles)}" to true
        else -> "longest ${formatDistance(best, useMiles)}" to false
    }
}

/** ("meta line", isPersonalBest) for the pace row, or (null, false) when there's no compare. */
private fun paceCompareMeta(
    entry: CardioEntry,
    compare: CardioSessionCompare?,
    useMiles: Boolean
): Pair<String?, Boolean> {
    val bestEntry = compare?.bestPaceEntry ?: return null to false
    val best = paceSecPerUnit(bestEntry.durationMin, bestEntry.distanceKm, useMiles) ?: return null to false
    val mine = paceSecPerUnit(entry.durationMin, entry.distanceKm, useMiles) ?: return null to false
    return when {
        compare.isPaceBest -> "your fastest · prev ${formatPaceSec(best)}" to true
        else -> "best ${formatPaceSec(best)} · +${formatPaceSec((mine - best).coerceAtLeast(0))}" to false
    }
}
