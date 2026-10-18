package com.quietsoftware.relay

import android.Manifest
import android.content.Intent
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.imePadding
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.compose.rememberNavController
import com.quietsoftware.relay.ui.AppNav
import com.quietsoftware.relay.ui.LocalNav
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.shell.HoldTray
import com.quietsoftware.relay.ui.shell.ShellModel
import com.quietsoftware.relay.ui.shell.ToastHost
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.RelayTheme
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.flow.MutableStateFlow

@AndroidEntryPoint
class MainActivity : ComponentActivity() {
    /** What a notification or a `relay://pair` link asked to open. */
    private val opening = MutableStateFlow<Intent?>(null)

    private val askNotify = registerForActivityResult(ActivityResultContracts.RequestPermission()) { }

    override fun onCreate(savedInstanceState: Bundle?) {
        val splash = installSplashScreen()
        enableEdgeToEdge(SystemBarStyle.dark(android.graphics.Color.TRANSPARENT), SystemBarStyle.dark(android.graphics.Color.TRANSPARENT))
        super.onCreate(savedInstanceState)
        // A recreated activity (process death, a font or locale change) has acted on its intent already.
        if (savedInstanceState == null) opening.value = intent
        if (Build.VERSION.SDK_INT >= 33) askNotify.launch(Manifest.permission.POST_NOTIFICATIONS)
        setContent {
            val shell: ShellModel = hiltViewModel()
            val s by shell.state.collectAsStateWithLifecycle()
            splash.setKeepOnScreenCondition { !s.loaded }
            if (!s.loaded) return@setContent
            val controller = rememberNavController()
            val nav = remember(controller, shell) { Nav(controller, shell) }
            val start = remember { if (s.pc == null) "pair" else "start" }
            RelayTheme(Palette.of(s.prefs.palette)) {
                CompositionLocalProvider(LocalNav provides nav) {
                    Box(Modifier.fillMaxSize().background(Palette.of(s.prefs.palette).wall).imePadding()) {
                        AppNav(nav, start)
                        HoldTray(shell, onOpen = { nav.inbox() })
                        ToastHost(shell)
                    }
                }
                val open by opening.collectAsStateWithLifecycle()
                LaunchedEffect(open) {
                    val i = open ?: return@LaunchedEffect
                    opening.value = null
                    val project = s.project?.num
                    when {
                        i.data?.scheme == "relay" -> nav.pair(i.dataString)
                        i.action == Intent.ACTION_SEND -> i.getStringExtra(Intent.EXTRA_TEXT)?.let(nav::share)
                        else -> when (val open = i.getStringExtra(EXTRA_OPEN)) {
                            // An agent's notification opens its conversation.
                            null -> Unit
                            "inbox" -> nav.inbox()
                            "new-task" -> nav.newTask(project)
                            "new-agent" -> nav.launch(project)
                            "new-thread" -> nav.space("threads")
                            "search" -> nav.search()
                            else -> open.removePrefix(OPEN_CHAT).takeIf { open.startsWith(OPEN_CHAT) && it.isNotEmpty() }?.let(nav::chat)
                        }
                    }
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        opening.value = intent
    }

    companion object {
        const val EXTRA_OPEN = "open"
        /** `EXTRA_OPEN` for an agent's conversation: this, then the session's name. */
        const val OPEN_CHAT = "chat:"
    }
}
