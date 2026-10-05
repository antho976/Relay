package com.tally.app

import android.content.Intent
import android.database.ContentObserver
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.Settings as SystemSettings
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.NoticeHost
import com.tally.app.ui.common.Notices
import com.tally.app.ui.common.ProvideTouchExploration
import com.tally.app.ui.common.rememberMoney
import com.tally.app.ui.nav.LocalSnackbarLift
import com.tally.app.ui.nav.TallyNavHost
import com.tally.app.ui.settings.IncomingFiles
import com.tally.app.ui.theme.TallyMotion
import com.tally.app.ui.theme.TallyTheme
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject

@AndroidEntryPoint
class MainActivity : ComponentActivity() {

    @Inject lateinit var notices: Notices
    @Inject lateinit var incoming: IncomingFiles

    private val root: RootViewModel by viewModels()

    /** Keeps [TallyMotion.durationScale] live with the system's Remove animations setting. */
    private val animatorObserver = object : ContentObserver(Handler(Looper.getMainLooper())) {
        override fun onChange(selfChange: Boolean) = readAnimatorScale()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        val splash = installSplashScreen()
        super.onCreate(savedInstanceState)
        splash.setKeepOnScreenCondition { root.settings.value == null }
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.dark(android.graphics.Color.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.dark(android.graphics.Color.TRANSPARENT),
        )
        readAnimatorScale()
        // A statement opened with Tally, or shared to it. Not on a rotation: it was taken already.
        if (savedInstanceState == null) receive(intent)
        contentResolver.registerContentObserver(
            SystemSettings.Global.getUriFor(SystemSettings.Global.ANIMATOR_DURATION_SCALE), false, animatorObserver,
        )

        setContent {
            val settings by root.settings.collectAsStateWithLifecycle()
            val s = settings ?: return@setContent
            val snackbar = remember { SnackbarHostState() }
            val lift = remember { mutableStateOf(0.dp) }
            TallyTheme(accent = Color(s.accent.argb), accentEnabled = s.accentEnabled, amoled = s.amoled) {
                CompositionLocalProvider(
                    LocalMoney provides rememberMoney(s.currency),
                    LocalSnackbarLift provides lift,
                ) {
                    ProvideTouchExploration {
                        Box(Modifier.fillMaxSize()) {
                            TallyNavHost(onboarded = s.onboarded, incoming = incoming)
                            NoticeHost(
                                notices,
                                snackbar,
                                Modifier
                                    .align(Alignment.BottomCenter)
                                    .navigationBarsPadding()
                                    .imePadding()
                                    .padding(bottom = lift.value),
                            )
                        }
                    }
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        receive(intent)
    }

    /** Hands a file opened with or shared to Tally to the import. Anything else is ignored. */
    private fun receive(intent: Intent?) {
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
        // Unregistered here: a registered observer holds this Activity past its death.
        contentResolver.unregisterContentObserver(animatorObserver)
        // A rotation hands the notice on screen to the next Activity, which shows it again. Leaving
        // the app lets it stand, so an old Undo never waits for the next launch.
        if (!isChangingConfigurations) notices.clear()
        super.onDestroy()
    }

    private fun readAnimatorScale() {
        TallyMotion.durationScale = SystemSettings.Global.getFloat(contentResolver, SystemSettings.Global.ANIMATOR_DURATION_SCALE, 1f)
    }
}
