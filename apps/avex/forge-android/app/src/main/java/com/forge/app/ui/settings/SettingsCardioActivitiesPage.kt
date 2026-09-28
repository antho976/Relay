package com.forge.app.ui.settings

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.DirectionsRun
import androidx.compose.material.icons.outlined.DeleteOutline
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.cardio.CardioGlyphs
import com.forge.app.domain.cardio.CustomCardioType
import com.forge.app.ui.cardio.components.CustomActivityDialog
import com.forge.app.ui.common.clickableLabeled

/**
 * Your own cardio activities (GYMAP-37): one row per activity (tap to edit, the bin to forget it)
 * and an add row. The create and edit sheet is the same [CustomActivityDialog] the log picker uses.
 */
@Composable
internal fun CardioActivitiesPage(vm: SettingsViewModel, onBack: () -> Unit) {
    val types by vm.customCardioTypes.collectAsStateWithLifecycle()
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

    SettingsScaffold("Cardio activities", onBack) {
        SettingsGroup(footer = "Your activities appear in the cardio picker beside the standard types.") {
            if (types.isEmpty()) {
                SettingsEmptyBlock(
                    Icons.AutoMirrored.Rounded.DirectionsRun,
                    "No custom activities yet",
                    "Add a sport the built-in list misses, like padel or kayaking."
                )
            } else {
                types.forEach { t ->
                    SettingsRowContainer(interaction = Modifier.clickableLabeled("Edit ${t.name}") { editing = t }) {
                        // The default "other" glyph is three dots, which reads as an overflow menu in a
                        // list; an activity without its own glyph takes the cardio mark instead.
                        SettingsIconTile(if (t.glyphKey == CardioGlyphs.DEFAULT_KEY) com.forge.app.ui.nav.NavIcons.Cardio else CardioGlyphs.icon(t.glyphKey))
                        SettingsRowText(t.name)
                        IconButton(onClick = { vm.deleteCustomCardioType(t.code) }) {
                            Icon(Icons.Outlined.DeleteOutline, contentDescription = "Delete ${t.name}", tint = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                }
            }
            SettingsActionRow("Add an activity", icon = Icons.Rounded.Add, tone = TileTone.Accent) { showCreate = true }
        }
    }
}
