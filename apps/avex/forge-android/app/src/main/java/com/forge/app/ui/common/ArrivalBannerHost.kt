package com.forge.app.ui.common

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
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
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.forge.app.data.repo.NotificationFeed
import com.forge.app.ui.theme.ForgeMotion
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Draws the arrival banner and flies it into the bell.
 *
 * Mounted once, at the app root beside [ProgramChangeGuardHost], so a receipt rides over whatever
 * screen is up. It is an OVERLAY: it sits in a `fillMaxSize` Box above the nav host and never joins
 * a page's layout, so nothing on the page below it moves. That is the whole difference between this
 * and the page-level banner strips `design/SETTLED.md` bans.
 *
 * The sequence is settle in, hold, fly to the bell. Under reduced motion the flight is skipped and
 * the banner simply appears and goes; the count it announces increments identically either way,
 * because §9 forbids gating meaning on motion.
 */
@Composable
fun ArrivalBannerHost(viewModel: ArrivalBannerViewModel = hiltViewModel()) {
    val queue by viewModel.controller.queue.collectAsStateWithLifecycle()
    val arrival = queue.firstOrNull() ?: return

    val anchor = LocalBellAnchor.current
    val openNotifications = LocalOpenNotifications.current
    val density = LocalDensity.current
    val surface = MaterialTheme.colorScheme.surface
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline

    // 0 → settled, 1 → arrived at the bell. One driver for translation, scale and fade together.
    val flight = remember(arrival.noticeId) { Animatable(0f) }
    val entrance = remember(arrival.noticeId) { Animatable(0f) }
    var bannerCentre by remember(arrival.noticeId) { mutableStateOf<Offset?>(null) }

    LaunchedEffect(arrival.noticeId) {
        entrance.animateTo(1f, ForgeMotion.enterTween(ForgeMotion.DurationStandard))
        delay(HOLD_MS)
        // No bell on screen means nothing to fly at, so it fades where it stands. Reduced motion
        // takes the same path, since ForgeMotion collapses the tween to a snap anyway.
        flight.animateTo(1f, ForgeMotion.exitTween(ForgeMotion.DurationEmphasized))
        viewModel.onAnnounced(arrival.noticeId)
    }

    val target = anchor.position
    val start = bannerCentre
    val travel = if (target != null && start != null) target - start else Offset.Zero

    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
        Column(
            Modifier
                .statusBarsPadding()
                .padding(horizontal = 24.dp)
                .padding(top = 8.dp)
                .widthIn(max = 420.dp)
                .onGloballyPositioned { bannerCentre = it.boundsInRoot().center }
                .graphicsLayer {
                    val f = flight.value
                    translationX = travel.x * f
                    // The entrance drops it in from just above its resting place.
                    translationY = travel.y * f - with(density) { 12.dp.toPx() } * (1f - entrance.value)
                    val shrink = 1f - 0.7f * f
                    scaleX = shrink
                    scaleY = shrink
                    alpha = entrance.value * (1f - f)
                }
                // §1: a surface is earned by interactivity, and this one is tappable. It is also a
                // modal-shaped thing floating over the page, which §5 gives the surface fill.
                .clip(RoundedCornerShape(16.dp))
                .background(surface)
                // Tapping the receipt goes where the receipt points: the feed. Deliberately not
                // straight to the lesson — the banner's claim is "this is waiting for you behind
                // the bell", and landing anywhere else would make that claim false.
                .clickableLabeled(arrival.title) {
                    openNotifications()
                    viewModel.onAnnounced(arrival.noticeId)
                }
                .padding(horizontal = 14.dp, vertical = 12.dp)
        ) {
            Text(
                arrival.eyebrow.uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = muted.copy(alpha = 0.65f)
            )
            Spacer(Modifier.height(4.dp))
            // §14: no maxLines on content. A long lesson title wraps rather than truncating.
            Text(arrival.title, style = MaterialTheme.typography.bodyMedium, color = onBg)
        }
    }
}

/** How long the banner rests before it leaves. Long enough to read a title, short enough to ignore. */
private const val HOLD_MS = 1_600L

/**
 * Bridges the feed's un-announced arrivals into the [ArrivalController] queue.
 *
 * The split matters: the controller owns the queue and knows nothing about notices, so its whole
 * state machine is unit-testable without a device, while this knows about the feed and owns only
 * the translation between the two.
 */
@HiltViewModel
class ArrivalBannerViewModel @Inject constructor(
    val controller: ArrivalController,
    private val feed: NotificationFeed,
) : ViewModel() {

    init {
        viewModelScope.launch {
            feed.pendingAnnouncements.collect { pending ->
                controller.enqueue(
                    pending.map { notice ->
                        ArrivalController.Arrival(
                            noticeId = notice.id,
                            eyebrow = notice.eyebrow,
                            title = notice.title,
                        )
                    }
                )
            }
        }
    }

    /**
     * The banner finished. Marked announced FIRST, then dequeued.
     *
     * Order matters on a cold kill: a crash between the two leaves the arrival marked announced but
     * still unread, so the reader loses a banner and keeps the notice. The other order would replay
     * the banner forever for an arrival the feed already dropped, which is the failure that reads
     * as a bug.
     */
    fun onAnnounced(noticeId: String) = viewModelScope.launch {
        runCatching { feed.markAnnounced(listOf(noticeId)) }
        controller.consume(noticeId)
    }
}
