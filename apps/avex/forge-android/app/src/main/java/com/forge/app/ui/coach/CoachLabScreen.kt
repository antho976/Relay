package com.forge.app.ui.coach

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import com.forge.app.ui.common.clickableLabeled
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.repo.CoachWatch
import com.forge.app.ui.theme.emphasized

/**
 * "Coach lab" (finding #5): the coach's inputs, in plain words — how far its learning has come,
 * which lifts it's watching, the recovery signals it reads, and what it's learned. Read-only.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CoachLabScreen(
    onBack: () -> Unit,
    onOpenTimeline: () -> Unit,
    viewModel: CoachLabViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("What I'm watching.", style = MaterialTheme.typography.headlineMedium) },
                navigationIcon = {
                    IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        when {
            state.loading -> Box(Modifier.fillMaxSize().padding(inner), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
            state.watch == null -> Box(
                Modifier.fillMaxSize().padding(inner).padding(horizontal = 32.dp),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    "Couldn't read the coach's notes right now. Log a workout and check back.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center
                )
            }
            else -> WatchContent(state.watch!!, onOpenTimeline, Modifier.padding(inner))
        }
    }
}

@Composable
private fun WatchContent(w: CoachWatch, onOpenTimeline: () -> Unit, modifier: Modifier = Modifier) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val outline = MaterialTheme.colorScheme.outline

    Column(
        modifier = modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 24.dp)
    ) {
        // ── Status: still learning, or active ─────────────────────────────────
        val learning = w.sessionsToGo > 0
        Text(if (learning) "STILL LEARNING" else "ACTIVE",
            style = MaterialTheme.typography.labelMedium, color = emphasized(muted))
        Spacer(Modifier.height(6.dp))
        Text(
            if (learning)
                "${w.sessionsToGo} more session${if (w.sessionsToGo == 1) "" else "s"} and the coach starts calling weekly " +
                    "adjustments. It's watching now — these are early signals, not concrete yet."
            else
                "The coach is calling weekly adjustments" +
                    (if (w.autopilot) ", and earned changes apply on autopilot." else ". Nothing changes unless you apply it."),
            style = MaterialTheme.typography.bodyMedium, color = onBg, fontStyle = FontStyle.Italic
        )
        Spacer(Modifier.height(8.dp))
        Text("${w.sessionsLogged} session${if (w.sessionsLogged == 1) "" else "s"} logged so far.",
            style = MaterialTheme.typography.bodySmall, color = muted)

        // ── Recovery load (fatigue pulse) ─────────────────────────────────────
        w.fatigueScore?.let { score ->
            Spacer(Modifier.height(8.dp))
            Text("Recovery load: $score of ${w.fatigueThreshold} — a deload is suggested at ${w.fatigueThreshold}.",
                style = MaterialTheme.typography.bodySmall, color = muted)
        }

        // ── Tracked lifts ─────────────────────────────────────────────────────
        Section("LIFTS I'M TRACKING", outline, muted)
        if (w.trackedLifts.isEmpty()) {
            Hint("No lift history yet — log a few weighted sets and they'll show up here.", muted)
        } else {
            w.trackedLifts.forEach { lift ->
                WatchRow(
                    label = lift.name,
                    detail = "${lift.bouts} session${if (lift.bouts == 1) "" else "s"}",
                    tag = when {
                        lift.stalling -> "stalling"
                        !lift.concrete -> "still forming"
                        else -> null
                    },
                    onBg = onBg, muted = muted
                )
            }
        }

        // ── Recovery signals ──────────────────────────────────────────────────
        Section("RECOVERY SIGNALS", outline, muted)
        w.recoverySignals.forEach { sig ->
            WatchRow(
                label = sig.label,
                detail = sig.detail,
                tag = if (sig.active) null else "—",
                onBg = if (sig.active) onBg else muted,
                muted = muted
            )
        }

        // ── Learned so far ────────────────────────────────────────────────────
        Section("LEARNED SO FAR", outline, muted)
        if (w.learnedBiases.isEmpty()) {
            Hint("Nothing yet — the changes you apply (and keep) are what teach it.", muted)
        } else {
            w.learnedBiases.forEach { b ->
                WatchRow(label = b.label, detail = b.detail, tag = null, onBg = onBg, muted = muted)
            }
        }

        Section("THE LONG VIEW", outline, muted)
        Text(
            "See the learning timeline →",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.primary,
            modifier = Modifier.fillMaxWidth().clickableLabeled("Open the learning timeline") { onOpenTimeline() }.padding(vertical = 8.dp)
        )
        Text(
            "Trust earned per change type, the milestones it's hit, and the week-by-week record.",
            style = MaterialTheme.typography.bodySmall, color = muted
        )

        Spacer(Modifier.height(24.dp))
        Text(
            "These are the coach's inputs, not verdicts. It stays quiet until the data is clear, and " +
                "nothing changes your program without your say-so.",
            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
        )
        Spacer(Modifier.height(40.dp))
    }
}

@Composable
private fun Section(title: String, outline: Color, muted: Color) {
    Spacer(Modifier.height(20.dp))
    HorizontalDivider(color = outline.copy(alpha = 0.3f))
    Spacer(Modifier.height(16.dp))
    Text(title, style = MaterialTheme.typography.labelMedium, color = emphasized(muted))
    Spacer(Modifier.height(8.dp))
}

@Composable
private fun WatchRow(label: String, detail: String, tag: String?, onBg: Color, muted: Color) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 6.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(Modifier.weight(1f)) {
            Text(label, style = MaterialTheme.typography.bodyMedium, color = onBg)
            Text(detail, style = MaterialTheme.typography.bodySmall, color = muted)
        }
        if (tag != null) {
            Text(tag, style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.7f), fontStyle = FontStyle.Italic)
        }
    }
}

@Composable
private fun Hint(text: String, muted: Color) {
    Text(text, style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.8f),
        fontStyle = FontStyle.Italic, modifier = Modifier.padding(vertical = 4.dp))
}
