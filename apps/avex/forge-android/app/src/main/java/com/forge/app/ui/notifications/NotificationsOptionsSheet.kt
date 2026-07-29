@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.notifications

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.clickableLabeled

/**
 * The notifications page's one action, opened from the top-bar gear (DESIGN §3, modal archetype):
 * surface fill, sheet top corners, and the two things that apply to the whole page rather than to
 * any one row.
 *
 * No divider between the rows. §5 permits a modal its dividers, and every other sheet in the app
 * took that permission — but §1's actual position is that air separates, and two rows do not need a
 * line to be told apart. The gate flagged it, and the gate was right.
 *
 * "Clear all" is tinted on its GLYPH, not its label: §14 measures `error` as text at 3.69:1 and
 * forbids new error-coloured body text until that's resolved, naming this exact substitution (an
 * onBg label beside a tinted mark) as the way to flag a destructive action meanwhile.
 */
@Composable
fun NotificationsOptionsSheet(
    canClear: Boolean,
    onClearAll: () -> Unit,
    onOpenNotificationSettings: () -> Unit,
    onDismiss: () -> Unit,
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val surface = MaterialTheme.colorScheme.surface
    val error = MaterialTheme.colorScheme.error

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = surface
    ) {
        Column(Modifier.fillMaxWidth().navigationBarsPadding()) {
            Text(
                "Settings",
                style = MaterialTheme.typography.headlineSmall,
                color = onBg,
                modifier = Modifier.padding(horizontal = 24.dp)
            )
            Spacer(Modifier.height(20.dp))

            SheetActionRow(
                icon = NoticeIcons.Bell,
                label = "Notification settings",
                tint = muted,
                labelColor = onBg,
                onClick = onOpenNotificationSettings
            )
            SheetActionRow(
                icon = NoticeIcons.Trash,
                label = "Clear all notifications",
                // Nothing to clear renders passive — no affordance that can't run (§2③).
                tint = if (canClear) error else muted.copy(alpha = 0.65f),
                labelColor = if (canClear) onBg else muted.copy(alpha = 0.65f),
                onClick = onClearAll.takeIf { canClear }
            )
            Spacer(Modifier.height(12.dp))
        }
    }
}

/** One sheet row: glyph, label, whole row as the tap target (§2③ — never a nested tap). */
@Composable
private fun SheetActionRow(
    icon: ImageVector,
    label: String,
    tint: Color,
    labelColor: Color,
    onClick: (() -> Unit)?,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .then(if (onClick != null) Modifier.clickableLabeled(label) { onClick() } else Modifier)
            // §14: heightIn, not height — the row grows with the font scale instead of clipping.
            .heightIn(min = 56.dp)
            .padding(horizontal = 24.dp, vertical = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Icon(icon, contentDescription = null, tint = tint, modifier = Modifier.size(20.dp))
        Text(label, style = MaterialTheme.typography.bodyLarge, color = labelColor)
    }
}
