package com.forge.app.ui.security

import androidx.compose.ui.input.pointer.pointerInput
import androidx.biometric.BiometricPrompt
import androidx.activity.compose.BackHandler
import androidx.activity.compose.LocalActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.fragment.app.FragmentActivity
import com.forge.app.appicon.AppIcon
import com.forge.app.appicon.AppIconManager
import com.forge.app.security.BiometricAuthenticator
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.settings.AppIconPreviewImage
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import com.forge.app.ui.theme.SettingsType
import com.forge.app.ui.theme.forgeBackgroundGradient
import com.forge.app.ui.theme.openOnAccentLarge
import kotlinx.coroutines.delay

/**
 * The full-screen unlock gate (GYMAP-69) for both the app lock and the gallery lock, drawn as the
 * redesign's lock screen (Relay designs lk1 to lk3): an OPAQUE theme-gradient ground over whatever it
 * covers, your app icon, the serif "Locked", one line, and the full-width red Unlock at the bottom
 * where the thumb is, which re-invokes the system prompt. The credential entry itself is the OS
 * [BiometricPrompt] sheet; Avex keeps no PIN. It carries no chrome at all: the notifications bell
 * every other screen shows would be an entry point into data this gate exists to withhold.
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
    /** The icon to show; null reads the launcher alias that is enabled now. */
    iconKey: String? = null,
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

    // The app gate covers a nav host that stays composed, and taps stop here but system Back did
    // not: it reached the screens beneath, popping Settings pages (and with them an in-flight
    // restore) or opening Program Builder's discard dialog. Back from the gate leaves the app, as it
    // would from any root screen. The gallery gate REPLACES its screen, so its Back is the nav
    // host's ordinary pop and is left alone.
    BackHandler(enabled = onCancel == null) { activity?.moveTaskToBack(true) }

    // The icon on the home screen right now: the enabled launcher alias. The swap to a new pick lands
    // when the app is backgrounded, which is exactly when this gate comes up, so it is the one the
    // user just tapped.
    val context = androidx.compose.ui.platform.LocalContext.current
    val icon = remember(iconKey) {
        iconKey?.let { AppIcon.fromKey(it) }
            ?: runCatching { AppIconManager(context.applicationContext).currentIcon() }.getOrDefault(AppIcon.Default)
    }

    AppLockContent(
        icon = icon,
        // The app gate names the app; the gallery gate (it has a way back) names the photos.
        caption = if (onCancel == null) "Unlock to open Avex" else "Unlock to see your progress photos",
        note = note,
        gradient = Brush.verticalGradient(listOf(gradTop, gradBottom)),
        onUnlock = launchPrompt
    )
}

/**
 * The gate as drawn (Relay designs lk1 and lk3), stateless so a golden can hold it still: your app
 * icon, the serif "Locked", one line, and, when unlocking has paused, a plain line with a small red
 * dot (no red text). The full-width red Unlock sits at the bottom where the thumb is, under the one
 * caption that says what unlocks it.
 */
@Composable
internal fun AppLockContent(
    icon: AppIcon,
    caption: String,
    note: String?,
    gradient: Brush,
    onUnlock: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(gradient)
            // Swallow taps so nothing behind the opaque gate is reachable — WITHOUT becoming a
            // control. `clickable` adds a click action to the semantics tree, so the gate announced
            // itself as an unnamed button wrapping the whole screen, sitting between TalkBack and
            // the one thing on it that matters. pointerInput consumes the same events and says
            // nothing about itself, which is the truth: it is a barrier, not a button.
            .pointerInput(Unit) { awaitPointerEventScope { while (true) { awaitPointerEvent() } } }
            .windowInsetsPadding(WindowInsets.systemBars)
            .padding(horizontal = 24.dp),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Column(
            Modifier.weight(1f).fillMaxWidth(),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center
        ) {
            AppIconPreviewImage(icon.previewRes, Modifier.size(72.dp).clip(CircleShape))
            Spacer(Modifier.height(28.dp))
            Text(
                "Locked",
                style = OpenType.title,
                color = MaterialTheme.colorScheme.onBackground,
                modifier = Modifier.semantics { heading() }
            )
            Spacer(Modifier.height(10.dp))
            Text(
                caption,
                style = SettingsType.lockCaption,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center
            )
            if (note != null) {
                Spacer(Modifier.height(24.dp))
                Row(
                    verticalAlignment = Alignment.Top,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite }
                ) {
                    Box(
                        Modifier
                            .padding(top = 7.dp)
                            .size(6.dp)
                            .clip(CircleShape)
                            .background(MaterialTheme.colorScheme.primary)
                    )
                    Text(note, style = OpenType.body, color = MaterialTheme.colorScheme.onBackground)
                }
            }
        }
        Column(
            Modifier.fillMaxWidth().padding(top = 24.dp, bottom = 36.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(14.dp)
        ) {
            Text(
                "Fingerprint, face or phone PIN".uppercase(),
                style = OpenType.monoXS,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center
            )
            UnlockButton(onUnlock)
        }
    }
}

/** The full-width red Unlock: 56dp, 4dp corners, the fingerprint glyph and the word, centred. */
@Composable
private fun UnlockButton(onClick: () -> Unit) {
    val ink = openOnAccentLarge()
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .bounceClick(role = Role.Button, onClick = onClick)
            .semantics { contentDescription = "Unlock" }
            .clip(RoundedCornerShape(4.dp))
            .background(MaterialTheme.colorScheme.primary),
        horizontalArrangement = Arrangement.spacedBy(10.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(OpenIcons.Fingerprint, contentDescription = null, tint = ink, modifier = Modifier.size(20.dp))
        Text("Unlock", style = OpenType.button, color = ink)
    }
}
