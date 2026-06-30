package com.forge.app.ui.cardio.components

import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Watch
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * A quiet, dismissible top banner inviting the user to connect a watch/ring for steps + GPS. Tapping
 * the strip opens Settings → Recovery to grant Forge the steps/GPS read (connecting a watch to Samsung
 * Health only feeds Health Connect — Forge still needs its own per-type grant). Tapping the × removes it
 * for good (the caller persists the dismissal); the caller also hides it automatically once a grant
 * exists, so it never nags after you're connected. Deliberately unobtrusive — a thin outlined strip.
 */
@Composable
internal fun CardioWatchBanner(
    onConnect: () -> Unit,
    onDismiss: () -> Unit,
    onBg: Color,
    muted: Color,
    outline: Color,
    modifier: Modifier = Modifier
) {
    Row(
        modifier = modifier
            .padding(horizontal = 24.dp, vertical = 8.dp)
            .border(1.dp, outline.copy(alpha = 0.3f), RoundedCornerShape(12.dp))
            .clip(RoundedCornerShape(12.dp))
            .clickable(onClick = onConnect)
            .padding(start = 14.dp, end = 6.dp, top = 10.dp, bottom = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Icon(Icons.Filled.Watch, contentDescription = null, tint = muted, modifier = Modifier.size(20.dp))
        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(1.dp)) {
            Text("Connect a watch or ring", style = MaterialTheme.typography.bodyMedium, color = onBg)
            Text(
                "Steps by the hour and your route on each session — tap to set up.",
                style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp
            )
        }
        Icon(
            Icons.Filled.Close,
            contentDescription = "Dismiss",
            tint = muted,
            modifier = Modifier
                .clickable(onClick = onDismiss)
                .padding(8.dp)
                .size(18.dp)
        )
    }
}
