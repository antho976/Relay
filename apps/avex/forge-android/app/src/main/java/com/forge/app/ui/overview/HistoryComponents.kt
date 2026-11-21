package com.forge.app.ui.overview

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** A small value-over-label stat cell — used in [SummarySheet]'s stats strip. */
@Composable
internal fun SummaryStat(value: String, label: String, muted: Color, onBg: Color) {
    Column(verticalArrangement = Arrangement.spacedBy(1.dp)) {
        Text(value, style = MaterialTheme.typography.bodyMedium, color = onBg, fontWeight = FontWeight.Normal)
        Text(label, style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.65f), fontSize = 9.sp, letterSpacing = 0.5.sp)
    }
}
