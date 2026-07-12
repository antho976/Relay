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
import com.forge.app.ui.coach.CoachLens
import com.forge.app.ui.coach.CoachScreen
import com.forge.app.ui.gym.freestyle.FreestyleLogScreen
import com.forge.app.ui.programbuilder.ProgramBuilderScreen
import com.forge.app.ui.common.ForgeWordmark
import com.forge.app.ui.common.ProgramChangeGuardHost
import com.forge.app.ui.gym.history.SessionHistoryScreen
import com.forge.app.ui.gym.session.SessionDetailScreen
import com.forge.app.ui.gym.notes.NotesSearchScreen
import com.forge.app.ui.gym.train.DayListScreen
import com.forge.app.ui.gym.train.DayScreen
import com.forge.app.ui.goals.GoalEditorScreen
import com.forge.app.ui.goals.GoalsScreen
import com.forge.app.ui.profile.BodyMeasurementsScreen
import com.forge.app.ui.profile.MirrorTestScreen
import com.forge.app.ui.profile.ProgressCameraScreen
import com.forge.app.ui.recap.RecapScreen
import com.forge.app.ui.settings.SettingsScreen
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.trophies.TrophiesScreen

@Composable
fun ForgeNavHost(initialDayKey: String? = null) {
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
    // The five hubs (Cardio/Stats/Overview/Coach/Profile) are pages of HubScreen's pager, reached by
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
    val initialHubPage = if (initialDayKey?.startsWith("cardio") == true) BottomTab.CARDIO.ordinal else BottomTab.HOME.ordinal

    // "I'll make my own" lands on Home, which shows the "No plan yet · Build a plan" state (its program
    // was cleared in onboarding), rather than jumping straight into the builder.
    // A deep screen (e.g. PRs → "open cardio") can request a hub tab: set this and pop back to the
    // hub, which animates to the page then clears it via onPendingConsumed.
    var pendingHubPage by remember { mutableStateOf<Int?>(null) }

    // Tapping the Avex wordmark anywhere returns Home in one tap: pop every deep route back to the
    // hub and select Home. No-op-safe when already on the hub (popBackStack just returns false).
    val goHome: () -> Unit = {
        pendingHubPage = BottomTab.HOME.ordinal
        nav.popBackStack(Routes.OVERVIEW, false)
    }

    // User-defined cardio activities, fed once here so every cardio surface can resolve a `custom_`
    // code to its name + glyph without per-screen plumbing (GYMAP-37).
    val cardioTypesVm: com.forge.app.ui.cardio.CardioTypesViewModel = hiltViewModel()
    val cardioTypes by cardioTypesVm.types.collectAsStateWithLifecycle()

    CompositionLocalProvider(
        com.forge.app.ui.common.LocalGoHome provides goHome,
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
                onPendingConsumed = { pendingHubPage = null }
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
        composable(Routes.COACH_BRIEF) {
            CoachScreen(
                onBack = { nav.popBackStack() },
                onConnectHealth = { nav.navigate(Routes.settings(com.forge.app.ui.settings.SettingsPage.Recovery.name)) }
            )
        }
        // The lab and timeline are now lenses of the one Coach page; the routes stay so every
        // existing "what it's watching" and "learning timeline" link lands on the right lens.
        composable(Routes.COACH_LAB) {
            CoachScreen(
                onBack = { nav.popBackStack() },
                initialLens = CoachLens.SIGNALS,
                onConnectHealth = { nav.navigate(Routes.settings(com.forge.app.ui.settings.SettingsPage.Recovery.name)) }
            )
        }
        composable(Routes.COACH_TIMELINE) {
            CoachScreen(
                onBack = { nav.popBackStack() },
                initialLens = CoachLens.JOURNEY,
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
            MirrorTestScreen(
                onBack = { nav.popBackStack() },
                onOpenCamera = { nav.navigate(Routes.PROGRESS_CAMERA) }
            )
        }
        composable(Routes.PROGRESS_CAMERA) {
            ProgressCameraScreen(onBack = { nav.popBackStack() })
        }
        composable(Routes.BODY_MEASUREMENTS) {
            BodyMeasurementsScreen(onBack = { nav.popBackStack() })
        }
    }
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
                // §2: wordmark + back in the chrome, never the screen's own name.
                title = { ForgeWordmark() },
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
