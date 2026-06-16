package com.forge.app.ui.settings

import android.content.Intent
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.health.connect.client.PermissionController
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.theme.emphasized

/**
 * Settings → Recovery. Connects Health Connect so the coach can read sleep + resting heart rate
 * as recovery signals (finding #2/#3). Everything is opt-in and reversible — denying or never
 * connecting leaves the coach exactly as it is, reading only on-app signals.
 */
@Composable
internal fun RecoveryPage(modifier: Modifier = Modifier, viewModel: HealthConnectViewModel = hiltViewModel()) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant

    // Health Connect's own permission flow — the result tells us what the user granted.
    val permissionLauncher = rememberLauncherForActivityResult(
        contract = PermissionController.createRequestPermissionResultContract()
    ) { viewModel.refresh() }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 56.dp)
    ) {
        Column(Modifier.padding(start = 24.dp, end = 24.dp, top = 20.dp, bottom = 12.dp)) {
            Text("RECOVERY", style = MaterialTheme.typography.headlineSmall, color = emphasized(onBg))
            Text(
                "Let the coach read your sleep and resting heart rate",
                style = MaterialTheme.typography.labelSmall, color = muted
            )
        }
        SectionDivider()

        Paragraph(
            "Connect Health Connect and the coach can factor your recovery into its deload call — short " +
                "sleep or an elevated resting heart rate add to the fatigue score, alongside your effort, " +
                "moods and rest-day flags."
        )
        Paragraph(
            "Health Connect is Android's on-device hub. Apps like Samsung Health and your watch write to " +
                "it; Forge only reads sleep and resting heart rate from it. Nothing leaves your phone — Forge " +
                "still has no Internet permission."
        )
        Paragraph(
            "It's optional and additive: skip it and the coach works exactly as before. You can disconnect " +
                "any time from the Health Connect app."
        )

        SectionLabel("STATUS")
        when {
            state.loading -> StatusLine("Checking…", muted)
            state.needsUpdate -> {
                StatusLine("Health Connect needs an update.", muted)
                ActionButton("Update Health Connect") { openHealthConnectInStore(context) }
            }
            !state.available -> {
                StatusLine("Health Connect isn't installed on this device.", muted)
                ActionButton("Get Health Connect") { openHealthConnectInStore(context) }
            }
            state.granted -> {
                StatusLine("Connected — reading sleep & resting heart rate.", emphasized(onBg))
                ActionButton("Manage in Health Connect") { openHealthConnectSettings(context) }
            }
            else -> {
                StatusLine("Health Connect is available but not connected yet.", muted)
                ActionButton("Connect Health Connect") { permissionLauncher.launch(viewModel.permissions) }
            }
        }

        Spacer(Modifier.height(12.dp))
    }
}

@Composable
private fun Paragraph(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp)
    )
}

@Composable
private fun StatusLine(text: String, color: Color) {
    Text(
        text,
        style = MaterialTheme.typography.bodyMedium,
        color = color,
        fontStyle = FontStyle.Italic,
        modifier = Modifier.padding(horizontal = 24.dp, vertical = 4.dp)
    )
}

@Composable
private fun ActionButton(label: String, onClick: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    Button(
        onClick = onClick,
        shape = RoundedCornerShape(50),
        colors = ButtonDefaults.buttonColors(containerColor = onBg, contentColor = MaterialTheme.colorScheme.background),
        modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 12.dp)
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium)
    }
}

private const val HEALTH_CONNECT_PACKAGE = "com.google.android.apps.healthdata"

/** Open the Play store page for the Health Connect provider (web fallback if Play is absent). */
private fun openHealthConnectInStore(context: android.content.Context) {
    val market = Intent(Intent.ACTION_VIEW, Uri.parse("market://details?id=$HEALTH_CONNECT_PACKAGE"))
    val web = Intent(Intent.ACTION_VIEW, Uri.parse("https://play.google.com/store/apps/details?id=$HEALTH_CONNECT_PACKAGE"))
    runCatching { context.startActivity(market) }.recoverCatching { context.startActivity(web) }
}

/** Open the Health Connect app's management UI so the user can review or revoke access. */
private fun openHealthConnectSettings(context: android.content.Context) {
    runCatching {
        context.startActivity(Intent(androidx.health.connect.client.HealthConnectClient.ACTION_HEALTH_CONNECT_SETTINGS))
    }.recoverCatching {
        context.startActivity(context.packageManager.getLaunchIntentForPackage(HEALTH_CONNECT_PACKAGE)!!)
    }
}
