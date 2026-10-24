package com.quietsoftware.relay.ui

import android.content.Context
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.navigation.NavHostController
import androidx.navigation.compose.rememberNavController
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import com.quietsoftware.relay.data.Relay
import com.quietsoftware.relay.data.Settings
import com.quietsoftware.relay.data.db.RelayDb
import com.quietsoftware.relay.data.link.PcStore
import com.quietsoftware.relay.ui.shell.ShellModel
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.RelayTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** A task opened from a task goes over it: Back returns to the first, not past it. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w411dp-h891dp-xxhdpi")
class NavStackTest {
    @get:Rule val compose = createComposeRule()

    @Test
    fun aSubTaskStacksOverItsParent() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val db = Room.inMemoryDatabaseBuilder(context, RelayDb::class.java).allowMainThreadQueries().build()
        val relay = Relay(context, db, PcStore(context), CoroutineScope(SupervisorJob() + Dispatchers.Default))
        val shell = ShellModel(relay, Settings(context))
        lateinit var nav: Nav
        lateinit var controller: NavHostController
        compose.setContent {
            controller = rememberNavController()
            nav = remember { Nav(controller, shell) }
            RelayTheme(Palette.Matte) {
                CompositionLocalProvider(LocalNav provides nav) { AppNav(nav, "board?project=1") }
            }
        }
        fun shown() = controller.currentBackStackEntry?.let { e -> e.destination.route to e.arguments?.getString("id") }
        compose.runOnIdle { nav.task("101") }
        compose.runOnIdle { nav.task("102") }
        compose.runOnIdle { assertEquals("task/{id}" to "102", shown()) }
        compose.runOnIdle { nav.task("102") }
        compose.runOnIdle {
            assertEquals("the same task again opens nothing more", listOf("101", "102"),
                controller.currentBackStack.value.filter { it.destination.route == "task/{id}" }.map { it.arguments?.getString("id") })
            nav.back()
        }
        compose.runOnIdle {
            assertEquals("task/{id}" to "101", shown())
            nav.back()
        }
        compose.runOnIdle { assertEquals("board?project={project}", shown()?.first) }
    }
}
