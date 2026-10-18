package com.quietsoftware.relay.ui.dev

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.Saver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.Hub
import com.quietsoftware.relay.core.model.Project
import com.quietsoftware.relay.core.model.Task
import com.quietsoftware.relay.core.sync.OutboxEntry
import com.quietsoftware.relay.core.sync.OutboxRunner
import com.quietsoftware.relay.core.sync.Refs
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.core.wire.b
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.data.Relay as RelayData
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Eyebrow
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Hairline
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.Pill
import com.quietsoftware.relay.ui.kit.Segmented
import com.quietsoftware.relay.ui.kit.Slab
import com.quietsoftware.relay.ui.kit.StaleNote
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.Relay
import com.quietsoftware.relay.ui.threads.MODELS
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/**
 * Start agents on the PC, as the PC's New session sheet does (launch.rs): one solo agent in its
 * own worktree, or a review group (one or two builders and a reviewer on one branch). Each agent
 * gets its provider, model, effort, first message and permissions; the first may take a staged
 * task. `session.create` and `session.spawn` both wait in the outbox, so with the PC away the
 * agents start when it is back.
 */
@Composable
fun LaunchScreen(projectId: Long?, taskId: Long?, prompt: String?, nav: Nav) {
    val c = Relay.colors
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var picked by rememberSaveable { mutableStateOf<Long?>(null) }
    val pid = picked ?: projectId ?: s.project?.num
    val project = s.projects.firstOrNull { it.num == pid }

    var group by rememberSaveable { mutableStateOf(false) }
    var builders by rememberSaveable { mutableIntStateOf(1) }
    var member by rememberSaveable { mutableIntStateOf(0) }
    var role by rememberSaveable { mutableStateOf("builder") }
    var profiles by rememberSaveable(stateSaver = PROFILES) { mutableStateOf(List(3) { if (it == 0) Profile(prompt = prompt.orEmpty()) else Profile() }) }
    var task by rememberSaveable { mutableStateOf(taskId) }
    var worktree by rememberSaveable { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var note by remember { mutableStateOf<String?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var pickingProject by remember { mutableStateOf(false) }
    var pickingTask by remember { mutableStateOf(false) }
    var leaving by remember { mutableStateOf(false) }
    val promptFocus = remember { FocusRequester() }
    // A new agent with nothing written yet: the keyboard is up for its first message.
    LaunchedEffect(Unit) { if (prompt.isNullOrEmpty() && taskId == null) runCatching { promptFocus.requestFocus() } }

    val providerAnswer by remember { nav.relay.live("provider.list") }.collectAsStateWithLifecycle(RelayData.Live())
    val providers = remember(providerAnswer.result) { providersOf(providerAnswer.result) }
    val tasks by remember(pid) { if (pid != null) nav.relay.tasks(pid) else flowOf(emptyList()) }.collectAsStateWithLifecycle(emptyList())
    val open = remember(tasks) { tasks.filter { it.column != "done" && it.num != null } }

    // An agent nobody switched runs on the first installed provider, as on the PC.
    val fallback = providers?.firstOrNull { it.installed }?.provider ?: "claude"
    val members = if (group) (if (builders == 2) listOf(0, 1, 2) else listOf(0, 2)) else listOf(0)
    val index = if (member in members) member else members.first()
    val profile = profiles[index]
    val leads = !group || index == 0
    fun roleOf(i: Int) = if (group) (if (i == 2) "reviewer" else "builder") else role
    fun providerOf(p: Profile) = p.provider ?: fallback
    fun patch(next: Profile) {
        profiles = profiles.toMutableList().also { it[index] = next }
        // The error was about the form as it was before this change.
        error = null
    }
    fun label(i: Int) = if (group) (if (i == 2) "Reviewer" else "Builder ${i + 1}") else "the agent"

    fun launch() {
        val id = pid ?: return
        if (busy) return
        // Allocation order (launch.rs): builder 1, the reviewer, builder 2, each pairing with the one before.
        val order = if (group) (if (builders == 2) listOf(0, 2, 1) else listOf(0, 2)) else listOf(0)
        for (i in order) {
            val info = providers?.firstOrNull { it.provider == providerOf(profiles[i]) }
            if (providers != null && info?.installed != true) {
                error = "Choose an installed provider for ${label(i)}."
                member = i
                return
            }
        }
        error = null
        busy = true
        note = null
        scope.launch {
            val created = mutableListOf<Pair<Hub.Change, String>>()
            try {
                // Queued together: leaving the screen must not split a create from its spawn.
                withContext(NonCancellable) {
                    val refs = mutableListOf<JsonElement>()
                    for ((step, i) in order.withIndex()) {
                        val p = profiles[i]
                        val payload = buildJsonObject {
                            put("project_id", id)
                            put("provider", providerOf(p))
                            put("role", roleOf(i))
                            put("effort", p.effortFor(providerOf(p)))
                            p.model.trim().takeIf { it.isNotEmpty() }?.let { put("model", it) }
                            // Stored as the launch assignment; the spawn below keeps it.
                            p.prompt.trim().takeIf { it.isNotEmpty() }?.let { put("prompt", it) }
                            if (i == 0) task?.let { put("task_id", it) }
                            put("bus_writes", p.writes)
                            put("allow_ui", p.ui)
                            when {
                                step > 0 -> put("pair_with", refs[step - 1])
                                worktree.isNotBlank() -> put("worktree", worktree.trim())
                                // A group keeps a fresh worktree of its own: in the primary checkout it would meet solo agents.
                                group -> put("worktree", "new")
                            }
                        }
                        val change = nav.relay.change("session.create", payload, if (group) "New ${label(i).lowercase()}" else "New agent")
                        refs += refOf(change)
                        created += change to roleOf(i)
                    }
                    // Reviewers first, so their mailbox is live before the builders publish anything.
                    for (step in order.indices.sortedBy { created[it].second != "reviewer" }) {
                        nav.relay.change("session.spawn", buildJsonObject { put("session", refs[step]) }, "Start agent")
                    }
                }
            } catch (e: BusException) {
                busy = false
                error = refusal(e.error)
                return@launch
            }
            val online = nav.shell.state.value.online
            if (group || !online) {
                nav.shell.toast(
                    when {
                        !online -> if (group) "Review group saved · starts when the PC is back" else "New agent saved · starts when the PC is back"
                        else -> "Launching ${created.size} agents"
                    },
                )
                nav.back()
                return@launch
            }
            note = "Preparing its worktree…"
            when (val done = settled(nav, created.first().first)) {
                is Settled.Made -> {
                    nav.back()
                    nav.terminal(done.name)
                }
                is Settled.Refused -> {
                    busy = false
                    note = null
                    error = "Not created: ${done.why}. The outbox holds it until you decide."
                }
                Settled.Waiting -> {
                    nav.shell.toast("New agent · starting when the PC has made it")
                    nav.back()
                }
            }
        }
    }

    // What was written for an agent is not dropped by Back without asking.
    val written = !busy && profiles.any { it.prompt.isNotBlank() }
    BackHandler(enabled = written) { leaving = true }

    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar("New agent", { if (written) leaving = true else nav.back() }, subtitle = project?.let { crumb(it, s.workspaces) })
            if (s.projects.isEmpty()) {
                Empty("folder", "No projects yet", "Add a project on the PC first.")
                return@Column
            }
            Column(
                Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Column(Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
                    ProjectRow(project, project?.let { crumb(it, s.workspaces) }) { pickingProject = true }

                    if (group) {
                        Label("Review group")
                        Segmented(listOf(1 to "1 builder", 2 to "2 builders"), builders, { builders = it }, Modifier.fillMaxWidth(), fill = true)
                        Segmented(members.map { it to (if (it == 2) "Reviewer" else "Builder ${it + 1}") }, index, { member = it }, Modifier.fillMaxWidth().padding(top = 8.dp), fill = true)
                        T(
                            members.joinToString("  ·  ") { i -> "${if (i == 2) "Reviewer" else "Builder ${i + 1}"}: ${providerName(providerOf(profiles[i]))}, ${profiles[i].effortFor(providerOf(profiles[i]))}" },
                            Modifier.padding(start = 4.dp, top = 6.dp),
                            Relay.type.caption,
                            c.ink3,
                        )
                    }

                    // What it should do comes first, as a new conversation starts: the rest has defaults.
                    Field(
                        profile.prompt,
                        { patch(profile.copy(prompt = it)) },
                        Modifier.fillMaxWidth().padding(top = 14.dp).focusRequester(promptFocus),
                        placeholder = if (group) "Instructions for ${label(index).lowercase()} (optional)" else "What should it work on?",
                        singleLine = false,
                        minLines = 4,
                        maxLines = 12,
                        style = Relay.type.body,
                        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
                    )

                    Label("Provider")
                    if (!providerAnswer.fresh && providerAnswer.result != null) StaleNote(providerAnswer.at)
                    val chosen = providerOf(profile)
                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        for (name in listOf("claude", "codex")) {
                            val info = providers?.firstOrNull { it.provider == name }
                            val usable = providers == null || info?.installed == true
                            Pill(
                                providerName(name),
                                Modifier.heightIn(min = 36.dp).alpha(if (usable) 1f else .45f),
                                selected = chosen == name,
                                glyph = name,
                                onClick = if (usable) ({ patch(profile.copy(provider = name)) }) else ({ nav.shell.toast("${providerName(name)} is not installed on the PC") }),
                            )
                        }
                    }
                    T(account(providers, chosen), Modifier.padding(start = 4.dp, top = 6.dp), Relay.type.caption, c.ink3, maxLines = 2)

                    Label("Model")
                    Models(chosen, profile.model, owner = index) { patch(profile.copy(model = it)) }

                    Label("Reasoning effort")
                    Efforts(effortsFor(chosen), profile.effortFor(chosen)) { patch(profile.copy(effort = it)) }

                    if (leads) {
                        Label(if (group) "The group's task" else "Task")
                        StagedTask(task, open) { pickingTask = true }
                    }

                    More(
                        group = group,
                        onGroup = {
                            group = it
                            member = 0
                        },
                        role = role,
                        onRole = { role = it },
                        profile = profile,
                        leads = leads,
                        worktree = worktree,
                        onWorktree = { worktree = it },
                        onProfile = { patch(it) },
                    )

                    T(
                        if (group) {
                            "The group shares one fresh worktree and branch. The reviewer starts first, so its mailbox is live before the builders publish anything."
                        } else {
                            "A solo agent gets the project's default checkout: its own worktree, or the primary checkout when a plugin asks for it."
                        },
                        Modifier.padding(start = 4.dp, top = 16.dp, bottom = 20.dp),
                        Relay.type.caption,
                        c.ink3,
                    )
                }
            }
            Footer(
                when {
                    error != null -> error!!
                    note != null -> note!!
                    !s.online -> "The PC is away · starts when it is back"
                    else -> null
                },
                error != null,
                if (group) "Launch review group" else "Launch agent",
                enabled = project != null && !busy,
                onStart = ::launch,
            )
        }
    }

    if (pickingProject) {
        ProjectSheet(s.projects, s.workspaces, pid, onPick = { p ->
            if (p.num != pid) task = null
            picked = p.num
        }, onDismiss = { pickingProject = false })
    }
    if (pickingTask) {
        TaskSheet(open, task, onPick = { task = it }, onDismiss = { pickingTask = false })
    }
    if (leaving) {
        AskSheet("Discard this agent?", "What you wrote for it is not kept.", "Discard", danger = true, onConfirm = { nav.back() }, onDismiss = { leaving = false })
    }
}

