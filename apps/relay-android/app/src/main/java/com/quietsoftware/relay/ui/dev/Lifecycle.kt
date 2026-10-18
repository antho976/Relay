package com.quietsoftware.relay.ui.dev

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.quietsoftware.relay.core.model.Session
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put

/** A session lifecycle key, as the PC's plate and agent menu offer them (shell.rs `session_actions`). */
internal enum class Act(val label: String, val glyph: String, val danger: Boolean = false) {
    Start("Start", "play"),
    Wake("Wake", "play"),
    Resume("Resume", "resume"),
    Park("Park", "pause"),
    Clear("Clear context", "refresh"),
    Close("Close…", "close", danger = true),
}

/**
 * What a session in [state] offers, in the PC's order: Start one never spawned, Wake a parked one,
 * Resume a stopped one (Clear context too when restorable), Park anything live; Close last.
 */
internal fun actsFor(state: String): List<Act> = buildList {
    when (state) {
        "created" -> add(Act.Start)
        "parked" -> add(Act.Wake)
        "restorable", "exited" -> add(Act.Resume)
        "closed", "spawning" -> Unit
        else -> add(Act.Park)
    }
    if (state == "restorable") add(Act.Clear)
    if (state != "closed") add(Act.Close)
}

/** The one key a plate's strip carries: what brings the agent back, or Park. */
internal fun leadAct(state: String): Act? = actsFor(state).firstOrNull { it != Act.Clear && it != Act.Close }

/** The key that brings a stopped agent back, for a slate or a banner. */
internal fun reviveAct(state: String): Act? = leadAct(state)?.takeIf { it != Act.Park }

/** An agent the wall folds into "N stopped agents": a restorable or exited session, or an offer after a restart. */
internal class Stopped(val name: String, val provider: String, val role: String, val offered: Boolean, val dirty: Boolean)

/**
 * Runs lifecycle acts against the PC, asking first where the PC asks: Start takes an optional
 * opening message, Close its cleanup switches, Clear context and Discard a confirmation, and a
 * worktree with uncommitted work a second one before it is deleted. Render [LifecycleSheets] once
 * on the screen that holds it.
 */
@Stable
internal class Lifecycle(private val nav: Nav) {
    /** The session an act is in flight for. */
    var busy by mutableStateOf<String?>(null)
        private set

    /** "Resume all": done so far and how many. */
    var progress by mutableStateOf<Pair<Int, Int>?>(null)
        private set

    internal var starting by mutableStateOf<Session?>(null)
    internal var closing by mutableStateOf<Session?>(null)
    internal var asking by mutableStateOf<Ask?>(null)

    var onClosed: (String) -> Unit = {}

    /** "Resume all" was asked to stop after the agent it is resuming. */
    var halt by mutableStateOf(false)
        private set

    /** Whether the screen that holds this is still shown: an answer arriving later must not navigate from another. */
    internal var shown = false

    class Ask(val title: String, val body: String, val confirm: String, val danger: Boolean, val run: () -> Unit)

    private fun reachable(): Boolean {
        if (nav.shell.state.value.online) return true
        nav.shell.toast(NEEDS_PC)
        return false
    }

    fun run(act: Act, s: Session) {
        if (busy != null || progress != null || !reachable()) return
        val name = s.name
        when (act) {
            Act.Start -> starting = s
            Act.Close -> closing = s
            Act.Clear -> asking = Ask(
                "Clear context?",
                "Start fresh in this same session and worktree. The saved provider conversation is cleared.",
                "Clear and start",
                danger = true,
            ) { send("session.clear_restorable", name, "Starting $name afresh") { named(name) } }
            Act.Wake -> send("session.wake", name, "Waking $name") { named(name) }
            Act.Resume -> send("session.resume", name, "Resuming $name") { named(name) }
            Act.Park -> send("session.park", name, "Parked $name") { named(name) }
        }
    }

