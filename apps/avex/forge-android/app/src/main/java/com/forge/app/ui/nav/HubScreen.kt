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
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.navigation.NavHostController
import com.forge.app.ui.cardio.CardioScreen
import com.forge.app.ui.coach.CoachBriefScreen
import com.forge.app.ui.gym.train.DayListScreen
import com.forge.app.ui.overview.OverviewScreen
import com.forge.app.ui.profile.ProfileScreen
import kotlinx.coroutines.launch

/**
 * The swipeable home: Cardio · Stats · Overview(Home) · Coach · Profile as pages of a
 * [HorizontalPager] under a shared [ForgeBottomBar], in [BottomTab] order (Home centered). Swipe
 * left/right to glide between hubs (the bar highlight follows the settled page); tapping a bar item
 * animates to that page.
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
) {
    val pagerState = rememberPagerState(initialPage = initialPage, pageCount = { BottomTab.entries.size })
    val scope = rememberCoroutineScope()
    fun goTo(page: Int) { scope.launch { pagerState.animateScrollToPage(page) } }

    // External tab requests (deep screen → tab, or a cardio widget launch).
    LaunchedEffect(pendingPage) {
        if (pendingPage != null) {
            pagerState.animateScrollToPage(pendingPage)
            onPendingConsumed()
        }
    }

    // Back from any non-Home hub returns to Home; on Home it falls through to the system (exit).
    BackHandler(enabled = pagerState.currentPage != BottomTab.HOME.ordinal) { goTo(BottomTab.HOME.ordinal) }

    Scaffold(
        containerColor = Color.Transparent,
        contentWindowInsets = WindowInsets(0, 0, 0, 0),
        // targetPage (not currentPage) so the highlight tracks the destination the instant a swipe or
        // tap commits, rather than snapping only once the page settles.
        bottomBar = { ForgeBottomBar(selectedIndex = pagerState.targetPage, onSelect = { goTo(it) }) }
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
            // Pages are keyed off the BottomTab order so they always line up with the bar (Home centered).
            // No onBack on hub pages: the bottom bar + system-back (BackHandler above) handle navigation,
            // so a per-page back arrow would be redundant chrome.
            when (BottomTab.entries[page]) {
                BottomTab.CARDIO -> CardioScreen()
                BottomTab.STATS -> DayListScreen(
                    onOpenDay = { dayKey -> nav.navigate(Routes.gymDay(dayKey)) },
                    onOpenDayQuick = { dayKey -> nav.navigate(Routes.gymDay(dayKey, skipWarmup = true)) },
                    onOpenHistory = { nav.navigate(Routes.SESSION_HISTORY) },
                    onOpenNotes = { nav.navigate(Routes.NOTES_SEARCH) },
                    onOpenRecap = { nav.navigate(Routes.RECAP) },
                    onEditProgram = { dayKey -> nav.navigate(Routes.programEditor(dayKey)) },
                    onOpenCardio = { goTo(BottomTab.CARDIO.ordinal) },
                    initialTab = 1,
                    title = "Stats"
                )
                BottomTab.HOME -> OverviewScreen(
                    // A cardio "day" is logged on the Cardio page, so its start CTA swipes there.
                    onStartSession = { dayKey -> if (dayKey.startsWith("cardio")) goTo(BottomTab.CARDIO.ordinal) else nav.navigate(Routes.gymDay(dayKey)) },
                    onStartSessionSkipWarmup = { dayKey -> if (dayKey.startsWith("cardio")) goTo(BottomTab.CARDIO.ordinal) else nav.navigate(Routes.gymDay(dayKey, skipWarmup = true)) },
                    onViewProgram = { nav.navigate(Routes.PROGRAM_VIEWER) },
                    onGoToCardio = { goTo(BottomTab.CARDIO.ordinal) },
                    onGoToTrophies = { nav.navigate(Routes.TROPHIES) },
                    onGoToPrs = { nav.navigate(Routes.GYM_PRS) },
                    onOpenNotes = { nav.navigate(Routes.NOTES_SEARCH) },
                    onGoToNutrition = { nav.navigate(Routes.NUTRITION) },
                    onGoToSettings = { nav.navigate(Routes.SETTINGS) },
                    // Coach is now its own hub page — swipe to it rather than pushing the modal brief.
                    onOpenCoachBrief = { goTo(BottomTab.COACH.ordinal) },
                    onOpenCoachLab = { nav.navigate(Routes.COACH_LAB) },
                    onOpenProfile = { goTo(BottomTab.PROFILE.ordinal) },
                    onOpenSession = { sessionId -> nav.navigate(Routes.sessionDetail(sessionId)) }
                )
                BottomTab.COACH -> CoachBriefScreen(
                    onOpenCoachLab = { nav.navigate(Routes.COACH_LAB) }
                )
                BottomTab.PROFILE -> ProfileScreen(
                    onOpenTrophies = { nav.navigate(Routes.TROPHIES) },
                    onOpenGoals = { nav.navigate(Routes.GOALS) },
                    onOpenPhotoGallery = { nav.navigate(Routes.MIRROR_TEST) }
                )
            }
        }
    }
}
