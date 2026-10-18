package com.quietsoftware.relay.ui.pages

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.sync.OutboxEntry
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.Slab
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.ago
import com.quietsoftware.relay.ui.kit.column
import com.quietsoftware.relay.ui.shell.SpaceFrame
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

/** Every change made on this phone that the PC has not applied yet, and what is holding each back. */
@Composable
fun OutboxScreen(nav: Nav) {
    // Null until the phone's copy answers, so "Nothing waiting" never shows before what waits.
    val entries by remember { nav.relay.outbox() }.collectAsStateWithLifecycle(initialValue = null)
    val s by nav.shell.state.collectAsStateWithLifecycle()
    // The page's scope, not the row's: a row leaves composition as soon as its entry is dropped or
    // resent, which would cancel the retry between taking the old entry out and putting the new in.
    val scope = rememberCoroutineScope()
    // The change Drop or Use the PC's is asking about: what the person wrote is not lost on one tap.
    var dropping by remember { mutableStateOf<OutboxEntry?>(null) }
    SpaceFrame(nav) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
            LazyColumn(
                Modifier.column().fillMaxSize(),
                contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 24.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                item { T("Outbox", style = Relay.type.heading, color = Relay.colors.ink) }
                item {
                    T(
                        "Changes made on this phone wait here until your PC takes them. They are sent once, in order.",
                        style = Relay.type.caption,
                        color = Relay.colors.ink3,
                    )
                }
                if (entries?.isEmpty() == true) {
                    item { Empty("outbox", "Nothing waiting", "Everything you changed is on the PC.") }
                }
                items(entries.orEmpty(), key = { it.id }) { e -> OutboxRow(e, nav, scope, away = !s.online, onDrop = { dropping = e }) }
            }
        }
    }

    dropping?.let { e ->
        val theirs = e.state == OutboxEntry.State.Conflict
        Ask(
            title = if (theirs) "Use the PC's version?" else "Drop this change?",
            body = if (theirs) "“${e.label}” is not sent, and the PC keeps what it has." else "“${e.label}” is not sent to the PC, and what it held is not kept.",
            onDismiss = { dropping = null },
        ) {
            Key("Cancel", { dropping = null }, kind = KeyKind.Quiet, compact = true)
            Key(
                if (theirs) "Use the PC's" else "Drop",
                {
                    dropping = null
                    scope.launch {
                        // A waiting change goes only while it has not left the phone, as the board's Undo has it.
                        val still = nav.relay.hub.outbox.get(e.id) ?: return@launch
                        val unsent = still.state == OutboxEntry.State.Pending && still.attempts == 0 && !nav.shell.state.value.online
                        if (unsent || still.state == OutboxEntry.State.Failed || still.state == OutboxEntry.State.Conflict) {
                            nav.relay.discard(still.id)
                        } else {
                            nav.shell.toast("“${e.label}” has gone to the PC already")
                        }
                    }
                },
                kind = KeyKind.Danger,
                compact = true,
            )
        }
    }
}

@Composable
private fun OutboxRow(e: OutboxEntry, nav: Nav, scope: CoroutineScope, away: Boolean, onDrop: () -> Unit) {
    val c = Relay.colors
    Slab(Modifier.fillMaxWidth(), padding = PaddingValues(14.dp)) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Top) {
            T(e.label, Modifier.weight(1f), Relay.type.uiMedium, c.ink, maxLines = 2)
            Spacer(Modifier.width(8.dp))
            T(ago(e.createdAt), style = Relay.type.caption, color = c.ink3, maxLines = 1)
        }
        T(e.op, style = Relay.type.mono, color = c.ink3, maxLines = 1)
        Gap(10.dp)
        when (e.state) {
            OutboxEntry.State.Held -> {
                Status(c.held, "Held by a guardrail on the PC", c.ink2)
                Gap(10.dp)
                Key("Open inbox", { nav.inbox() }, compact = true)
            }
            OutboxEntry.State.Conflict -> {
                Status(c.waiting, "Changed on the PC too", c.ink2)
                e.error?.message?.takeIf { it.isNotBlank() }?.let {
                    Gap(4.dp)
                    T(it, style = Relay.type.caption, color = c.ink3)
                }
                Gap(10.dp)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Key("Keep mine", { scope.launch { nav.relay.retry(e.id, overwrite = true) } }, kind = KeyKind.Primary, compact = true)
                    Key("Use the PC's", onDrop, compact = true)
                }
            }
            OutboxEntry.State.Failed -> {
                Status(c.held, "The PC refused: ${e.error?.message?.takeIf { it.isNotBlank() } ?: "no reason given"}", c.heldText)
                Gap(10.dp)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Key("Try again", { scope.launch { nav.relay.retry(e.id, overwrite = false) } }, compact = true)
                    Key("Drop", onDrop, kind = KeyKind.Quiet, compact = true)
                }
            }
            else -> {
                Status(c.waiting, if (e.attempts > 0) "Sent, waiting for an answer" else "Waiting for the PC", c.ink2)
                // Not sent yet, with the PC away: it can still be taken back, as the board's Undo does.
                if (e.attempts == 0 && away) {
                    Gap(10.dp)
                    Key("Drop", onDrop, kind = KeyKind.Quiet, compact = true)
                }
            }
        }
    }
}

@Composable
private fun Status(dot: Color, text: String, textColor: Color) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Dot(dot, 7.dp)
        Spacer(Modifier.width(8.dp))
        T(text, Modifier.weight(1f), Relay.type.ui, textColor)
    }
}