// ---- The form's state ----

/** One agent's launch settings (launch.rs `Profile`). A null provider follows the first installed one. */
private data class Profile(
    val provider: String? = null,
    val model: String = "",
    val effort: String = DEFAULT_EFFORT,
    val prompt: String = "",
    val writes: Boolean = true,
    val ui: Boolean = false,
) {
    /** The chosen effort, or the default when this provider does not take it. */
    fun effortFor(provider: String) = effort.takeIf { it in effortsFor(provider) } ?: DEFAULT_EFFORT
}

private const val DEFAULT_EFFORT = "high"

private val PROFILES = Saver<List<Profile>, ArrayList<Any>>(
    save = { list -> ArrayList(list.flatMap { listOf(it.provider.orEmpty(), it.model, it.effort, it.prompt, it.writes, it.ui) }) },
    restore = { flat ->
        flat.chunked(6).map {
            Profile((it[0] as String).ifEmpty { null }, it[1] as String, it[2] as String, it[3] as String, it[4] as Boolean, it[5] as Boolean)
        }
    },
)

private class ProviderInfo(val provider: String, val installed: Boolean, val version: String?, val signedInAs: String?)

/** `provider.list`'s answer; null until one has been read. */
private fun providersOf(result: JsonElement?): List<ProviderInfo>? {
    val o = result as? JsonObject ?: return null
    return o.arr("providers").mapNotNull { e ->
        val p = e as? JsonObject ?: return@mapNotNull null
        ProviderInfo(p.s("provider") ?: return@mapNotNull null, p.b("installed") == true, p.s("version"), p.s("signed_in_as"))
    }
}

