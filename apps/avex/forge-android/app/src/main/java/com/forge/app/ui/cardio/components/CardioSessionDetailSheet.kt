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
import androidx.compose.foundation.layout.width
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
import com.forge.app.domain.cardio.CardioActivity
import com.forge.app.domain.cardio.CardioCondition
import com.forge.app.domain.cardio.CardioEffort
import com.forge.app.domain.cardio.CardioRestReason
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
import com.forge.app.ui.common.EditorialFigure
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.clickableLabeled
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/** Air between sections, matching the overview's rhythm (§7). */
private val SECTION_GAP = 28.dp

/**
 * ONE cardio session (§3 Detail archetype).
 *
 * Rebuilt 2026-08-23. It used to be ten `label — value` rows separated by hairlines, which is §4.10's
 * checklist look and drew no mark at all: without a watch connected the whole page was text. Now the
 * three readings a session is judged on are figures, how it STANDS against the same activity is a
 * ranked mark (built from data every session already has, watch or not), and the handful of tags that
 * used to each own a row are one mono line — §2② is explicit that a lone categorical state is a
 * caption, not a section.
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
    val unit = distanceUnitLabel(useMiles)
    val pace = pacePerUnit(entry.durationMin, entry.distanceKm, useMiles)

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
                    Text(dateLine.uppercase(), style = MaterialTheme.typography.labelSmall, color = muted, letterSpacing = 1.sp)
                    Spacer(Modifier.height(8.dp))
                    // §11 — a serif title takes no terminal period.
                    Text(activity.displayName, style = MaterialTheme.typography.displaySmall, color = onBg)
                }
            }

            if (activity.isRest) {
                item("rest") {
                    Spacer(Modifier.height(SECTION_GAP))
                    Text(
                        CardioRestReason.fromCode(entry.restReason)?.displayName ?: "Rest day",
                        style = MaterialTheme.typography.bodyLarge,
                        color = onBg,
                        modifier = Modifier.padding(horizontal = 24.dp)
                    )
                }
            } else {
                // The three readings a session is judged on. Honest zeros, never hidden (§12); they
                // wrap rather than clip at large font scales (§14).
                item("figures") {
                    Spacer(Modifier.height(24.dp))
                    Row(
                        Modifier.fillMaxWidth().padding(horizontal = 24.dp),
                        horizontalArrangement = Arrangement.spacedBy(20.dp)
                    ) {
                        EditorialFigure(
                            value = "${entry.durationMin}",
                            label = "minutes",
                            onBg = onBg, muted = muted, accent = accent,
                            modifier = Modifier.weight(1f)
                        )
                        // A session with no distance RECORDED is not a session of zero distance —
                        // printing "0 km" would be ghost data, which §12 bans as firmly as it
                        // requires honest zeros for things that really are zero.
                        val distance = entry.distanceKm
                        if (distance != null) {
                            EditorialFigure(
                                value = formatDistance(distance, useMiles).removeSuffix(" $unit"),
                                label = unit,
                                onBg = onBg, muted = muted, accent = accent,
                                modifier = Modifier.weight(1f)
                            )
                        } else {
                            Spacer(Modifier.weight(1f))
                        }
                        // No distance means there is no pace to show — a placeholder dash would be
                        // both an em dash (§11) and a figure standing in for data it does not have (§12).
                        if (pace != null) {
                            EditorialFigure(
                                value = pace,
                                label = "/$unit",
                                onBg = onBg, muted = muted, accent = accent,
                                modifier = Modifier.weight(1f)
                            )
                        } else {
                            Spacer(Modifier.weight(1f))
                        }
                    }
                }

                // The tags that each used to own a table row. §2②: a lone categorical state is not a
                // section, it is a caption — and four of them are still a caption, not four sections.
                val tags = sessionTags(entry, useMiles)
                if (tags.isNotEmpty()) {
                    item("tags") {
                        Spacer(Modifier.height(16.dp))
                        Text(
                            tags.joinToString(" · ").uppercase(),
                            style = MaterialTheme.typography.labelSmall,
                            color = muted, letterSpacing = 1.sp,
                            modifier = Modifier.padding(horizontal = 24.dp)
                        )
                    }
                }

                // STANDING — the compare data drawn instead of written. This is the section that makes
                // the page work with no watch connected: every session has it from the second outing on.
                if (compare != null) {
                    item("standing") {
                        Spacer(Modifier.height(SECTION_GAP))
                        StandingSection(
                            entry = entry,
                            activityName = activity.displayName,
                            compare = compare,
                            useMiles = useMiles,
                            onBg = onBg, muted = muted, outline = outline, accent = accent
                        )
                    }
                }
            }

            // The watch's HR series over this workout (W5) — the graph IS the section (§12); avg/max
            // ride the header line as its reading. Hidden entirely when no watch session matched.
            if (!activity.isRest && hr != null && hr.size >= 2) {
                item("heart-rate") {
                    Spacer(Modifier.height(SECTION_GAP))
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

            if (!activity.isRest && route != null && route.size >= 2) {
                item("route") {
                    Spacer(Modifier.height(SECTION_GAP))
                    Column(Modifier.padding(horizontal = 24.dp)) {
                        EditorialHeader(label = "Route", muted = muted, accent = accent)
                        Spacer(Modifier.height(12.dp))
                        Box(modifier = Modifier.fillMaxWidth().height(160.dp)) {
                            RouteThumbnail(route = route, color = onBg, modifier = Modifier.fillMaxSize().padding(8.dp))
                        }
                    }
                }
            } else if (!activity.isRest && onShowRoute != null) {
                // A matching watch session has a route, but Health Connect needs per-route consent first.
                item("route-cta") {
                    Spacer(Modifier.height(SECTION_GAP))
                    Column(Modifier.padding(horizontal = 24.dp)) {
                        EditorialHeader(label = "Route", muted = muted, accent = accent)
                        Spacer(Modifier.height(12.dp))
                        ForgeOutlineCapsule(label = "Show GPS route", onClick = onShowRoute)
                    }
                }
            }

            // Wearable steps — shown when a watch fed data, or as a quiet placeholder once connected.
            // Hidden entirely on a rest day, and when nothing's connected (the feed carries the invite).
            if (!activity.isRest && (wearable?.hasData == true || wearableConnected)) {
                item("steps") {
                    Spacer(Modifier.height(SECTION_GAP))
                    StepsByHourSection(wearable = wearable, connected = wearableConnected, onBg = onBg, muted = muted, outline = outline, accent = accent)
                }
            }

            entry.note?.takeIf { it.isNotBlank() }?.let { note ->
                item("note") {
                    Spacer(Modifier.height(SECTION_GAP))
                    Column(Modifier.padding(horizontal = 24.dp)) {
                        EditorialHeader(label = "Note", muted = muted, accent = accent)
                        Spacer(Modifier.height(10.dp))
                        Text(note, style = MaterialTheme.typography.bodyMedium, color = onBg, fontStyle = FontStyle.Italic)
                    }
                }
            }

            // §8 — one-shot actions grouped at the END of the page, never mid-scroll.
            item("actions") {
                Spacer(Modifier.height(SECTION_GAP))
                Row(
                    modifier = Modifier.padding(horizontal = 24.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp)
                ) {
                    ForgeOutlineCapsule(label = "Edit", onClick = onEdit)
                    // A destructive one-shot is level ② tinted error, paired with the Undo snackbar —
                    // never a filled red button, and never accent- or error-coloured body text (§14).
                    ForgeOutlineCapsule(
                        label = "Delete",
                        onClick = onDelete,
                        contentColor = MaterialTheme.colorScheme.error
                    )
                }
            }
        }
    }
}

/**
 * STANDING — this session against every other outing of the same activity, as the ranked bars the
 * rest of cardio uses (§4.10: adjacent comparisons share one visual language).
 *
 * Distance is a share of your longest; pace inverts (lower is faster), so the bar fills toward your
 * best rather than away from it. A record row says so in its reading — and the word rides the mono
 * meta, not an accent-coloured sentence (§14).
 */
