package com.forge.app.ui.settings

import android.content.Intent
import android.net.Uri
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.clickableLabeled

/**
 * Building blocks for the Watch & health page ([RecoveryPage]): the indented rows that hang off a
 * reading that is on, and the ways out to Health Connect and the Play listing.
 */

/** Where a reading's own options start: a step in from its name, so they read as its children. */
internal val SignalDetailIndent = 16.dp

/**
 * One thing a reading that is on can do now, indented under it: an import, or allowing write-back.
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

/** The device chip's short label; the note under it names the companion app in full. */
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

/** Avex's own Play listing, which offers the Wear OS app's install on a paired watch. */
internal fun openAvexInStore(context: android.content.Context) {
    val pkg = context.packageName.removeSuffix(".debug")
    val market = Intent(Intent.ACTION_VIEW, Uri.parse("market://details?id=$pkg"))
    val web = Intent(Intent.ACTION_VIEW, Uri.parse("https://play.google.com/store/apps/details?id=$pkg"))
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
