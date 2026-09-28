package com.forge.app.ui.nav

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavHostController
import com.forge.app.ui.cardio.CardioScreen
import com.forge.app.ui.coach.CoachScreen
import com.forge.app.ui.gym.train.DayListScreen
import com.forge.app.ui.overview.OverviewScreen
import com.forge.app.ui.academy.AcademyScreen
import com.forge.app.ui.theme.ForgeMotion
import kotlinx.coroutines.launch

/**
 * The swipeable home: Cardio · Stats · Overview(Home) · Coach · Academy as pages of a
 * [HorizontalPager] under a shared [ForgeBottomBar], in [BottomTab] order (Home centered). Swipe
 * left/right to glide between hubs (the bar highlight follows the settled page); tapping a bar item
 * animates to that page.
 *
 * The Coach tab is removed when the user has disabled the coach (onboarding / Settings). Coach sits
 * after Home, so removing it leaves Cardio/Stats/Home at the same indices — only Profile shifts.
 *
 * Deep screens (a live session, settings, the coach, PRs, the program editor, …) are pushed onto
 * [nav] *on top of* this hub and bring their own back arrow — they are NOT pages here, so the bar
 * disappears with them.
 *
 * [pendingPage] lets the outside world request a tab (a widget launch, or a deep screen's "open
 * cardio"): set it, the pager animates there, then [onPendingConsumed] clears it.
 */
