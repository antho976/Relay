package com.forge.app.ui.coach

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.theme.ForgeMotion
import kotlinx.coroutines.delay

/**
 * The Coach page's offer of advanced tracking, as a pop-up rather than a line at the foot of the
 * account (Antho, 2026-09-25: "should be a pop-up like the notifications ... with a remind me
 * later or ignore").
 *
 * It borrows the arrival receipt's shape (`ArrivalBannerHost`): an OVERLAY settled under the status
 * bar on the surface fill, so nothing on the page moves when it appears or leaves. Unlike the
 * receipt it carries a decision, so it stays until one is made: Turn on flips the preference in
 * place and the page grows under it; Remind me later holds it back a week; Ignore retires it for
 * good, leaving Settings → Coach as the switch's home. Every exit is a preference write, so it can
 * never be there when you come back unless you asked for it to be.
 *
 * [visible] is the caller's whole rule for when to offer it; this only draws and animates.
 */
@Composable
internal fun CoachAdvancedPrompt(
    visible: Boolean,
    onTurnOn: () -> Unit,
    onRemindLater: () -> Unit,
    onIgnore: () -> Unit,
    modifier: Modifier = Modifier
) {
    // Held back a beat so the account's own entrance lands first and the offer reads as arriving
    // over it, the way a notification does, rather than as part of the page.
    var armed by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        delay(PROMPT_ARRIVAL_DELAY_MS)
        armed = true
    }
    val surface = MaterialTheme.colorScheme.surface
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline

    Box(modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
        AnimatedVisibility(
            visible = visible && armed,
            enter = fadeIn(ForgeMotion.enterTween()) +
                slideInVertically(ForgeMotion.enterTween()) { -it / 4 },
            exit = fadeOut(ForgeMotion.exitTween()) +
                slideOutVertically(ForgeMotion.exitTween()) { -it / 4 }
        ) {
            Column(
                Modifier
                    .statusBarsPadding()
                    .padding(horizontal = 24.dp)
                    .padding(top = 8.dp)
                    .widthIn(max = 420.dp)
                    .fillMaxWidth()
                    .semantics {
                        paneTitle = "Advanced tracking"
                        liveRegion = LiveRegionMode.Polite
                    }
                    // A modal-shaped thing floating over the page with controls on it: §1 and §5
                    // give it the surface fill, exactly as the arrival receipt has.
                    .clip(RoundedCornerShape(16.dp))
                    .background(surface)
                    // Unlike the receipt it can rest over the account's own filled tile, which is
                    // nearly the same tone; the unselected-control rung keeps the two apart.
                    .border(1.dp, outline.copy(alpha = 0.35f), RoundedCornerShape(16.dp))
                    .padding(horizontal = 14.dp, vertical = 12.dp)
            ) {
                Text(
                    "COACH",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted.copy(alpha = 0.65f)
                )
                Spacer(Modifier.height(4.dp))
                Text(
                    "You can turn on advanced tracking",
                    style = MaterialTheme.typography.bodyMedium,
                    color = onBg
                )
                Spacer(Modifier.height(8.dp))
                Text(
                    "It shows the readings behind each call: signals, block, inputs and what has " +
                        "been learned about you.",
                    style = MaterialTheme.typography.bodySmall,
                    color = muted
                )
                Spacer(Modifier.height(12.dp))
                // Wraps rather than clips at 200% font scale (§14).
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                    itemVerticalAlignment = Alignment.CenterVertically
                ) {
                    ForgePrimaryCapsule("Turn on", onClick = onTurnOn)
                    ForgeOutlineCapsule("Remind me later", onClick = onRemindLater)
                    CoachAction("Ignore", muted, "Ignore advanced tracking", onIgnore)
                }
            }
        }
    }
}

/** How long after the page appears the offer settles in. Long enough for the entrance cascade. */
private const val PROMPT_ARRIVAL_DELAY_MS = 700L
