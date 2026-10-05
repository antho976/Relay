package com.tally.app.ui.nav

import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.tally.app.ui.accounts.AccountEditRoute
import com.tally.app.ui.accounts.AccountsRoute
import com.tally.app.ui.activity.HistoryRoute
import com.tally.app.ui.activity.TransactionsRoute
import com.tally.app.ui.categories.CategoriesRoute
import com.tally.app.ui.categories.CategoryEditRoute
import com.tally.app.ui.entry.EntryRoute
import com.tally.app.ui.onboarding.OnboardingRoute
import com.tally.app.ui.plan.BillEditRoute
import com.tally.app.ui.plan.BudgetEditRoute
import com.tally.app.ui.plan.GoalEditRoute
import com.tally.app.ui.plan.GoalRoute
import com.tally.app.ui.settings.AboutRoute
import com.tally.app.ui.settings.AppearanceRoute
import com.tally.app.ui.settings.BackupRoute
import com.tally.app.ui.settings.ExportRoute
import com.tally.app.ui.settings.FormatRoute
import com.tally.app.ui.settings.ImportRoute
import com.tally.app.ui.settings.IncomingFiles
import com.tally.app.ui.settings.SettingsRoute
import com.tally.app.ui.theme.TallyMotion

private fun longArg(name: String) = navArgument(name) { type = NavType.LongType; defaultValue = 0L }
private fun stringArg(name: String, default: String) = navArgument(name) { type = NavType.StringType; defaultValue = default }

@Composable
fun TallyNavHost(onboarded: Boolean, incoming: IncomingFiles? = null) {
    val controller = rememberNavController()
    val nav = remember(controller) { AppNav(controller) }
    // Decided once: flipping onboarded later must not swap the graph's root under the user.
    val start = remember { if (onboarded) Routes.HUB else Routes.ONBOARDING }

    // A CSV shared into Tally opens the import, once the owner is past the first run.
    if (incoming != null) {
        LaunchedEffect(incoming, onboarded) {
            incoming.pending.collect { uri -> if (uri != null && onboarded) nav.importShared() }
        }
    }

    // Material shared-axis X: the new screen comes in from the end, the old one fades back.
    val enter: EnterTransition = slideInHorizontally(TallyMotion.enter(TallyMotion.Emphasized)) { it / 8 } + fadeIn(TallyMotion.enter())
    val exit: ExitTransition = fadeOut(TallyMotion.exit(TallyMotion.Fast))
    val popEnter: EnterTransition = fadeIn(TallyMotion.enter())
    val popExit: ExitTransition = slideOutHorizontally(TallyMotion.exit()) { it / 8 } + fadeOut(TallyMotion.exit())

    NavHost(
        navController = controller,
        startDestination = start,
        enterTransition = { enter },
        exitTransition = { exit },
        popEnterTransition = { popEnter },
        popExitTransition = { popExit },
    ) {
        composable(Routes.ONBOARDING) { OnboardingRoute(nav) }
        composable(Routes.HUB) { Hub(nav) }
        composable(Routes.HISTORY) { HistoryRoute(nav) }
        composable(
            Routes.ENTRY,
            arguments = listOf(longArg(Args.ID), stringArg(Args.TYPE, "EXPENSE"), longArg(Args.ACCOUNT), longArg(Args.CATEGORY)),
        ) { EntryRoute(nav) }
        composable(Routes.TRANSACTIONS, arguments = listOf(longArg(Args.CATEGORY), longArg(Args.ACCOUNT))) { TransactionsRoute(nav) }
        composable(Routes.ACCOUNTS) { AccountsRoute(nav) }
        composable(Routes.ACCOUNT_EDIT, arguments = listOf(longArg(Args.ID))) { AccountEditRoute(nav) }
        composable(Routes.CATEGORIES) { CategoriesRoute(nav) }
        composable(Routes.CATEGORY_EDIT, arguments = listOf(longArg(Args.ID), stringArg(Args.KIND, "EXPENSE"))) { CategoryEditRoute(nav) }
        composable(Routes.BUDGET_EDIT, arguments = listOf(longArg(Args.CATEGORY))) { BudgetEditRoute(nav) }
        composable(Routes.BILL_EDIT, arguments = listOf(longArg(Args.ID))) { BillEditRoute(nav) }
        composable(Routes.GOAL, arguments = listOf(longArg(Args.ID))) { GoalRoute(nav) }
        composable(Routes.GOAL_EDIT, arguments = listOf(longArg(Args.ID))) { GoalEditRoute(nav) }
        composable(Routes.SETTINGS) { SettingsRoute(nav) }
        composable(Routes.APPEARANCE) { AppearanceRoute(nav) }
        composable(Routes.FORMAT) { FormatRoute(nav) }
        composable(Routes.BACKUP) { BackupRoute(nav) }
        composable(Routes.EXPORT) { ExportRoute(nav) }
        composable(Routes.IMPORT, arguments = listOf(stringArg(Args.SOURCE, ""))) { ImportRoute(nav) }
        composable(Routes.ABOUT) { AboutRoute(nav) }
    }
}
