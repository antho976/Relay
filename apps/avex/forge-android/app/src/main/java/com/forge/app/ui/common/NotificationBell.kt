package com.forge.app.ui.common

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/**
 * A process-wide "return to the Home tab" action, provided once by ForgeNavHost. It survives as the
 * bell's long-press: the tap-anywhere-to-go-home shortcut the wordmark used to carry, without the
 * wordmark. Defaults to a no-op so previews / tests don't need it.
 */
val LocalGoHome = staticCompositionLocalOf<() -> Unit> { {} }

/** Opens the notifications page from any screen's chrome. Provided once by ForgeNavHost. */
val LocalOpenNotifications = staticCompositionLocalOf<() -> Unit> { {} }

/** How many notices are waiting, fed from the one [com.forge.app.data.repo.NotificationFeed]. */
val LocalUnreadNotifications = staticCompositionLocalOf { 0 }

/**
 * The top-bar bell — the app's one entry to the notifications page, standing where the `• Avex`
 * wordmark used to sit (DESIGN §4.6).
 *
 * Tap opens the page; long-press goes Home, keeping the wordmark's shortcut alive. The unread disc
 * is drawn only when something is actually waiting: a dot earns its accent by flagging an exception,
 * never the resting state (§8).
 */
@Composable
fun NotificationBell(
    onClick: () -> Unit = LocalOpenNotifications.current,
    onLongClick: () -> Unit = LocalGoHome.current,
    modifier: Modifier = Modifier,
) {
    val unread = LocalUnreadNotifications.current
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val label = if (unread > 0) "Notifications, $unread waiting" else "Notifications"

    Box(
        // §14: the ≥44dp target comes from the box, while the glyph stays visually small.
        modifier = modifier
            .sizeIn(minWidth = 44.dp, minHeight = 44.dp)
            .bounceCombinedClick(
                onClickLabel = label,
                onLongClickLabel = "Go to Home",
                onLongClick = onLongClick,
                onClick = onClick
            ),
        contentAlignment = Alignment.Center
    ) {
        Icon(
            Icons.Default.Notifications,
            contentDescription = label,
            tint = muted.copy(alpha = 0.7f),
            modifier = Modifier.size(20.dp)
        )
        if (unread > 0) {
            Box(
                // `offset` (not `absoluteOffset`) so the disc mirrors to the other corner under RTL.
                Modifier
                    .offset(x = 7.dp, y = (-7).dp)
                    .size(6.dp)
                    .background(accent, CircleShape)
            )
        }
    }
}
