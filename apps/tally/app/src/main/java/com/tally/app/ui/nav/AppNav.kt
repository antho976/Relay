package com.tally.app.ui.nav

import androidx.lifecycle.Lifecycle
import androidx.navigation.NavHostController
import com.tally.core.CategoryKind
import com.tally.core.TxType

/** Route strings. Arguments are optional query params so every screen has a "new" form. */
object Routes {
    const val ONBOARDING = "onboarding"
    const val HUB = "hub"
    const val ENTRY = "entry?id={id}&type={type}&account={account}&category={category}"
    const val TRANSACTIONS = "transactions?category={category}&account={account}"
    const val ACCOUNTS = "accounts"
    const val ACCOUNT_EDIT = "account/edit?id={id}"
    const val CATEGORIES = "categories"
    const val CATEGORY_EDIT = "category/edit?id={id}&kind={kind}"
    const val BUDGET_EDIT = "budget/edit?category={category}"
    const val BILL_EDIT = "bill/edit?id={id}"
    const val GOAL = "goal?id={id}"
    const val GOAL_EDIT = "goal/edit?id={id}"
    const val SETTINGS = "settings"
    const val APPEARANCE = "settings/appearance"
    const val FORMAT = "settings/format"
    const val DATA = "settings/data"
}

/** Argument keys, read by ViewModels through SavedStateHandle. */
object Args {
    const val ID = "id"
    const val TYPE = "type"
    const val ACCOUNT = "account"
    const val CATEGORY = "category"
    const val KIND = "kind"
}

/**
 * Every navigation the screens can ask for. Screens never touch the controller, so they stay
 * stateless and screenshot-testable, and a double tap can never push a screen twice.
 */
class AppNav(private val controller: NavHostController) {

    private val resumed: Boolean
        get() = controller.currentBackStackEntry?.lifecycle?.currentState == Lifecycle.State.RESUMED

    private fun go(route: String) {
        if (!resumed) return
        controller.navigate(route) { launchSingleTop = true }
    }

    /** Back, at most once per visible screen. */
    fun back() {
        if (resumed && controller.previousBackStackEntry != null) controller.popBackStack()
    }

    fun entry(id: Long = 0, type: TxType = TxType.EXPENSE, accountId: Long = 0, categoryId: Long = 0) =
        go("entry?id=$id&type=${type.name}&account=$accountId&category=$categoryId")

    fun transactions(categoryId: Long = 0, accountId: Long = 0) = go("transactions?category=$categoryId&account=$accountId")

    /**
     * An account's entries, asked for from its editor. An existing account's editor opens from
     * those entries, so this goes back to them: pushing them again would stack a second editor on
     * a first one that still holds the account as it was loaded.
     */
    fun accountEntries(accountId: Long) {
        if (!resumed) return
        val previous = controller.previousBackStackEntry
        val cameFromThem = previous != null &&
            previous.destination.route == Routes.TRANSACTIONS &&
            previous.arguments?.getLong(Args.ACCOUNT) == accountId
        if (cameFromThem) controller.popBackStack() else transactions(accountId = accountId)
    }

    fun accounts() = go(Routes.ACCOUNTS)
    fun accountEdit(id: Long = 0) = go("account/edit?id=$id")
    fun categories() = go(Routes.CATEGORIES)
    fun categoryEdit(id: Long = 0, kind: CategoryKind = CategoryKind.EXPENSE) = go("category/edit?id=$id&kind=${kind.name}")
    /** [categoryId] 0 edits the overall monthly budget. */
    fun budgetEdit(categoryId: Long) = go("budget/edit?category=$categoryId")
    fun billEdit(id: Long = 0) = go("bill/edit?id=$id")
    fun goal(id: Long) = go("goal?id=$id")
    fun goalEdit(id: Long = 0) = go("goal/edit?id=$id")
    fun settings() = go(Routes.SETTINGS)
    fun appearance() = go(Routes.APPEARANCE)
    fun format() = go(Routes.FORMAT)
    fun data() = go(Routes.DATA)

    fun finishOnboarding() {
        controller.navigate(Routes.HUB) { popUpTo(Routes.ONBOARDING) { inclusive = true } }
    }

    /** After erase-all: back to the first-run flow with nothing behind it. */
    fun restart() {
        controller.navigate(Routes.ONBOARDING) { popUpTo(0) { inclusive = true } }
    }
}
