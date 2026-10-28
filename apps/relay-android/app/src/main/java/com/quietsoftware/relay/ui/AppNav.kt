package com.quietsoftware.relay.ui

import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavBackStackEntry
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.navArgument
import com.quietsoftware.relay.ui.code.DesignPageScreen
import com.quietsoftware.relay.ui.code.DesignScreen
import com.quietsoftware.relay.ui.code.DesignsScreen
import com.quietsoftware.relay.ui.code.FileScreen
import com.quietsoftware.relay.ui.code.FilesScreen
import com.quietsoftware.relay.ui.code.GitScreen
import com.quietsoftware.relay.ui.dev.AgentsScreen
import com.quietsoftware.relay.ui.dev.ChatScreen
import com.quietsoftware.relay.ui.dev.LaunchScreen
import com.quietsoftware.relay.ui.kit.LocalStaleLamps
import com.quietsoftware.relay.ui.dev.SessionScreen
import com.quietsoftware.relay.ui.dev.TerminalScreen
import com.quietsoftware.relay.ui.pair.PairEntry
import com.quietsoftware.relay.ui.pair.PcScreen
import com.quietsoftware.relay.ui.pages.GuardrailsScreen
import com.quietsoftware.relay.ui.pages.InboxScreen
import com.quietsoftware.relay.ui.pages.MailboxScreen
import com.quietsoftware.relay.ui.pages.NoteScreen
import com.quietsoftware.relay.ui.pages.NotesScreen
import com.quietsoftware.relay.ui.pages.OutboxScreen
import com.quietsoftware.relay.ui.pages.PluginScreen
import com.quietsoftware.relay.ui.pages.PluginsScreen
import com.quietsoftware.relay.ui.pages.SearchScreen
import com.quietsoftware.relay.ui.pages.SettingsScreen
import com.quietsoftware.relay.ui.pages.ShareScreen
import com.quietsoftware.relay.ui.pages.SkillScreen
import com.quietsoftware.relay.ui.pages.SkillsScreen
import com.quietsoftware.relay.ui.start.StartScreen
import com.quietsoftware.relay.ui.home.HomeScreen
import com.quietsoftware.relay.ui.tasks.BoardScreen
import com.quietsoftware.relay.ui.tasks.ModulesScreen
import com.quietsoftware.relay.ui.tasks.TaskEditScreen
import com.quietsoftware.relay.ui.tasks.TaskScreen
import com.quietsoftware.relay.ui.threads.ArbiterScreen
import com.quietsoftware.relay.ui.threads.ArbiterStrategyScreen
import com.quietsoftware.relay.ui.threads.AvexLiftScreen
import com.quietsoftware.relay.ui.threads.AvexScreen
import com.quietsoftware.relay.ui.threads.TallyScreen
import com.quietsoftware.relay.ui.threads.ThreadScreen
import com.quietsoftware.relay.ui.threads.ThreadsHome
import com.quietsoftware.relay.ui.tools.AddProjectScreen
import com.quietsoftware.relay.ui.tools.DevicesScreen
import com.quietsoftware.relay.ui.tools.IntegrationScreen
import com.quietsoftware.relay.ui.tools.PcToolsScreen
import com.quietsoftware.relay.ui.tools.ProjectSettingsScreen
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map

private fun NavBackStackEntry.str(key: String): String? = arguments?.getString(key)?.takeIf { it.isNotEmpty() }
private fun NavBackStackEntry.long(key: String): Long? = str(key)?.toLongOrNull()

private fun opt(name: String) = navArgument(name) { type = NavType.StringType; nullable = true; defaultValue = null }

/** Every destination and the screen that draws it. Arguments are strings; screens get them typed. */
@Composable
fun AppNav(nav: Nav, startRoute: String) {
    val stale by remember(nav) { nav.shell.state.map { !it.online }.distinctUntilChanged() }.collectAsStateWithLifecycle(false)
    CompositionLocalProvider(LocalStaleLamps provides stale) { Destinations(nav, startRoute) }
}

