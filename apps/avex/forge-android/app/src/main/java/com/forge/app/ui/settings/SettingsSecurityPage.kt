package com.forge.app.ui.settings

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import com.forge.app.security.BiometricAuthenticator

/** The auto-lock grace options (seconds) offered on the Security page — 0 means "immediately". */
private val AUTO_LOCK_OPTIONS = listOf(
    0 to "Immediately",
    60 to "After 1 minute",
    300 to "After 5 minutes"
)

/**
 * Privacy & security (GYMAP-69): the app and photo-gallery locks, when they lock again, and the
 * screen-privacy switch. The locks use the phone's own biometric or screen-lock credential, so no
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

    SettingsScaffold("Privacy & security", onBack) {
        SettingsGroup(
            "Lock",
            footer = if (noCredentialNote) "Set a screen lock in your phone's settings first."
                     else "Uses your phone's fingerprint, face or screen lock. Avex stores no PIN of its own.",
            footerIsError = noCredentialNote
        ) {
            SettingsSwitchRow("App lock", "Unlock to open Avex", state.appLockEnabled) { toggle(it, vm::setAppLockEnabled) }
            SettingsSwitchRow("Photo gallery lock", "Unlock to see your progress photos", state.galleryLockEnabled) {
                toggle(it, vm::setGalleryLockEnabled)
            }
            // The grace period only matters once something locks.
            if (state.appLockEnabled || state.galleryLockEnabled) {
                val i = AUTO_LOCK_OPTIONS.indexOfFirst { it.first == state.appLockTimeoutSec }.coerceAtLeast(0)
                SettingsDropdownRow(
                    title = "Lock again",
                    value = AUTO_LOCK_OPTIONS[i].second,
                    options = AUTO_LOCK_OPTIONS.map { it.second },
                    selectedIndex = i,
                    supporting = "After you leave the app"
                ) { vm.setAppLockTimeoutSec(AUTO_LOCK_OPTIONS[it].first) }
            }
        }

        SettingsGroup("Screen") {
            SettingsSwitchRow(
                "Privacy mode",
                "Hides Avex in recent apps and blocks screenshots",
                state.privacyMode,
                onCheckedChange = vm::setPrivacyMode
            )
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
        onSuccess = { vm.protectionAuthenticated(); action() }, onError = { _, _ -> onDenied() })
}
