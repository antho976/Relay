package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.LoggedSet

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun SetRow(
    set: LoggedSet,
    isPr: Boolean,
    onDelete: () -> Unit,
    /** Long-press to immediately log another set with the same weight and reps. */
    onLongPress: (() -> Unit)? = null,
    modifier: Modifier = Modifier
) {
    val accent = MaterialTheme.colorScheme.primary
    val bg = if (isPr) accent.copy(alpha = 0.14f)
             else MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.4f)

    val baseModifier = modifier
        .fillMaxWidth()
        .clip(RoundedCornerShape(8.dp))
        .background(bg)
        .let { mod ->
            if (isPr) mod.border(width = 1.dp, color = accent, shape = RoundedCornerShape(8.dp))
            else mod
        }

    val rowModifier = if (onLongPress != null) {
        baseModifier.combinedClickable(onClick = {}, onLongClick = onLongPress)
    } else {
        baseModifier
    }.padding(horizontal = 12.dp, vertical = 6.dp)

    Row(
        modifier = rowModifier,
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Box(
            Modifier
                .size(28.dp)
                .clip(RoundedCornerShape(50))
                .background(accent.copy(alpha = if (isPr) 0.4f else 0.2f)),
            contentAlignment = Alignment.Center
        ) {
            Text(
                "${set.setIndex + 1}",
                color = accent,
                fontWeight = FontWeight.Bold,
                style = MaterialTheme.typography.labelLarge
            )
        }
        Text(
            "${set.weightText} × ${set.reps}",
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.weight(1f)
        )
        if (isPr) {
            Text(
                "PR",
                color = accent,
                fontWeight = FontWeight.Black,
                style = MaterialTheme.typography.labelLarge
            )
        }
        IconButton(
            onClick = onDelete,
            modifier = Modifier.size(28.dp)
        ) {
            Icon(
                Icons.Default.Close,
                contentDescription = "Delete set",
                tint = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}
