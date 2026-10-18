package com.quietsoftware.relay.ui

import android.net.Uri
import androidx.compose.runtime.Composable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.navigation.NavHostController
import com.quietsoftware.relay.data.Relay
import com.quietsoftware.relay.ui.shell.ShellModel
import com.quietsoftware.relay.ui.shell.SidebarNav

/**
 * Every place in the app, by name. Screens never touch the NavController: they call these,
 * which keeps routes and their arguments in one file.
 */
class Nav(val controller: NavHostController, val shell: ShellModel) : SidebarNav {
    val relay: Relay get() = shell.relay

    /** Set while [instead] runs: the page opened takes the place of the one showing. */
    private var replacing = false

    private fun go(route: String, top: Boolean = false) {
        val here = if (replacing) controller.currentBackStackEntry?.destination?.id else null
        controller.navigate(route) {
            launchSingleTop = true
            if (top) popUpTo(0) { inclusive = true } else if (here != null) popUpTo(here) { inclusive = true }
        }
    }

    /** Opens what [page] opens in place of this page, so back does not come to it again (Share). */
    fun instead(page: Nav.() -> Unit) {
        replacing = true
        try {
            page()
        } finally {
            replacing = false
        }
    }

    private fun q(vararg pairs: Pair<String, Any?>): String =
        pairs.filter { it.second != null }.joinToString("&", "?") { (k, v) -> "$k=${Uri.encode(v.toString())}" }.let { if (it == "?") "" else it }

    fun back() {
        if (!controller.popBackStack()) go(if (shell.state.value.prefs.space == "threads") "threads" else "agents", top = true)
    }

    fun start() = go("start", top = true)

    /** The space's home: the agent wall for Dev, a new thread for Threads. */
    fun space(space: String) {
        shell.pickSpace(space)
        go(if (space == "threads") "threads" else "agents", top = true)
    }

    fun agents() = go("agents", top = true)
    /** An agent's own page: its conversation ([chat]), with its terminal one tap under it. */
    fun terminal(name: String) = chat(name)

    /** The agent as a conversation; back to it when the screen under this one already is. */
    fun chat(name: String) = swap("chat", name)

    /** The agent's raw terminal, for what a conversation cannot show. */
    fun console(name: String) = swap("terminal", name)

    /** Chat and terminal open each other: going back to the one below beats stacking them. */
    private fun swap(screen: String, name: String) {
        val below = controller.previousBackStackEntry
        if (below?.destination?.route == "$screen/{name}" && below.arguments?.getString("name") == name) controller.popBackStack()
        else go("$screen/${Uri.encode(name)}")
    }
    fun session(name: String) = go("session/${Uri.encode(name)}")
    fun launch(projectId: Long? = null, taskId: Long? = null, prompt: String? = null) = go("launch" + q("project" to projectId, "task" to taskId, "prompt" to prompt))
    fun board(projectId: Long?) = go("board" + q("project" to projectId))

    /** A project's board from a page opened on it: back to it when it is the screen below, rather than a second one. */
    fun boardOf(projectId: Long) {
        val below = controller.previousBackStackEntry
        val shows = below?.arguments?.getString("project")?.toLongOrNull() ?: shell.state.value.project?.num
        if (below?.destination?.route == "board?project={project}" && shows == projectId) controller.popBackStack() else board(projectId)
    }
    fun task(id: String) = go("task/${Uri.encode(id)}")
    fun newTask(projectId: Long?, parentId: Long? = null, column: String? = null) = go("task-edit" + q("project" to projectId, "parent" to parentId, "column" to column))
    fun editTask(id: String) = go("task-edit" + q("id" to id))
    fun notes(projectId: Long?) = go("notes" + q("project" to projectId))
    fun note(id: String) = go("note" + q("id" to id))
    fun newNote(projectId: Long?) = go("note" + q("project" to projectId))
    fun files(projectId: Long, worktree: String? = null) = go("files/$projectId" + q("worktree" to worktree))
    fun file(projectId: Long, path: String, worktree: String? = null, line: Int? = null) = go("file/$projectId" + q("path" to path, "worktree" to worktree, "line" to line))
    fun git(projectId: Long, worktree: String? = null, session: String? = null) = go("git/$projectId" + q("worktree" to worktree, "session" to session))
    /** A project's design canvases, and one of them (read only). */
    fun designs(projectId: Long) = go("designs/$projectId")
    fun design(id: Long) = go("design/$id")
    fun mailbox(projectId: Long, session: String? = null) = go("mailbox/$projectId" + q("session" to session))
    fun modules(projectId: Long) = go("modules/$projectId")
    fun guardrails(projectId: Long?) = go("guardrails" + q("project" to projectId))
    /** One skill or plugin, read only, from its list. */
    fun skill(id: Long) = go("skill/$id")
    fun plugin(id: String) = go("plugin/${Uri.encode(id)}")
    fun search() = go("search")
    override fun projectSettings(id: Long) = go("project-settings/$id")
    fun devices(projectId: Long) = go("devices/$projectId")
    fun integration(projectId: Long) = go("integration/$projectId")
    override fun addProject() = go("add-project")
    fun pcTools() = go("pc-tools")
    fun share(text: String) = go("share" + q("text" to text))
    /** Pairing; [top] clears what was behind it, as forgetting the PC does. */
    fun pair(link: String? = null, top: Boolean = false) = go("pair" + q("link" to link), top)

    override fun board() = board(shell.state.value.project?.num)
    override fun notes() = notes(shell.state.value.project?.num)
    override fun inbox() = go("inbox")
    override fun outbox() = go("outbox")
    override fun skills() = go("skills")
    override fun plugins() = go("plugins")
    override fun project(id: Long) {
        shell.pickProject(id)
        go("agents", top = true)
    }
    override fun pc() = go("pc")
    override fun settings() = go("settings")
    override fun newThread() = go("threads", top = true)
    override fun thread(id: String) = go("thread/${Uri.encode(id)}")
    override fun tally() = go("tally")
    override fun arbiter() = go("arbiter")
    override fun avex() = go("avex")
    /** One of Avex's lifts, by its name as Avex shows it. */
    fun avexLift(name: String) = go("avex-lift" + q("name" to name))
    /** One of Arbiter's strategies. */
    fun arbiterStrategy(id: Long) = go("arbiter-strategy/$id")
}

val LocalNav = staticCompositionLocalOf<Nav> { error("No Nav") }

/** The nav for screens that need it without threading it through every call. */
val nav: Nav @Composable get() = LocalNav.current
