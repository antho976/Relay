package com.tally.app.ui.nav

import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NamedNavArgument
import androidx.navigation.NavGraphBuilder
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.tally.app.ui.accounts.AccountEditRoute
import com.tally.app.ui.accounts.AccountsRoute
import com.tally.app.ui.activity.EntriesRoute
import com.tally.app.ui.activity.SearchRoute
import com.tally.app.ui.activity.TransactionsRoute
import com.tally.app.ui.categories.CategoriesRoute
import com.tally.app.ui.categories.CategoryEditRoute
import com.tally.app.ui.entry.EntryRoute
import com.tally.app.ui.home.OverviewRoute
import com.tally.app.ui.insights.InsightsRoute
import com.tally.app.ui.invest.InvestRoute
import com.tally.app.ui.invest.RoomEditRoute
import com.tally.app.ui.onboarding.OnboardingRoute
import com.tally.app.ui.plan.BillEditRoute
import com.tally.app.ui.plan.BillsRoute
import com.tally.app.ui.plan.BudgetEditRoute
import com.tally.app.ui.plan.BudgetsRoute
import com.tally.app.ui.plan.GoalEditRoute
import com.tally.app.ui.plan.GoalRoute
import com.tally.app.ui.plan.GoalsRoute
import com.tally.app.ui.settings.AboutRoute
import com.tally.app.ui.settings.AppearanceRoute
import com.tally.app.ui.settings.BackupRoute
import com.tally.app.ui.settings.ExportRoute
import com.tally.app.ui.settings.FormatRoute
import com.tally.app.ui.settings.ImportRoute
import com.tally.app.ui.settings.IncomingFiles
import com.tally.app.ui.settings.SettingsRoute
import com.tally.app.ui.settings.SyncRoute
import com.tally.app.ui.shell.ShellViewModel
import com.tally.app.ui.shell.SpaceFrame
import com.tally.core.TxType

private fun longArg(name: String) = navArgument(name) { type = NavType.LongType; defaultValue = 0L }
private fun stringArg(name: String, default: String) = navArgument(name) { type = NavType.StringType; defaultValue = default }

