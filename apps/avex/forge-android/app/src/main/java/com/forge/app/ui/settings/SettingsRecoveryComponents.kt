package com.forge.app.ui.settings

import android.content.Intent
import android.net.Uri
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.ForgeSwitch
import com.forge.app.ui.common.clickableLabeled

/**
 * Building blocks for the Recovery page ([RecoveryPage]). One [RecoveryCard] per Health Connect
 * integration keeps each one self-contained and roomy, while the collapsed "Why this?" detail keeps
 * the overall page short.
 */

/**
 * One Health Connect integration: an open section with a title + status pill, a body (summary,
 * action, any toggles) and a "Why this?" expander that reveals [details] on demand. Sits directly
 * on the page background — no card shell.
 */
@Composable
internal fun RecoveryCard(
    title: String,
    statusLabel: String,
    connected: Boolean,
    details: String,
    body: @Composable ColumnScope.() -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    var expanded by rememberSaveable(title) { mutableStateOf(false) }
    HorizontalDivider(
        modifier = Modifier.padding(horizontal = 24.dp),
        color = outline.copy(alpha = 0.25f)
    )
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp, vertical = 16.dp)
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Text(title, style = MaterialTheme.typography.titleSmall, color = onBg, modifier = Modifier.weight(1f))
            StatusPill(statusLabel, connected)
        }
        Spacer(Modifier.height(10.dp))
        body()
        Spacer(Modifier.height(12.dp))
        Row(
            modifier = Modifier.clickableLabeled(if (expanded) "Hide details" else "Show details") {
                expanded = !expanded
            },
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                if (expanded) "Hide details" else "Why this?",
                style = MaterialTheme.typography.labelSmall, color = muted, letterSpacing = 0.5.sp
            )
            Text(if (expanded) "  ▴" else "  ▾", style = MaterialTheme.typography.labelSmall, color = muted)
        }
        AnimatedVisibility(visible = expanded) {
            Text(
                details,
                style = MaterialTheme.typography.bodySmall, color = muted,
                modifier = Modifier.padding(top = 10.dp)
            )
        }
    }
}

/** Small connection indicator: a filled dot + label — passive status, no border shell. */
@Composable
private fun StatusPill(label: String, connected: Boolean) {
    val color = if (connected) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        Spacer(Modifier.size(7.dp).background(color.copy(alpha = if (connected) 1f else 0.5f), CircleShape))
        Text(label.uppercase(), style = MaterialTheme.typography.labelSmall, color = color, letterSpacing = 0.5.sp)
    }
}

/** Muted body line inside a card (summary or a status message). */
@Composable
internal fun CardText(text: String) {
    Text(text, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

/** Full-width pill action button sized for inside a card (the card supplies the outer padding). */
@Composable
internal fun CardButton(label: String, onClick: () -> Unit) {
    Button(
        onClick = onClick,
        shape = RoundedCornerShape(50),
        colors = ButtonDefaults.buttonColors(
            containerColor = MaterialTheme.colorScheme.onBackground,
            contentColor = MaterialTheme.colorScheme.background
        ),
        modifier = Modifier.fillMaxWidth().padding(top = 8.dp)
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium)
    }
}

/** Compact label + switch row for the write-back toggles inside a card. */
@Composable
internal fun CardToggleRow(label: String, checked: Boolean, onCheckedChange: (Boolean) -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text(
            label,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onBackground,
            modifier = Modifier.weight(1f)
        )
        ForgeSwitch(checked = checked, onCheckedChange = onCheckedChange)
    }
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
