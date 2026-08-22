package com.forge.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.cardio.CardioGlyphs
import com.forge.app.domain.cardio.CustomCardioType
import com.forge.app.ui.cardio.components.CustomActivityDialog
import com.forge.app.ui.common.GlyphButton
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.clickableLabeled

/**
 * Manage user-defined cardio activities (GYMAP-37). Mirrors [VacationPage]: a mono section anchor, an
 * accent "+ add" action, drawn-empty hint, then one tappable row per activity (tap = edit, ✕ = forget).
 * The create/edit sheet is the same [CustomActivityDialog] the log picker's inline flow uses.
 */
@Composable
internal fun CardioActivitiesPage(vm: SettingsViewModel, modifier: Modifier = Modifier) {
    val types by vm.customCardioTypes.collectAsStateWithLifecycle()
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val accent = MaterialTheme.colorScheme.primary

    var showCreate by remember { mutableStateOf(false) }
    var editing by remember { mutableStateOf<CustomCardioType?>(null) }

    if (showCreate) {
        CustomActivityDialog(
            initial = null,
            onDismiss = { showCreate = false },
            onConfirm = { vm.addCustomCardioType(it); showCreate = false }
        )
    }
    editing?.let { current ->
        CustomActivityDialog(
            initial = current,
            onDismiss = { editing = null },
            onConfirm = { vm.updateCustomCardioType(it); editing = null }
        )
    }

    LazyColumn(modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 56.dp)) {
        // The top bar never names the screen (§2) — the page opens with its own mono anchor.
        item("header") { SettingsSectionHeader("Custom activities", top = 12.dp) }
        item("intro") {
            Text(
                "Log a sport the built-in list misses. It shows up in the cardio picker beside the standard types.",
                style = MaterialTheme.typography.bodySmall, color = muted,
                modifier = Modifier.padding(horizontal = SETTINGS_GUTTER, vertical = 8.dp)
            )
        }
        item("add") {
            // The accent mono action idiom (DESIGN §11) — same voice as SettingsActionLink.
            Text(
                "+ add activity",
                style = MaterialTheme.typography.labelLarge, color = accent, letterSpacing = 0.3.sp,
                modifier = Modifier.fillMaxWidth()
                    .clickableLabeled("Add custom activity") { showCreate = true }
                    .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD)
            )
        }
        if (types.isEmpty()) {
            item("empty") {
                InlineEmptyHint(
                    "No custom activities yet.",
                    color = muted,
                    modifier = Modifier.padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD)
                )
            }
        } else {
            items(types, key = { it.code }) { t ->
                Row(
                    modifier = Modifier.fillMaxWidth()
                        .clickableLabeled("Edit ${t.name}") { editing = t }
                        .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD),
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Icon(
                        CardioGlyphs.icon(t.glyphKey),
                        contentDescription = null,
                        tint = muted,
                        modifier = Modifier.size(20.dp)
                    )
                    Text(t.name, style = MaterialTheme.typography.bodyMedium, color = onBg, modifier = Modifier.weight(1f))
                    // §2③/§14: the row taps to EDIT, so the delete cannot be a second target
                    // nested inside it. GlyphButton carries its own ≥48dp target and TalkBack
                    // label; Compose routes the tap to the innermost handler, so the two no
                    // longer overlap the way a padded Text inside a clickable Row did.
                    GlyphButton("✕", "Delete ${t.name}", muted, { vm.deleteCustomCardioType(t.code) })
                }
            }
        }
    }
}
