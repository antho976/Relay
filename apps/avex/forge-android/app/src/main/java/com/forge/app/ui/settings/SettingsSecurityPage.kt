package com.forge.app.ui.settings

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import com.forge.app.security.BiometricAuthenticator
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.theme.OpenType

/** The auto-lock grace options (seconds) offered on the Security page — 0 means "immediately". */
private val AUTO_LOCK_OPTIONS = listOf(
    0 to "Immediately",
    60 to "1 minute",
    300 to "5 minutes"
)

/**
 * Privacy & security (GYMAP-69, Relay design "lk5"): the app and photo-gallery locks, when they lock
 * again as three chips, and the screen-privacy switch, shown on while a lock holds it on. The locks use the phone's own biometric or screen-lock credential, so no
 * app PIN is stored; turning one on needs that credential to exist, and turning one off needs it
 * to be used.
 */
@Composable
internal fun SecurityPage(state: SettingsUiState, vm: SettingsViewModel, onBack: () -> Unit) {
    val context = LocalContext.current
    var noCredentialNote by remember { mutableStateOf(false) }

    // Disabling a protection requires a successful system credential, including gallery-only mode.
    fun toggle(want: Boolean, set: (Boolean) -> Unit) {
        when {
            !want -> authenticateSettingsAction(context, vm, "Confirm before turning off this lock") {
                set(false); noCredentialNote = false
            }
            BiometricAuthenticator.canAuthenticate(context) -> { set(true); noCredentialNote = false }
            else -> noCredentialNote = true
        }
    }

    SecurityContent(
        state = state,
        noCredentialNote = noCredentialNote,
        onAppLock = { toggle(it, vm::setAppLockEnabled) },
        onGalleryLock = { toggle(it, vm::setGalleryLockEnabled) },
        onTimeout = vm::setAppLockTimeoutSec,
        onPrivacyMode = vm::setPrivacyMode,
        onBack = onBack
    )
}

/** The page as drawn, without its view model or the credential prompt behind the switches. */
@Composable
internal fun SecurityContent(
    state: SettingsUiState,
    noCredentialNote: Boolean,
    onAppLock: (Boolean) -> Unit,
    onGalleryLock: (Boolean) -> Unit,
    onTimeout: (Int) -> Unit,
    onPrivacyMode: (Boolean) -> Unit,
    onBack: () -> Unit
) {
    val anyLock = state.appLockEnabled || state.galleryLockEnabled
    SettingsScaffold("Privacy & security", onBack) {
        Column {
            SettingsGroup("Lock") {
                SettingsSwitchRow("App lock", "Unlock to open Avex", state.appLockEnabled, onCheckedChange = onAppLock)
                SettingsSwitchRow("Photo gallery lock", "Unlock to see your progress photos", state.galleryLockEnabled, onCheckedChange = onGalleryLock)
            }
            // Three chips rather than a dropdown, as drawn. The grace only matters once something
            // locks, so with no lock on they stay shown, dimmed and inert.
            Spacer(Modifier.height(28.dp))
            OpenLabel("Lock again after you leave", style = OpenType.monoXS)
            Spacer(Modifier.height(12.dp))
            val i = AUTO_LOCK_OPTIONS.indexOfFirst { it.first == state.appLockTimeoutSec }.coerceAtLeast(0)
            Box(
                Modifier
                    .graphicsLayer { alpha = if (anyLock) 1f else 0.35f }
                    .then(if (anyLock) Modifier else Modifier.clearAndSetSemantics { })
            ) {
                SettingsSegmented(
                    options = AUTO_LOCK_OPTIONS.map { it.second },
                    selectedIndex = i,
                    onSelect = { if (anyLock) onTimeout(AUTO_LOCK_OPTIONS[it].first) },
                    contentDescription = "Lock again after you leave"
                )
            }
            if (noCredentialNote) {
                SettingsGroupFooter("Set a screen lock in your phone's settings first.", isError = true)
            } else {
                SettingsGroupFooter("Uses your phone's fingerprint, face or screen lock. Avex stores no PIN of its own.")
            }
        }

        Column {
            SettingsGroup("Screen") {
                if (anyLock) {
                    // A lock turns privacy mode on with it (MainActivity keeps FLAG_SECURE while any
                    // lock is on), so the switch shows on and cannot be turned off here.
                    SettingsRowContainer(Modifier.semantics(mergeDescendants = true) { stateDescription = "On" }) {
                        SettingsRowText("Privacy mode", "Hides Avex in recents, blocks screenshots")
                        SettingsSwitch(true)
                    }
                } else {
                    SettingsSwitchRow(
                        "Privacy mode",
                        "Hides Avex in recents, blocks screenshots",
                        state.privacyMode,
                        onCheckedChange = onPrivacyMode
                    )
                }
            }
            if (anyLock) {
                Spacer(Modifier.height(10.dp))
                Text("Always on while a lock is on.", style = OpenType.bodyS, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

/** Shared credential boundary for disabling protection and manually exporting protected photos. */
internal fun authenticateSettingsAction(
    context: android.content.Context,
    vm: SettingsViewModel,
    subtitle: String,
    onDenied: () -> Unit = {},
    action: () -> Unit
) {
    var host = context
    while (host is android.content.ContextWrapper && host !is androidx.fragment.app.FragmentActivity) {
        host = host.baseContext
    }
    val activity = host as? androidx.fragment.app.FragmentActivity ?: return onDenied()
    BiometricAuthenticator.authenticate(activity, subtitle,
        onSuccess = { vm.protectionAuthenticated(); action() },
        onError = { code, _ ->
            // A cancel is self-evident; anything else (lockout, credential removed) needs saying.
            if (code != androidx.biometric.BiometricPrompt.ERROR_USER_CANCELED &&
                code != androidx.biometric.BiometricPrompt.ERROR_NEGATIVE_BUTTON &&
                code != androidx.biometric.BiometricPrompt.ERROR_CANCELED
            ) vm.reportAuthDenied()
            onDenied()
        })
}
