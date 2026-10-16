package com.forge.app.ui.overview

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.experiment.SectionAnchor
import com.forge.app.ui.settings.SettingsIcons

/**
 * Home's BODY section: the watch's overnight and today readings, or the invite to connect one.
 *
 * Readings sit in one tappable panel that opens the full Body view on Stats. The invite is one row
 * that opens Settings → Wearable, with a quiet "not now" that hides it for good (Settings brings it
 * back), so a user without a watch is asked once rather than forever.
 */
@Composable
internal fun HomeBodySection(
    body: HomeBody,
    onOpenBody: () -> Unit,
    onConnectWatch: () -> Unit,
    onDismissInvite: () -> Unit
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    when (body) {
        HomeBody.Invite -> {
            SectionAnchor(
                label = "Body",
                muted = muted,
                onBg = onBg,
                secondaryAction = "not now",
                secondaryActionLabel = "Hide the watch prompt",
                onSecondaryAction = onDismissInvite
            )
            Spacer(Modifier.height(4.dp))
            ConnectWatchRow(onConnectWatch)
        }
        is HomeBody.Readings -> {
            SectionAnchor(
                label = "Body",
                muted = muted,
                onBg = onBg,
                action = "details",
                actionLabel = "Open body details",
                onAction = onOpenBody
            )
            Spacer(Modifier.height(4.dp))
            if (body.isEmpty) {
                Text(
                    "Nothing from your watch yet today. Readings show up here after it syncs.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = muted
                )
            } else {
                ReadingsPanel(body, onOpenBody)
            }
        }
    }
}

@Composable
private fun ReadingsPanel(body: HomeBody.Readings, onClick: () -> Unit) {
    val tiles = listOfNotNull(
        body.sleep?.let { "Sleep" to it },
        body.restingHr?.let { "Resting HR" to it },
        body.hrv?.let { "HRV" to it },
        body.steps?.let { "Steps" to it }
    )
    Column(
        Modifier
            .fillMaxWidth()
            .clip(PanelShape)
            .background(MaterialTheme.colorScheme.surfaceContainerHigh)
            .bounceCombinedClick(onClickLabel = "Open body details", onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp)
    ) {
        tiles.chunked(2).forEach { pair ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                pair.forEach { (label, reading) -> ReadingTile(label, reading, Modifier.weight(1f)) }
                if (pair.size == 1) Spacer(Modifier.weight(1f))
            }
        }
    }
}

@Composable
private fun ReadingTile(label: String, reading: HomeBody.Reading, modifier: Modifier) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Column(modifier.clearAndSetSemantics { contentDescription = reading.description }) {
        Text(label.uppercase(), style = MaterialTheme.typography.labelMedium, color = muted)
        Spacer(Modifier.height(4.dp))
        Text(reading.value, style = MaterialTheme.typography.headlineSmall, color = MaterialTheme.colorScheme.onBackground)
        reading.compare?.let {
            Spacer(Modifier.height(2.dp))
            Text(it, style = MaterialTheme.typography.bodySmall, color = muted)
        }
    }
}

@Composable
private fun ConnectWatchRow(onClick: () -> Unit) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        Modifier
            .fillMaxWidth()
            .clip(PanelShape)
            .border(1.dp, MaterialTheme.colorScheme.outline, PanelShape)
            .bounceCombinedClick(onClickLabel = "Connect a watch or ring", onClick = onClick)
            .heightIn(min = 56.dp)
            .padding(horizontal = 16.dp, vertical = 14.dp)
            .semantics(mergeDescendants = true) {},
        verticalAlignment = Alignment.CenterVertically
    ) {
        Box(
            Modifier
                .size(40.dp)
                .clip(RoundedCornerShape(10.dp))
                .background(MaterialTheme.colorScheme.surfaceContainerHigh),
            contentAlignment = Alignment.Center
        ) {
            Icon(SettingsIcons.Wearable, contentDescription = null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(22.dp))
        }
        Spacer(Modifier.width(14.dp))
        Column(Modifier.weight(1f)) {
            Text("Connect a watch or ring", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onBackground)
            Spacer(Modifier.height(2.dp))
            Text(
                "Sleep, heart rate and steps through Health Connect. The coach reads them to judge recovery.",
                style = MaterialTheme.typography.bodySmall,
                color = muted
            )
        }
        Spacer(Modifier.width(12.dp))
        Text("→", style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.primary)
    }
}

private val PanelShape = RoundedCornerShape(12.dp)
