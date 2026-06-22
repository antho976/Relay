package com.forge.app.ui.gym.stats.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import com.forge.app.ui.theme.emphasized
import com.forge.app.ui.theme.emphasizedWeight
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

// ─── Shared card shell ────────────────────────────────────────────────────────
// The Stats-screen analytics cards were removed in the Stats revamp; StatCard stays because it's the
// shared surfaceVariant card shell reused outside the stats package (e.g. SessionDetailScreen).

@Composable
internal fun StatCard(
    title: String,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit
) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(12.dp))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Text(
            title,
            style = MaterialTheme.typography.labelLarge,
            color = emphasized(MaterialTheme.colorScheme.onSurfaceVariant),
            fontWeight = emphasizedWeight(FontWeight.SemiBold)
        )
        content()
    }
}
