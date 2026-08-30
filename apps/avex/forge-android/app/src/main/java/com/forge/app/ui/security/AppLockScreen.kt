package com.forge.app.ui.security

import androidx.compose.ui.input.pointer.pointerInput
import androidx.biometric.BiometricPrompt
import androidx.activity.compose.LocalActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.fragment.app.FragmentActivity
import com.forge.app.security.BiometricAuthenticator
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.forgeBackgroundGradient
import kotlinx.coroutines.delay

/**
 * The full-screen unlock gate (GYMAP-69) for both the app lock and the gallery lock. It is the modal
 * archetype (DESIGN §3): an OPAQUE theme-gradient scrim over whatever it covers, one serif "Locked"
 * line (§11, no terminal period), one ≤12-word caption, and a single filled capsule that re-invokes
 * the system prompt. The credential entry itself is the OS [BiometricPrompt] sheet, so this screen
 * stays deliberately minimal — no icons, no boxes. It carries no chrome at all: the notifications
 * bell every other screen shows would be an entry point into data this gate exists to withhold.
 *
 * The prompt auto-invokes once [promptReady] is true (the caller withholds it while the launch intro
 * plays). If the device has no enrolled credential we FAIL OPEN — a user must never be trapped out of
 * their own data by a lock they can no longer satisfy.
 *
 * @param subtitle the one-line context shown on the system sheet ("Unlock Avex" is the fixed title).
 * @param onUnlocked called on a successful (or fail-open) unlock.
 * @param onCancel if non-null, called when the user dismisses the sheet (gallery gate → go back);
 *   null keeps the user on this screen (app gate → they tap Unlock or leave).
 */
@Composable
fun AppLockScreen(
    subtitle: String,
    promptReady: Boolean,
    onUnlocked: () -> Unit,
    onCancel: (() -> Unit)? = null,
) {
    val activity = LocalActivity.current as? FragmentActivity
    val amoled = LocalForgeSettings.current.amoledMode
    val (gradTop, gradBottom) = forgeBackgroundGradient(amoled)

    val initialAvailability = remember(activity) {
        activity?.let(BiometricAuthenticator::availability)
            ?: BiometricAuthenticator.Availability.UNAVAILABLE
    }
    var note by remember { mutableStateOf<String?>(null) }
    var hasAutoPrompted by remember { mutableStateOf(false) }

    val launchPrompt: () -> Unit = launchPrompt@{
        val act = activity
        if (act == null) {
            note = "Unlock isn't available right now. Try again."
            return@launchPrompt
        }
        when (BiometricAuthenticator.availability(act)) {
            BiometricAuthenticator.Availability.NO_CREDENTIAL -> {
                onUnlocked()
                return@launchPrompt
            }
            BiometricAuthenticator.Availability.UNAVAILABLE -> {
                note = "Unlock isn't available right now. Try again."
                return@launchPrompt
            }
            BiometricAuthenticator.Availability.AVAILABLE -> Unit
        }
        note = null
        BiometricAuthenticator.authenticate(
            activity = act,
            subtitle = subtitle,
            onSuccess = onUnlocked,
            onError = { code, message ->
                when {
                    // A removed credential cannot be satisfied, so do not trap the user.
                    BiometricAuthenticator.availability(act) ==
                        BiometricAuthenticator.Availability.NO_CREDENTIAL -> onUnlocked()
                    code == BiometricPrompt.ERROR_USER_CANCELED ||
                        code == BiometricPrompt.ERROR_NEGATIVE_BUTTON ||
                        code == BiometricPrompt.ERROR_CANCELED -> onCancel?.invoke()
                    code == BiometricPrompt.ERROR_LOCKOUT ||
                        code == BiometricPrompt.ERROR_LOCKOUT_PERMANENT ->
                        note = "Too many attempts. Try again in a moment."
                    else -> note = message.toString().ifBlank { "Couldn't verify. Try again." }
                }
            }
        )
    }

    LaunchedEffect(initialAvailability) {
        when (initialAvailability) {
            BiometricAuthenticator.Availability.NO_CREDENTIAL -> onUnlocked()
            BiometricAuthenticator.Availability.UNAVAILABLE ->
                note = "Unlock isn't available right now. Try again."
            BiometricAuthenticator.Availability.AVAILABLE -> Unit
        }
    }

    // Auto-invoke the prompt once, when ready (the intro has finished).
    LaunchedEffect(initialAvailability, promptReady) {
        if (initialAvailability == BiometricAuthenticator.Availability.AVAILABLE &&
            promptReady && !hasAutoPrompted
        ) {
            hasAutoPrompted = true
            delay(150) // let the activity settle to RESUMED before showing the system sheet
            launchPrompt()
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(Brush.verticalGradient(listOf(gradTop, gradBottom)))
            // Swallow taps so nothing behind the opaque gate is reachable — WITHOUT becoming a
            // control. `clickable` adds a click action to the semantics tree, so the gate announced
            // itself as an unnamed button wrapping the whole screen, sitting between TalkBack and
            // the one thing on it that matters. pointerInput consumes the same events and says
            // nothing about itself, which is the truth: it is a barrier, not a button.
            .pointerInput(Unit) { awaitPointerEventScope { while (true) { awaitPointerEvent() } } }
            .padding(horizontal = 24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center
    ) {
        Text(
            "Locked",
            style = MaterialTheme.typography.headlineSmall,
            color = MaterialTheme.colorScheme.onBackground
        )
        Spacer(Modifier.height(10.dp))
        Text(
            subtitle,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
            textAlign = TextAlign.Center
        )
        Spacer(Modifier.height(28.dp))
        ForgePrimaryCapsule("Unlock", onClick = launchPrompt, modifier = Modifier.widthIn(min = 200.dp))
        note?.let {
            Spacer(Modifier.height(14.dp))
            Text(
                it,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error,
                textAlign = TextAlign.Center
            )
        }
    }
}
