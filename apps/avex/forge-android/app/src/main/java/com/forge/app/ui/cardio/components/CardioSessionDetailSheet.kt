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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.cardio.CardioCalorieEstimator
import com.forge.app.domain.cardio.CardioEffort
import com.forge.app.domain.cardio.CardioRestReason
import com.forge.app.domain.cardio.CardioType
import com.forge.app.domain.cardio.CardioWearableDay
import com.forge.app.domain.cardio.RoutePoint
import com.forge.app.domain.cardio.pacePerUnit
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.formatDistance
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * The complete stats for ONE cardio session — opened by tapping a row in "What I did". Every field
 * the entry carries (duration, distance, pace, effort, HR zone, intervals, calories, route, note) is
 * laid out as a clean labelled list, with Edit / Delete actions. This is the home for the full detail
 * the list rows deliberately omit.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CardioSessionDetailSheet(
    entry: CardioEntry,
    bodyweightLb: Double?,
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

    Scaffold(
        topBar = {
            TopAppBar(
                title = { com.forge.app.ui.common.ForgeWordmark(onClick = onHome) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back", tint = muted)
                    }
                },
                actions = {
                    Text("SESSION", style = MaterialTheme.typography.labelSmall, letterSpacing = 2.sp, color = muted, modifier = Modifier.padding(end = 16.dp))
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
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        Icon(type.icon, contentDescription = null, tint = onBg, modifier = Modifier.size(28.dp))
                        Text(type.displayName, style = MaterialTheme.typography.displaySmall, color = onBg)
                    }
                    Spacer(Modifier.height(16.dp))
                    HorizontalDivider(color = outline.copy(alpha = 0.25f))
                }
            }

            item("stats") {
                Column(Modifier.padding(horizontal = 24.dp, vertical = 8.dp)) {
                    if (type.isRest) {
                        StatRow("Rest", CardioRestReason.fromCode(entry.restReason)?.displayName ?: "Rest day", onBg, muted, outline)
                    } else {
                        StatRow("Duration", if (entry.durationMin > 0) "${entry.durationMin} min" else "—", onBg, muted, outline)
                        entry.distanceKm?.let { StatRow("Distance", formatDistance(it, useMiles), onBg, muted, outline) }
                        pacePerUnit(entry.durationMin, entry.distanceKm, useMiles)?.let { StatRow("Pace", "$it /${distanceUnitLabel(useMiles)}", onBg, muted, outline) }
                        CardioEffort.fromCode(entry.effort)?.let { StatRow("Effort", it.displayName, onBg, muted, outline) }
                        entry.hrZone?.let { StatRow("HR zone", "Z$it", onBg, muted, outline) }
                        entry.intervalCount?.takeIf { it > 0 }?.let { StatRow("Intervals", "$it", onBg, muted, outline) }
                        CardioCalorieEstimator.estimate(type, entry.durationMin, CardioEffort.fromCode(entry.effort), bodyweightLb)
                            ?.let { StatRow("Calories", "≈ $it kcal", onBg, muted, outline) }
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
                        Text("ROUTE (GPS)", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
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
                        Text("ROUTE (GPS)", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
                        Spacer(Modifier.height(10.dp))
                        OutlinedButton(
                            onClick = onShowRoute,
                            shape = RoundedCornerShape(50),
                            border = androidx.compose.foundation.BorderStroke(1.5.dp, onBg),
                            colors = ButtonDefaults.outlinedButtonColors(contentColor = onBg),
                            contentPadding = PaddingValues(horizontal = 20.dp, vertical = 10.dp)
                        ) {
                            Text("Show GPS route", style = MaterialTheme.typography.bodyMedium)
                        }
                    }
                }
            }

            entry.note?.takeIf { it.isNotBlank() }?.let { note ->
                item("note") {
                    Column(Modifier.padding(horizontal = 24.dp, vertical = 8.dp)) {
                        Text("NOTE", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp, letterSpacing = 1.sp)
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
                    OutlinedButton(
                        onClick = onEdit,
                        shape = RoundedCornerShape(50),
                        border = androidx.compose.foundation.BorderStroke(1.5.dp, onBg),
                        colors = ButtonDefaults.outlinedButtonColors(contentColor = onBg),
                        contentPadding = PaddingValues(horizontal = 24.dp, vertical = 12.dp)
                    ) {
                        Text("Edit →", style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.SemiBold)
                    }
                    TextButton(onClick = onDelete) {
                        Text("Delete", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
                    }
                }
            }
        }
    }
}

@Composable
private fun StatRow(label: String, value: String, onBg: Color, muted: Color, outline: Color) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 12.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, color = muted)
        Text(value, style = MaterialTheme.typography.bodyLarge, color = onBg)
    }
    HorizontalDivider(color = outline.copy(alpha = 0.12f))
}