@Composable
private fun Destinations(nav: Nav, startRoute: String) {
    NavHost(
        navController = nav.controller,
        startDestination = startRoute,
        enterTransition = { fadeIn() },
        exitTransition = { fadeOut() },
    ) {
        composable("start") { StartScreen(nav) }
        composable("home") { HomeScreen(nav) }
        composable("agents") { AgentsScreen(nav) }
        composable("chat/{name}") { ChatScreen(it.str("name").orEmpty(), nav) }
        composable("terminal/{name}") { TerminalScreen(it.str("name").orEmpty(), nav) }
        composable("session/{name}") { SessionScreen(it.str("name").orEmpty(), nav) }
        composable("launch?project={project}&task={task}&prompt={prompt}", listOf(opt("project"), opt("task"), opt("prompt"))) {
            LaunchScreen(it.long("project"), it.long("task"), it.str("prompt"), nav)
        }
        composable("board?project={project}", listOf(opt("project"))) { BoardScreen(it.long("project"), nav) }
        composable("task/{id}") { TaskScreen(it.str("id").orEmpty(), nav) }
        composable("task-edit?id={id}&project={project}&parent={parent}&column={column}", listOf(opt("id"), opt("project"), opt("parent"), opt("column"))) {
            TaskEditScreen(it.str("id"), it.long("project"), it.long("parent"), it.str("column"), nav)
        }
        composable("modules/{project}") { ModulesScreen(it.long("project") ?: 0, nav) }
        composable("notes?project={project}", listOf(opt("project"))) { NotesScreen(it.long("project"), nav) }
        composable("note?id={id}&project={project}", listOf(opt("id"), opt("project"))) { NoteScreen(it.str("id"), it.long("project"), nav) }
        composable("inbox") { InboxScreen(nav) }
        composable("outbox") { OutboxScreen(nav) }
        composable("search") { SearchScreen(nav) }
        composable("share?text={text}", listOf(opt("text"))) { ShareScreen(it.str("text").orEmpty(), nav) }
        composable("skills") { SkillsScreen(nav) }
        composable("plugins") { PluginsScreen(nav) }
        composable("skill/{id}") { SkillScreen(it.long("id") ?: 0, nav) }
        composable("plugin/{id}") { PluginScreen(it.str("id").orEmpty(), nav) }
        composable("guardrails?project={project}", listOf(opt("project"))) { GuardrailsScreen(it.long("project"), nav) }
        composable("mailbox/{project}?session={session}", listOf(opt("session"))) { MailboxScreen(it.long("project") ?: 0, it.str("session"), nav) }
        composable("files/{project}?worktree={worktree}", listOf(opt("worktree"))) { FilesScreen(it.long("project") ?: 0, it.str("worktree"), nav) }
        composable("file/{project}?path={path}&worktree={worktree}&line={line}", listOf(opt("path"), opt("worktree"), opt("line"))) {
            FileScreen(it.long("project") ?: 0, it.str("path").orEmpty(), it.str("worktree"), it.str("line")?.toIntOrNull(), nav)
        }
        composable("git/{project}?worktree={worktree}&session={session}", listOf(opt("worktree"), opt("session"))) {
            GitScreen(it.long("project") ?: 0, it.str("worktree"), it.str("session"), nav)
        }
        composable("designs/{project}") { DesignsScreen(it.long("project") ?: 0, nav) }
        composable("design/{id}") { DesignScreen(it.long("id") ?: 0, nav) }
        composable("design/{id}/page/{page}") { DesignPageScreen(it.long("id") ?: 0, it.str("page").orEmpty(), nav) }
        composable("threads") { ThreadsHome(nav) }
        composable("thread/{id}") { ThreadScreen(it.str("id").orEmpty(), nav) }
        composable("tally") { TallyScreen(nav) }
        composable("arbiter") { ArbiterScreen(nav) }
        composable("avex") { AvexScreen(nav) }
        composable("avex-lift?name={name}", listOf(opt("name"))) { AvexLiftScreen(it.str("name").orEmpty(), nav) }
        composable("arbiter-strategy/{id}") { ArbiterStrategyScreen(it.long("id") ?: 0, nav) }
        composable("pair?link={link}", listOf(opt("link"))) { PairEntry(it.str("link"), nav) }
        composable("pc") { PcScreen(nav) }
        composable("settings") { SettingsScreen(nav) }
        composable("project-settings/{project}") { ProjectSettingsScreen(it.long("project") ?: 0, nav) }
        composable("devices/{project}") { DevicesScreen(it.long("project") ?: 0, nav) }
        composable("integration/{project}") { IntegrationScreen(it.long("project") ?: 0, nav) }
        composable("add-project") { AddProjectScreen(nav) }
        composable("pc-tools") { PcToolsScreen(nav) }
    }
}