@Composable
private fun StandingSection(
    entry: CardioEntry,
    activityName: String,
    compare: CardioSessionCompare,
    useMiles: Boolean,
    onBg: Color,
    muted: Color,
    outline: Color,
    accent: Color
) {
    val unit = distanceUnitLabel(useMiles)
    val myDistance = entry.distanceKm?.takeIf { it > 0.0 }
    val bestDistance = compare.bestOtherDistanceKm
    val myPaceSec = paceSecPerUnit(entry.durationMin, entry.distanceKm, useMiles)
    val bestPaceSec = compare.bestPaceEntry?.let { paceSecPerUnit(it.durationMin, it.distanceKm, useMiles) }

    // Nothing comparable on either axis (a duration-only activity, first distance session) — the
    // previous-outing line below still stands on its own, so no empty shell here (§12).
    val comparable = (myDistance != null && bestDistance != null) || (myPaceSec != null && bestPaceSec != null)
    if (!comparable && compare.previous == null) return

    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        EditorialHeader(label = "Against your $activityName", muted = muted, accent = accent)
        Spacer(Modifier.height(12.dp))
        Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
            if (myDistance != null && bestDistance != null) {
                val leader = maxOf(myDistance, bestDistance)
                RankedBarRow(
                    label = "Distance",
                    value = if (compare.isDistanceBest) {
                        "${formatDistance(myDistance, useMiles)} · your longest"
                    } else {
                        "${formatDistance(myDistance, useMiles)} of ${formatDistance(bestDistance, useMiles)}"
                    },
                    fraction = (myDistance / leader).toFloat(),
                    onBg = onBg, muted = muted, outline = outline, accent = accent
                )
            }
            if (myPaceSec != null && bestPaceSec != null) {
                val mine = formatPaceSec(myPaceSec)
                val best = formatPaceSec(bestPaceSec)
                // Lower is faster, so the fill is best/mine — your record reads as a full bar.
                RankedBarRow(
                    label = "Pace",
                    value = if (compare.isPaceBest) "$mine /$unit · your fastest" else "$mine · best $best /$unit",
                    fraction = (bestPaceSec.toFloat() / myPaceSec).coerceIn(0f, 1f),
                    onBg = onBg, muted = muted, outline = outline, accent = accent
                )
            }
        }
        // The same activity's previous outing — the head-to-head the "am I improving" glance needs.
        // One line, under the mark it qualifies, rather than a section of its own (§4.3).
        compare.previous?.let { prev ->
            Spacer(Modifier.height(14.dp))
            val prevDate = remember(prev.date) {
                SimpleDateFormat("MMM d", Locale.getDefault()).format(Date(prev.date))
            }
            val parts = remember(prev, useMiles) { cardioDetailParts(prev, useMiles = useMiles).joinToString(" · ") }
            Text(
                (if (parts.isBlank()) "Previous · $prevDate" else "Previous · $prevDate · $parts").uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = muted, letterSpacing = 0.5.sp
            )
        }
    }
}

