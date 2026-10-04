package com.tally.app.ui.nav

import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.runtime.remember
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.lifecycle.Lifecycle
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** [AppNav] over the app's real routes, with empty screens and no transitions. */
@RunWith(RobolectricTestRunner::class)
class AppNavTest {

    @get:Rule val compose = createComposeRule()

    private lateinit var controller: NavHostController
    private lateinit var nav: AppNav

    private fun longArg(name: String) = navArgument(name) { type = NavType.LongType; defaultValue = 0L }

    @Before fun graph() {
        compose.setContent {
            val c = rememberNavController()
            controller = c
            nav = remember(c) { AppNav(c) }
            NavHost(
                navController = c,
                startDestination = Routes.HUB,
                enterTransition = { EnterTransition.None },
                exitTransition = { ExitTransition.None },
            ) {
                composable(Routes.HUB) {}
                composable(Routes.TRANSACTIONS, arguments = listOf(longArg(Args.CATEGORY), longArg(Args.ACCOUNT))) {}
                composable(Routes.ACCOUNT_EDIT, arguments = listOf(longArg(Args.ID))) {}
            }
        }
    }

    /** Runs [action] once the screen on top is resumed: AppNav ignores taps on a screen still arriving. */
    private fun go(action: AppNav.() -> Unit) {
        compose.waitForIdle()
        repeat(20) {
            val resumed = compose.runOnIdle { controller.currentBackStackEntry?.lifecycle?.currentState == Lifecycle.State.RESUMED }
            if (!resumed) {
                compose.mainClock.advanceTimeByFrame()
                compose.waitForIdle()
            }
        }
        compose.runOnIdle { nav.action() }
        compose.waitForIdle()
    }

    private fun top(): Pair<String?, Long?> = compose.runOnIdle {
        val e = controller.currentBackStackEntry
        e?.destination?.route to e?.arguments?.getLong(Args.ACCOUNT)
    }

    private fun below(): String? = compose.runOnIdle { controller.previousBackStackEntry?.destination?.route }

    @Test fun viewEntriesFromAnAccountsEditorGoesBackToThoseEntries() {
        go { transactions(accountId = 7) }
        go { accountEdit(7) }
        go { accountEntries(7) }

        assertEquals(Routes.TRANSACTIONS to 7L, top())
        assertEquals("No second copy of the entries, or of the editor, is stacked", Routes.HUB, below())
    }

    @Test fun viewEntriesFromAnywhereElseOpensThem() {
        go { transactions(categoryId = 5) }
        go { accountEdit(7) }
        go { accountEntries(7) }

        assertEquals(Routes.TRANSACTIONS to 7L, top())
        assertEquals(Routes.ACCOUNT_EDIT, below())
    }
}
