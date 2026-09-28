package com.forge.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/**
 * Settings → What's new: the changelog ([CHANGELOG]), one group per release headed by its version
 * and date, each change led by its kind so a release reads New, then Improved, then Fixed.
 */
@Composable
internal fun WhatsNewPage(onBack: () -> Unit) {
    SettingsScaffold("What's new", onBack) {
        CHANGELOG.forEachIndexed { index, release ->
            SettingsGroup(
                if (index == 0) "Version ${release.version} · latest" else "Version ${release.version}",
                headerTrailing = release.date
            ) {
                SettingsGroupBlock {
                    release.notes.forEachIndexed { i, note ->
                        if (i > 0) Spacer(Modifier.height(14.dp))
                        ChangeRow(note)
                    }
                }
            }
        }
    }
}

/** One change: its kind as a small pill in a fixed column so the descriptions align, then the line. */
@Composable
private fun ChangeRow(note: ChangeNote) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        // widthIn(min), not width(): the pill may grow past it at 200% font rather than clip.
        Box(Modifier.widthIn(min = 84.dp)) {
            SettingsPill(
                note.kind.tag,
                when (note.kind) {
                    ChangeKind.New -> PillTone.Accent
                    ChangeKind.Improved -> PillTone.Neutral
                    ChangeKind.Fixed -> PillTone.Quiet
                }
            )
        }
        Text(
            note.text,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.weight(1f)
        )
    }
}
