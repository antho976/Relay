package com.forge.app.ui.overview

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.window.ModalBottomSheet
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType

private fun HomeSlot.glyph(): ImageVector = when (this) {
    HomeSlot.SLEEP -> OpenIcons.Moon
    HomeSlot.RESTING_HR -> OpenIcons.Heart
    HomeSlot.HRV -> OpenIcons.Activity
    HomeSlot.STEPS -> OpenIcons.Navigation
    HomeSlot.BODY_WEIGHT -> OpenIcons.User
    HomeSlot.LAST_SESSION -> OpenIcons.History
    HomeSlot.WEEK_VOLUME -> OpenIcons.Dumbbell
}

/**
 * "Show in this spot": every reading Home can hold, each with what it reads right now. The one in
 * this spot is washed red with a check; the one in the other spot sits at half strength and says so,
 * and picking it swaps the two.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun HomeSlotSheet(
    left: Boolean,
    slots: OverviewViewModel.HomeSlots,
    onPick: (HomeSlot) -> Unit,
    onDismiss: () -> Unit,
) {
    val current = if (left) slots.left else slots.right
    val other = if (left) slots.right else slots.left
    val otherName = slots.readings[other]?.label ?: other.label
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = MaterialTheme.colorScheme.surface,
        shape = RoundedCornerShape(topStart = 20.dp, topEnd = 20.dp),
        dragHandle = {
            Box(Modifier.padding(top = 12.dp, bottom = 10.dp).width(36.dp).height(4.dp).clip(RoundedCornerShape(2.dp)).background(OpenColors.Rule))
        }
    ) {
        Column(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, bottom = 28.dp).navigationBarsPadding()) {
            Column(Modifier.padding(start = 8.dp, end = 8.dp, top = 8.dp, bottom = 14.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text("Show in this spot", style = OpenType.sheetTitle, color = MaterialTheme.colorScheme.onBackground)
                Text(
                    "${if (left) "Left" else "Right"} reading · the ${if (left) "right" else "left"} one shows $otherName",
                    style = OpenType.bodyS,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                HomeSlot.entries.forEach { slot ->
                    val selected = slot == current
                    val elsewhere = slot == other
                    val value = when {
                        elsewhere -> if (left) "Right spot" else "Left spot"
                        else -> slots.readings[slot]?.picker ?: "No data"
                    }
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(4.dp))
                            .then(if (selected) Modifier.background(MaterialTheme.colorScheme.primaryContainer) else Modifier)
                            .bounceClick(role = Role.RadioButton) { onPick(slot) }
                            .semantics { this.selected = selected }
                            .alpha(if (elsewhere) 0.5f else 1f)
                            .padding(horizontal = 12.dp, vertical = 14.dp),
                        horizontalArrangement = Arrangement.spacedBy(14.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Icon(
                            slot.glyph(), contentDescription = null,
                            tint = if (selected) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.size(20.dp)
                        )
                        Text(
                            slot.pickerTitle,
                            style = OpenType.rowTitle.copy(fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal),
                            color = MaterialTheme.colorScheme.onBackground,
                            modifier = Modifier.weight(1f)
                        )
                        OpenLabel(value, style = OpenType.monoSTight, color = if (selected) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant)
                        if (selected) {
                            Icon(OpenIcons.Check, contentDescription = null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(18.dp))
                        }
                    }
                }
            }
        }
    }
}
