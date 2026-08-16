package com.forge.app.ui.common

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
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
 *
 * The disc carries a COUNT rather than a bare dot (2026-08-15). A dot answered "something happened"
 * and stopped there, which was enough while notices were rare and is not once the Academy feeds the
 * feed: two waiting things and five waiting things are different decisions about whether to look
 * now. The numeral is `onPrimary` on the accent fill rather than accent-coloured text, because
 * accent as text fails AA at every one of the four accents (§14).
 */
@Composable
fun NotificationBell(
    onClick: () -> Unit = LocalOpenNotifications.current,
    onLongClick: () -> Unit = LocalGoHome.current,
    modifier: Modifier = Modifier,
) {
    val unread = LocalUnreadNotifications.current
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val label = if (unread > 0) "Notifications, $unread waiting" else "Notifications"
    val anchor = LocalBellAnchor.current

    // Publishes where the bell sits so an arrival banner can fly into it. Cleared on dispose,
    // because the bell is Home-only: on any other page the banner must fade in place rather than
    // fly at a stale corner.
    DisposableEffect(anchor) { onDispose { anchor.position = null } }

    Box(
        // §14: the ≥44dp target comes from the box, while the glyph stays visually small.
        modifier = modifier
            .sizeIn(minWidth = 44.dp, minHeight = 44.dp)
            .onGloballyPositioned { anchor.position = it.boundsInRoot().center }
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
        // `offset` (not `absoluteOffset`) so the disc mirrors to the other corner under RTL.
        CountBadge(unread, Modifier.offset(x = 9.dp, y = (-8).dp))
    }
}

/**
 * The shared unread disc: an accent pill with the count on it, or nothing at all.
 *
 * Shared by the bell and the Academy tab so the two can never drift into looking like different
 * kinds of signal. It draws nothing at zero rather than an empty ring, because §8 reserves the
 * accent for flagging an exception and "nothing is waiting" is the resting state.
 *
 * Sized by padding rather than a fixed box: at 200% font a two-digit count still has to fit, and
 * §14 bans a fixed `.height()` on anything containing text.
 */
@Composable
fun CountBadge(
    count: Int,
    modifier: Modifier = Modifier,
    /** What TalkBack reads. Null leaves the disc silent, for a host that says the count itself. */
    contentDescription: String? = null,
) {
    if (count <= 0) return
    val accent = MaterialTheme.colorScheme.primary
    val onAccent = MaterialTheme.colorScheme.onPrimary
    Box(
        modifier
            .sizeIn(minWidth = 14.dp, minHeight = 14.dp)
            .background(accent, CircleShape)
            .padding(horizontal = 4.dp, vertical = 1.dp)
            .then(
                if (contentDescription != null)
                    Modifier.semantics { this.contentDescription = contentDescription }
                else Modifier
            ),
        contentAlignment = Alignment.Center
    ) {
        Text(
            // Past 9 the exact number stops changing the decision, and a three-digit disc would
            // shove the glyph it sits on off its own touch target.
            if (count > 9) "9+" else "$count",
            style = MaterialTheme.typography.labelSmall,
            color = onAccent
        )
    }
}