@Composable
fun HubScreen(
    nav: NavHostController,
    initialTab: BottomTab = BottomTab.HOME,
    pendingTab: BottomTab? = null,
    onPendingConsumed: () -> Unit = {},
    /** Unread lessons, badged on the Academy tab. See `ForgeBottomBar`'s `badges` for why. */
    academyUnread: Int = 0,
    viewModel: HubViewModel = hiltViewModel(),
) {
    val coachEnabled by viewModel.coachEnabled.collectAsStateWithLifecycle()
    val freestyleMode by viewModel.freestyleMode.collectAsStateWithLifecycle()
    // The Coach tab is shown only when the coach is on AND the user follows a plan — a freestyle user
    // has nothing to coach against.
    val showCoach = coachEnabled && !freestyleMode
    val tabs = remember(showCoach) {
        if (showCoach) BottomTab.entries.toList() else BottomTab.entries.filterNot { it == BottomTab.COACH }
    }
    // Re-create the pager when the tab set changes (coach toggled), so a settled currentPage can't be
    // left pointing past the now-shorter list — which would otherwise strand the user on a stale page.
    // Resolved by IDENTITY, not by ordinal. `tabs` DROPS Coach when the coach is off, so an ordinal
    // is an index into a list that may be one shorter than the enum — and which entry it lands on
    // then depends on where Coach happens to sit in the declaration order. Today HOME and CARDIO
    // survive that by luck; the next reorder of the enum silently sends a widget tap to a different
    // page. indexOf answers the question actually being asked.
    val pagerState = key(showCoach) {
        rememberPagerState(
            initialPage = tabs.indexOf(initialTab).coerceAtLeast(0),
            pageCount = { tabs.size }
        )
    }
    val scope = rememberCoroutineScope()
    // Tapping a bar item (or a "go home"/external request) should glide like a swipe, not snap. The
    // default animateScrollToPage spec lands fast + sharp; this smooth page-level tween matches the
    // swipe-settle feel (and collapses to an instant jump under reduced-motion).
    val pageSpec = ForgeMotion.standardTween<Float>(ForgeMotion.DurationEmphasized)
    fun goTo(page: Int) {
        scope.launch { pagerState.animateScrollToPage(page.coerceIn(0, tabs.lastIndex), animationSpec = pageSpec) }
    }
    fun goToTab(tab: BottomTab) { tabs.indexOf(tab).takeIf { it >= 0 }?.let { goTo(it) } }
    // A screen with nothing in its route to tell two opens apart. The hub stays tappable while the
    // push animates, so a double tap on Profile, Settings or History stacked two copies, and Back
    // had to be pressed twice to get home. Routes that carry an id (a day, a session) are left as
    // plain pushes: single-top would re-point the open screen at the new id instead.
    fun pushOnce(route: String) = nav.navigate(route) { launchSingleTop = true }
    val homeIndex = tabs.indexOf(BottomTab.HOME)

    // External tab requests (deep screen → tab, or a cardio widget launch).
    LaunchedEffect(pendingTab, tabs) {
        val tab = pendingTab ?: return@LaunchedEffect
        val index = tabs.indexOf(tab)
        // A tab that is not currently shown (Coach, for a freestyle user) has nowhere to go —
        // consume the request rather than leaving it pending forever.
        if (index >= 0) pagerState.animateScrollToPage(index, animationSpec = pageSpec)
        onPendingConsumed()
    }

    // Back from any non-Home hub returns to Home; on Home it falls through to the system (exit).
    BackHandler(enabled = pagerState.currentPage != homeIndex) { goTo(homeIndex) }

    Scaffold(
        containerColor = Color.Transparent,
        contentWindowInsets = WindowInsets(0, 0, 0, 0),
        // targetPage (not currentPage) so the highlight tracks the destination the instant a swipe or
        // tap commits, rather than snapping only once the page settles.
        bottomBar = {
            ForgeBottomBar(
                tabs = tabs,
                selectedIndex = pagerState.targetPage,
                onSelect = { goTo(it) },
                badges = mapOf(BottomTab.ACADEMY to academyUnread)
            )
        }
    ) { innerPadding ->
        HorizontalPager(
            state = pagerState,
            // Reserve the bar's height, and mark it consumed so each page's own Scaffold doesn't add
            // a second bottom inset on top of it.
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .consumeWindowInsets(innerPadding)
        ) { page ->
            // Pages are keyed off the visible tab list so they always line up with the bar.
            when (tabs.getOrElse(page) { BottomTab.HOME }) {
                BottomTab.CARDIO -> CardioScreen(
                    onOpenHistory = { pushOnce(Routes.SESSION_HISTORY) },
                    onOpenGoals = { pushOnce(Routes.GOALS) },
                    onOpenWeeks = { pushOnce(Routes.cardioWeeks()) },
                    // Tapping the hero's Mon–Sun strip opens THIS week in full.
                    onOpenWeek = { weekStartMs -> nav.navigate(Routes.cardioWeeks(weekStartMs)) }
                )
                BottomTab.STATS -> DayListScreen(
                    onOpenDay = { dayKey -> nav.navigateToGymDay(dayKey) },
                    onOpenDayQuick = { dayKey -> nav.navigateToGymDay(dayKey, skipWarmup = true) },
                    onOpenHistory = { pushOnce(Routes.SESSION_HISTORY) },
                    onOpenNotes = { pushOnce(Routes.NOTES_SEARCH) },
                    onOpenRecap = { pushOnce(Routes.RECAP) },
                    // Long-press "Edit program for this day" → the streamlined program builder (the same
                    // editor Settings → Program opens), replacing the old per-day editor screen.
                    onEditProgram = { _ -> pushOnce(Routes.programBuilder()) },
                    onOpenCardio = { goToTab(BottomTab.CARDIO) },
                    onLogFreestyle = { pushOnce(Routes.FREESTYLE_LOG) },
                    onBuildPlan = { pushOnce(Routes.programBuilder()) },
                    // The consistency-heatmap day sheet drills into the same detail screens History uses.
                    onOpenSession = { sessionId -> nav.navigate(Routes.sessionDetail(sessionId)) },
                    onOpenCardioSession = { cardioId -> nav.navigate(Routes.cardioSession(cardioId)) },
                    initialTab = 1,
                    title = "Stats"
                )
                BottomTab.HOME -> OverviewScreen(
                    onOpenAcademy = { goToTab(BottomTab.ACADEMY) },
                    // A cardio "day" is logged on the Cardio page, so its start CTA swipes there.
                    onStartSession = { dayKey -> if (dayKey.startsWith("cardio")) goToTab(BottomTab.CARDIO) else nav.navigateToGymDay(dayKey) },
                    onStartSessionSkipWarmup = { dayKey -> if (dayKey.startsWith("cardio")) goToTab(BottomTab.CARDIO) else nav.navigateToGymDay(dayKey, skipWarmup = true) },
                    // "View program" opens the program screen read-only — the top-bar pencil
                    // unlocks the same editor Settings → Program opens (GYMAP-28).
                    onViewProgram = { pushOnce(Routes.programBuilder(view = true)) },
                    onGoToCardio = { goToTab(BottomTab.CARDIO) },
                    onGoToTrophies = { pushOnce(Routes.TROPHIES) },
                    onOpenNotes = { pushOnce(Routes.NOTES_SEARCH) },
                    onGoToNutrition = { pushOnce(Routes.NUTRITION) },
                    onOpenProfile = { pushOnce(Routes.PROFILE) },
                    onOpenSettings = { pushOnce(Routes.settings()) },
                    // Coach is its own hub page when enabled — swipe to it rather than pushing the modal brief.
                    onOpenCoachBrief = { goToTab(BottomTab.COACH) },
                    onOpenCoachLab = { pushOnce(Routes.COACH_LAB) },
                    onOpenGoals = { pushOnce(Routes.GOALS) },
                    onOpenSession = { sessionId -> nav.navigate(Routes.sessionDetail(sessionId)) },
                    // "View all" opens the real searchable History destination (a proper back-stack
                    // entry) rather than a bottom sheet, so Back from a session returns to the list.
                    onViewAllHistory = { pushOnce(Routes.SESSION_HISTORY) },
                    onLogFreestyle = { pushOnce(Routes.FREESTYLE_LOG) },
                    onBuildPlan = { pushOnce(Routes.programBuilder()) }
                )
                // Coach and Academy sit furthest from Home, so each carries the top-bar back arrow
                // to it; the system Back already goes there (the BackHandler above).
                BottomTab.COACH -> CoachScreen(
                    onBack = { goTo(homeIndex) },
                    isVisible = pagerState.settledPage == page,
                    onConnectHealth = { nav.navigate(Routes.settings(com.forge.app.ui.settings.SettingsPage.Recovery.name)) }
                )
                BottomTab.ACADEMY -> AcademyScreen(
                    onBack = { goTo(homeIndex) },
                    onOpenLesson = { nav.navigate(Routes.lesson(it)) }
                )
            }
        }
    }
}
