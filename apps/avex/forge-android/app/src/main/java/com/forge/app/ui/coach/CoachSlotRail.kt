package com.forge.app.ui.coach

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import com.forge.app.domain.coach.SignalRegistry

/**
 * What the coach can read, and what it will read (Coach v3 A2's SignalRegistry) — rendered inside
 * the Signals lens's own vocabulary rather than as a second "signals" home.
 *
 * §12: this IS the drawn zero-state. A filled disc means the slot is live on your data, a hollow
 * ring means it's wired but has nothing yet, and a dim ring means it's declared and not built —
 * so the product is visibly growing instead of hiding its roadmap behind a changelog.
 */
@Composable
internal fun SignalSlotRail(
    slots: List<Pair<SignalRegistry.Slot, SignalRegistry.Availability>>,
    c: CoachColors
) {
    if (slots.isEmpty()) return
    Column(Modifier.fillMaxWidth()) {
        slots.forEach { (slot, availability) ->
            Row(
                Modifier.fillMaxWidth().padding(vertical = COACH_ROW_PAD),
                verticalAlignment = Alignment.CenterVertically
            ) {
                SlotDot(availability, c)
                Spacer(Modifier.width(10.dp))
                Text(
                    slot.label,
                    style = MaterialTheme.typography.bodyMedium,
                    color = if (availability == SignalRegistry.Availability.COMING_SOON) c.muted else c.onBg,
                    modifier = Modifier.weight(1f)
                )
            }
        }
    }
}

/**
 * Filled accent = reading you now · hollow accent ring = wired, waiting on your data ·
 * dim outline ring = declared, not built. One mark, three honest states, no status words.
 */
@Composable
private fun SlotDot(availability: SignalRegistry.Availability, c: CoachColors) {
    when (availability) {
        SignalRegistry.Availability.ACTIVE ->
            Box(Modifier.size(7.dp).clip(RoundedCornerShape(50)).background(c.accent))
        SignalRegistry.Availability.AWAITING_DATA ->
            Box(
                Modifier.size(7.dp).clip(RoundedCornerShape(50))
                    .border(1.5.dp, c.accent, RoundedCornerShape(50))
            )
        SignalRegistry.Availability.COMING_SOON ->
            Box(
                Modifier.size(7.dp).clip(RoundedCornerShape(50))
                    .border(1.5.dp, c.outline, RoundedCornerShape(50))
            )
    }
}
