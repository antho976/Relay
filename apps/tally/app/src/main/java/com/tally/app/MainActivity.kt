package com.tally.app

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.data.sync.SyncScheduler
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.Notices
import com.tally.app.ui.common.rememberMoney
import com.tally.app.ui.nav.Launches
import com.tally.app.ui.nav.TallyNavHost
import com.tally.app.ui.settings.IncomingFiles
import com.tally.app.ui.shell.ToastHost
import com.tally.app.ui.theme.Palette
import com.tally.app.ui.theme.Tally
import com.tally.app.ui.theme.TallyTheme
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject

@AndroidEntryPoint
class MainActivity : ComponentActivity() {

    @Inject lateinit var notices: Notices
    @Inject lateinit var incoming: IncomingFiles
    @Inject lateinit var launches: Launches
    @Inject lateinit var syncScheduler: SyncScheduler

    private val root: RootViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        val splash = installSplashScreen()
        super.onCreate(savedInstanceState)
        splash.setKeepOnScreenCondition { root.settings.value == null }
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.dark(android.graphics.Color.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.dark(android.graphics.Color.TRANSPARENT),
        )
        // A statement opened with Tally, or a shortcut. Not on a rotation: it was taken already.
        if (savedInstanceState == null) receive(intent)

        setContent {
            val settings by root.settings.collectAsStateWithLifecycle()
            val s = settings ?: return@setContent
            TallyTheme(Palette.of(s.palette)) {
                CompositionLocalProvider(LocalMoney provides rememberMoney(s.currency)) {
                    Box(Modifier.fillMaxSize().background(Tally.colors.wall)) {
                        TallyNavHost(onboarded = s.onboarded, incoming = incoming, launches = launches)
                        ToastHost(notices)
                    }
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        receive(intent)
    }

    override fun onStart() {
        super.onStart()
        // Back in front: catch up with the server (or connect, if this phone is not yet).
        syncScheduler.onForeground()
    }

    /** A file opened with or shared to Tally goes to the import; a shortcut to what it names. */
    private fun receive(intent: Intent?) {
        when (intent?.action) {
            Launches.ADD, Launches.SEARCH -> {
                launches.offer(intent.action ?: return)
                return
            }
        }
        val uri: Uri? = when (intent?.action) {
            Intent.ACTION_VIEW -> intent.data
            Intent.ACTION_SEND -> if (Build.VERSION.SDK_INT >= 33) {
                intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
            } else {
                @Suppress("DEPRECATION")
                intent.getParcelableExtra(Intent.EXTRA_STREAM)
            }
            else -> null
        }
        if (uri != null) incoming.offer(uri)
    }

    override fun onDestroy() {
        // A rotation hands the toast on screen to the next Activity. Leaving the app lets its act stand.
        if (!isChangingConfigurations) notices.clear()
        super.onDestroy()
    }
}
