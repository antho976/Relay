package com.forge.app.ui.settings

import android.content.Intent
import android.net.Uri
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.clickableLabeled

/**
 * Building blocks for the Wearable page ([RecoveryPage]): one row per Health Connect signal, and
 * the indented rows that hang off a connected one.
 */

/** Where a signal's own options start: under its title, past the 40dp tile and its gap. */
internal val SignalDetailIndent = 56.dp

/**
 * One Health Connect signal: its glyph, title and what it feeds, and at its end either its state
 * (Receiving; On for a write-only signal; Nothing yet when granted but silent, which usually means
 * the companion app's sharing is off) or, when it can be connected, a Connect button the whole row
 * answers to. Rows stay passive while loading or without a provider, so nothing offers an action
 * that can't run.
 */
@Composable
internal fun RecoveryRow(
    title: String,
    explainer: String,
    icon: ImageVector,
    connected: Boolean,
    connectable: Boolean,
    onConnect: () -> Unit,
    receiving: Boolean? = null
) {
    SettingsAdaptiveRow(
        title = title,
        supporting = explainer,
        interaction = if (!connected && connectable) Modifier.clickableLabeled("Connect $title", onClick = onConnect) else Modifier,
        leading = { SettingsIconTile(icon, if (connected) TileTone.Accent else TileTone.Neutral) }
    ) {
        when {
            connected -> when (receiving) {
                false -> SettingsStatus("Nothing yet", live = false)
                true -> SettingsStatus("Receiving", live = true)
                null -> SettingsStatus("On", live = true)
            }
            connectable -> SettingsCompactButton("Connect")
        }
    }
}

/**
 * One thing a connected signal can do now, indented under it: an import, or allowing write-back.
 * Its result ("Imported 82.4 kg from today") replaces its description, so the answer lands where
 * the question was asked.
 */
@Composable
internal fun SignalAction(title: String, description: String, button: String, onClick: () -> Unit) {
    SettingsAdaptiveRow(
        title = title,
        supporting = description,
        indent = SignalDetailIndent,
        interaction = Modifier.clickableLabeled("$button: $title", onClick = onClick)
    ) { SettingsCompactButton(button) }
}

/** The device segment's short label; the explainer under it names the companion app in full. */
internal fun brandShortLabel(brand: com.forge.app.domain.health.WearableBrand): String = when (brand) {
    com.forge.app.domain.health.WearableBrand.GALAXY -> "Galaxy"
    com.forge.app.domain.health.WearableBrand.PIXEL -> "Pixel"
    com.forge.app.domain.health.WearableBrand.NONE -> "Other"
}

private const val HEALTH_CONNECT_PACKAGE = "com.google.android.apps.healthdata"

/** Open the Play store page for the Health Connect provider (web fallback if Play is absent). */
internal fun openHealthConnectInStore(context: android.content.Context) {
    val market = Intent(Intent.ACTION_VIEW, Uri.parse("market://details?id=$HEALTH_CONNECT_PACKAGE"))
    val web = Intent(Intent.ACTION_VIEW, Uri.parse("https://play.google.com/store/apps/details?id=$HEALTH_CONNECT_PACKAGE"))
    runCatching { context.startActivity(market) }.recoverCatching { context.startActivity(web) }
}

/** Open the Health Connect app's management UI so the user can review or revoke access. */
internal fun openHealthConnectSettings(context: android.content.Context) {
    runCatching {
        context.startActivity(Intent(androidx.health.connect.client.HealthConnectClient.ACTION_HEALTH_CONNECT_SETTINGS))
    }.recoverCatching {
        context.startActivity(context.packageManager.getLaunchIntentForPackage(HEALTH_CONNECT_PACKAGE)!!)
    }
}
