package com.forge.wear

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import com.forge.wear.data.WearDataRepository
import com.forge.wear.data.WristHaptics
import com.forge.wear.ui.WearRoot
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {

    /**
     * The callback used to be empty, which quietly made the whole ask decorative: the current
     * session is applied by [WearApp] the moment the repository emits it, which is before the
     * system dialog resolves. Granting notifications therefore did not post the OngoingActivity
     * chip, and granting the sensor permission did not start HR — both waited for the phone to
     * publish a different session, or for the app to be restarted.
     */
    private val permissions =
        registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { granted ->
            // Whatever the user decided, re-apply the session under the permissions we now hold.
            // Nothing here is required: a refusal simply leaves the works-without state in place.
            if (granted.values.any { it }) {
                WearApp.applySession(this, WearDataRepository.instance(this).session.value)
            }
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // One ask, all optional: the OngoingActivity chip rides a notification, live HR (W3) needs
        // the api-level-appropriate heart-rate permission, and the exercise's calorie metric needs
        // activity recognition. Denying any of them leaves a permanent works-without state.
        val wanted = buildList {
            addAll(WearHealthPermissions.missing(this@MainActivity))
            if (Build.VERSION.SDK_INT >= 33 &&
                checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
            ) {
                add(Manifest.permission.POST_NOTIFICATIONS)
            }
        }
        if (wanted.isNotEmpty()) permissions.launch(wanted.toTypedArray())

        val repo = WearDataRepository.instance(this)
        // Mid-set glances shouldn't fight the screen timeout — but only mid-SET (P-04). The flag
        // used to be set unconditionally in onCreate and never cleared, so a wrist raised on the
        // idle glance held a watch display awake with nothing running on it. It follows the live
        // session and the rest timer instead: the two states the athlete is actually standing
        // there watching.
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.STARTED) {
                combine(repo.session, repo.timer) { session, timer -> session != null || timer != null }
                    .distinctUntilChanged()
                    .collect { keepAwake -> setKeepScreenOn(keepAwake) }
            }
        }
        val haptics = WristHaptics(this)
        setContent { WearRoot(repo, haptics) }
    }

    /** Backgrounding is never a reason to hold the display: the collector above stops, so clear it. */
    override fun onStop() {
        super.onStop()
        setKeepScreenOn(false)
    }

    private fun setKeepScreenOn(on: Boolean) {
        val flag = android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON
        if (on) window.addFlags(flag) else window.clearFlags(flag)
    }
}
