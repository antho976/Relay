package com.forge.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenMeter
import com.forge.app.ui.theme.OpenType

/**
 * Storage breakdown and cache clear (GYMAP-68), as a reading: the space used as a serif figure, then
 * what each kind of data takes as a share of it on a meter, and the one thing that can be reclaimed.
 * Cache is the only category you can clear, so it owns the page's button.
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
        // The figure with its unit at half size, muted, as every reading draws it.
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            OpenLabel("Space used")
            val (figure, unit) = formatBytes(total).split(' ').let { it[0] to it.getOrElse(1) { "" } }
            val muted = MaterialTheme.colorScheme.onSurfaceVariant
            Text(
                buildAnnotatedString {
                    append(figure)
                    pushStyle(OpenType.figureXLUnit.toSpanStyle().copy(color = muted)); append(" $unit"); pop()
                },
                style = OpenType.figureXL,
                color = MaterialTheme.colorScheme.onBackground
            )
        }

        if (!b?.categories.isNullOrEmpty()) {
            SettingsGroup("What takes it") {
                b?.categories?.forEach { cat ->
                    StorageRow(cat.label, formatBytes(cat.bytes), if (total > 0) cat.bytes.toFloat() / total else 0f)
                }
            }
        }

        // Disabled and inert when there's nothing to reclaim, so it never looks like it does something.
        Column {
            SettingsButton(
                if (cache > 0) "Clear cache · ${formatBytes(cache)}" else "Clear cache",
                kind = ButtonKind.Outlined,
                enabled = cache > 0,
                onClick = vm::clearCache
            )
            SettingsGroupFooter("Cache holds temporary files the app rebuilds. Your workouts and photos stay.")
        }
    }
}

/** One category: its name and size over a meter of its share of the total. */
@Composable
private fun StorageRow(label: String, sizeLabel: String, fraction: Float) {
    Column(Modifier.fillMaxWidth().clearAndSetSemantics { contentDescription = "$label, $sizeLabel" }) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Text(label, style = OpenType.rowTitleL, color = MaterialTheme.colorScheme.onBackground, modifier = Modifier.weight(1f))
            SettingsReading(sizeLabel)
        }
        Spacer(Modifier.height(10.dp))
        // A floor so a small, non-zero category still reads as a sliver, not an empty track.
        OpenMeter(if (fraction > 0f) fraction.coerceAtLeast(0.02f) else 0f)
    }
}
