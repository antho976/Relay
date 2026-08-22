package com.forge.app.ui.settings

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.security.BiometricAuthenticator
import com.forge.app.ui.common.clickableLabeled

/** The auto-lock grace options (seconds) offered on the Security page — 0 means "immediately". */
private val AUTO_LOCK_OPTIONS = listOf(
    0 to "Immediately",
    60 to "After 1 minute",
    300 to "After 5 minutes"
)

private fun autoLockLabel(sec: Int): String =
    AUTO_LOCK_OPTIONS.firstOrNull { it.first == sec }?.second ?: "Immediately"

/**
 * Security settings (GYMAP-69): the app-lock and photo-gallery-lock toggles + an auto-lock grace
 * chooser. A settings page (DESIGN §3) — mono anchors + air, one-line explainers, no dividers. The
 * lock authenticates against the device's biometric / screen-lock credential (no app PIN is stored);
 * enabling a lock is gated on that credential existing, with a quiet inline error line when it doesn't.
 */
@Composable
internal fun SecurityPage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    var noCredentialNote by remember { mutableStateOf(false) }
    var showTimeoutDialog by remember { mutableStateOf(false) }

    // Turning a lock ON needs an enrolled credential; OFF is always allowed. Without a device lock we
    // refuse and explain rather than enabling a lock that can never be satisfied.
    fun toggle(want: Boolean, set: (Boolean) -> Unit) {
        when {
            !want -> { set(false); noCredentialNote = false }
            BiometricAuthenticator.canAuthenticate(context) -> { set(true); noCredentialNote = false }
            else -> noCredentialNote = true
        }
    }

    Column(
        modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 24.dp)
    ) {
        SettingsSectionHeader("Lock", top = 12.dp)
        ToggleRow(
            "App lock",
            "Require an unlock to open Avex",
            state.appLockEnabled
        ) { toggle(it, vm::setAppLockEnabled) }
        ToggleRow(
            "Photo gallery lock",
            "Require an unlock to view your progress photos",
            state.galleryLockEnabled
        ) { toggle(it, vm::setGalleryLockEnabled) }

        if (noCredentialNote) {
            // §12's quiet inline error line, wording the consequence.
            Text(
                "Set a screen lock in your phone's settings first.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(horizontal = SETTINGS_GUTTER, vertical = 4.dp)
            )
        }

        // Auto-lock grace only matters once a lock is on.
        if (state.appLockEnabled || state.galleryLockEnabled) {
            SettingsSectionHeader("Auto-lock")
            AutoLockRow(state.appLockTimeoutSec) { showTimeoutDialog = true }
        }
    }

    if (showTimeoutDialog) {
        AlertDialog(
            containerColor = MaterialTheme.colorScheme.surface,
            onDismissRequest = { showTimeoutDialog = false },
            title = { Text("Auto-lock") },
            text = {
                Column {
                    AUTO_LOCK_OPTIONS.forEach { (sec, label) ->
                        val selected = state.appLockTimeoutSec == sec
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickableLabeled(label) {
                                    vm.setAppLockTimeoutSec(sec)
                                    showTimeoutDialog = false
                                }
                                .padding(vertical = SETTINGS_ROW_PAD),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(12.dp)
                        ) {
                            // The shared §12 dot, not a "●"/"○" text pair: one drawing for
                            // present-vs-absent across the whole app, and a real mark rather
                            // than two unicode characters standing in for an icon system.
                            StatusDot(active = selected)
                            Text(
                                label,
                                style = MaterialTheme.typography.bodyMedium,
                                color = if (selected) MaterialTheme.colorScheme.onBackground
                                else MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                    }
                }
            },
            confirmButton = { TextButton(onClick = { showTimeoutDialog = false }) { Text("Done") } }
        )
    }
}

/** A settings value row: the label + its live value, the whole row tapping to the chooser. */
@Composable
private fun AutoLockRow(currentSec: Int, onClick: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickableLabeled("Auto-lock", onClick = onClick)
            .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text("Auto-lock", style = MaterialTheme.typography.bodyMedium, color = onBg)
            SettingsExplainer("When Avex re-locks after you leave")
        }
        Text(autoLockLabel(currentSec), style = MaterialTheme.typography.bodyMedium, color = onBg)
    }
}
