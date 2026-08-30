package com.forge.app.ui.nav

import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.font.FontStyle
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.forge.app.ui.coach.CoachEntryPoint
import com.forge.app.ui.coach.CoachScreen
import com.forge.app.ui.gym.freestyle.FreestyleLogScreen
import com.forge.app.ui.programbuilder.ProgramBuilderScreen
import com.forge.app.ui.common.ProgramChangeGuardHost
import com.forge.app.ui.gym.history.SessionHistoryScreen
import com.forge.app.ui.gym.session.SessionDetailScreen
import com.forge.app.ui.gym.notes.NotesSearchScreen
import com.forge.app.ui.gym.train.DayListScreen
import com.forge.app.ui.gym.train.DayScreen
import com.forge.app.ui.goals.GoalEditorScreen
import com.forge.app.ui.goals.GoalsScreen
import com.forge.app.security.LocalAppLock
import com.forge.app.ui.profile.BodyMeasurementsScreen
import com.forge.app.ui.profile.MirrorTestScreen
import com.forge.app.ui.profile.ProgressCameraScreen
import com.forge.app.ui.security.AppLockScreen
import com.forge.app.ui.recap.RecapScreen
import com.forge.app.ui.settings.SettingsScreen
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.trophies.TrophiesScreen

@Composable
fun ForgeNavHost(initialDayKey: String? = null, privacyPolicyRequest: Int = 0) {
    val nav = rememberNavController()
    // App-wide "push" navigation (Material shared-axis X): a short directional slide + fade
    // rather than a full-width slide-and-fade — the eye travels less so it reads snappier.
    // Incoming content decelerates in; outgoing accelerates away (ForgeMotion easings).
    //
    // "Mode" destinations (a focused session, recap, program editor) instead RISE from below
    // and drop back down, so motion expresses the hierarchy — you enter/exit a mode rather
    // than stepping sideways between sibling hubs. The screen underneath just fades.
    val dur = ForgeMotion.DurationEmphasized
    val slide: (Int) -> Int = { it / 6 }       // horizontal distance — modest, not full width
    val rise: (Int) -> Int = { it / 4 }        // vertical distance for modal mode screens
    // The five hubs (Cardio/Stats/Overview/Coach/Academy) are pages of HubScreen's pager, reached by
    // swipe — they aren't nav destinations. Only the deep "mode" screens remain, and they RISE as modals.
    val modalRoutes = setOf(Routes.GYM_DAY, Routes.RECAP, Routes.PROGRAM_BUILDER, Routes.COACH_BRIEF, Routes.FREESTYLE_LOG)
    // One-shot fade so the first screen eases in on cold launch instead of snapping on.
    var appeared by remember { mutableStateOf(false) }
    val rootAlpha by animateFloatAsState(
        targetValue = if (appeared) 1f else 0f,
        animationSpec = ForgeMotion.standardTween(dur),
        label = "cold-start"
    )
    LaunchedEffect(Unit) { appeared = true }

    // Widget deep-link: a gym day opens on top of the hub (Back returns home). A cardio day instead
    // selects the hub's Cardio page via initialHubPage below — there's no separate cardio destination.
    // Re-fires whenever the key changes — a fresh launch, or a widget tap routed through onNewIntent
    // while the app is already running (launchMode=singleTask); a config-change recreate supplies null.
    LaunchedEffect(initialDayKey) {
        val key = initialDayKey ?: return@LaunchedEffect
        if (key in com.forge.app.program.Program.dayKeys) nav.navigate(Routes.gymDay(key))
    }
    LaunchedEffect(privacyPolicyRequest) {
        if (privacyPolicyRequest > 0) {
            nav.navigate(Routes.settings(com.forge.app.ui.settings.SettingsPage.PrivacyPolicy.name))
        }
    }
    val initialHubPage = if (initialDayKey?.startsWith("cardio") == true) BottomTab.CARDIO.ordinal else BottomTab.HOME.ordinal

    // "I'll make my own" lands on Home, which shows the "No plan yet · Build a plan" state (its program
    // was cleared in onboarding), rather than jumping straight into the builder.
    // A deep screen (e.g. PRs → "open cardio") can request a hub tab: set this and pop back to the
    // hub, which animates to the page then clears it via onPendingConsumed.
    var pendingHubPage by remember { mutableStateOf<Int?>(null) }

    // Long-pressing the notifications bell anywhere returns Home in one gesture: pop every deep route
    // back to the hub and select Home. No-op-safe when already on the hub (popBackStack returns false).
    val goHome: () -> Unit = {
        pendingHubPage = BottomTab.HOME.ordinal
        nav.popBackStack(Routes.OVERVIEW, false)
    }

    // The bell's tap, from any screen's chrome. Guarded so a double-tap can't stack two copies of the
    // page on the back stack.
    val openNotifications: () -> Unit = {
        if (nav.currentDestination?.route != Routes.NOTIFICATIONS) nav.navigate(Routes.NOTIFICATIONS)
    }

    // The unread count feeding every bell. Held once here (rather than per screen) so the chrome and
    // the page read the same feed, and re-polled on resume — the weekly coach pass and the Health
    // Connect grants can both change while the app is backgrounded.
    val notificationsVm: com.forge.app.ui.notifications.NotificationsViewModel = hiltViewModel()
    val notices by notificationsVm.notices.collectAsStateWithLifecycle()
    val notificationsLifecycleOwner = androidx.lifecycle.compose.LocalLifecycleOwner.current
    androidx.compose.runtime.DisposableEffect(notificationsLifecycleOwner) {
        val observer = androidx.lifecycle.LifecycleEventObserver { _, event ->
            if (event == androidx.lifecycle.Lifecycle.Event.ON_RESUME) notificationsVm.refresh()
        }
        notificationsLifecycleOwner.lifecycle.addObserver(observer)
        onDispose { notificationsLifecycleOwner.lifecycle.removeObserver(observer) }
    }

    // User-defined cardio activities, fed once here so every cardio surface can resolve a `custom_`
    // code to its name + glyph without per-screen plumbing (GYMAP-37).
    val cardioTypesVm: com.forge.app.ui.cardio.CardioTypesViewModel = hiltViewModel()
    val cardioTypes by cardioTypesVm.types.collectAsStateWithLifecycle()

    // Where the bell sits, so an arrival banner can fly into it. One anchor for the whole app: the
    // bell is Home-only (§4.6), so at most one is ever composed.
    val bellAnchor = remember { com.forge.app.ui.common.BellAnchor() }

    CompositionLocalProvider(
        com.forge.app.ui.common.LocalGoHome provides goHome,
        com.forge.app.ui.common.LocalOpenNotifications provides openNotifications,
        com.forge.app.ui.common.LocalUnreadNotifications provides notices.size,
        com.forge.app.ui.common.LocalBellAnchor provides bellAnchor,
        com.forge.app.ui.cardio.LocalCardioTypes provides cardioTypes
    ) {
    NavHost(
        navController = nav,
        startDestination = Routes.OVERVIEW,
        modifier = Modifier.fillMaxSize().graphicsLayer { alpha = rootAlpha },
        enterTransition = {
            if (targetState.destination.route in modalRoutes)
                slideInVertically(ForgeMotion.enterTween(dur)) { rise(it) } + fadeIn(ForgeMotion.enterTween(dur))
            else
                slideInHorizontally(ForgeMotion.enterTween(dur)) { slide(it) } + fadeIn(ForgeMotion.enterTween(dur))
        },
        exitTransition = {
            if (targetState.destination.route in modalRoutes)
                fadeOut(ForgeMotion.exitTween(dur))
            else
                slideOutHorizontally(ForgeMotion.exitTween(dur)) { -slide(it) } + fadeOut(ForgeMotion.exitTween(dur))
        },
        popEnterTransition = {
            if (initialState.destination.route in modalRoutes)
                fadeIn(ForgeMotion.enterTween(dur))
            else
                slideInHorizontally(ForgeMotion.enterTween(dur)) { -slide(it) } + fadeIn(ForgeMotion.enterTween(dur))
        },
        popExitTransition = {
            if (initialState.destination.route in modalRoutes)
                slideOutVertically(ForgeMotion.exitTween(dur)) { rise(it) } + fadeOut(ForgeMotion.exitTween(dur))
            else
                slideOutHorizontally(ForgeMotion.exitTween(dur)) { slide(it) } + fadeOut(ForgeMotion.exitTween(dur))
        }
    ) {
        composable(Routes.OVERVIEW) {
            // The swipeable home: Overview · Cardio · Stats · Profile as pager pages under the bar.
            HubScreen(
                nav = nav,
                initialPage = initialHubPage,
                pendingPage = pendingHubPage,
                onPendingConsumed = { pendingHubPage = null },
                // Counted from the same feed the bell reads, so a kind switched off in Settings
                // drops out of both at once and they can never disagree.
                academyUnread = notices.count { it.kind == com.forge.app.data.repo.NoticeKind.ACADEMY }
            )
        }
        composable(Routes.FREESTYLE_LOG) {
            FreestyleLogScreen(onBack = { nav.popBackStack() })
        }
        composable(
            route = Routes.PROGRAM_BUILDER,
            arguments = listOf(
                navArgument(Routes.ARG_BLANK) { type = NavType.BoolType; defaultValue = false },
                navArgument(Routes.ARG_VIEW) { type = NavType.BoolType; defaultValue = false }
            )
        ) { entry ->
            ProgramBuilderScreen(
                blank = entry.arguments?.getBoolean(Routes.ARG_BLANK) ?: false,
                startInView = entry.arguments?.getBoolean(Routes.ARG_VIEW) ?: false,
                onClose = { nav.popBackStack() }
            )
        }
        composable(Routes.NUTRITION) {
            NutritionPlaceholderScreen(onBack = { nav.popBackStack() })
        }
        composable(Routes.SESSION_HISTORY) {
            SessionHistoryScreen(
                onBack = { nav.popBackStack() },
                onOpenSession = { sessionId -> nav.navigate(Routes.sessionDetail(sessionId)) },
                onOpenCardio = { cardioId -> nav.navigate(Routes.cardioSession(cardioId)) }
            )
        }
        composable(
            route = Routes.SESSION_DETAIL,
            arguments = listOf(navArgument(Routes.ARG_SESSION_ID) { type = NavType.LongType })
        ) {
            SessionDetailScreen(onBack = { nav.popBackStack() })
        }
        composable(
            route = Routes.CARDIO_SESSION,
            arguments = listOf(navArgument(Routes.ARG_CARDIO_ID) { type = NavType.LongType })
        ) {
            com.forge.app.ui.cardio.CardioSessionDetailScreen(onBack = { nav.popBackStack() })
        }
        composable(
            route = Routes.CARDIO_WEEKS,
            arguments = listOf(
                navArgument(Routes.ARG_WEEK_START) { type = NavType.LongType; defaultValue = -1L }
            )
        ) {
            com.forge.app.ui.cardio.CardioWeeksScreen(
                onBack = { nav.popBackStack() },
                onOpenSession = { cardioId -> nav.navigate(Routes.cardioSession(cardioId)) }
            )
        }
        composable(Routes.NOTES_SEARCH) {
            NotesSearchScreen(onBack = { nav.popBackStack() })
        }
        composable(
            route = Routes.GYM_DAY,
            arguments = listOf(
                navArgument(Routes.ARG_DAY_KEY) { type = NavType.StringType },
                navArgument(Routes.ARG_SKIP_WARMUP) {
                    type = NavType.BoolType
                    defaultValue = false
                }
            )
        ) { entry ->
            val dayKey = entry.arguments?.getString(Routes.ARG_DAY_KEY).orEmpty()
            DayScreen(dayKey = dayKey, onBack = { nav.popBackStack() })
        }
        // Profile left the bottom bar for the Home top bar (2026-07-27), so it is a pushed route
        // now and brings its own back arrow.
        composable(Routes.PROFILE) {
            com.forge.app.ui.profile.ProfileScreen(
                onBack = { nav.popBackStack() },
                onOpenTrophies = { nav.navigate(Routes.TROPHIES) },
                onOpenPhotoGallery = { nav.navigate(Routes.MIRROR_TEST) },
                onOpenMeasurements = { nav.navigate(Routes.BODY_MEASUREMENTS) }
            )
        }
        composable(Routes.TROPHIES) {
            TrophiesScreen(onBack = { nav.popBackStack() })
        }
        composable(
            route = Routes.SETTINGS,
            arguments = listOf(navArgument(Routes.ARG_SETTINGS_PAGE) {
                type = NavType.StringType
                defaultValue = ""
            })
        ) { entry ->
            val pageArg = entry.arguments?.getString(Routes.ARG_SETTINGS_PAGE).orEmpty()
            val initialPage = com.forge.app.ui.settings.SettingsPage.entries.firstOrNull { it.name == pageArg }
            SettingsScreen(
                onBack = { nav.popBackStack() },
                onOpenCoachBrief = { nav.navigate(Routes.COACH_BRIEF) },
                onOpenBuilder = { nav.navigate(Routes.programBuilder()) },
                initialPage = initialPage
            )
        }
        composable(Routes.RECAP) {
            RecapScreen(onBack = { nav.popBackStack() })
        }
        composable(Routes.NOTIFICATIONS) {
            com.forge.app.ui.notifications.NotificationsScreen(
                onBack = { nav.popBackStack() },
                // Acting on a notice leaves the feed behind rather than stacking on top of it, so Back
                // from the session (or the brief) returns to where the user actually was.
                onResumeSession = { dayKey ->
                    nav.popBackStack()
                    if (dayKey.startsWith("cardio")) {
                        pendingHubPage = BottomTab.CARDIO.ordinal
                    } else {
                        nav.navigate(Routes.gymDay(dayKey))
                    }
                },
                onOpenCoachBrief = { nav.popBackStack(); pendingHubPage = BottomTab.COACH.ordinal },
                onConnectWearable = {
                    nav.popBackStack()
                    nav.navigate(Routes.settings(com.forge.app.ui.settings.SettingsPage.Recovery.name))
                },
                // Kept on the stack, not popped: the per-notification toggles live there, so Back
                // returns to the feed you were just looking at to see the effect.
                onOpenNotificationSettings = {
                    nav.navigate(Routes.settings(com.forge.app.ui.settings.SettingsPage.Notifications.name))
                },
                // Popped like the other acting rows, so Back from the lesson returns to where the
                // user actually was rather than to a feed row that has since cleared itself.
                onOpenLesson = { lessonId ->
                    nav.popBackStack()
                    nav.navigate(Routes.lesson(lessonId))
                }
            )
        }
        composable(Routes.COACH_BRIEF) {
            CoachScreen(
                onBack = { nav.popBackStack() },
                onConnectHealth = { nav.navigate(Routes.settings(com.forge.app.ui.settings.SettingsPage.Recovery.name)) }
            )
        }
        // The lab and timeline used to be lenses of the Coach page. The page is one column now,
        // so the routes stay and resolve to a scroll position rather than a tab.
        composable(Routes.ACADEMY) {
            com.forge.app.ui.academy.AcademyScreen(
                onBack = { nav.popBackStack() },
                onOpenLesson = { nav.navigate(Routes.lesson(it)) },
                onOpenArticle = { nav.navigate(Routes.article(it)) }
            )
        }
        composable(
            route = Routes.LESSON,
            arguments = listOf(navArgument(Routes.ARG_LESSON_ID) { type = NavType.StringType })
        ) {
            // A retired id resolves to null and the screen says so inline (§12) rather than popping
            // the back stack: a link from an old coach reason should explain itself, not vanish.
            com.forge.app.ui.academy.LessonScreen(
                onBack = { nav.popBackStack() },
                // Reading on from the end of a lesson REPLACES it on the stack, so Back from the
                // fourth lesson in a row returns to the gallery rather than walking the chain in
                // reverse. The chain is a way forward, not a history.
                onOpenLesson = { nav.navigate(Routes.lesson(it)) { popUpTo(Routes.LESSON) { inclusive = true } } }
            )
        }
        composable(
            route = Routes.ARTICLE,
            arguments = listOf(navArgument(Routes.ARG_ARTICLE_ID) { type = NavType.StringType })
        ) {
            // A retired id resolves to null and the screen says so inline (§12) rather than popping
            // the back stack: a link from an old coach reason should explain itself, not vanish.
            com.forge.app.ui.academy.ArticleScreen(
                onBack = { nav.popBackStack() },
                onOpenArticle = { nav.navigate(Routes.article(it)) { popUpTo(Routes.ARTICLE) { inclusive = true } } }
            )
        }
        composable(Routes.COACH_LAB) {
            CoachScreen(
                onBack = { nav.popBackStack() },
                entryPoint = CoachEntryPoint.WHERE_YOU_STAND,
                onConnectHealth = { nav.navigate(Routes.settings(com.forge.app.ui.settings.SettingsPage.Recovery.name)) }
            )
        }
        composable(Routes.COACH_TIMELINE) {
            CoachScreen(
                onBack = { nav.popBackStack() },
                entryPoint = CoachEntryPoint.ACCOUNT,
                onConnectHealth = { nav.navigate(Routes.settings(com.forge.app.ui.settings.SettingsPage.Recovery.name)) }
            )
        }
        composable(Routes.GOALS) {
            GoalsScreen(
                onBack = { nav.popBackStack() },
                onAddGoal = { nav.navigate(Routes.goalEditor()) },
                onEditLift = { id -> nav.navigate(Routes.goalEditor(exerciseId = id)) },
                onEditCustom = { id -> nav.navigate(Routes.goalEditor(customId = id)) }
            )
        }
        composable(
            route = Routes.GOAL_EDITOR,
            arguments = listOf(
                navArgument(Routes.ARG_GOAL_EXERCISE_ID) { type = NavType.StringType; defaultValue = "" },
                navArgument(Routes.ARG_GOAL_CUSTOM_ID) { type = NavType.StringType; defaultValue = "" }
            )
        ) { entry ->
            GoalEditorScreen(
                exerciseId = entry.arguments?.getString(Routes.ARG_GOAL_EXERCISE_ID)?.takeIf { it.isNotBlank() },
                customId = entry.arguments?.getString(Routes.ARG_GOAL_CUSTOM_ID)?.toLongOrNull(),
                onDone = { nav.popBackStack() }
            )
        }
        composable(Routes.MIRROR_TEST) {
            // Gallery lock (GYMAP-69): gate the progress photos unless the session is already
            // authenticated (unlocking the app satisfies this too — no second prompt). Cancel → back.
            val appLock = LocalAppLock.current
            val galleryLocked by appLock.galleryLocked.collectAsStateWithLifecycle()
            if (galleryLocked) {
                AppLockScreen(
                    subtitle = "Unlock your progress photos",
                    promptReady = true,
                    onUnlocked = { appLock.markAuthenticated() },
                    onCancel = { nav.popBackStack() }
                )
            } else {
                MirrorTestScreen(
                    onBack = { nav.popBackStack() },
                    onOpenCamera = { nav.navigate(Routes.PROGRESS_CAMERA) }
                )
            }
        }
        composable(Routes.PROGRESS_CAMERA) {
            // The same gate as MIRROR_TEST, for the same photos. The camera screen shows the
            // previous shot as a translucent pose-alignment ghost, so it displays private imagery
            // whether or not anything has been captured — and it was reachable with the gallery
            // locked in two ways: the lock expiring while this route was already on the back stack
            // (backgrounding the app is exactly what starts that timer), and a return to it from
            // anywhere the route can be pushed. A gate on the gallery that leaves the camera open
            // is a gate on one of two doors into the same room.
            val appLock = LocalAppLock.current
            val galleryLocked by appLock.galleryLocked.collectAsStateWithLifecycle()
            if (galleryLocked) {
                AppLockScreen(
                    subtitle = "Unlock your progress photos",
                    promptReady = true,
                    onUnlocked = { appLock.markAuthenticated() },
                    onCancel = { nav.popBackStack() }
                )
            } else {
                ProgressCameraScreen(onBack = { nav.popBackStack() })
            }
        }
        composable(Routes.BODY_MEASUREMENTS) {
            BodyMeasurementsScreen(onBack = { nav.popBackStack() })
        }
    }

    // The arrival banner. Inside the provider so it can read the bell's anchor, and drawn AFTER the
    // nav host so it overlays every screen without joining any of their layouts (§4.6: a notice
    // belongs in the feed, and this is only the receipt for one landing there).
    com.forge.app.ui.common.ArrivalBannerHost()
    }

    // App-wide guard: intercepts any program change that would discard an in-progress workout,
    // surfacing an inline "discard & continue" confirm over whatever screen triggered it.
    ProgramChangeGuardHost()
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun NutritionPlaceholderScreen(onBack: () -> Unit) {
    Scaffold(
        topBar = {
            TopAppBar(
                // §4.6: bell + back in the chrome, never the screen's own name.
                title = {},
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back",
                            tint = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        // A real "coming soon" with a short roadmap, so the entry reads as a planned feature rather
        // than a broken / abandoned link (Nutrition P0s — Cat 22).
        Column(
            modifier = Modifier.fillMaxSize().padding(inner).padding(horizontal = 32.dp),
            verticalArrangement = Arrangement.Center
        ) {
            Text("Nutrition", style = MaterialTheme.typography.headlineMedium, color = MaterialTheme.colorScheme.onBackground)
            Spacer(Modifier.height(12.dp))
            Text(
                "Coming in a future update. Built like the rest of Avex: offline, on your device, no account, and no giant food database to wrestle with.",
                style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Spacer(Modifier.height(20.dp))
            Text("WHAT'S PLANNED", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(8.dp))
            Text("•  Daily protein and calories, tracked simply.",
                style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(6.dp))
            Text("•  A bodyweight trend tied to your training volume.",
                style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(6.dp))
            Text("•  A simple cut / bulk readout from your weight and workload.",
                style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(18.dp))
            Text(
                "Already here: log your bodyweight any time from Stats → Body.",
                style = MaterialTheme.typography.bodySmall, fontStyle = FontStyle.Italic,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}