/** A provider as a sentence names it: "Claude Code", "Codex", or its id. */
internal fun providerName(p: String) = when (p) {
    "claude" -> "Claude Code"
    "codex" -> "Codex"
    else -> p
}

/** A create's session name: the PC's answer, or a reference the outbox fills in once it has one. */
private fun refOf(c: Hub.Change): JsonElement = when (c) {
    is Hub.Change.Now -> JsonPrimitive((c.result as? JsonObject)?.s("name").orEmpty())
    is Hub.Change.Queued -> Refs.ref(c.entry.id, "name")
}

private sealed interface Settled {
    data class Made(val name: String) : Settled
    data class Refused(val why: String) : Settled
    data object Waiting : Settled
}

/** How a create ends: made (with its name), refused, or still waiting when the PC goes away or takes too long. */
private suspend fun settled(nav: Nav, change: Hub.Change): Settled {
    val entry = when (change) {
        is Hub.Change.Now -> return (change.result as? JsonObject)?.s("name")?.let { Settled.Made(it) } ?: Settled.Waiting
        is Hub.Change.Queued -> change.entry
    }
    val hub = nav.relay.hub
    val end = withTimeoutOrNull(OutboxRunner.timeoutFor("session.create")) {
        var e = hub.outbox.get(entry.id)
        while (e != null && e.state == OutboxEntry.State.Pending && nav.shell.state.value.online) {
            delay(250)
            e = hub.outbox.get(entry.id)
        }
        e
    }
    return when {
        end?.state == OutboxEntry.State.Done -> (end.result as? JsonObject)?.s("name")?.let { Settled.Made(it) } ?: Settled.Waiting
        end?.parked == true -> Settled.Refused(end.error?.let(::refusal) ?: "the PC refused it")
        else -> Settled.Waiting
    }
}

