package com.forge.app.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle

/**
 * Storage breakdown + cache clear (GYMAP-68). A settings page, so it keeps the archetype toolkit —
 * mono section anchors + air, no dividers, the one do-it-now action grouped at the END. The §12 mark
 * is the per-category proportion bar (the SAME data-line vocabulary at every size); the actionable
 * category — cache — is the only one that can be reclaimed, so it owns the page-end capsule.
 */
@Composable
internal fun StoragePage(vm: SettingsViewModel, modifier: Modifier = Modifier) {
    val storage by vm.storage.collectAsStateWithLifecycle()
    // Local reads are instant (DESIGN §13 — no spinner); (re)measure whenever the page is shown.
    LaunchedEffect(Unit) { vm.refreshStorage() }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant

    Column(
        modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 24.dp)
    ) {
        SettingsSectionHeader("Storage", top = 12.dp)

        val b = storage
        val total = b?.totalBytes ?: 0L
        Text(
            "${formatBytes(total)} used",
            style = MaterialTheme.typography.bodyLarge,
            color = onBg,
            modifier = Modifier.padding(start = SETTINGS_GUTTER, end = SETTINGS_GUTTER, top = 2.dp)
        )
        Spacer(Modifier.height(12.dp))

        b?.categories?.forEach { cat ->
            val fraction = if (total > 0) (cat.bytes.toFloat() / total) else 0f
            StorageRow(cat.label, formatBytes(cat.bytes), fraction)
        }

        // Clear-cache — the page's one do-it-now action, at the END (§8). Disabled + inert when there's
        // nothing to reclaim, so it never looks tappable while doing nothing (§4.5).
        val cache = b?.cacheBytes ?: 0L
        Spacer(Modifier.height(24.dp))
        Text(
            "Cache holds temporary files the app rebuilds. Your workouts and photos stay.",
            style = MaterialTheme.typography.bodySmall,
            color = muted,
            modifier = Modifier.padding(horizontal = SETTINGS_GUTTER)
        )
        Spacer(Modifier.height(10.dp))
        SettingsActionRow {
            SettingsPrimaryAction(
                label = if (cache > 0) "Clear cache · ${formatBytes(cache)}" else "Clear cache",
                enabled = cache > 0,
                onClick = vm::clearCache
            )
        }
    }
}

/** One breakdown line: label + size over a thin proportion bar (§12 mark, §5 primary-on-outline bar). */
@Composable
private fun StorageRow(label: String, sizeLabel: String, fraction: Float) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val primary = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline
    Column(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD)
    ) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(label, style = MaterialTheme.typography.bodyMedium, color = onBg)
            Text(sizeLabel, style = MaterialTheme.typography.labelMedium, color = muted)
        }
        Spacer(Modifier.height(6.dp))
        Box(
            Modifier
                .fillMaxWidth()
                .height(3.dp)
                .clip(RoundedCornerShape(50))
                .background(outline.copy(alpha = 0.25f))
        ) {
            // A tiny floor so a non-zero-but-tiny category still reads as a sliver, not an empty track.
            Box(
                Modifier
                    .fillMaxWidth(fraction.coerceIn(0.02f, 1f))
                    .height(3.dp)
                    .clip(RoundedCornerShape(50))
                    .background(primary)
            )
        }
    }
}
