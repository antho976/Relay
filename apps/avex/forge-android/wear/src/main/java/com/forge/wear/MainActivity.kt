package com.forge.wear

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import com.forge.wear.data.WearDataRepository
import com.forge.wear.data.WristHaptics
import com.forge.wear.ui.WearRoot

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
        // Mid-set glances shouldn't fight the screen timeout while the app is foreground.
        window.addFlags(android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)

        val repo = WearDataRepository.instance(this)
        val haptics = WristHaptics(this)
        setContent { WearRoot(repo, haptics) }
    }
}
