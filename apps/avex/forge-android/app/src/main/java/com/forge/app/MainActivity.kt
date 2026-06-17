package com.forge.app

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.view.KeyEvent
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.service.AutoBackupWorker
import com.forge.app.ui.nav.ForgeNavHost
import com.forge.app.ui.onboarding.OnboardingScreen
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.ForgeTheme
import com.forge.app.ui.theme.ForgeUiSettings
import com.forge.app.ui.theme.LocalForgeSettings
import androidx.lifecycle.lifecycleScope
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import javax.inject.Inject

@AndroidEntryPoint
class MainActivity : ComponentActivity() {

    @Inject lateinit var settingsRepo: SettingsRepository

    /** Emits volume-down presses for the "log same as last set" shortcut (#151). */
    var onVolumeDown: (() -> Unit)? = null

    override fun onKeyDown(keyCode: Int, event: KeyEvent?): Boolean {
        if (keyCode == KeyEvent.KEYCODE_VOLUME_DOWN) {
            onVolumeDown?.invoke()?.let { return true }
        }
        return super.onKeyDown(keyCode, event)
    }

    private fun applyPrivacyMode(enabled: Boolean) {
        if (enabled) {
            window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        } else {
            window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
        }
    }

    private val notifPermLauncher = registerForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { /* proceed either way; notifications no-op if denied */ }

    /** One-time, explained POST_NOTIFICATIONS request (N1) — shown after onboarding, never a cold blast. */
    @Composable
    private fun NotifPermissionRationale() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
        val asked by settingsRepo.notifPermAsked.collectAsState(initial = true) // true until loaded → no flash
        var show by remember { mutableStateOf(false) }
        LaunchedEffect(asked) {
            show = !asked && this@MainActivity.checkSelfPermission(
                Manifest.permission.POST_NOTIFICATIONS
            ) != PackageManager.PERMISSION_GRANTED
        }
        if (!show) return
        val markAsked: () -> Unit = {
            show = false
            lifecycleScope.launch { settingsRepo.setNotifPermAsked() }
        }
        AlertDialog(
            onDismissRequest = markAsked,
            title = { Text("Stay on track?") },
            text = {
                Text(
                    "Forge can nudge you to train, send a weekly recap, and alert you when your rest ends. " +
                        "You can turn each off any time in Settings → Notifications."
                )
            },
            confirmButton = {
                TextButton(onClick = { markAsked(); notifPermLauncher.launch(Manifest.permission.POST_NOTIFICATIONS) }) {
                    Text("Allow")
                }
            },
            dismissButton = { TextButton(onClick = markAsked) { Text("Not now") } }
        )
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        val splash = installSplashScreen()
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        // Hold the splash until prefs resolve, so the bare theme gradient (the null onboarding state)
        // never flashes before the first real screen (P1). A 2s backstop releases it even if the prefs
        // flow never emits (corrupt/stalled DataStore) — a brief gradient beats a permanent splash.
        var contentReady = false
        splash.setKeepOnScreenCondition { !contentReady }
        lifecycleScope.launch { delay(2000); contentReady = true }

        // Honor the system "Remove animations" preference so ForgeMotion gates every transition.
        ForgeMotion.durationScale =
            Settings.Global.getFloat(contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f)

        // POST_NOTIFICATIONS is requested through a rationale gate in the UI (N1) — after onboarding,
        // explained, and only once — instead of a context-free system prompt on cold launch.

        AutoBackupWorker.schedule(this)

        // Apply privacy mode (#152) synchronously BEFORE the first frame so it's never unsecured,
        // then keep a collector for live changes.
        runBlocking { applyPrivacyMode(settingsRepo.privacyMode.first()) }
        lifecycleScope.launch {
            settingsRepo.privacyMode.collect { enabled -> applyPrivacyMode(enabled) }
        }

        setContent {
            val uiSettingsFlow = remember {
                combine(
                    settingsRepo.amoledMode,
                    settingsRepo.useKg,
                    settingsRepo.dateFormat,
                    settingsRepo.timeFormat24h,
                    settingsRepo.firstDayMonday,
                    settingsRepo.hapticStrength,
                    settingsRepo.quietHoursEnabled,
                    settingsRepo.quietHoursStart,
                    settingsRepo.quietHoursEnd
                ) { values ->
                    ForgeUiSettings(
                        amoledMode = values[0] as Boolean,
                        useKg = values[1] as Boolean,
                        dateFormat = values[2] as String,
                        timeFormat24h = values[3] as Boolean,
                        firstDayMonday = values[4] as Boolean,
                        hapticStrength = values[5] as String,
                        quietHoursEnabled = values[6] as Boolean,
                        quietHoursStart = values[7] as Int,
                        quietHoursEnd = values[8] as Int
                    )
                }.combine(settingsRepo.hiddenOverviewTiles) { s, hidden ->
                    s.copy(hiddenOverviewTiles = hidden)
                }.combine(settingsRepo.compactSetLogging) { s, v ->
                    s.copy(compactSetLogging = v)
                }.combine(settingsRepo.overviewTileOrder) { s, order ->
                    s.copy(overviewTileOrder = order)
                }.combine(settingsRepo.accentColorHex) { s, v ->
                    s.copy(accentColorHex = v)
                }.combine(settingsRepo.accentEmphasis) { s, v ->
                    s.copy(accentEmphasis = v)
                }.combine(settingsRepo.plateWeightLb) { s, v ->
                    s.copy(plateWeightLb = v)
                }.combine(settingsRepo.firstWorkoutDone) { s, v ->
                    s.copy(firstWorkoutDone = v)
                }
            }
            val uiSettings by uiSettingsFlow.collectAsState(initial = ForgeUiSettings())
            val onboardingDone by settingsRepo.onboardingDone.collectAsState(initial = null)
            LaunchedEffect(onboardingDone) { if (onboardingDone != null) contentReady = true }

            CompositionLocalProvider(LocalForgeSettings provides uiSettings) {
                ForgeTheme(
                    amoledMode     = uiSettings.amoledMode,
                    accentColorHex = uiSettings.accentColorHex
                ) {
                    when (onboardingDone) {
                        false -> OnboardingScreen(onFinished = {})
                        true -> {
                            ForgeNavHost()
                            NotifPermissionRationale()
                        }
                        null -> {} // DataStore still loading; the theme's gradient shows briefly
                    }
                }
            }
        }
    }
}
