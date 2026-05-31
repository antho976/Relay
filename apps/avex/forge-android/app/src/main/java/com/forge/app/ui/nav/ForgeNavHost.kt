package com.forge.app.ui.nav

import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.font.FontStyle
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.forge.app.ui.cardio.CardioScreen
import com.forge.app.ui.gym.history.SessionHistoryScreen
import com.forge.app.ui.gym.notes.NotesSearchScreen
import com.forge.app.ui.gym.train.DayListScreen
import com.forge.app.ui.gym.train.DayScreen
import com.forge.app.ui.overview.OverviewScreen
import com.forge.app.ui.programeditor.ProgramEditorScreen
import com.forge.app.ui.recap.RecapScreen
import com.forge.app.ui.settings.SettingsScreen
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.trophies.TrophiesScreen
import com.forge.app.ui.welcome.WelcomeScreen

@Composable
fun ForgeNavHost() {
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
    val modalRoutes = setOf(Routes.GYM_DAY, Routes.RECAP, Routes.PROGRAM_EDITOR)
    // One-shot fade so the first screen eases in on cold launch instead of snapping on.
    var appeared by remember { mutableStateOf(false) }
    val rootAlpha by animateFloatAsState(
        targetValue = if (appeared) 1f else 0f,
        animationSpec = ForgeMotion.standardTween(dur),
        label = "cold-start"
    )
    LaunchedEffect(Unit) { appeared = true }

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
        composable(Routes.WELCOME) {
            WelcomeScreen(onFinished = {
                nav.navigate(Routes.OVERVIEW) { popUpTo(Routes.WELCOME) { inclusive = true } }
            })
        }
        composable(Routes.OVERVIEW) {
            OverviewScreen(
                onStartSession = { dayKey -> nav.navigate(Routes.gymDay(dayKey)) },
                onStartSessionSkipWarmup = { dayKey -> nav.navigate(Routes.gymDay(dayKey, skipWarmup = true)) },
                onGoToGym = { nav.navigate(Routes.GYM_TRAIN) },
                onGoToCardio = { nav.navigate(Routes.CARDIO) },
                onGoToTrophies = { nav.navigate(Routes.TROPHIES) },
                onGoToStats = { nav.navigate(Routes.GYM_STATS) },
                onGoToNutrition = { nav.navigate(Routes.NUTRITION) },
                onGoToSettings = { nav.navigate(Routes.SETTINGS) }
            )
        }
        composable(Routes.GYM_TRAIN) {
            DayListScreen(
                onBack = { nav.popBackStack() },
                onOpenDay = { dayKey -> nav.navigate(Routes.gymDay(dayKey)) },
                onOpenDayQuick = { dayKey -> nav.navigate(Routes.gymDay(dayKey, skipWarmup = true)) },
                onOpenHistory = { nav.navigate(Routes.SESSION_HISTORY) },
                onOpenNotes = { nav.navigate(Routes.NOTES_SEARCH) },
                onOpenRecap = { nav.navigate(Routes.RECAP) },
                onEditProgram = { dayKey -> nav.navigate(Routes.programEditor(dayKey)) }
            )
        }
        composable(Routes.GYM_STATS) {
            DayListScreen(
                onBack = { nav.popBackStack() },
                onOpenDay = { dayKey -> nav.navigate(Routes.gymDay(dayKey)) },
                onOpenDayQuick = { dayKey -> nav.navigate(Routes.gymDay(dayKey, skipWarmup = true)) },
                onOpenHistory = { nav.navigate(Routes.SESSION_HISTORY) },
                onOpenNotes = { nav.navigate(Routes.NOTES_SEARCH) },
                onOpenRecap = { nav.navigate(Routes.RECAP) },
                onEditProgram = { dayKey -> nav.navigate(Routes.programEditor(dayKey)) },
                initialTab = 1
            )
        }
        composable(Routes.NUTRITION) {
            NutritionPlaceholderScreen(onBack = { nav.popBackStack() })
        }
        composable(Routes.SESSION_HISTORY) {
            SessionHistoryScreen(onBack = { nav.popBackStack() })
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
        composable(Routes.CARDIO) {
            CardioScreen(onBack = { nav.popBackStack() })
        }
        composable(Routes.TROPHIES) {
            TrophiesScreen(onBack = { nav.popBackStack() })
        }
        composable(Routes.SETTINGS) {
            SettingsScreen(onBack = { nav.popBackStack() })
        }
        composable(Routes.RECAP) {
            RecapScreen(onBack = { nav.popBackStack() })
        }
        composable(
            route = Routes.PROGRAM_EDITOR,
            arguments = listOf(navArgument("dayKey") { type = NavType.StringType })
        ) { entry ->
            ProgramEditorScreen(
                dayKey = entry.arguments?.getString("dayKey").orEmpty(),
                onBack = { nav.popBackStack() }
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun NutritionPlaceholderScreen(onBack: () -> Unit) {
    Scaffold(
        topBar = {
            TopAppBar(
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
        Box(
            modifier = Modifier.fillMaxSize().padding(inner),
            contentAlignment = Alignment.Center
        ) {
            Text(
                "Nutrition — coming soon.",
                style = MaterialTheme.typography.headlineSmall,
                fontStyle = FontStyle.Italic,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}
