package com.margin.app

import android.database.ContentObserver
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
import com.margin.app.ui.common.LocalMoney
import com.margin.app.ui.common.NoticeHost
import com.margin.app.ui.common.Notices
import com.margin.app.ui.common.ProvideTouchExploration
import com.margin.app.ui.common.rememberMoney
import com.margin.app.ui.nav.LocalSnackbarLift
import com.margin.app.ui.nav.MarginNavHost
import com.margin.app.ui.theme.MarginMotion
import com.margin.app.ui.theme.MarginTheme
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject

@AndroidEntryPoint
class MainActivity : ComponentActivity() {

    @Inject lateinit var notices: Notices

    private val root: RootViewModel by viewModels()

    /** Keeps [MarginMotion.durationScale] live with the system's Remove animations setting. */
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
        contentResolver.registerContentObserver(
            SystemSettings.Global.getUriFor(SystemSettings.Global.ANIMATOR_DURATION_SCALE), false, animatorObserver,
        )

        setContent {
            val settings by root.settings.collectAsStateWithLifecycle()
            val s = settings ?: return@setContent
            val snackbar = remember { SnackbarHostState() }
            val lift = remember { mutableStateOf(0.dp) }
            MarginTheme(accent = Color(s.accent.argb), accentEnabled = s.accentEnabled, amoled = s.amoled) {
                CompositionLocalProvider(
                    LocalMoney provides rememberMoney(s.currency),
                    LocalSnackbarLift provides lift,
                ) {
                    ProvideTouchExploration {
                        Box(Modifier.fillMaxSize()) {
                            MarginNavHost(onboarded = s.onboarded)
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

    override fun onDestroy() {
        // Unregistered here: a registered observer holds this Activity past its death.
        contentResolver.unregisterContentObserver(animatorObserver)
        super.onDestroy()
    }

    private fun readAnimatorScale() {
        MarginMotion.durationScale = SystemSettings.Global.getFloat(contentResolver, SystemSettings.Global.ANIMATOR_DURATION_SCALE, 1f)
    }
}
