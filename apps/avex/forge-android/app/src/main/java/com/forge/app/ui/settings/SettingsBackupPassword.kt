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
    val onBg = MaterialTheme.colorScheme.onBackground

    fun guarded(reason: String, action: () -> Unit) =
        if (galleryLocked) authenticateSettingsAction(context, vm, reason) { action() } else action()

    SettingsSectionHeader("Password")
    ToggleRow(
        label = "Password-protect backups",
        subtitle = when (state) {
            BackupPasswordState.NEEDS_RESET -> "Set it again to keep backing up."
            else -> "Encrypts backups so only your password opens them."
        },
        checked = state != BackupPasswordState.OFF,
        onCheckedChange = { on -> if (on) askNew = true else confirmOff = true }
    )
    if (state != BackupPasswordState.OFF) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickableLabeled("Change backup password") { guarded("Unlock to change the backup password") { askNew = true } }
                .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text("Backup password", style = MaterialTheme.typography.bodyMedium, color = onBg)
                SettingsExplainer(
                    if (state == BackupPasswordState.NEEDS_RESET) "This phone lost its copy of the key"
                    else "Older backups keep their old password"
                )
            }
            ConnectPill(if (state == BackupPasswordState.NEEDS_RESET) "Set" else "Change")
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
        AlertDialog(
            containerColor = MaterialTheme.colorScheme.surface,
            onDismissRequest = { confirmOff = false },
            text = {
                Text(
                    "New backups will be saved without a password, so anyone with the file can open " +
                        "them. Backups you already made still need their password."
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    confirmOff = false
                    guarded("Unlock to remove the backup password") { vm.clearBackupPassword() }
                }) { Text("Turn off", color = MaterialTheme.colorScheme.onBackground) }
            },
            dismissButton = {
                TextButton(onClick = { confirmOff = false }) {
                    Text("Cancel", color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
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
    AlertDialog(
        containerColor = MaterialTheme.colorScheme.surface,
        onDismissRequest = onDismiss,
        text = {
            Column {
                Text(
                    "If you forget this password, backups made with it can't be opened. " +
                        "Avex can't recover it for you."
                )
                Spacer(Modifier.height(16.dp))
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
        },
        confirmButton = {
            TextButton(onClick = { onSubmit(first.toCharArray()); first = ""; second = "" }, enabled = canSave) {
                Text(
                    if (busy) "Saving" else "Save",
                    color = MaterialTheme.colorScheme.onBackground.copy(alpha = if (canSave) 1f else 0.35f)
                )
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancel", color = MaterialTheme.colorScheme.onSurfaceVariant) }
        }
    )
}

/** Hosted at the Settings root: a restore found a protected backup and needs its password. */
@Composable
internal fun RestorePasswordDialogHost(vm: SettingsViewModel) {
    val prompt by vm.restorePasswordPrompt.collectAsStateWithLifecycle()
    val p = prompt ?: return
    var typed by remember(p.source) { mutableStateOf("") }
    val canOpen = typed.isNotEmpty() && !p.busy
    AlertDialog(
        containerColor = MaterialTheme.colorScheme.surface,
        onDismissRequest = { if (!p.busy) vm.dismissRestorePassword() },
        text = {
            Column {
                Text("This backup is password-protected. Enter the password it was made with.")
                Spacer(Modifier.height(16.dp))
                PasswordField(
                    value = typed,
                    onValueChange = { typed = it },
                    label = "Backup password",
                    isError = p.wrong,
                    supporting = if (p.wrong) "That password doesn't open this backup" else null,
                    imeAction = ImeAction.Done
                )
            }
        },
        confirmButton = {
            TextButton(onClick = { vm.submitRestorePassword(typed.toCharArray()); typed = "" }, enabled = canOpen) {
                Text(
                    if (p.busy) "Opening" else "Restore & restart",
                    color = MaterialTheme.colorScheme.error.copy(alpha = if (canOpen) 1f else 0.35f)
                )
            }
        },
        dismissButton = {
            TextButton(onClick = { vm.dismissRestorePassword() }, enabled = !p.busy) {
                Text("Cancel", color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    )
}
