package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.Corners
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ROW_H
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.common.memberFill
import com.forge.app.ui.common.memberShape
import com.forge.app.ui.common.parseAccentHex
import com.forge.app.ui.gym.train.state.DayListItem

/**
 * One day of the plan, drawn as a member of the Train tab's day group (2026-09-27): the raised
 * fill, the group's outer or inner corners from [corners], and 2dp seams to its neighbours instead
 * of a separate card each. The day's own colour stays on its spine.
 */
@Composable
internal fun DayCard(
    item: DayListItem,
    onClick: () -> Unit,
    onQuickStart: (() -> Unit)? = null,
    onLongPress: (() -> Unit)? = null,
    modifier: Modifier = Modifier,
    corners: Corners = Corners(true, true, true, true)
) {
    if (item.isNextUp) {
        NextUpCard(item = item, onClick = onClick, onQuickStart = onQuickStart, onLongPress = onLongPress, modifier = modifier, corners = corners)
    } else {
        CompactCard(item = item, onClick = onClick, onLongPress = onLongPress, modifier = modifier, corners = corners)
    }
}

// ── Next-up variant ──────────────────────────────────────────────────────────

@Composable
private fun NextUpCard(
    item: DayListItem,
    onClick: () -> Unit,
    onQuickStart: (() -> Unit)? = null,
    onLongPress: (() -> Unit)? = null,
    modifier: Modifier = Modifier,
    corners: Corners
) {
    // parseAccentHex, not the throwing parser this used to call: the value crosses DataStore
    // (a user-set day colour) and the program_day.accent_hex column, so a restored backup or a
    // blank hex would have taken the whole Train tab down on composition. One parser, non-throwing
    // — the same one ForgeTheme and ForgeWidget already use for the same kind of value.
    val accent = parseAccentHex(item.customAccentHex ?: item.plan.accentHex)
    val shape = memberShape(corners, selected = false)

    // The spine fills whatever height the content gives the card (matchParentSize), so the card
    // needs no intrinsic measurement and grows freely with font scale.
    Box(
        modifier
            .fillMaxWidth()
            .memberFill(shape, selected = false)
            .bounceCombinedClick(
                onClickLabel = "Open ${item.displayName}",
                onLongClickLabel = "Options for ${item.displayName}",
                onLongClick = onLongPress,
                onClick = onClick
            )
    ) {
        Box(Modifier.matchParentSize()) { SpineStrip(accent = accent, word = item.plan.word) }
        Row(Modifier.fillMaxWidth().padding(start = SPINE_W)) {
        Column(
            Modifier
                .weight(1f)
                .padding(horizontal = ROW_H, vertical = 16.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            NextUpPill(accent = accent)

            Text(
                item.displayName,
                style = MaterialTheme.typography.headlineMedium,
                color = MaterialTheme.colorScheme.onSurface,
                fontWeight = FontWeight.Bold
            )
            Text(
                buildString {
                    append("${item.plan.subtitle} · ${item.exerciseCount} exercises")
                    // Tuned to the user's realized rest pace once the engine has data.
                    val mins = item.estimatedMinutes
                        ?: com.forge.app.program.SessionEstimate.estimateMinutes(item.plan)
                    if (mins > 0) append(" · ~$mins min")
                },
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )

            Spacer(Modifier.height(10.dp))

            ForgePrimaryCapsule(
                if (item.isActive) "Continue →" else "Start →",
                onClick = onClick,
                modifier = Modifier.fillMaxWidth()
            )
            if (!item.isActive && onQuickStart != null) {
                Spacer(Modifier.height(6.dp))
                // One rung up the surface ladder: the card is already the raised fill.
                GymSecondaryCapsule(
                    "Quick start (skip warmup)",
                    onClick = onQuickStart,
                    modifier = Modifier.fillMaxWidth()
                )
            }
        }
    }
}
}