// ---- Pieces ----

@Composable
private fun Label(text: String) = Eyebrow(text, Modifier.padding(start = 4.dp, top = 18.dp, bottom = 8.dp))

@Composable
private fun ProjectRow(project: Project?, crumb: String?, onClick: () -> Unit) {
    val c = Relay.colors
    Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 4.dp, vertical = 2.dp), onClick = onClick) {
        ListRow {
            Glyph("folder", 17.dp, c.ink2)
            Column(Modifier.weight(1f)) {
                T(crumb ?: "Pick a project", style = Relay.type.uiMedium, color = c.ink, maxLines = 1)
                project?.let { T(it.baseBranch, style = Relay.type.mono, color = c.ink3, maxLines = 1) }
            }
            Glyph("chevron-down", 13.dp, c.ink3)
        }
    }
}

/** A choice card (launch.rs): a title, what it means, a check when chosen. */
@Composable
private fun Choice(title: String, detail: String, selected: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val c = Relay.colors
    Slab(
        modifier.heightIn(min = 72.dp),
        padding = PaddingValues(12.dp),
        color = if (selected) c.wash else c.slab,
        edge = if (selected) c.lineFocus else c.edge,
        onClick = onClick,
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            T(title, Modifier.weight(1f), Relay.type.uiMedium, c.ink, maxLines = 1)
            if (selected) Glyph("check", 15.dp, c.ink)
        }
        T(detail, Modifier.padding(top = 4.dp), Relay.type.caption, c.ink3, maxLines = 3)
    }
}

/** The chosen provider's account and version, as the PC's provider cards say them. */
private fun account(providers: List<ProviderInfo>?, name: String): String {
    val info = providers?.firstOrNull { it.provider == name }
    return when {
        providers == null -> "Checked when the PC is in reach"
        info == null || !info.installed -> "${providerName(name)} is not installed on the PC"
        else -> listOfNotNull(info.signedInAs?.let { "Signed in as $it" } ?: "Not signed in", info.version).joinToString(" · ")
    }
}

