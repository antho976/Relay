package com.tally.app.ui.nav

import androidx.lifecycle.Lifecycle
import androidx.navigation.NavHostController
import com.tally.core.CategoryKind
import com.tally.core.Registration
import com.tally.core.TxType

/**
 * Route strings. Arguments are optional query params so every editor has a "new" form. The
 * spaces' pages (Spend: overview, entries, insights; Plan: budgets, bills, goals, accounts,
 * invest) draw Relay's frame; everything else is a page with its own bar.
 */
object Routes {
    const val ONBOARDING = "onboarding"
    const val OVERVIEW = "overview"
    const val ENTRIES = "entries"
    const val INSIGHTS = "insights"
    const val BUDGETS = "budgets"
    const val BILLS = "bills"
    const val GOALS = "goals"
    const val ACCOUNTS = "accounts"
    const val INVESTMENTS = "investments"

    /** Every entry ever, by what was typed: the top bar's search. */
    const val SEARCH = "search"
    const val ENTRY = "entry?id={id}&type={type}&account={account}&category={category}"
    const val TRANSACTIONS = "transactions?category={category}&account={account}"
    const val ACCOUNT_EDIT = "account/edit?id={id}"
    const val CATEGORIES = "categories"
    const val CATEGORY_EDIT = "category/edit?id={id}&kind={kind}"
    const val BUDGET_EDIT = "budget/edit?category={category}"
    const val BILL_EDIT = "bill/edit?id={id}"
    const val GOAL = "goal?id={id}"
    const val GOAL_EDIT = "goal/edit?id={id}"
    const val SETTINGS = "settings"
    const val SYNC = "settings/sync"
    const val APPEARANCE = "settings/appearance"
    const val FORMAT = "settings/format"
    const val BACKUP = "settings/backup"
    const val EXPORT = "settings/export"
    const val IMPORT = "settings/import?source={source}"
    const val ABOUT = "settings/about"
    /** This year's room for one registration; [Args.KIND] is its stored name ("TFSA"). */
    const val ROOM_EDIT = "investments/room?kind={kind}"

    /** The pages that draw the frame (top bar, drawer, status bar). */
    val SPACE_PAGES = setOf(OVERVIEW, ENTRIES, INSIGHTS, BUDGETS, BILLS, GOALS, ACCOUNTS, INVESTMENTS)
    val SPEND = listOf(OVERVIEW, ENTRIES, INSIGHTS)
    val PLAN = listOf(BUDGETS, BILLS, GOALS, ACCOUNTS, INVESTMENTS)
}

/** Argument keys, read by ViewModels through SavedStateHandle. */
object Args {
    const val ID = "id"
    const val TYPE = "type"
    const val ACCOUNT = "account"
    const val CATEGORY = "category"
    const val KIND = "kind"
    const val SOURCE = "source"
}

/** Relay's two spaces, Tally's way: what happened this month, and what is planned. */
enum class Space(val label: String, val home: String) {
    SPEND("Spend", Routes.OVERVIEW),
    PLAN("Plan", Routes.BUDGETS);

    companion object {
        fun of(route: String?): Space = if (route in Routes.PLAN) PLAN else SPEND
    }
}

/**
 * Every navigation the screens can ask for. Screens never touch the controller, so they stay
 * stateless and testable, and a double tap can never push a screen twice.
 */
class AppNav(private val controller: NavHostController) {

    private val resumed: Boolean
        get() = controller.currentBackStackEntry?.lifecycle?.currentState == Lifecycle.State.RESUMED

    private fun go(route: String) {
        if (!resumed) return
        controller.navigate(route) { launchSingleTop = true }
    }

    /**
     * A space page: they are siblings, not a stack, so going to one replaces the one showing and
     * Back from any of them leaves the app from Overview.
     */
    fun page(route: String) {
        val current = controller.currentBackStackEntry?.destination?.route
        if (current == route) return
        controller.navigate(route) {
            popUpTo(Routes.OVERVIEW) { inclusive = route == Routes.OVERVIEW }
            launchSingleTop = true
        }
    }