    fun start(name: String, prompt: String) = send("session.spawn", name, "Starting $name") {
        buildJsonObject {
            put("session", name)
            if (prompt.isNotBlank()) put("prompt", prompt.trim())
        }
    }

    fun close(name: String, removeWorktree: Boolean, purgeBuild: Boolean) = send(
        "session.close",
        name,
        null,
        then = { r ->
            val freed = ((r as? JsonObject)?.get("freed_mb"))?.jsonPrimitive?.doubleOrNull ?: 0.0
            nav.shell.toast(if (freed > 0) "Closed $name · freed ${megabytes(freed)}" else "Closed $name")
            if (shown) onClosed(name)
        },
    ) { discard ->
        buildJsonObject {
            put("session", name)
            put("remove_worktree", removeWorktree)
            put("purge_build", purgeBuild)
            if (discard) put("discard_changes", true)
        }
    }

    // ---- The stopped agents' fold ----

    fun resume(st: Stopped, then: () -> Unit) {
        if (busy != null || progress != null || !reachable()) return
        send("session.resume", st.name, "Resuming ${st.name}", then = { if (shown) then() }) { named(st.name) }
    }

    fun startFresh(st: Stopped, then: () -> Unit) {
        if (busy != null || progress != null || !reachable()) return
        asking = Ask(
            "Start fresh?",
            "Start ${st.name} again in its own session and worktree. The saved provider conversation is cleared.",
            "Clear and start",
            danger = true,
        ) { send("session.clear_restorable", st.name, "Starting ${st.name} afresh", then = { if (shown) then() }) { named(st.name) } }
    }

    fun discard(st: Stopped) {
        if (busy != null || progress != null || !reachable()) return
        asking = Ask(
            "Discard ${st.name}?",
            (if (st.dirty) "Its worktree has uncommitted changes. " else "") +
                "The session is cleaned up and taken off the PC; its branch is kept unless its work is already merged.",
            "Discard",
            danger = true,
        ) {
            // An offer after a restart is declined; an exited session the wall knows is closed.
            if (st.offered) {
                send("session.discard_restorable", st.name, "Discarded ${st.name}") { d ->
                    buildJsonObject {
                        put("session", st.name)
                        if (d) put("discard_changes", true)
                    }
                }
            } else {
                send("session.close", st.name, "Discarded ${st.name}") { d ->
                    buildJsonObject {
                        put("session", st.name)
                        put("remove_worktree", false)
                        put("purge_build", false)
                        if (d) put("discard_changes", true)
                    }
                }
            }
        }
    }

    /** Each agent in turn, as the PC would; [stopAfterCurrent] ends it after the one running. */
    fun resumeAll(names: List<String>) {
        if (busy != null || progress != null || names.isEmpty() || !reachable()) return
        val one = names.singleOrNull()
        asking = Ask(
            if (one != null) "Resume $one?" else "Resume all ${names.size} agents?",
            if (one != null) {
                "It starts its CLI again and picks up its conversation, which uses provider quota."
            } else {
                "Each agent starts its CLI again and picks up its conversation, which uses provider quota. They are resumed one after another; tap the row to stop after the current one."
            },
            if (one != null) "Resume" else "Resume all",
            danger = false,
        ) {
            halt = false
            Feeds.scope.launch {
                var resumed = 0
                for ((i, name) in names.withIndex()) {
                    if (halt) break
                    progress = i to names.size
                    busy = name
                    try {
                        nav.relay.call("session.resume", named(name))
                        resumed++
                    } catch (e: BusException) {
                        // A held resume waits on the person; the rest should not run past it.
                        if (e.error.kind == "held" || e.error.code == "link.down") {
                            nav.shell.toast(refusal(e.error))
                            break
                        }
                        nav.shell.toast("$name: ${refusal(e.error)}")
                    }
                }
                busy = null
                progress = null
                nav.shell.toast("Resumed $resumed of ${names.size}")
            }
        }
    }

    fun stopAfterCurrent() {
        halt = true
    }

