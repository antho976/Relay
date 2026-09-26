package com.forge.app.ui.nav

import androidx.compose.material3.Text
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.navigation.NavBackStackEntry
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * A double-tapped exit on the live session sent two PopBacks, and a plain popBackStack() pops
 * whatever is on top: the second took the hub with it and the NavHost rendered nothing
 * (audit 2026-09-26). The day route now pops only while it is still the current entry.
 */
@RunWith(RobolectricTestRunner::class)
class PopIfCurrentTest {

    @get:Rule
    val compose = createComposeRule()

    @Test
    fun aSecondPopFromTheSameScreenLeavesTheHub() {
        lateinit var nav: NavHostController
        compose.setContent {
            nav = rememberNavController()
            NavHost(nav, startDestination = "hub") {
                composable("hub") { Text("hub") }
                composable("day") { Text("day") }
            }
        }
        lateinit var day: NavBackStackEntry
        compose.runOnIdle {
            nav.navigate("day")
            day = nav.currentBackStackEntry!!
            assertEquals("day", day.destination.route)
        }
        compose.runOnIdle {
            assertTrue(nav.popIfCurrent(day))
            assertFalse(nav.popIfCurrent(day))
            assertEquals("hub", nav.currentBackStackEntry?.destination?.route)
        }
    }
}