/** A model id as the pickers name it: Claude's by name, the provider's default, or the id itself. */
internal fun modelName(id: String): String =
    if (id.isBlank()) "Provider default" else MODELS.firstOrNull { it.first == id }?.second ?: id

/** The provider's default, Claude's models by name, or any model id. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun Models(provider: String, model: String, owner: Any, onModel: (String) -> Unit) {
    val named = if (provider == "claude") MODELS.filter { it.first.isNotEmpty() } else emptyList()
    // Per agent: a review group's members each keep their own choice.
    var other by rememberSaveable(provider, owner) { mutableStateOf(model.isNotEmpty() && named.none { it.first == model }) }
    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Pill("Default", Modifier.heightIn(min = 36.dp), selected = model.isEmpty() && !other, onClick = { other = false; onModel("") })
        for ((id, name) in named) Pill(name, Modifier.heightIn(min = 36.dp), selected = model == id && !other, onClick = { other = false; onModel(id) })
        Pill("Other", Modifier.heightIn(min = 36.dp), selected = other, onClick = { other = true })
    }
    if (other) {
        Field(
            model,
            onModel,
            Modifier.fillMaxWidth().padding(top = 8.dp),
            placeholder = "A model id",
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false),
        )
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Efforts(choices: List<String>, selected: String, onPick: (String) -> Unit) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        for (e in choices) Pill(e, Modifier.heightIn(min = 36.dp), selected = e == selected, onClick = { onPick(e) })
    }
}

/** The task the agent begins with, or a key to pick one of the project's open tasks. */
@Composable
private fun StagedTask(taskId: Long?, open: List<Task>, onPick: () -> Unit) {
    val c = Relay.colors
    val t = open.firstOrNull { it.num == taskId }
    Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 4.dp, vertical = 2.dp), onClick = onPick) {
        ListRow {
            if (taskId == null) {
                Glyph("board", 16.dp, c.ink3)
                T(if (open.isEmpty()) "No open tasks · launch without one" else "No task · pick one of ${open.size} open", Modifier.weight(1f), Relay.type.ui, c.ink2, maxLines = 1)
            } else {
                Dot(Palette.column(t?.column ?: "backlog", c), 7.dp)
                Column(Modifier.weight(1f)) {
                    T(t?.title ?: "Task #$taskId", style = Relay.type.uiMedium, color = c.ink, maxLines = 2)
                    T("#$taskId" + (t?.let { " · ${Task.columnLabel(it.column)}" } ?: ""), style = Relay.type.mono, color = c.ink3, maxLines = 1)
                }
            }
            Glyph("chevron-down", 13.dp, c.ink3)
        }
    }
}

