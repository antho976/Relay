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
import com.forge.app.ui.profile.ProfileScreen
import com.forge.app.ui.theme.ForgeMotion
import kotlinx.coroutines.launch

/**
 * The swipeable home: Cardio · Stats · Overview(Home) · Coach · Profile as pages of a
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
    initialPage: Int = BottomTab.HOME.ordinal,
    pendingPage: Int? = null,
    onPendingConsumed: () -> Unit = {},
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
    val pagerState = key(showCoach) {
        rememberPagerState(
            initialPage = initialPage.coerceIn(0, tabs.lastIndex),
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
    val homeIndex = tabs.indexOf(BottomTab.HOME)

    // External tab requests (deep screen → tab, or a cardio widget launch).
    LaunchedEffect(pendingPage) {
        if (pendingPage != null) {
            pagerState.animateScrollToPage(pendingPage.coerceIn(0, tabs.lastIndex), animationSpec = pageSpec)
            onPendingConsumed()
        }
    }

    // Back from any non-Home hub returns to Home; on Home it falls through to the system (exit).
    BackHandler(enabled = pagerState.currentPage != homeIndex) { goTo(homeIndex) }

    Scaffold(
        containerColor = Color.Transparent,
        contentWindowInsets = WindowInsets(0, 0, 0, 0),
        // targetPage (not currentPage) so the highlight tracks the destination the instant a swipe or
        // tap commits, rather than snapping only once the page settles.
        bottomBar = { ForgeBottomBar(tabs = tabs, selectedIndex = pagerState.targetPage, onSelect = { goTo(it) }) }
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
                    onOpenHistory = { nav.navigate(Routes.SESSION_HISTORY) },
                    onConnectWearable = { nav.navigate(Routes.settings(com.forge.app.ui.settings.SettingsPage.Recovery.name)) },
                    onOpenGoals = { nav.navigate(Routes.GOALS) }
                )
                BottomTab.STATS -> DayListScreen(
                    onOpenDay = { dayKey -> nav.navigate(Routes.gymDay(dayKey)) },
                    onOpenDayQuick = { dayKey -> nav.navigate(Routes.gymDay(dayKey, skipWarmup = true)) },
                    onOpenHistory = { nav.navigate(Routes.SESSION_HISTORY) },
                    onOpenNotes = { nav.navigate(Routes.NOTES_SEARCH) },
                    onOpenRecap = { nav.navigate(Routes.RECAP) },
                    // Long-press "Edit program for this day" → the streamlined program builder (the same
                    // editor Settings → Program opens), replacing the old per-day editor screen.
                    onEditProgram = { _ -> nav.navigate(Routes.programBuilder()) },
                    onOpenCardio = { goToTab(BottomTab.CARDIO) },
                    onLogFreestyle = { nav.navigate(Routes.FREESTYLE_LOG) },
                    onBuildPlan = { nav.navigate(Routes.programBuilder()) },
                    // The consistency-heatmap day sheet drills into the same detail screens History uses.
                    onOpenSession = { sessionId -> nav.navigate(Routes.sessionDetail(sessionId)) },
                    onOpenCardioSession = { cardioId -> nav.navigate(Routes.cardioSession(cardioId)) },
                    initialTab = 1,
                    title = "Stats"
                )
                BottomTab.HOME -> OverviewScreen(
                    // A cardio "day" is logged on the Cardio page, so its start CTA swipes there.
                    onStartSession = { dayKey -> if (dayKey.startsWith("cardio")) goToTab(BottomTab.CARDIO) else nav.navigate(Routes.gymDay(dayKey)) },
                    onStartSessionSkipWarmup = { dayKey -> if (dayKey.startsWith("cardio")) goToTab(BottomTab.CARDIO) else nav.navigate(Routes.gymDay(dayKey, skipWarmup = true)) },
                    // "View program" opens the streamlined program builder (the same editor Settings →
                    // Program opens), replacing the old temporary read-only viewer.
                    onViewProgram = { nav.navigate(Routes.programBuilder()) },
                    onGoToCardio = { goToTab(BottomTab.CARDIO) },
                    onGoToTrophies = { nav.navigate(Routes.TROPHIES) },
                    onOpenNotes = { nav.navigate(Routes.NOTES_SEARCH) },
                    onGoToNutrition = { nav.navigate(Routes.NUTRITION) },
                    onGoToSettings = { nav.navigate(Routes.settings()) },
                    // Coach is its own hub page when enabled — swipe to it rather than pushing the modal brief.
                    onOpenCoachBrief = { goToTab(BottomTab.COACH) },
                    onOpenCoachLab = { nav.navigate(Routes.COACH_LAB) },
                    onOpenGoals = { nav.navigate(Routes.GOALS) },
                    onOpenProfile = { goToTab(BottomTab.PROFILE) },
                    onOpenSession = { sessionId -> nav.navigate(Routes.sessionDetail(sessionId)) },
                    // "View all" opens the real searchable History destination (a proper back-stack
                    // entry) rather than a bottom sheet, so Back from a session returns to the list.
                    onViewAllHistory = { nav.navigate(Routes.SESSION_HISTORY) },
                    onLogFreestyle = { nav.navigate(Routes.FREESTYLE_LOG) },
                    onBuildPlan = { nav.navigate(Routes.programBuilder()) }
                )
                BottomTab.COACH -> CoachScreen(
                    onConnectHealth = { nav.navigate(Routes.settings(com.forge.app.ui.settings.SettingsPage.Recovery.name)) }
                )
                BottomTab.PROFILE -> ProfileScreen(
                    onOpenTrophies = { nav.navigate(Routes.TROPHIES) },
                    onOpenPhotoGallery = { nav.navigate(Routes.MIRROR_TEST) }
                )
            }
        }
    }
}
