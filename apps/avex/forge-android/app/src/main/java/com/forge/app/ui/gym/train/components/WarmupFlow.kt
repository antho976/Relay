package com.forge.app.ui.gym.train.components

import androidx.compose.animation.animateColorAsState
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.domain.warmup.WarmupDrill
import com.forge.app.domain.warmup.WarmupProtocol
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.Corners
import com.forge.app.ui.common.ForgeChromeButton
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.GROUP_SEAM
import com.forge.app.ui.common.ROW_H
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.memberFill
import com.forge.app.ui.common.memberShape

/**
 * The pre-session warmup: one screen, one button.
 *
 * Not a stepper and not a gate. Rows tick off so the user can keep their place across a set of
 * jumping jacks, but the ticks are their own scratchpad, not a checklist the app grades: nothing is
 * required, nothing is stored, and the button works from the first frame. There is exactly one
 * button because "start" and "skip" were the same action wearing two labels.
 *
 * Grouped-surface pass (2026-09-27): the drills are one group of filled members (a ticked one wears
 * the accent ring and wash), the start is the standard light [ForgePrimaryCapsule], and the two
 * opt-outs are small filled chrome capsules.
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
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary

    val prep = protocol.steps.filterIsInstance<WarmupDrill>()

    Column(modifier.fillMaxWidth().padding(horizontal = 16.dp)) {

        // The gate draws with no top bar above it, so it owns its own clearance from the status bar.
        Spacer(Modifier.height(24.dp))
        Box(Modifier.padding(horizontal = 8.dp)) {
            EditorialHeader(label = "Warm-up", muted = muted, accent = accent)
        }

        if (prep.isNotEmpty()) {
            Spacer(Modifier.height(10.dp))
            Column(verticalArrangement = Arrangement.spacedBy(GROUP_SEAM)) {
                prep.forEachIndexed { i, drill ->
                    WarmupRow(
                        drill.name, drill.prescription, drill.id in checked,
                        Corners.ofRow(i, prep.size), onBg, muted, accent
                    ) { onToggle(drill.id) }
                }
            }
        }

        Spacer(Modifier.height(24.dp))

        // It starts the session at any tick count; nothing here is gated.
        ForgePrimaryCapsule("Start lifting", onClick = onStart, modifier = Modifier.fillMaxWidth())

        Spacer(Modifier.height(12.dp))
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
            verticalAlignment = Alignment.CenterVertically
        ) {
            OptOut("NOT TODAY", muted, onDisableToday)
            OptOut("NOT THIS WEEK", muted, onDisableWeek)
        }
        Spacer(Modifier.height(8.dp))
    }
}

/**
 * One tickable warmup row, a member of the drill group: a check disc, what to do, and its dose as
 * right-hand meta. The whole row is the tap target, so there is never a nested tap (§14). A ticked
 * row keeps the group's corners (pick-many) and wears the ring and wash alone.
 */
@Composable
private fun WarmupRow(
    label: String,
    meta: String,
    checked: Boolean,
    corners: Corners,
    onBg: Color,
    muted: Color,
    accent: Color,
    onToggle: () -> Unit
) {
    val shape = memberShape(corners, selected = false)
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .memberFill(shape, checked)
            .clickableLabeled(
                label = if (checked) "Untick $label" else "Tick $label",
                role = Role.Checkbox,
                onClick = onToggle
            )
            .padding(horizontal = ROW_H, vertical = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        CheckDisc(checked, muted, accent)
        // At a large font scale the dose drops under the drill rather than squeezing its name.
        if (LocalDensity.current.fontScale > 1.3f) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(label, style = MaterialTheme.typography.bodyLarge, color = if (checked) muted else onBg)
                if (meta.isNotBlank()) Text(meta, style = MaterialTheme.typography.labelMedium, color = muted)
            }
        } else {
            Text(
                label,
                style = MaterialTheme.typography.bodyLarge,
                color = if (checked) muted else onBg,
                modifier = Modifier.weight(1f)
            )
            if (meta.isNotBlank()) {
                Text(meta, style = MaterialTheme.typography.labelMedium, color = muted)
            }
        }
    }
}

/** Filled accent when ticked, hollow ring when not. Drawn, never independently clickable. */
@Composable
private fun CheckDisc(checked: Boolean, muted: Color, accent: Color) {
    Box(
        Modifier
            .size(20.dp)
            .clip(CircleShape)
            .then(
                if (checked) Modifier.background(accent)
                else Modifier.border(1.5.dp, muted.copy(alpha = 0.65f), CircleShape)
            )
    )
}

/** A persistent opt-out: a small filled chrome capsule with its mono label. */
@Composable
private fun OptOut(label: String, muted: Color, onClick: () -> Unit) {
    ForgeChromeButton(onClick = onClick, label = label) {
        Text(
            label,
            style = MaterialTheme.typography.labelMedium,
            color = muted,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp)
        )
    }
}
