package com.quietsoftware.relay.ui.shell

import androidx.compose.runtime.mutableStateMapOf
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.quietsoftware.relay.core.Hub
import com.quietsoftware.relay.core.link.LinkState
import com.quietsoftware.relay.core.link.PcProfile
import com.quietsoftware.relay.core.model.Hold
import com.quietsoftware.relay.core.model.Project
import com.quietsoftware.relay.core.model.Session
import com.quietsoftware.relay.core.model.Thread
import com.quietsoftware.relay.core.model.Workspace
import com.quietsoftware.relay.core.sync.OutboxEntry
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.data.Prefs
import com.quietsoftware.relay.data.Relay
import com.quietsoftware.relay.data.Settings
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.refusal
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import javax.inject.Inject

/** What the frame around every page shows: the link, the projects, what needs the person. */
data class ShellState(
    val link: LinkState = LinkState.Connecting(0),
    val pc: PcProfile? = null,
    val lastSeen: Long = 0,
    val workspaces: List<Workspace> = emptyList(),
    val projects: List<Project> = emptyList(),
    val sessions: List<Session> = emptyList(),
    val holds: List<Hold> = emptyList(),
    /** Unread notifications besides the holds' own, so a hold counts once beside them. */
    val unread: Int = 0,
    val outbox: List<OutboxEntry> = emptyList(),
    val threads: List<Thread> = emptyList(),
    val prefs: Prefs = Prefs(),
    val loaded: Boolean = false,
) {
    val online: Boolean get() = link is LinkState.Online
    val project: Project? get() = projects.firstOrNull { it.num == prefs.lastProject } ?: projects.firstOrNull()
    val needsYou: Int get() = holds.size + sessions.count { it.blocked }
    /** Agents at work on a turn: Home, Start and the drawer count the same ones. */
    val working: Int get() = sessions.count { it.working }
    val parked: Int get() = outbox.count { it.parked }
    fun liveIn(projectId: Long?) = sessions.count { it.projectId == projectId && it.attachable }
}

@HiltViewModel
class ShellModel @Inject constructor(val relay: Relay, private val settings: Settings) : ViewModel() {
    private val pcFlow = combine(relay.state, relay.profile, relay.lastSeen) { link, pc, seen -> Triple(link, pc, seen) }
    // Not the guardrail notices: the PC files one with each hold, which [holds] counts already.
    private val dataFlow = combine(relay.workspaces(), relay.projects(), relay.sessions(), relay.holds(), relay.notifications()) { w, p, s, h, n ->
        Data(w, p, s, h, n.count { !it.read && it.category != "guardrail" })
    }

    private data class Data(val w: List<Workspace>, val p: List<Project>, val s: List<Session>, val h: List<Hold>, val unread: Int)

    val state: StateFlow<ShellState> = combine(pcFlow, dataFlow, relay.outbox(), relay.threads(), settings.prefs) { (link, pc, seen), d, out, threads, prefs ->
        // Aide's chats are reached from Aide's page, not listed among Threads'.
        ShellState(link, pc, seen, d.w, d.p, d.s, d.h, d.unread, out, threads.filter { it.kind != "aide" }, prefs, loaded = true)
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), ShellState())

    private val _toasts = MutableSharedFlow<Toast>(extraBufferCapacity = 8)

    /** One-line messages at the bottom of the screen, some with Undo or another key. */
    val toasts: SharedFlow<Toast> = _toasts

    /** A message and, when it has one, its key: [key] is what it says, [act] what it does. */
    data class Toast(val text: String, val key: String = "Undo", val act: (suspend (Nav) -> Unit)? = null)

    fun toast(text: String, undo: (suspend () -> Unit)? = null) {
        _toasts.tryEmit(if (undo == null) Toast(text) else Toast(text) { undo() })
    }

    /** A toast whose key is not Undo: [key] ("Open") does [act], which may go somewhere. */
    fun toast(text: String, key: String, act: suspend (Nav) -> Unit) {
        _toasts.tryEmit(Toast(text, key, act))
    }

    /**
     * Where the bottom bars of the screens showing begin, in pixels from the top ([LiftToasts]).
     * Toasts sit above the highest; with none, above the status bar.
     */
    val toastFloors = mutableStateMapOf<Any, Float>()

    init {
        viewModelScope.launch {
            relay.parked.collect { e -> toast("${e.label}: ${e.error?.message ?: "not applied"}. It waits in the outbox.", "Open") { it.outbox() } }
        }
        // Only a shell expires: wanted now, and dropped when the PC stayed away too long.
        viewModelScope.launch { relay.expired.collect { toast("The PC was away; open the shell again.") } }
    }

    fun pickProject(id: Long?) = viewModelScope.launch { settings.update { it.copy(lastProject = id ?: 0) } }

    fun pickSpace(space: String) = viewModelScope.launch { settings.update { it.copy(space = space) } }

    fun update(change: (Prefs) -> Prefs) = viewModelScope.launch { settings.update(change) }

    /**
     * An edit through the outbox; says so when it waits for the PC. [after] runs with the answer
     * when it went straight through.
     */
    fun change(op: String, payload: JsonObject, label: String, after: (Hub.Change) -> Unit = {}) = viewModelScope.launch {
        try {
            val c = relay.change(op, payload, label)
            if (c is Hub.Change.Queued && !state.value.online && op !in QUIET) toast("$label · saved on this phone, sent when the PC is back")
            after(c)
        } catch (e: BusException) {
            toast(refusal(e))
        }
    }

    /** An act that needs the PC now. */
    fun act(op: String, payload: JsonObject = JsonObject(emptyMap()), done: String? = null, after: (kotlinx.serialization.json.JsonElement) -> Unit = {}) = viewModelScope.launch {
        try {
            val r = relay.call(op, payload)
            done?.let { toast(it) }
            after(r)
        } catch (e: BusException) {
            toast(refusal(e))
        }
    }

    fun connect() = relay.connect()

    fun disconnect() = relay.disconnect()

    fun wake() = viewModelScope.launch {
        toast(if (relay.wake()) "Wake-up sent to the PC · it may take a minute" else "This PC has not told the phone how to wake it")
    }

    private companion object {
        /** Edits not worth a word when they wait for the PC: reading a notification, or all of them. */
        val QUIET = setOf("notify.ack", "notify.ack_all")
    }
}