/** What most launches leave as it is: solo or a review group, the role, the worktree and the permissions. */
@Composable
private fun More(
    group: Boolean,
    onGroup: (Boolean) -> Unit,
    role: String,
    onRole: (String) -> Unit,
    profile: Profile,
    leads: Boolean,
    worktree: String,
    onWorktree: (String) -> Unit,
    onProfile: (Profile) -> Unit,
) {
    val c = Relay.colors
    var open by rememberSaveable { mutableStateOf(false) }
    Slab(Modifier.fillMaxWidth().padding(top = 18.dp), padding = PaddingValues(horizontal = 4.dp, vertical = 2.dp)) {
        ListRow(onClick = { open = !open }) {
            Glyph("sliders", 16.dp, c.ink2)
            Column(Modifier.weight(1f)) {
                T("More options", style = Relay.type.uiMedium, color = c.ink)
                if (!open) {
                    T(
                        listOfNotNull(if (group) "Review group" else "Solo", if (!group) role.replaceFirstChar(Char::uppercase) else null, worktree.trim().ifEmpty { null }).joinToString(" · "),
                        style = Relay.type.caption,
                        color = c.ink3,
                        maxLines = 1,
                    )
                }
            }
            Glyph(if (open) "chevron-up" else "chevron-down", 13.dp, c.ink3)
        }
        if (open) {
            Column(Modifier.padding(start = 10.dp, end = 10.dp, bottom = 8.dp)) {
                Eyebrow("Session type", Modifier.padding(top = 4.dp, bottom = 6.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Choice("Solo", "Its own worktree and branch.", !group, Modifier.weight(1f)) { onGroup(false) }
                    Choice("Review group", "Builders and a reviewer share one branch.", group, Modifier.weight(1f)) { onGroup(true) }
                }
                if (!group) {
                    Eyebrow("Role", Modifier.padding(top = 14.dp, bottom = 6.dp))
                    Segmented(listOf("builder" to "Builder", "reviewer" to "Reviewer", "docs" to "Docs"), role, onRole, Modifier.fillMaxWidth(), fill = true)
                }
                if (leads) {
                    Eyebrow("Worktree", Modifier.padding(top = 14.dp, bottom = 6.dp))
                    Field(
                        worktree,
                        onWorktree,
                        Modifier.fillMaxWidth(),
                        placeholder = "Project default — or new, primary, or a path",
                        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false),
                    )
                }
                Hairline(Modifier.padding(top = 10.dp))
                SwitchRow("Allow agent bus writes", "Let the agent create tasks, notes and mail.", profile.writes, { onProfile(profile.copy(writes = it)) })
                SwitchRow("Allow UI control", "Let the agent drive the desktop's panes and pages.", profile.ui, { onProfile(profile.copy(ui = it)) })
            }
        }
    }
}

/** The fixed foot: what is happening or wrong, and the one primary key. */
@Composable
private fun Footer(line: String?, bad: Boolean, start: String, enabled: Boolean, onStart: () -> Unit) {
    val c = Relay.colors
    Column(Modifier.fillMaxWidth().navigationBarsPadding()) {
        Hairline(color = c.edge)
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            T(line.orEmpty(), Modifier.weight(1f), Relay.type.caption, if (bad) c.heldText else c.ink3, maxLines = 3)
            Key(start, onStart, kind = KeyKind.Primary, glyph = "play", enabled = enabled)
        }
    }
}

/** The project's open tasks, searchable and narrowed by column. */
@Composable
private fun TaskSheet(open: List<Task>, current: Long?, onPick: (Long?) -> Unit, onDismiss: () -> Unit) {
    val c = Relay.colors
    var query by rememberSaveable { mutableStateOf("") }
    var column by rememberSaveable { mutableStateOf("all") }
    val q = query.trim().lowercase()
    val shown = open.filter { t ->
        (column == "all" || t.column == column) && (q.isEmpty() || "${t.ref} ${t.title} ${t.body}".lowercase().contains(q))
    }
    DevSheet(onDismiss, "Assign the work", "The agent begins with this task. In a review group it is queued for every member.") { close ->
        Field(query, { query = it }, Modifier.fillMaxWidth().padding(horizontal = 6.dp), placeholder = "Search tasks", leading = "search")
        Row(
            Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 6.dp, vertical = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            for ((key, text) in listOf("all" to "All open", "backlog" to "Backlog", "ready" to "Ready", "active" to "Active", "in_review" to "In review")) {
                Pill(text, Modifier.heightIn(min = 34.dp), selected = column == key, onClick = { column = key })
            }
        }
        if (current != null) SheetRow("No task", "close", { close { onPick(null) } })
        if (shown.isEmpty()) {
            T(if (open.isEmpty()) "No open tasks. You can launch without an assignment." else "Nothing matches.", Modifier.padding(10.dp), Relay.type.caption, c.ink3)
        }
        for (t in shown.take(80)) {
            ListRow(selected = t.num == current, onClick = { close { onPick(t.num) } }, padding = PaddingValues(horizontal = 10.dp, vertical = 8.dp)) {
                Box(Modifier.padding(top = 1.dp)) { Dot(Palette.column(t.column, c), 7.dp) }
                Column(Modifier.weight(1f)) {
                    T(t.title.ifBlank { "Untitled" }, style = Relay.type.ui, color = c.ink, maxLines = 2)
                    T("${t.ref} · ${Task.columnLabel(t.column)}", style = Relay.type.mono, color = c.ink3, maxLines = 1)
                }
                if (t.num == current) Glyph("check", 15.dp, c.ink)
            }
        }
    }
}