@Composable
fun TallyNavHost(onboarded: Boolean, incoming: IncomingFiles? = null, launches: Launches? = null) {
    val controller = rememberNavController()
    val nav = remember(controller) { AppNav(controller) }
    // Decided once: flipping onboarded later must not swap the graph's root under the person.
    val start = remember { if (onboarded) Routes.OVERVIEW else Routes.ONBOARDING }
    // One for the Activity: every page's frame reads the same sync and month line. Read inside each
    // page, never here: a graph builder that captured a changing value would be rebuilt with it.
    val shellVm: ShellViewModel = hiltViewModel()

    // A CSV shared into Tally opens the import, once the person is past the first run.
    if (incoming != null) {
        LaunchedEffect(incoming, onboarded) {
            incoming.pending.collect { uri -> if (uri != null && onboarded) nav.importShared() }
        }
    }

    // A launcher shortcut: add an entry, or search, once the person is past the first run.
    if (launches != null) {
        LaunchedEffect(launches, onboarded) {
            launches.pending.collect { action ->
                if (action == null || !onboarded) return@collect
                when (launches.take()) {
                    Launches.ADD -> nav.entry()
                    Launches.SEARCH -> nav.search()
                }
            }
        }
    }

    // Relay's motion: quick, a short slide for a page pushed over another, a fade between siblings.
    val enter: EnterTransition = slideInHorizontally(tween(220)) { it / 10 } + fadeIn(tween(180))
    val exit: ExitTransition = fadeOut(tween(120))
    val popEnter: EnterTransition = fadeIn(tween(160))
    val popExit: ExitTransition = slideOutHorizontally(tween(200)) { it / 10 } + fadeOut(tween(160))

    NavHost(
        navController = controller,
        startDestination = start,
        enterTransition = { if (initialState.destination.route in Routes.SPACE_PAGES && targetState.destination.route in Routes.SPACE_PAGES) fadeIn(tween(140)) else enter },
        exitTransition = { if (initialState.destination.route in Routes.SPACE_PAGES && targetState.destination.route in Routes.SPACE_PAGES) fadeOut(tween(100)) else exit },
        popEnterTransition = { popEnter },
        popExitTransition = { popExit },
    ) {
        composable(Routes.ONBOARDING) { OnboardingRoute(nav) }

        space(Routes.OVERVIEW, nav, shellVm, add = { nav.entry() }) { OverviewRoute(nav, shellVm::deleteEntry) }
        space(Routes.ENTRIES, nav, shellVm, add = { nav.entry() }) { EntriesRoute(nav, shellVm::deleteEntry) }
        space(Routes.INSIGHTS, nav, shellVm, add = { nav.entry() }) { InsightsRoute(nav, shellVm::deleteEntry) }
        space(Routes.BUDGETS, nav, shellVm, add = { nav.entry() }) { BudgetsRoute(nav) }
        space(Routes.BILLS, nav, shellVm, add = { nav.billEdit() }) { BillsRoute(nav) }
        space(Routes.GOALS, nav, shellVm, add = { nav.goalEdit() }) { GoalsRoute(nav) }
        space(Routes.ACCOUNTS, nav, shellVm, add = { nav.entry(type = TxType.TRANSFER) }) { AccountsRoute(nav) }
        space(Routes.INVESTMENTS, nav, shellVm, add = null) { InvestRoute(nav) }

        composable(Routes.SEARCH) { SearchRoute(nav, shellVm::deleteEntry) }
        composable(
            Routes.ENTRY,
            arguments = listOf(longArg(Args.ID), stringArg(Args.TYPE, "EXPENSE"), longArg(Args.ACCOUNT), longArg(Args.CATEGORY)),
        ) { EntryRoute(nav) }
        composable(Routes.TRANSACTIONS, arguments = listOf(longArg(Args.CATEGORY), longArg(Args.ACCOUNT))) { TransactionsRoute(nav, shellVm::deleteEntry) }
        composable(Routes.ACCOUNT_EDIT, arguments = listOf(longArg(Args.ID))) { AccountEditRoute(nav) }
        composable(Routes.CATEGORIES) { CategoriesRoute(nav) }
        composable(Routes.CATEGORY_EDIT, arguments = listOf(longArg(Args.ID), stringArg(Args.KIND, "EXPENSE"))) { CategoryEditRoute(nav) }
        composable(Routes.BUDGET_EDIT, arguments = listOf(longArg(Args.CATEGORY))) { BudgetEditRoute(nav) }
        composable(Routes.BILL_EDIT, arguments = listOf(longArg(Args.ID))) { BillEditRoute(nav) }
        composable(Routes.GOAL, arguments = listOf(longArg(Args.ID))) { GoalRoute(nav) }
        composable(Routes.GOAL_EDIT, arguments = listOf(longArg(Args.ID))) { GoalEditRoute(nav) }
        composable(Routes.SETTINGS) { SettingsRoute(nav) }
        composable(Routes.SYNC) { SyncRoute(nav) }
        composable(Routes.APPEARANCE) { AppearanceRoute(nav) }
        composable(Routes.FORMAT) { FormatRoute(nav) }
        composable(Routes.BACKUP) { BackupRoute(nav) }
        composable(Routes.EXPORT) { ExportRoute(nav) }
        composable(Routes.IMPORT, arguments = listOf(stringArg(Args.SOURCE, ""))) { ImportRoute(nav) }
        composable(Routes.ABOUT) { AboutRoute(nav) }
        composable(Routes.ROOM_EDIT, arguments = listOf(stringArg(Args.KIND, "TFSA"))) { RoomEditRoute(nav) }
    }
}

/** A space's page: Relay's frame around it. */
private fun NavGraphBuilder.space(
    route: String,
    nav: AppNav,
    shellVm: ShellViewModel,
    add: (() -> Unit)?,
    arguments: List<NamedNavArgument> = emptyList(),
    content: @Composable () -> Unit,
) {
    composable(route, arguments = arguments) {
        val shell by shellVm.state.collectAsStateWithLifecycle()
        SpaceFrame(nav, route, shell, onSync = shellVm::syncNow, onAdd = add) { content() }
    }
}
