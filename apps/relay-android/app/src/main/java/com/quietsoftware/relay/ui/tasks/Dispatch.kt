package com.quietsoftware.relay.ui.tasks

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.quietsoftware.relay.core.model.Task
import com.quietsoftware.relay.core.sync.Optimistic
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.core.wire.b
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.data.Relay as Data
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Eyebrow
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.LampDot
import com.quietsoftware.relay.ui.kit.Pill
import com.quietsoftware.relay.ui.kit.Segmented
import com.quietsoftware.relay.ui.kit.StaleNote
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.Toggle
import com.quietsoftware.relay.ui.kit.plural
import com.quietsoftware.relay.ui.kit.switchRow
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/** launch.rs: Codex has no `max`, Claude no `minimal`; both start on `high`. */
private val EFFORTS = mapOf(
    "claude" to listOf("low", "medium", "high", "xhigh", "max"),
    "codex" to listOf("minimal", "low", "medium", "high", "xhigh"),
)

private val ROLES = listOf("builder" to "Builder", "reviewer" to "Reviewer", "docs" to "Docs")

/**
 * Send a task to an agent (`task.dispatch`): a live session of the project, or a new one shaped
 * as the launch sheet shapes it. "Stage only" assigns without starting. Needs the PC now.
 */
@Composable
internal fun DispatchSheet(task: Task, nav: Nav, onDismiss: () -> Unit) {
    val c = Relay.colors
    val shell by nav.shell.state.collectAsStateWithLifecycle()
    // A session made on this phone (`tmp:…`) has no name on the PC yet, so it cannot be targeted.
    val live = remember(shell.sessions, task.projectId) { shell.sessions.filter { it.projectId == task.projectId && it.state in DISPATCHABLE && !Optimistic.isTemp(it.name) } }
    val providers by remember { nav.relay.live("provider.list") }.collectAsStateWithLifecycle(Data.Live())
    val installed = remember(providers.result) {
        (providers.result as? JsonObject)?.arr("providers")?.mapNotNull { (it as? JsonObject)?.takeIf { p -> p.b("installed") == true }?.s("provider") }.orEmpty()
    }
    val choices = installed.ifEmpty { listOf("claude", "codex") }

    var mode by rememberSaveable { mutableStateOf("new") }
    var target by rememberSaveable { mutableStateOf<String?>(null) }
    var provider by rememberSaveable { mutableStateOf("claude") }
    var role by rememberSaveable { mutableStateOf("builder") }
    var model by rememberSaveable { mutableStateOf("") }
    var effort by rememberSaveable { mutableStateOf("high") }
    var worktree by rememberSaveable { mutableStateOf("new") }
    var busWrites by rememberSaveable { mutableStateOf(true) }
    var allowUi by rememberSaveable { mutableStateOf(false) }
    var fanout by rememberSaveable { mutableStateOf(false) }
    var busy by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()

    val shownProvider = if (provider in choices) provider else choices.first()
    val efforts = EFFORTS[shownProvider] ?: EFFORTS.getValue("claude")
    // Switching provider keeps the effort when the new one has it, else its default.
    val shownEffort = if (effort in efforts) effort else "high"
    val num = task.num
    val ready = num != null && shell.online && !busy && (mode == "new" || target != null)

    BoardSheet(onDismiss, title = "Dispatch ${task.named()}", subtitle = task.title) { close ->
        fun send(start: Boolean) {
            val id = num ?: return
            val payload = if (mode == "live") {
                buildJsonObject { put("task_id", id); put("session", target); put("start", start) }
            } else {
                buildJsonObject {
                    put("task_id", id)
                    put("create", buildJsonObject {
                        put("project_id", task.projectId)
                        put("provider", shownProvider)
                        put("role", role)
                        model.trim().takeIf { it.isNotEmpty() }?.let { put("model", it) }
                        put("effort", shownEffort)
                        put("worktree", worktree.trim().ifEmpty { "new" })
                        put("bus_writes", busWrites)
                        put("allow_ui", allowUi)
                    })
                    put("start", start)
                    if (fanout) put("fanout", true)
                }
            }
            busy = true
            // On the app's scope, so a sheet swiped away meanwhile still says how it went; only
            // a sheet still open goes on to the agent.
            nav.shell.viewModelScope.launch {
                try {
                    val out = nav.relay.call("task.dispatch", payload) as? JsonObject
                    val name = out?.o("session")?.s("name")
                    val more = out?.arr("fanned")?.size?.takeIf { it > 0 }?.let { " (+${plural(it, "sub-task")})" }.orEmpty()
                    nav.shell.toast(if (start) "Sent to ${name ?: "the agent"}$more" else "Staged on ${name ?: "the agent"}$more")
                    if (scope.isActive) close { if (start && name != null) nav.terminal(name) }
                } catch (e: BusException) {
                    busy = false
                    nav.shell.toast(if (e.error.code == "link.down") "Needs the PC, which is out of reach" else e.error.message.ifBlank { e.error.code })
                }
            }
        }

        Column(Modifier.fillMaxWidth().weight(1f, fill = false).verticalScroll(rememberScrollState()).padding(horizontal = 6.dp)) {
            if (!shell.online) {
                Row(Modifier.padding(bottom = 10.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    Dot(c.waiting, 6.dp)
                    T("Dispatching needs the PC, which is out of reach.", style = Relay.type.caption, color = c.ink2)
                }
            }
            if (num == null) T("This task is saved on the phone only. It can be dispatched once the PC has it.", Modifier.padding(bottom = 10.dp), Relay.type.caption, c.ink2)
            Segmented(listOf("new" to "New agent", "live" to "A live agent · ${live.size}"), mode, { mode = it }, Modifier.fillMaxWidth(), fill = true)
            Gap(12.dp)
            if (mode == "live") {
                if (live.isEmpty()) {
                    T("No live session in this project. Start a new agent instead.", Modifier.padding(vertical = 12.dp), Relay.type.caption, c.ink3)
                } else {
                    for (s in live) {
                        MenuRow(
                            s.name, "terminal", { target = s.name },
                            detail = listOfNotNull(titled(s.provider), titled(s.role), s.stateLabel.lowercase(), s.taskId?.let { "on #$it" }).joinToString(" · "),
                            selected = target == s.name,
                            lead = { LampDot(s.lamp, 8.dp) },
                        )
                    }
                }
            } else {
                Eyebrow("Provider")
                Gap(6.dp)
                Segmented(choices.map { it to if (it == "codex") "Codex" else "Claude" }, shownProvider, { provider = it }, Modifier.fillMaxWidth(), fill = true)
                Gap(12.dp)
                Eyebrow("Role")
                Gap(6.dp)
                Segmented(ROLES, role, { role = it }, Modifier.fillMaxWidth(), fill = true)
                Gap(12.dp)
                Eyebrow("Model")
                Gap(6.dp)
                Field(model, { model = it }, Modifier.fillMaxWidth(), placeholder = "Provider default, or a model id", style = Relay.type.mono, keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false))
                Gap(12.dp)
                Eyebrow("Effort")
                Gap(6.dp)
                Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    for (e in efforts) Pill(e, selected = e == shownEffort, onClick = { effort = e })
                }
                Gap(12.dp)
                Eyebrow("Worktree")
                Gap(6.dp)
                Field(worktree, { worktree = it }, Modifier.fillMaxWidth(), placeholder = "new", style = Relay.type.mono, keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false))
                T("new, primary, or the path of a worktree on the PC", Modifier.padding(start = 4.dp, top = 4.dp), Relay.type.caption, c.ink3)
                Gap(8.dp)
                SwitchRow("Agent bus writes", "Let it write files, stage and commit through Relay, with guardrails checked as it goes.", busWrites) { busWrites = it }
                SwitchRow("UI control", "Let it drive the Relay window.", allowUi) { allowUi = it }
                if (task.children.isNotEmpty()) SwitchRow("Sub-tasks too", "One new agent per open sub-task, with the same options", fanout) { fanout = it }
                if (!providers.fresh) StaleNote(providers.at)
            }
        }
        Gap(12.dp)
        Row(Modifier.fillMaxWidth().padding(horizontal = 6.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Key("Stage only", { send(false) }, Modifier.weight(1f), enabled = ready)
            Key(if (busy) "Sending…" else "Dispatch", { send(true) }, Modifier.weight(1f), kind = KeyKind.Primary, glyph = "play", enabled = ready)
        }
    }
}

@Composable
internal fun SwitchRow(label: String, detail: String?, checked: Boolean, onChange: (Boolean) -> Unit) {
    val c = Relay.colors
    Row(Modifier.fillMaxWidth().heightIn(min = 48.dp).switchRow(checked, onChange = onChange).padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f).padding(end = 12.dp)) {
            T(label, style = Relay.type.ui, color = c.ink)
            detail?.let { T(it, style = Relay.type.caption, color = c.ink3) }
        }
        Toggle(checked, null)
    }
}