/**
 * The session's descriptive tags, in the order they were rows: effort · HR zone · intervals · the one
 * per-type field (laps / incline / elevation) · weather. Each is a single word or number that
 * qualifies the session — §2②'s "fold it into a caption" case, not five sections.
 */
private fun sessionTags(entry: CardioEntry, useMiles: Boolean): List<String> = buildList {
    CardioEffort.fromCode(entry.effort)?.let { add(it.displayName) }
    entry.hrZone?.let { add("Z$it") }
    entry.intervalCount?.takeIf { it > 0 }?.let { add("$it intervals") }
    entry.laps?.takeIf { it > 0 }?.let { add("$it laps") }
    entry.inclinePct?.takeIf { it > 0 }?.let { add("${formatInclinePct(it)} incline") }
    entry.elevationM?.takeIf { it > 0 }?.let { add("${formatElevation(it, useMiles)} gain") }
    CardioCondition.decode(entry.conditions).forEach { add(it.displayName) }
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
    Column(Modifier.padding(horizontal = 24.dp)) {
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
                    color = muted, letterSpacing = 0.5.sp
                )
            }
        }
        Spacer(Modifier.height(12.dp))
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
            Spacer(Modifier.height(10.dp))
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    "Watch measured ${watchParts.joinToString(" · ")}",
                    style = MaterialTheme.typography.bodySmall, color = muted,
                    modifier = Modifier.weight(1f)
                )
                Spacer(Modifier.width(12.dp))
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
