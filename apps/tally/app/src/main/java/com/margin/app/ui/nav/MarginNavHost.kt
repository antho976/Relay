package com.margin.app.ui.nav

import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.margin.app.ui.accounts.AccountEditRoute
import com.margin.app.ui.accounts.AccountsRoute
import com.margin.app.ui.activity.TransactionsRoute
import com.margin.app.ui.categories.CategoriesRoute
import com.margin.app.ui.categories.CategoryEditRoute
import com.margin.app.ui.entry.EntryRoute
import com.margin.app.ui.onboarding.OnboardingRoute
import com.margin.app.ui.plan.BillEditRoute
import com.margin.app.ui.plan.BudgetEditRoute
import com.margin.app.ui.plan.GoalEditRoute
import com.margin.app.ui.plan.GoalRoute
import com.margin.app.ui.settings.AppearanceRoute
import com.margin.app.ui.settings.DataRoute
import com.margin.app.ui.settings.FormatRoute
import com.margin.app.ui.settings.SettingsRoute
import com.margin.app.ui.theme.MarginMotion

private fun longArg(name: String) = navArgument(name) { type = NavType.LongType; defaultValue = 0L }
private fun stringArg(name: String, default: String) = navArgument(name) { type = NavType.StringType; defaultValue = default }

@Composable
fun MarginNavHost(onboarded: Boolean) {
    val controller = rememberNavController()
    val nav = remember(controller) { AppNav(controller) }
    // Decided once: flipping onboarded later must not swap the graph's root under the user.
    val start = remember { if (onboarded) Routes.HUB else Routes.ONBOARDING }

    // Material shared-axis X: the new screen comes in from the end, the old one fades back.
    val enter: EnterTransition = slideInHorizontally(MarginMotion.enter(MarginMotion.Emphasized)) { it / 8 } + fadeIn(MarginMotion.enter())
    val exit: ExitTransition = fadeOut(MarginMotion.exit(MarginMotion.Fast))
    val popEnter: EnterTransition = fadeIn(MarginMotion.enter())
    val popExit: ExitTransition = slideOutHorizontally(MarginMotion.exit()) { it / 8 } + fadeOut(MarginMotion.exit())

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
        composable(Routes.DATA) { DataRoute(nav) }
    }
}
