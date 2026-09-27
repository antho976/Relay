package com.forge.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Key
import androidx.compose.material.icons.rounded.Lock
import androidx.compose.material.icons.rounded.LockOpen
import com.forge.app.security.BackupEncryption
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.window.AlertDialog
import com.forge.app.ui.settings.SettingsViewModel.BackupPasswordState

/**
 * The Backup page's password block: a toggle, and once it is on, a row to change the password in
 * the same whole-row + drawn-pill shape as the folder row (§8). Turning it off is a downgrade, so it
 * confirms, and sits behind the gallery lock like every other path that can widen who reads a
 * backup.
 */
@Composable
internal fun BackupPasswordSection(vm: SettingsViewModel, galleryLocked: Boolean) {
    val context = androidx.compose.ui.platform.LocalContext.current
    val state by vm.backupPassword.collectAsStateWithLifecycle()
    val busy by vm.backupPasswordBusy.collectAsStateWithLifecycle()
    var askNew by rememberSaveable { mutableStateOf(false) }
    var confirmOff by rememberSaveable { mutableStateOf(false) }
    fun guarded(reason: String, action: () -> Unit) =
        if (galleryLocked) authenticateSettingsAction(context, vm, reason) { action() } else action()

    SettingsGroup("Password") {
        SettingsSwitchRow(
            "Password-protect backups",
            when (state) {
                BackupPasswordState.NEEDS_RESET -> "Set it again to keep backing up"
                else -> "Encrypts backups so only your password opens them"
            },
            checked = state != BackupPasswordState.OFF,
            onCheckedChange = { on -> if (on) askNew = true else confirmOff = true }
        )
        if (state != BackupPasswordState.OFF) {
            SettingsAdaptiveRow(
                title = "Backup password",
                supporting = if (state == BackupPasswordState.NEEDS_RESET) "This phone lost its copy of the key"
                             else "Older backups keep their old password",
                interaction = Modifier.clickableLabeled("Change backup password") {
                    guarded("Unlock to change the backup password") { askNew = true }
                },
                leading = { SettingsIconTile(Icons.Rounded.Key, if (state == BackupPasswordState.NEEDS_RESET) TileTone.Danger else TileTone.Neutral) }
            ) { SettingsCompactButton(if (state == BackupPasswordState.NEEDS_RESET) "Set" else "Change") }
        }
    }

    if (askNew) {
        NewBackupPasswordDialog(
            busy = busy,
            onSubmit = { pw -> vm.setBackupPassword(pw); askNew = false },
            onDismiss = { askNew = false }
        )
    }
    if (confirmOff) {
        SettingsConfirmDialog(
            title = "Turn off the backup password?",
            body = "New backups will be saved without a password, so anyone with the file can open them. " +
                "Backups you already made still need their password.",
            confirmLabel = "Turn off",
            icon = Icons.Rounded.LockOpen,
            onConfirm = {
                confirmOff = false
                guarded("Unlock to remove the backup password") { vm.clearBackupPassword() }
            },
            onDismiss = { confirmOff = false }
        )
    }
}

/** A password field: masked, no suggestions or autocorrect, so the keyboard never learns it. */
@Composable
private fun PasswordField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    isError: Boolean = false,
    supporting: String? = null,
    imeAction: ImeAction = ImeAction.Next
) {
    OutlinedTextField(
        value = value,
        onValueChange = { if (it.length <= BackupEncryption.MAX_PASSWORD_LENGTH) onValueChange(it) },
        singleLine = true,
        label = { Text(label) },
        isError = isError,
        supportingText = supporting?.let { { Text(it, color = MaterialTheme.colorScheme.onSurfaceVariant) } },
        visualTransformation = PasswordVisualTransformation(),
        colors = settingsFieldColors(),
        shape = androidx.compose.foundation.shape.RoundedCornerShape(12.dp),
        keyboardOptions = KeyboardOptions(
            keyboardType = KeyboardType.Password,
            autoCorrectEnabled = false,
            imeAction = imeAction
        ),
        modifier = Modifier.fillMaxWidth()
    )
}

/**
 * Choose a backup password. NIST SP 800-63B rules: a length floor, no composition rules, long
 * passphrases welcome, typed twice because a typo here locks the user out of their own backups.
 */
@Composable
private fun NewBackupPasswordDialog(busy: Boolean, onSubmit: (CharArray) -> Unit, onDismiss: () -> Unit) {
    // Deliberately not rememberSaveable: a password must not be written into the saved-state bundle.
    var first by remember { mutableStateOf("") }
    var second by remember { mutableStateOf("") }
    val min = BackupEncryption.MIN_PASSWORD_LENGTH
    val longEnough = first.length >= min
    val matches = first == second
    val canSave = longEnough && matches && !busy
    SettingsConfirmDialog(
        title = "Set a backup password",
        body = "If you forget it, backups made with it can't be opened. Avex can't recover it for you.",
        confirmLabel = if (busy) "Saving" else "Save password",
        icon = Icons.Rounded.Key,
        destructive = false,
        confirmEnabled = canSave,
        onConfirm = { onSubmit(first.toCharArray()); first = ""; second = "" },
        onDismiss = onDismiss
    ) {
        Column {
            PasswordField(
                value = first,
                onValueChange = { first = it },
                label = "Password",
                supporting = if (first.isNotEmpty() && !longEnough) "At least $min characters" else null
            )
            Spacer(Modifier.height(8.dp))
            PasswordField(
                value = second,
                onValueChange = { second = it },
                label = "Type it again",
                isError = second.isNotEmpty() && !matches,
                supporting = if (second.isNotEmpty() && !matches) "The two don't match" else null,
                imeAction = ImeAction.Done
            )
        }
    }
}

/** Hosted at the Settings root: a restore found a protected backup and needs its password. */
@Composable
internal fun RestorePasswordDialogHost(vm: SettingsViewModel) {
    val prompt by vm.restorePasswordPrompt.collectAsStateWithLifecycle()
    val p = prompt ?: return
    var typed by remember(p.source) { mutableStateOf("") }
    val canOpen = typed.isNotEmpty() && !p.busy
    SettingsConfirmDialog(
        title = "Enter the backup password",
        body = "This backup is password-protected. Enter the password it was made with.",
        confirmLabel = if (p.busy) "Opening" else "Restore and restart",
        icon = Icons.Rounded.Lock,
        confirmEnabled = canOpen,
        onConfirm = { vm.submitRestorePassword(typed.toCharArray()); typed = "" },
        onDismiss = { if (!p.busy) vm.dismissRestorePassword() }
    ) {
        PasswordField(
            value = typed,
            onValueChange = { typed = it },
            label = "Backup password",
            isError = p.wrong,
            supporting = if (p.wrong) "That password doesn't open this backup" else null,
            imeAction = ImeAction.Done
        )
    }
}
