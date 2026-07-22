package com.forge.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Settings → What's new. The app changelog ([CHANGELOG]) as a settings sub-page: one mono section
 * anchor per release (version · date, DESIGN §7 air rhythm — no dividers), then its change lines.
 * Follows the Settings archetype like its neighbour [AboutPage] — no serif hero, no figures — so the
 * two app-info pages read as one. Each line leads with its kind (New/Improved/Fixed) as a quiet mono
 * tag, the distinct "reading" that earns a list over a run of identical bullets (§4.10).
 */
@Composable
internal fun WhatsNewPage(modifier: Modifier = Modifier) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 56.dp)
    ) {
        CHANGELOG.forEachIndexed { index, release ->
            SettingsSectionHeader("${release.version} · ${release.date}", top = if (index == 0) 12.dp else 26.dp)
            release.notes.forEach { note -> ChangeRow(note) }
        }
        Spacer(Modifier.height(8.dp))
    }
}

/** One changelog line: a fixed-width mono kind tag so the descriptions align, then the change. */
@Composable
private fun ChangeRow(note: ChangeNote) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp, vertical = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text(
            note.kind.tag.uppercase(),
            style = MaterialTheme.typography.labelSmall,
            color = muted,
            fontSize = 10.sp,
            letterSpacing = 0.5.sp,
            maxLines = 1,
            modifier = Modifier
                .width(72.dp)
                .padding(top = 2.dp)
        )
        Text(
            note.text,
            style = MaterialTheme.typography.bodySmall,
            color = onBg,
            modifier = Modifier.weight(1f)
        )
    }
}
