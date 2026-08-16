package com.forge.app.ui.common

import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.geometry.Offset
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The arrival banner queue: the app's ONE way to say "this just showed up" without interrupting.
 *
 * The rule this exists to serve is that nothing unsolicited opens in front of the user. A thing
 * that happened is a notice, notices live behind the bell, and the banner is only the receipt for
 * one having arrived: it settles briefly over whatever is on screen, never in the layout flow, then
 * flies into the bell and increments its count.
 *
 * §4.6 and `design/SETTLED.md` ban page-level banner strips, and this is deliberately not one. The
 * banned object is a RESIDENT strip that sits above a page's own answer and pushes its content
 * down; this displaces nothing, is never dismissible (it leaves on its own), and cannot be present
 * when the user next looks at the page. Recorded in `design/SETTLED.md`, 2026-08-15.
 *
 * The queue lives here, in a plain singleton with no Compose or Android types in its logic, so the
 * whole state machine is unit-testable without a device. The host owns only the drawing.
 */
@Singleton
class ArrivalController @Inject constructor() {

    /**
     * One arrival worth announcing.
     *
     * [noticeId] is the feed row this receipt belongs to, which is what the host marks announced so
     * the same arrival can never be replayed on a later launch.
     */
    data class Arrival(
        val noticeId: String,
        val eyebrow: String,
        val title: String,
    )

    private val _queue = MutableStateFlow<List<Arrival>>(emptyList())

    /** Everything waiting to be announced, oldest first. The host shows [current]. */
    val queue: StateFlow<List<Arrival>> = _queue.asStateFlow()

    /** The banner on screen right now, or null when nothing is being announced. */
    val current: Arrival? get() = _queue.value.firstOrNull()

    /**
     * Queue arrivals, ignoring any already queued.
     *
     * Idempotent by [Arrival.noticeId] because the source is a flow that re-emits on every
     * unrelated feed change: a milestone landing must not re-queue a lesson banner that is already
     * waiting its turn.
     */
    fun enqueue(arrivals: List<Arrival>) {
        if (arrivals.isEmpty()) return
        val known = _queue.value.map { it.noticeId }.toSet()
        val fresh = arrivals.filter { it.noticeId !in known }
        if (fresh.isEmpty()) return
        _queue.value = _queue.value + fresh
    }

    /** The front banner finished (flew, faded, or was tapped). Advances to the next one. */
    fun consume(noticeId: String) {
        _queue.value = _queue.value.filterNot { it.noticeId == noticeId }
    }

    /** Drop everything pending, for a sign-out or a wipe. */
    fun clear() {
        _queue.value = emptyList()
    }
}

/**
 * Where the bell is on screen, in root coordinates, so a banner can fly to it.
 *
 * [position] is null when no bell is composed. The bell is Home-only (§4.6), so on every other page
 * it is null and the banner fades in place rather than flying at a corner that holds nothing. The
 * count still increments either way, because meaning is never gated on motion (§9).
 */
@Stable
class BellAnchor {
    var position: Offset? by mutableStateOf(null)
}

val LocalBellAnchor = staticCompositionLocalOf { BellAnchor() }
