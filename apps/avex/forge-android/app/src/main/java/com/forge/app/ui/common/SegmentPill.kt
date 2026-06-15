package com.forge.app.ui.common

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * The shared rounded selectable pill behind the app's filter/segment toggles (trophy filters,
 * the session-detail metric/style switches). Selected = accent border + faint accent tint.
 */
@Composable
internal fun SegmentPill(
    text: String,
    selected: Boolean,
    onClick: () -> Unit,
    accent: Color,
    onBg: Color,
    muted: Color,
    outline: Color,
    fontSize: TextUnit = 10.sp
) {
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(50))
            .border(0.5.dp, if (selected) accent else outline.copy(alpha = 0.35f), RoundedCornerShape(50))
            .background(if (selected) accent.copy(alpha = 0.15f) else Color.Transparent)
            .clickable { onClick() }
            .padding(horizontal = 12.dp, vertical = 5.dp)
    ) {
        Text(
            text,
            style = MaterialTheme.typography.labelSmall,
            color = if (selected) onBg else muted.copy(alpha = 0.65f),
            fontSize = fontSize
        )
    }
}
