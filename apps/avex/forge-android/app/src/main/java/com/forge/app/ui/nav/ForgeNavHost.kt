package com.forge.app.ui.nav

import androidx.compose.runtime.Composable
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.forge.app.ui.cardio.CardioScreen
import com.forge.app.ui.gym.train.DayListScreen
import com.forge.app.ui.gym.train.DayScreen
import com.forge.app.ui.overview.OverviewScreen
import com.forge.app.ui.trophies.TrophiesScreen
import com.forge.app.ui.welcome.WelcomeScreen

@Composable
fun ForgeNavHost() {
    val nav = rememberNavController()
    NavHost(navController = nav, startDestination = Routes.OVERVIEW) {
        composable(Routes.WELCOME) {
            WelcomeScreen(onFinished = {
                nav.navigate(Routes.OVERVIEW) { popUpTo(Routes.WELCOME) { inclusive = true } }
            })
        }
        composable(Routes.OVERVIEW) {
            OverviewScreen(
                onGoToGym = { nav.navigate(Routes.GYM_TRAIN) },
                onGoToCardio = { nav.navigate(Routes.CARDIO) },
                onGoToTrophies = { nav.navigate(Routes.TROPHIES) }
            )
        }
        composable(Routes.GYM_TRAIN) {
            DayListScreen(
                onBack = { nav.popBackStack() },
                onOpenDay = { dayKey -> nav.navigate(Routes.gymDay(dayKey)) },
                onOpenDayQuick = { dayKey -> nav.navigate(Routes.gymDay(dayKey, skipWarmup = true)) }
            )
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
    }
}
