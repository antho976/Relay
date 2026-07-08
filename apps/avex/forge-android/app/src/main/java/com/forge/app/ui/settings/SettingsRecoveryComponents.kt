package com.forge.app.ui.settings

import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.ForgeSwitch
import com.forge.app.ui.common.clickableLabeled

/**
 * Building blocks for the Recovery page ([RecoveryPage]). The page leads with a connection rail —
 * the §12 filled/hollow dot row that shows the whole state at a glance and is honest at zero — and
 * each integration is one quiet row: title + explainer on the left, its state on the right (a
 * passive accent `• ON`, or a mono accent `connect →` that the whole row taps). No per-row capsule
 * buttons: a capsule repeated down a list is a wall of buttons (DESIGN §8); the only capsule is the
 * page-end Get/Update Health Connect action when no provider exists.
 */

/**
 * The page-lead mark: one dot per integration in row order — a solid accent disc when connected, a
 * clearly-drawn ring (muted, 1.5dp) when not — with a mono running count. Reads honestly at zero
 * ("0 OF 5 CONNECTED"); the ring weight keeps the empty state legible on the near-black page.
 */
@Composable
internal fun RecoveryConnectionRail(states: List<Boolean>) {
    val accent = MaterialTheme.colorScheme.primary
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = Modifier.padding(horizontal = 24.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        states.forEach { on ->
            if (on) Box(Modifier.size(10.dp).background(accent, CircleShape))
            else Box(Modifier.size(10.dp).border(1.5.dp, muted.copy(alpha = 0.55f), CircleShape))
        }
        Spacer(Modifier.width(6.dp))
        Text(
            "${states.count { it }} OF ${states.size} CONNECTED",
            style = MaterialTheme.typography.labelMedium,
            color = muted,
            letterSpacing = 1.sp
        )
    }
}

/**
 * One Health Connect integration as a quiet row: title + one-line explainer, and on the right its
 * state — connected = passive accent dot + mono ON; connectable = a compact OUTLINED "Connect" pill
 * (a clear affordance without the filled-white weight, §8 ②), the WHOLE row as its tap target. The
 * pill is drawn, not independently clickable, so it never nests inside the row's tap. Rows render
 * passive while loading or without a provider, so nothing promises an action that can't run.
 */
@Composable
internal fun RecoveryRow(
    title: String,
    explainer: String,
    connected: Boolean,
    connectable: Boolean,
    onConnect: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .then(
                if (!connected && connectable) Modifier.clickableLabeled("Connect $title", onClick = onConnect)
                else Modifier
            )
            .padding(horizontal = 24.dp, vertical = 14.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Text(title, style = MaterialTheme.typography.titleSmall, color = onBg)
            Text(explainer, style = MaterialTheme.typography.bodySmall, color = muted)
        }
        when {
            connected -> Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                Box(Modifier.size(7.dp).background(accent, CircleShape))
                Text("ON", style = MaterialTheme.typography.labelMedium, color = onBg, letterSpacing = 1.sp)
            }
            connectable -> Box(
                modifier = Modifier
                    .border(1.dp, outline.copy(alpha = 0.35f), RoundedCornerShape(50))
                    .padding(horizontal = 16.dp, vertical = 8.dp),
                contentAlignment = Alignment.Center
            ) {
                Text("Connect", style = MaterialTheme.typography.labelMedium, color = onBg, letterSpacing = 0.3.sp)
            }
        }
    }
}

/** Compact label + switch row for a connected integration's write-back toggle (no subtitle line). */
@Composable
internal fun RecoveryToggleRow(label: String, checked: Boolean, onCheckedChange: (Boolean) -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val bg = MaterialTheme.colorScheme.background
    val outline = MaterialTheme.colorScheme.outline
    Row(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Text(
            label,
            style = MaterialTheme.typography.bodyMedium,
            color = onBg,
            modifier = Modifier.weight(1f)
        )
        ForgeSwitch(
            checked = checked,
            onCheckedChange = onCheckedChange,
            checkedTrackColor = onBg,
            checkedThumbColor = bg,
            checkedBorderColor = Color.Transparent,
            uncheckedTrackColor = Color.Transparent,
            uncheckedThumbColor = outline,
            uncheckedBorderColor = outline.copy(alpha = 0.35f)
        )
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
