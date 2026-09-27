package com.forge.app.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle

/**
 * Storage breakdown and cache clear (GYMAP-68): the total, what each kind of data takes as a share
 * of it, and the one thing that can be reclaimed. Cache is the only category you can clear, so it
 * owns the page's button.
 */
@Composable
internal fun StoragePage(vm: SettingsViewModel, onBack: () -> Unit) {
    val storage by vm.storage.collectAsStateWithLifecycle()
    // Local reads are instant; (re)measure whenever the page is shown.
    LaunchedEffect(Unit) { vm.refreshStorage() }

    val b = storage
    val total = b?.totalBytes ?: 0L
    val cache = b?.cacheBytes ?: 0L

    SettingsScaffold("Storage", onBack) {
        SettingsGroup {
            SettingsAdaptiveRow("Space used") {
                Text(formatBytes(total), style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurface)
            }
            b?.categories?.forEach { cat ->
                StorageRow(cat.label, formatBytes(cat.bytes), if (total > 0) cat.bytes.toFloat() / total else 0f)
            }
        }

        // Disabled and inert when there's nothing to reclaim, so it never looks like it does something.
        Column {
            SettingsButton(
                if (cache > 0) "Clear cache · ${formatBytes(cache)}" else "Clear cache",
                Modifier.fillMaxWidth(),
                enabled = cache > 0,
                onClick = vm::clearCache
            )
            SettingsGroupFooter("Cache holds temporary files the app rebuilds. Your workouts and photos stay.")
        }
    }
}

/** One category: its name and size over a thin bar of its share of the total. */
@Composable
private fun StorageRow(label: String, sizeLabel: String, fraction: Float) {
    SettingsGroupBlock(padding = PaddingValues(horizontal = KitRowPad, vertical = 14.dp)) {
        Row(
            Modifier.fillMaxWidth().clearAndSetSemantics { contentDescription = "$label, $sizeLabel" },
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(label, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurface)
            Text(sizeLabel, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Spacer(Modifier.height(10.dp))
        Box(
            Modifier
                .fillMaxWidth()
                .height(4.dp)
                .clip(RoundedCornerShape(50))
                .background(MaterialTheme.colorScheme.outline.copy(alpha = 0.35f))
        ) {
            // A floor so a small, non-zero category still reads as a sliver, not an empty track.
            Box(
                Modifier
                    .fillMaxWidth(fraction.coerceIn(0.02f, 1f))
                    .height(4.dp)
                    .clip(RoundedCornerShape(50))
                    .background(MaterialTheme.colorScheme.primary)
            )
        }
    }
}
