package com.forge.app

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.view.KeyEvent
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.service.AutoBackupWorker
import com.forge.app.ui.nav.ForgeNavHost
import com.forge.app.ui.theme.ForgeTheme
import com.forge.app.ui.theme.ForgeUiSettings
import com.forge.app.ui.theme.LocalForgeSettings
import androidx.lifecycle.lifecycleScope
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch
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

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            notifPermLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
        }

        AutoBackupWorker.schedule(this)

        // Apply privacy mode (#152) — collect once synchronously via runBlocking to avoid layout flash
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
                }.combine(settingsRepo.showEncouragement) { s, v ->
                    s.copy(showEncouragement = v)
                }.combine(settingsRepo.compactSetLogging) { s, v ->
                    s.copy(compactSetLogging = v)
                }.combine(settingsRepo.overviewTileOrder) { s, order ->
                    s.copy(overviewTileOrder = order)
                }.combine(settingsRepo.accentColorHex) { s, v ->
                    s.copy(accentColorHex = v)
                }
            }
            val uiSettings by uiSettingsFlow.collectAsState(initial = ForgeUiSettings())

            CompositionLocalProvider(LocalForgeSettings provides uiSettings) {
                ForgeTheme(
                    amoledMode     = uiSettings.amoledMode,
                    accentColorHex = uiSettings.accentColorHex
                ) {
                    ForgeNavHost()
                }
            }
        }
    }
}
