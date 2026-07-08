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
import com.forge.app.domain.cardio.CardioEffort
import com.forge.app.domain.cardio.CardioRestReason
import com.forge.app.domain.cardio.CardioSessionCompare
import com.forge.app.domain.cardio.CardioType
import com.forge.app.domain.cardio.CardioWearableDay
import com.forge.app.domain.cardio.RoutePoint
import com.forge.app.domain.cardio.cardioDetailParts
import com.forge.app.domain.cardio.compareCardioSession
import com.forge.app.domain.cardio.formatPaceSec
import com.forge.app.domain.cardio.paceSecPerKm
import com.forge.app.domain.cardio.paceSecPerUnit
import com.forge.app.domain.cardio.pacePerUnit
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.formatDistance
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
    onEdit: () -> Unit,
    onDelete: () -> Unit,
    onBack: () -> Unit,
    /** Tapping the Avex wordmark — defaults to "go Home"; the cardio tab overrides it to close first. */
    onHome: () -> Unit = com.forge.app.ui.common.LocalGoHome.current
) {
    val type = CardioType.fromCode(entry.type)
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
                title = { com.forge.app.ui.common.ForgeWordmark(onClick = onHome) },
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
                    Text(type.displayName, style = MaterialTheme.typography.displaySmall, color = onBg)
                    Spacer(Modifier.height(16.dp))
                }
            }

            item("stats") {
                Column(Modifier.padding(horizontal = 24.dp, vertical = 8.dp)) {
                    if (type.isRest) {
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
                        CardioEffort.fromCode(entry.effort)?.let { StatRow("Effort", it.displayName, onBg, muted, outline) }
                        entry.hrZone?.let { StatRow("HR zone", "Z$it", onBg, muted, outline) }
                        entry.intervalCount?.takeIf { it > 0 }?.let { StatRow("Intervals", "$it", onBg, muted, outline) }
                    }
                }
            }

            // The same activity's previous outing — the head-to-head the "am I improving" glance needs.
            compare?.previous?.let { prev ->
                item("previous") {
                    Column(Modifier.padding(horizontal = 24.dp, vertical = 8.dp)) {
                        EditorialHeader(label = "Previous ${type.displayName}", muted = muted, accent = accent)
                        Spacer(Modifier.height(8.dp))
                        val prevDate = remember(prev.date) {
                            SimpleDateFormat("MMM d", Locale.getDefault()).format(Date(prev.date))
                        }
                        val parts = cardioDetailParts(prev, useMiles = useMiles).joinToString(" · ")
                        Text(
                            if (parts.isBlank()) prevDate else "$prevDate · $parts",
                            style = MaterialTheme.typography.bodyMedium, color = onBg
                        )
                    }
                }
            }

            // Wearable steps — shown when a watch fed data, or as a quiet placeholder once connected
            // (so a connected user sees the section is live before that day's steps sync). Hidden
            // entirely on a rest day, and when nothing's connected (the banner carries the invite).
            if (!type.isRest && (wearable?.hasData == true || wearableConnected)) {
                item("steps") {
                    StepsByHourSection(wearable = wearable, connected = wearableConnected, onBg = onBg, muted = muted, outline = outline, accent = accent)
                }
            }
            if (!type.isRest && route != null && route.size >= 2) {
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
            } else if (!type.isRest && onShowRoute != null) {
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
    val bestKm = compare?.bestOtherPaceSecPerKm ?: return null to false
    val myKm = paceSecPerKm(entry.durationMin, entry.distanceKm) ?: return null to false
    val best = paceSecPerUnit(bestKm, useMiles)
    val mine = paceSecPerUnit(myKm, useMiles)
    return when {
        compare.isPaceBest -> "your fastest · prev ${formatPaceSec(best)}" to true
        else -> "best ${formatPaceSec(best)} · +${formatPaceSec((mine - best).coerceAtLeast(0))}" to false
    }
}