    fun space(space: Space) = page(space.home)

    /** Back, at most once per visible screen. */
    fun back() {
        if (resumed && controller.previousBackStackEntry != null) controller.popBackStack()
    }

    fun entry(id: Long = 0, type: TxType = TxType.EXPENSE, accountId: Long = 0, categoryId: Long = 0) =
        go("entry?id=$id&type=${type.name}&account=$accountId&category=$categoryId")

    fun transactions(categoryId: Long = 0, accountId: Long = 0) = go("transactions?category=$categoryId&account=$accountId")

    /**
     * An account's entries, asked for from its editor. An existing account's editor opens from those
     * entries, so this goes back to them rather than stack a second copy.
     */
    fun accountEntries(accountId: Long) {
        if (!resumed) return
        val previous = controller.previousBackStackEntry
        val cameFromThem = previous != null &&
            previous.destination.route == Routes.TRANSACTIONS &&
            previous.arguments?.getLong(Args.ACCOUNT) == accountId
        if (cameFromThem) controller.popBackStack() else transactions(accountId = accountId)
    }

    fun search() = go(Routes.SEARCH)
    fun overview() = page(Routes.OVERVIEW)
    fun entries() = page(Routes.ENTRIES)
    fun insights() = page(Routes.INSIGHTS)
    fun budgets() = page(Routes.BUDGETS)
    fun bills() = page(Routes.BILLS)
    fun goals() = page(Routes.GOALS)
    fun accounts() = page(Routes.ACCOUNTS)
    fun investments() = page(Routes.INVESTMENTS)

    fun accountEdit(id: Long = 0) = go("account/edit?id=$id")
    fun categories() = go(Routes.CATEGORIES)
    fun categoryEdit(id: Long = 0, kind: CategoryKind = CategoryKind.EXPENSE) = go("category/edit?id=$id&kind=${kind.name}")
    /** [categoryId] 0 edits the overall monthly budget. */
    fun budgetEdit(categoryId: Long) = go("budget/edit?category=$categoryId")
    fun billEdit(id: Long = 0) = go("bill/edit?id=$id")
    fun goal(id: Long) = go("goal?id=$id")
    fun goalEdit(id: Long = 0) = go("goal/edit?id=$id")
    fun settings() = go(Routes.SETTINGS)
    fun sync() = go(Routes.SYNC)
    fun appearance() = go(Routes.APPEARANCE)
    fun format() = go(Routes.FORMAT)
    fun backup() = go(Routes.BACKUP)
    fun export() = go(Routes.EXPORT)
    fun about() = go(Routes.ABOUT)
    fun roomEdit(registration: Registration) = go("investments/room?kind=${registration.name}")

    /**
     * The portfolio, asked for once an investment import is done: the import gives its place to the
     * portfolio, so Back never lands on a finished import.
     */
    fun investmentsAfterImport() {
        if (!resumed) return
        controller.navigate(Routes.INVESTMENTS) {
            popUpTo(Routes.IMPORT) { inclusive = true }
            launchSingleTop = true
        }
    }

    /** The import, opened on [source]'s directions ("desjardins", "wealthsimple", "investments", or none). */
    fun import(source: String = "") = go("settings/import?source=$source")

    /**
     * A CSV shared into the app from a browser's downloads or a file manager. It arrives while the
     * Activity is still starting, before any screen is resumed, so it skips [go]'s resumed check.
     */
    fun importShared() {
        controller.navigate("settings/import?source=shared") { launchSingleTop = true }
    }

    fun finishOnboarding() {
        controller.navigate(Routes.OVERVIEW) { popUpTo(Routes.ONBOARDING) { inclusive = true } }
    }

    /** After erase-all: back to the first-run flow with nothing behind it. */
    fun restart() {
        controller.navigate(Routes.ONBOARDING) { popUpTo(0) { inclusive = true } }
    }
}