    /**
     * [op] for [name], toasting [done] or the refusal. A `worktree.dirty` refusal asks before the
     * same act goes again with `discard_changes`.
     */
    private fun send(op: String, name: String, done: String?, then: (JsonElement) -> Unit = {}, payload: (discard: Boolean) -> JsonObject) {
        if (busy != null) return
        busy = name
        Feeds.scope.launch {
            try {
                val r = nav.relay.call(op, payload(false))
                done?.let(nav.shell::toast)
                then(r)
            } catch (e: BusException) {
                if (e.error.code == "worktree.dirty") {
                    asking = Ask(
                        "Discard $name's changes?",
                        "${e.error.message.trimEnd('.')}. Removing the worktree deletes them for good; commit or stash them first to keep them.",
                        "Discard changes",
                        danger = true,
                    ) { send(op, name, done, then) { payload(true) } }
                } else {
                    nav.shell.toast(refusal(e.error))
                }
            } finally {
                busy = null
            }
        }
    }
}

internal fun megabytes(mb: Double): String = if (mb >= 1024) "%.1f GB".format(mb / 1024) else "${mb.toLong().coerceAtLeast(1)} MB"

@Composable
internal fun rememberLifecycle(nav: Nav, onClosed: (String) -> Unit = {}): Lifecycle {
    val life = remember(nav) { Lifecycle(nav) }
    val closed by rememberUpdatedState(onClosed)
    life.onClosed = { closed(it) }
    DisposableEffect(life) {
        life.shown = true
        onDispose { life.shown = false }
    }
    return life
}

/**
 * After an agent is closed: its pages (conversation, terminal, details) come off the back stack
 * together, so Back never lands on one of them; with nothing under them, the wall.
 */
internal fun Nav.leaveAgent(name: String) {
    while (true) {
        val top = controller.currentBackStackEntry ?: return
        val route = top.destination.route ?: return
        if (route !in AGENT_PAGES || top.arguments?.getString("name") != name) return
        back()
    }
}

private val AGENT_PAGES = setOf("chat/{name}", "terminal/{name}", "session/{name}")

/** The sheets [Lifecycle] opens: Start, Close and its questions. */
@Composable
internal fun LifecycleSheets(life: Lifecycle) {
    val c = Relay.colors
    life.starting?.let { s ->
        var prompt by rememberSaveable(s.name) { mutableStateOf("") }
        DevSheet({ life.starting = null }, "Start ${s.name}", "Launch the agent's CLI in its worktree. Leave the message empty to keep the assignment it was created with.") { close ->
            Field(prompt, { prompt = it }, Modifier.fillMaxWidth().padding(horizontal = 6.dp), placeholder = "Opening message (optional)", singleLine = false, minLines = 3, maxLines = 6)
            SheetKeys("Start", { close { life.start(s.name, prompt) } }, { close {} })
        }
    }
    life.closing?.let { s ->
        var removeWorktree by rememberSaveable(s.name) { mutableStateOf(false) }
        var purgeBuild by rememberSaveable(s.name) { mutableStateOf(false) }
        DevSheet({ life.closing = null }, "Close ${s.name}?", "Stops its process and takes it off the wall. Its branch is kept unless its work is already merged.") { close ->
            Column(Modifier.padding(horizontal = 6.dp)) {
                if (s.worktree.isNotBlank()) T(s.worktree, Modifier.padding(bottom = 4.dp), Relay.type.mono, c.ink3, maxLines = 2)
                SwitchRow("Remove worktree", "Delete the checkout after closing. Refused while a paired session still uses it.", removeWorktree, { removeWorktree = it })
                SwitchRow("Purge build output", "Free the disk its build directories use.", purgeBuild, { purgeBuild = it })
            }
            SheetKeys("Close session", { close { life.close(s.name, removeWorktree, purgeBuild) } }, { close {} }, KeyKind.Danger)
        }
    }
    life.asking?.let { a ->
        AskSheet(a.title, a.body, a.confirm, a.danger, onConfirm = a.run, onDismiss = { life.asking = null })
    }
}
