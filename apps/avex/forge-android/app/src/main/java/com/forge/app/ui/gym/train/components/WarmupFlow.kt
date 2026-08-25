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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.domain.warmup.WarmupDrill
import com.forge.app.domain.warmup.WarmupProtocol
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.clickableLabeled

/**
 * The pre-session warmup: one screen, one button.
 *
 * Not a stepper and not a gate. Rows tick off so the user can keep their place across a set of
 * jumping jacks, but the ticks are their own scratchpad, not a checklist the app grades: nothing is
 * required, nothing is stored, and the button works from the first frame. There is exactly one
 * button because "start" and "skip" were the same action wearing two labels.
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

    Column(modifier.fillMaxWidth().padding(horizontal = 24.dp)) {

        // The gate draws with no top bar above it, so it owns its own clearance from the status bar.
        Spacer(Modifier.height(24.dp))
        EditorialHeader(label = "Warm-up", muted = muted, accent = accent)

        if (prep.isNotEmpty()) {
            Spacer(Modifier.height(10.dp))
            prep.forEach {
                WarmupRow(it.name, it.prescription, it.id in checked, onBg, muted, accent) {
                    onToggle(it.id)
                }
            }
        }

        Spacer(Modifier.height(24.dp))

        // Accent-filled, matching Home's start CTA: this is the same act, so it wears the same coat.
        StartButton(label = "Start lifting", onClick = onStart)

        Spacer(Modifier.height(12.dp))
        // Separated by air rather than a mid dot: at mono's baseline the dot reads as a full stop
        // between two labels, which turns the pair into one broken sentence.
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterHorizontally),
            verticalAlignment = Alignment.CenterVertically
        ) {
            OptOut("NOT TODAY", muted, onDisableToday)
            OptOut("NOT THIS WEEK", muted, onDisableWeek)
        }
        Spacer(Modifier.height(8.dp))
    }
}

/**
 * One tickable warmup row: a check disc, what to do, and its dose as right-hand meta. The whole row
 * is the tap target, so there is never a nested tap (§14).
 */
@Composable
private fun WarmupRow(
    label: String,
    meta: String,
    checked: Boolean,
    onBg: Color,
    muted: Color,
    accent: Color,
    onToggle: () -> Unit
) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .clickableLabeled(
                label = if (checked) "Untick $label" else "Tick $label",
                role = Role.Checkbox,
                onClick = onToggle
            )
            .padding(vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        CheckDisc(checked, muted, accent)
        Text(
            label,
            style = MaterialTheme.typography.bodyMedium,
            color = if (checked) muted else onBg,
            modifier = Modifier.weight(1f)
        )
        if (meta.isNotBlank()) {
            Text(meta, style = MaterialTheme.typography.labelMedium, color = muted)
        }
    }
}

/** Filled accent when ticked, hollow ring when not. Drawn, never independently clickable. */
@Composable
private fun CheckDisc(checked: Boolean, muted: Color, accent: Color) {
    Box(
        Modifier
            .size(18.dp)
            .clip(CircleShape)
            .then(
                if (checked) Modifier.background(accent)
                else Modifier.border(1.5.dp, muted.copy(alpha = 0.65f), CircleShape)
            )
    )
}

/** The one action. Accent fill, `onPrimary` label, sized like Home's start CTA. */
@Composable
private fun StartButton(label: String, onClick: () -> Unit) {
    Box(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .clip(StartShape)
            .background(MaterialTheme.colorScheme.primary)
            .bounceClick(onClick = onClick)
            .padding(horizontal = 20.dp, vertical = 14.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            label,
            style = MaterialTheme.typography.labelLarge,
            fontWeight = FontWeight.Bold,
            color = MaterialTheme.colorScheme.onPrimary
        )
    }
}

/** A persistent opt-out. Mono micro-label, with the 48dp target coming from its own padding (§14). */
@Composable
private fun OptOut(label: String, muted: Color, onClick: () -> Unit) {
    Text(
        label,
        style = MaterialTheme.typography.labelMedium,
        color = muted,
        modifier = Modifier
            .heightIn(min = 48.dp)
            .clickableLabeled(label = label, onClick = onClick)
            .padding(horizontal = 10.dp, vertical = 15.dp)
    )
}

/** Matches Home's CTA corner so the two start buttons read as the same control. */
private val StartShape = RoundedCornerShape(12.dp)
