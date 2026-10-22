package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import com.forge.app.domain.warmup.WarmupDrill
import com.forge.app.domain.warmup.WarmupProtocol
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenPrimaryButton
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType

/**
 * The pre-session warmup: one screen, one button.
 *
 * Not a stepper and not a gate. Rows tick off so the user can keep their place across a set of
 * jumping jacks, but the ticks are their own scratchpad, not a checklist the app grades: nothing is
 * required, nothing is stored, and the button works from the first frame. There is exactly one
 * button because "start" and "skip" were the same action wearing two labels.
 *
 * Open panel pass (2026-10-21): no cards. A mono head with the count ticked, each drill as a ring,
 * its name and its dose under it (so a long name never squeezes the dose off the row, at any font
 * size), the red Start lifting, and the two opt-outs as quiet mono links.
 */
@Composable
fun WarmupFlow(
    protocol: WarmupProtocol,
    checked: Set<String>,
    onToggle: (String) -> Unit,
    onStart: () -> Unit,
    onDisableToday: () -> Unit,
    onDisableWeek: () -> Unit,
    modifier: Modifier = Modifier
) {
    val prep = protocol.steps.filterIsInstance<WarmupDrill>()
    val done = prep.count { it.id in checked }

    Column(modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        // The gate draws with no top bar above it, so it owns its own clearance from the status bar.
        Spacer(Modifier.height(24.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
            OpenLabel("Warm-up")
            if (prep.isNotEmpty()) OpenLabel("$done of ${prep.size} done", color = if (done > 0) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant)
        }

        if (prep.isNotEmpty()) {
            Spacer(Modifier.height(20.dp))
            Column(verticalArrangement = Arrangement.spacedBy(20.dp)) {
                prep.forEach { drill ->
                    WarmupRow(drill.name, drill.prescription, drill.id in checked) { onToggle(drill.id) }
                }
            }
        }

        Spacer(Modifier.height(32.dp))
        // It starts the session at any tick count; nothing here is gated.
        OpenPrimaryButton("Start lifting", onClick = onStart)

        Spacer(Modifier.height(8.dp))
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(24.dp, Alignment.CenterHorizontally),
            verticalAlignment = Alignment.CenterVertically
        ) {
            OptOut("Not today", onDisableToday)
            OptOut("Not this week", onDisableWeek)
        }
        Spacer(Modifier.height(8.dp))
    }
}

/**
 * One tickable warmup row: a ring (red with a check once ticked), the drill, and its dose under it
 * in mono. The whole row is the tap target, so there is never a nested tap (§14).
 */
@Composable
private fun WarmupRow(label: String, meta: String, checked: Boolean, onToggle: () -> Unit) {
    val accent = MaterialTheme.colorScheme.primary
    Row(
        Modifier
            .fillMaxWidth()
            .clickableLabeled(
                label = if (checked) "Untick $label" else "Tick $label",
                role = Role.Checkbox,
                onClick = onToggle
            ),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Box(
            Modifier
                .size(24.dp)
                .clip(CircleShape)
                .then(if (checked) Modifier.background(accent) else Modifier.border(1.5.dp, OpenColors.Rule, CircleShape)),
            contentAlignment = Alignment.Center
        ) {
            if (checked) Icon(OpenIcons.Check, contentDescription = null, tint = MaterialTheme.colorScheme.onPrimary, modifier = Modifier.size(14.dp))
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Text(
                label,
                style = OpenType.rowTitle,
                color = if (checked) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onBackground
            )
            if (meta.isNotBlank()) OpenLabel(meta, style = OpenType.monoXS)
        }
    }
}

/** A persistent opt-out: a quiet mono link with a full-height touch target. */
@Composable
private fun OptOut(label: String, onClick: () -> Unit) {
    Box(
        Modifier
            .heightIn(min = 48.dp)
            .clickableLabeled(label, onClick = onClick)
            .padding(horizontal = 8.dp),
        contentAlignment = Alignment.Center
    ) {
        OpenLabel(label)
    }
}
