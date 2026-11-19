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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import com.forge.app.program.DayPlan
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.window.ModalBottomSheet
import com.forge.app.ui.theme.DayIcons
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType

/**
 * "Different day": the plan's other days, to train instead of the one Home named.
 *
 * The lasting way to leave a body part out is Settings → Program → Body parts, which rebuilds the
 * plan. This is the one-off: legs are on the card and today is not the day for them, so pick another
 * session and the plan itself stays exactly as it is. Each row starts that day's session.
 *
 * @param days every plan day except the one Home is offering.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun NotTodaySheet(
    todayName: String,
    days: List<DayPlan>,
    onPick: (dayKey: String) -> Unit,
    onDismiss: () -> Unit,
) {
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
                Text("Train something else", style = OpenType.sheetTitle, color = MaterialTheme.colorScheme.onBackground)
                Text(
                    "Instead of $todayName, today only. Your plan stays as it is.",
                    style = OpenType.bodyS,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                days.forEach { day ->
                    val body = DayIcons.forDayName(day.defaultName)
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(4.dp))
                            .bounceClick(role = Role.Button) { onPick(day.key) }
                            .padding(horizontal = 12.dp, vertical = 12.dp),
                        horizontalArrangement = Arrangement.spacedBy(14.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Icon(
                            body ?: OpenIcons.Dumbbell, contentDescription = null,
                            tint = if (body != null) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.size(if (body != null) 28.dp else 20.dp)
                        )
                        Text(
                            day.defaultName,
                            style = OpenType.rowTitle,
                            color = MaterialTheme.colorScheme.onBackground,
                            modifier = Modifier.weight(1f)
                        )
                        OpenLabel("${day.exercises.size} lifts", style = OpenType.monoSTight)
                    }
                }
            }
        }
    }
}
