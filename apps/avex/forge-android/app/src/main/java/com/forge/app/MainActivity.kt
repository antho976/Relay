package com.forge.app

import android.Manifest
import android.content.pm.PackageManager
import android.database.ContentObserver
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
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
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.service.AutoBackupWorker
import com.forge.app.ui.common.ProvideTouchExploration
import com.forge.app.ui.nav.ForgeNavHost
import com.forge.app.ui.onboarding.OnboardingScreen
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.ForgeTheme
import com.forge.app.ui.theme.ForgeUiSettings
import com.forge.app.ui.theme.LocalForgeSettings
import androidx.lifecycle.lifecycleScope
import dagger.hilt.android.AndroidEntryPoint
import java.io.File
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

    /** The system animation-scale (0 when "Remove animations" is on) — single read used at startup
     *  and by the live observer below. */
    private fun readDurationScale(): Float =
        Settings.Global.getFloat(contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f)

    /** Re-reads the system animation-scale into ForgeMotion live, so toggling "Remove animations"
     *  (a vestibular accessibility need) takes effect without a cold restart (A11y). */
    private val durationScaleObserver = object : ContentObserver(Handler(Looper.getMainLooper())) {
        override fun onChange(selfChange: Boolean) {
            ForgeMotion.durationScale = readDurationScale()
        }
    }

    override fun onDestroy() {
        contentResolver.unregisterContentObserver(durationScaleObserver)
        super.onDestroy()
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

    /** True exactly once after a boot-time restore swap (ForgeApp wrote the flag), then cleared. */
    private fun consumeRestoreFlag(): Boolean {
        val f = File(filesDir, ForgeApp.RESTORE_DONE_FLAG)
        return if (f.exists()) { f.delete(); true } else false
    }

    /** One-time "your backup was restored" confirmation — the silent boot-swap otherwise gives no sign.
     *  rememberSaveable so a rotation mid-dialog keeps it on screen (the flag is already consumed). */
    @Composable
    private fun RestoreConfirmedDialog(initiallyShown: Boolean) {
        var show by rememberSaveable { mutableStateOf(initiallyShown) }
        if (!show) return
        AlertDialog(
            onDismissRequest = { show = false },
            title = { Text("Backup restored") },
            text = { Text("Your backup was restored successfully.") },
            confirmButton = { TextButton(onClick = { show = false }) { Text("OK") } }
        )
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        val splash = installSplashScreen()
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // Consume the restore flag before the UI composes so the confirmation shows exactly once.
        // Only on a fresh launch (not a config-change recreate) — on rotation the dialog's own
        // rememberSaveable state keeps it visible, since the flag is already gone.
        val restoreJustCompleted = savedInstanceState == null && consumeRestoreFlag()

        // Widget deep-link: a home-screen widget tap carries the day to open (the next-up day, or the
        // active session's day when one is in progress). Read it once on a fresh launch and hand it to
        // the nav host, which opens it on top of Overview so Back returns home.
        val widgetDayKey = if (savedInstanceState == null)
            intent?.getStringExtra(com.forge.app.widget.EXTRA_START_DAY_KEY) else null

        // Hold the splash until prefs resolve, so the bare theme gradient (the null onboarding state)
        // never flashes before the first real screen (P1). A 2s backstop releases it even if the prefs
        // flow never emits (corrupt/stalled DataStore) — a brief gradient beats a permanent splash.
        var contentReady = false
        splash.setKeepOnScreenCondition { !contentReady }
        lifecycleScope.launch { delay(2000); contentReady = true }

        // Honor the system "Remove animations" preference so ForgeMotion gates every transition,
        // and keep honoring it LIVE: register an observer so toggling it mid-session takes effect
        // on the next animation instead of needing a cold restart (vestibular accessibility need).
        ForgeMotion.durationScale = readDurationScale()
        contentResolver.registerContentObserver(
            Settings.Global.getUriFor(Settings.Global.ANIMATOR_DURATION_SCALE), false, durationScaleObserver
        )

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
                    // One app-level touch-exploration observer feeds every bounceClick (A11y).
                    ProvideTouchExploration {
                        when (onboardingDone) {
                            false -> OnboardingScreen(onFinished = {})
                            true -> {
                                ForgeNavHost(initialDayKey = widgetDayKey)
                                NotifPermissionRationale()
                                RestoreConfirmedDialog(restoreJustCompleted)
                            }
                            null -> {} // DataStore still loading; the theme's gradient shows briefly
                        }
                    }
                }
            }
        }
    }
}
