package com.quietsoftware.relay.ui.dev

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.remember
import androidx.compose.ui.text.input.TextFieldValue
import com.quietsoftware.relay.data.Relay
import com.quietsoftware.relay.data.SessionChat
import com.quietsoftware.relay.data.TerminalFeed
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * One [TerminalFeed] per agent for the whole app. The engine streams a session once per
 * connection: a second attach replaces the first, and a detach ends it for everyone on the link.
 * So the wall's miniature and the full terminal share one feed, or leaving the wall would detach
 * the terminal just opened. A feed is kept a moment after its last screen lets go, so moving
 * between the wall and a terminal keeps the stream rather than replaying it.
 *
 * Used from the main thread only (composition, effects and [scope]).
 */
internal object Feeds {
    /** Outlives screens: the feeds' streams, and acts whose answer is still worth a toast after the screen closed. */
    val scope: CoroutineScope = MainScope()

    private class Entry(val feed: TerminalFeed) {
        var users = 0
        var closing: Job? = null
        var closed = false

        /** Held at least once; one only just made is about to be, in this frame's effects. */
        var held = false
    }

    private val open = LinkedHashMap<String, Entry>()

    /** The feed for [name], made if there is none; nobody holds it until [hold]. */
    fun of(relay: Relay, name: String): TerminalFeed {
        open[name]?.let { return it.feed }
        val e = Entry(TerminalFeed(relay, name, scope))
        open[name] = e
        letGo(e)
        return e.feed
    }

    fun hold(feed: TerminalFeed) {
        val e = open[feed.name]?.takeIf { it.feed === feed } ?: Entry(feed).also { fresh ->
            open.put(feed.name, fresh)?.let(::close)
        }
        e.closing?.cancel()
        e.closing = null
        e.users++
        e.held = true
        feed.start()
    }

    fun release(feed: TerminalFeed) {
        val e = open[feed.name]?.takeIf { it.feed === feed } ?: return
        e.users = (e.users - 1).coerceAtLeast(0)
        if (e.users > 0) return
        // To the end of the order: the idle cap closes the longest unwatched first.
        open.remove(feed.name)
        open[feed.name] = e
        letGo(e)
    }

    private fun letGo(e: Entry) {
        e.closing?.cancel()
        e.closing = scope.launch {
            delay(GRACE_MS)
            close(e)
        }
        // Streams nobody watches cost the PC and the link: only the newest few wait out the grace.
        val idle = open.values.filter { it.users == 0 && it.held }
        if (idle.size > MAX_IDLE) idle.take(idle.size - MAX_IDLE).forEach(::close)
    }

    /** Once only: a second stop would detach whatever feed holds the session by then. */
    private fun close(e: Entry) {
        if (e.users > 0 || e.closed) return
        e.closed = true
        if (open[e.feed.name] === e) open.remove(e.feed.name)
        e.feed.save()
        e.feed.stop()
    }

    private const val GRACE_MS = 2_500L
    private const val MAX_IDLE = 3
}

/** The shared feed for [name], held while this composable is on screen. */
@Composable
internal fun rememberFeed(relay: Relay, name: String): TerminalFeed {
    val feed = remember(name) { Feeds.of(relay, name) }
    DisposableEffect(feed) {
        Feeds.hold(feed)
        onDispose { Feeds.release(feed) }
    }
    return feed
}

/**
 * One [SessionChat] per agent, kept after its conversation closes: back from the terminal or the
 * details, it shows as it was and reads on from where it stopped, rather than from nothing. It
 * stops reading a moment after its last screen lets go, as a feed does; the newest few
 * conversations nobody watches are kept, the rest let go.
 *
 * Used from the main thread only (composition, effects and [Feeds.scope]).
 */
internal object Chats {
    private class Entry(val relay: Relay, val chat: SessionChat) {
        var users = 0
        var reading = false
        var pausing: Job? = null
    }

    private val kept = LinkedHashMap<String, Entry>()

    /** The conversation for [name]: the one kept, or a new one; nothing reads it until [hold]. */
    fun of(relay: Relay, name: String): SessionChat {
        kept[name]?.let { e -> if (e.relay === relay) return e.chat else drop(e) }
        val e = Entry(relay, SessionChat(relay, name, Feeds.scope))
        kept[name] = e
        return e.chat
    }

    fun hold(relay: Relay, chat: SessionChat) {
        val e = kept[chat.name]?.takeIf { it.chat === chat } ?: Entry(relay, chat).also { fresh ->
            kept.put(chat.name, fresh)?.let(::drop)
        }
        e.pausing?.cancel()
        e.pausing = null
        e.users++
        if (!e.reading) {
            e.reading = true
            chat.start()
        }
    }

    fun release(chat: SessionChat) {
        // One let go of while it was on screen: nothing keeps it now.
        val e = kept[chat.name]?.takeIf { it.chat === chat } ?: return chat.stop()
        e.users = (e.users - 1).coerceAtLeast(0)
        if (e.users > 0) return
        // To the end of the order: the cap lets go of the longest unwatched first.
        kept.remove(chat.name)
        kept[chat.name] = e
        e.pausing?.cancel()
        e.pausing = Feeds.scope.launch {
            delay(GRACE_MS)
            pause(e)
        }
        val idle = kept.values.filter { it.users == 0 }
        if (idle.size > MAX_IDLE) idle.take(idle.size - MAX_IDLE).forEach(::drop)
    }

    /** Stops reading and saves what was read: once, so letting go of it later never writes it again. */
    private fun pause(e: Entry) {
        if (e.users > 0 || !e.reading) return
        e.reading = false
        e.chat.stop()
    }

    private fun drop(e: Entry) {
        e.pausing?.cancel()
        e.pausing = null
        if (kept[e.chat.name] === e) kept.remove(e.chat.name)
        pause(e)
    }

    private const val GRACE_MS = 2_500L
    private const val MAX_IDLE = 3
}

/** The kept conversation for [name], read while this composable is on screen. */
@Composable
internal fun rememberChat(relay: Relay, name: String): SessionChat {
    val chat = remember(name) { Chats.of(relay, name) }
    DisposableEffect(chat) {
        Chats.hold(relay, chat)
        onDispose { Chats.release(chat) }
    }
    return chat
}

/**
 * What is being written to each agent, kept while the app runs: the words, and the pictures
 * already on their way to its checkout. Leaving the conversation for the terminal or the
 * details, or backing out of it, finds them there when it opens again; sending clears them.
 *
 * Used from the main thread only.
 */
internal object ChatDrafts {
    class Draft {
        var text = TextFieldValue()
        val attached = mutableStateListOf<Attachment>()
    }

    private val drafts = HashMap<String, Draft>()

    fun of(name: String): Draft = drafts.getOrPut(name) { Draft() }
}
