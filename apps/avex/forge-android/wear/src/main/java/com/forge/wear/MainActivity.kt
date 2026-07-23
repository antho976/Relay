package com.forge.wear

import android.Manifest
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import com.forge.wear.data.WearDataRepository
import com.forge.wear.data.WristHaptics
import com.forge.wear.ui.WearRoot

class MainActivity : ComponentActivity() {

    private val permissions =
        registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { /* both optional */ }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // One ask, both optional: the OngoingActivity chip rides a notification, and live HR (W3)
        // needs the body sensor. Denying either leaves a permanent works-without state.
        val wanted = buildList {
            if (checkSelfPermission(Manifest.permission.BODY_SENSORS) != android.content.pm.PackageManager.PERMISSION_GRANTED) {
                add(Manifest.permission.BODY_SENSORS)
            }
            if (Build.VERSION.SDK_INT >= 33 &&
                checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != android.content.pm.PackageManager.PERMISSION_GRANTED
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
