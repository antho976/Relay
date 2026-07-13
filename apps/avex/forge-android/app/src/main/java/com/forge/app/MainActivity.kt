package com.forge.app

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.database.ContentObserver
import android.net.Uri
import android.graphics.drawable.GradientDrawable
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
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.core.content.IntentCompat
import com.forge.app.data.importer.ImportResult
import com.forge.app.appicon.AppIcon
import com.forge.app.appicon.AppIconManager
import com.forge.app.data.importer.WorkoutImportRepository
import com.forge.app.data.importer.userMessage
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.service.AutoBackupWorker
import com.forge.app.ui.common.AvexIntro
import com.forge.app.ui.common.ProvideTouchExploration
import com.forge.app.ui.nav.ForgeNavHost
import com.forge.app.ui.onboarding.OnboardingScreen
import androidx.compose.ui.graphics.toArgb
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.ForgeTheme
import com.forge.app.ui.theme.ForgeUiSettings
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.forgeBackgroundGradient
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
    @Inject lateinit var importRepo: WorkoutImportRepository
    @Inject lateinit var appIconManager: AppIconManager

    /** Set when a shared/opened export file has been imported — shows a one-time result dialog (#GYMAP-17). */
    private var shareImportMessage by mutableStateOf<String?>(null)

    /**
     * The widget deep-link day to open. State (not a local) because `launchMode=singleTask` delivers a
     * widget tap while the app is already running via [onNewIntent], not a fresh [onCreate] — updating
     * this recomposes the nav host so it opens the day, instead of the tap silently doing nothing.
     */
    private var pendingWidgetDayKey by mutableStateOf<String?>(null)

    /** Emits volume-down presses for the "log same as last set" shortcut (#151). */
    var onVolumeDown: (() -> Unit)? = null

    /** The chosen app-icon key, seeded in onCreate and kept live by a collector so [onStop] never has to
     *  block on a DataStore read. @Volatile because onStop (main thread) reads what the collector writes. */
    @Volatile private var appIconKey: String = ""

    /** True only when the user themselves sent the app to the background (Home/Recents) — set by
     *  [onUserLeaveHint], which the framework does NOT call when WE launch a sub-activity (the system
     *  photo picker, share sheet, export/file picker). [onStop] fires for those overlays too, so gating
     *  the icon-alias swap on this flag keeps the swap out of the mid-session overlay case that can tear
     *  the task down on some OEMs (see [AppIconManager]). */
    private var userLeaving = false

    /**
     * A CSV/JSON export shared into or opened with Avex (#GYMAP-17): import it directly, no file
     * browsing. The sender grants a one-shot read permission on the URI, so we read it immediately.
     */
    private fun handleImportIntent(intent: Intent?) {
        val uri: Uri? = when (intent?.action) {
            Intent.ACTION_SEND -> IntentCompat.getParcelableExtra(intent, Intent.EXTRA_STREAM, Uri::class.java)
            Intent.ACTION_VIEW -> intent.data
            else -> null
        }
        if (uri == null) return
        lifecycleScope.launch {
            val result = runCatching { importRepo.import(uri) }.getOrDefault(ImportResult.ReadError)
            shareImportMessage = result.userMessage()
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleImportIntent(intent)
        // singleTask reuse: a widget tap arrives here, not in onCreate — pick up the day so the nav
        // host opens it (the deep-link would otherwise be dropped and the tap do nothing).
        intent.getStringExtra(com.forge.app.widget.EXTRA_START_DAY_KEY)?.let { pendingWidgetDayKey = it }
    }

    /** One-time "here's what came in" dialog after a share-to-Avex import. */
    @Composable
    private fun ShareImportResultDialog() {
        val msg = shareImportMessage ?: return
        AlertDialog(
            onDismissRequest = { shareImportMessage = null },
            title = { Text("Import") },
            text = { Text(msg) },
            confirmButton = { TextButton(onClick = { shareImportMessage = null }) { Text("OK") } }
        )
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent?): Boolean {
        if (keyCode == KeyEvent.KEYCODE_VOLUME_DOWN) {
            onVolumeDown?.invoke()?.let { return true }
        }
        return super.onKeyDown(keyCode, event)
    }

    /** The user pressed Home/Recents — the app is genuinely leaving the foreground (not just being
     *  covered by a sub-activity we launched). This is the safe moment to flip the launcher alias. */
    override fun onUserLeaveHint() {
        super.onUserLeaveHint()
        userLeaving = true
    }

    override fun onResume() {
        super.onResume()
        userLeaving = false
    }

    /**
     * Swap the home-screen launcher icon to the saved pick now that the USER has backgrounded us — the
     * only safe time to flip the `.icon.*` aliases. Doing it while the app is still foreground, OR while
     * a sub-activity WE launched (system photo picker, share sheet, export/file picker) covers us, can
     * tear the task down on some OEMs (e.g. Samsung) even with DONT_KILL_APP. onStop fires for those
     * overlays too, so we gate on [userLeaving] ([onUserLeaveHint], which the framework does not call for
     * self-launched sub-activities) and skip [isChangingConfigurations] (rotation). Done SYNCHRONOUSLY so
     * the swap lands before the process can be reaped (a swipe-away from recents kills us moments after
     * onStop); reads the collector-cached [appIconKey] (no blocking DataStore read on the main thread),
     * and the toggle only runs when the pick actually changed. Wrapped so a failure can't crash onStop.
     */
    override fun onStop() {
        super.onStop()
        if (isChangingConfigurations || !userLeaving) return
        userLeaving = false
        runCatching { appIconManager.reconcileTo(AppIcon.fromKey(appIconKey)) }
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

    /**
     * Paint the activity window with the active theme's background gradient, overriding the static
     * XML `windowBackground`. This is what shows in the brief "UI not loaded" frame after the splash
     * and before Compose's first draw — keying it on the live [amoled] setting (via the shared
     * [forgeBackgroundGradient]) means it always matches the current theme and auto-adapts when the
     * theme is later changed, with no resource edit.
     */
    private fun applyAdaptiveWindowBackground(amoled: Boolean) {
        val (top, bottom) = forgeBackgroundGradient(amoled)
        window.setBackgroundDrawable(
            GradientDrawable(
                GradientDrawable.Orientation.TOP_BOTTOM,
                intArrayOf(top.toArgb(), bottom.toArgb())
            )
        )
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
                    "Avex can nudge you to train, send a weekly recap, and alert you when your rest ends. " +
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
        if (savedInstanceState == null)
            pendingWidgetDayKey = intent?.getStringExtra(com.forge.app.widget.EXTRA_START_DAY_KEY)

        // A cold-start share/open of an export file (#GYMAP-17) — import it once, not again on a
        // config-change recreate (the dialog's own state keeps the result visible across rotation).
        if (savedInstanceState == null) handleImportIntent(intent)

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
        // then keep a collector for live changes. Paint the window background to match the active
        // theme in the same pass so the post-splash "UI not loaded" frame never flashes an off-theme
        // color — it tracks the live amoled setting, so a later theme change adapts the boot
        // background on its own (no XML edit needed).
        // Also read the chosen app icon here so the launch intro can theme itself to it on the very
        // first frame (no plain→themed pop), plus the "Custom startup animation" setting so a user who
        // turned it off goes straight to the plain black-and-white Avex with no themed flash. One cached
        // DataStore read pass, same as privacy/amoled.
        val (introIconKey, themedIntro) = runBlocking {
            applyPrivacyMode(settingsRepo.privacyMode.first())
            applyAdaptiveWindowBackground(settingsRepo.amoledMode.first())
            settingsRepo.appIcon.first() to settingsRepo.themedLaunchIntro.first()
        }
        appIconKey = introIconKey
        lifecycleScope.launch {
            settingsRepo.privacyMode.collect { enabled -> applyPrivacyMode(enabled) }
        }
        // Keep the icon pick live so onStop can read it without blocking on DataStore (and never stale).
        lifecycleScope.launch {
            settingsRepo.appIcon.collect { appIconKey = it }
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
                }.combine(settingsRepo.keepScreenOn) { s, v ->
                    s.copy(keepScreenOn = v)
                }.combine(settingsRepo.overviewTileOrder) { s, order ->
                    s.copy(overviewTileOrder = order)
                }.combine(settingsRepo.accentColorHex) { s, v ->
                    s.copy(accentColorHex = v)
                }.combine(settingsRepo.accentEnabled) { s, v ->
                    s.copy(accentEnabled = v)
                }.combine(settingsRepo.plateWeightLb) { s, v ->
                    s.copy(plateWeightLb = v)
                }.combine(settingsRepo.firstWorkoutDone) { s, v ->
                    s.copy(firstWorkoutDone = v)
                }.combine(settingsRepo.useMiles) { s, v ->
                    s.copy(useMiles = v)
                }
            }
            val uiSettings by uiSettingsFlow.collectAsState(initial = ForgeUiSettings())
            val onboardingDone by settingsRepo.onboardingDone.collectAsState(initial = null)
            LaunchedEffect(onboardingDone) { if (onboardingDone != null) contentReady = true }

            CompositionLocalProvider(LocalForgeSettings provides uiSettings) {
                ForgeTheme(
                    amoledMode     = uiSettings.amoledMode,
                    accentColorHex = uiSettings.accentColorHex,
                    accentEnabled  = uiSettings.accentEnabled
                ) {
                    // One app-level touch-exploration observer feeds every bounceClick (A11y).
                    ProvideTouchExploration {
                        // Launch wordmark plays once per cold launch, over the first screen composed
                        // beneath it. rememberSaveable so a rotation mid-intro doesn't replay it.
                        var showIntro by rememberSaveable { mutableStateOf(true) }
                        Box(Modifier.fillMaxSize()) {
                            when (onboardingDone) {
                                false -> OnboardingScreen(onFinished = {})
                                true -> {
                                    ForgeNavHost(initialDayKey = pendingWidgetDayKey)
                                    NotifPermissionRationale()
                                    RestoreConfirmedDialog(restoreJustCompleted)
                                }
                                null -> {} // DataStore still loading; the theme's gradient shows briefly
                            }
                            if (showIntro) AvexIntro(iconKey = introIconKey, themed = themedIntro, onDone = { showIntro = false })
                            // Result of a share-to-Avex import — shown over whatever screen is up.
                            ShareImportResultDialog()
                        }
                    }
                }
            }
        }
    }
}
