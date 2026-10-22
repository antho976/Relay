package com.forge.app.ui.launch

import com.forge.app.appicon.AppIcon
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * The launch hand-off's pure parts, and the resource wiring that makes the Android 12+ splash wear
 * the chosen icon (checked from the files, since no device runs in unit tests).
 */
class LaunchHandOffTest {

    @Test fun aLauncherAliasNamesItsIcon() {
        assertEquals(AppIcon.MetalGold, launchIconFromComponent("com.forge.app.icon.MetalGold"))
        assertEquals(AppIcon.Default, launchIconFromComponent("com.forge.app.icon.Default"))
    }

    @Test fun aRetiredAliasNamesItsSurvivor() =
        assertEquals(AppIcon.SolidEmber, launchIconFromComponent("com.forge.app.icon.SolidAmber"))

    @Test fun anythingElseIsUnknown() {
        assertNull(launchIconFromComponent("com.forge.app.MainActivity"))
        assertNull(launchIconFromComponent(null))
    }

    @Test fun everyIconHasALauncherAlias() {
        val manifest = File("src/main/AndroidManifest.xml").readText()
        AppIcon.entries.forEach {
            assertTrue("No .icon.${it.name} alias", manifest.contains("android:name=\".icon.${it.name}\""))
        }
    }

    /**
     * The Android 12+ splash names no icon, so the platform draws the launching alias's own icon.
     * Naming one here would pin every pick to that drawable (an activity-alias cannot have a theme).
     */
    @Test fun theV31SplashLeavesTheIconToTheLaunchingAlias() {
        val v31 = File("src/main/res/values-v31/themes.xml").readText()
        assertTrue(v31.contains("""<item name="android:windowSplashScreenAnimatedIcon">@null</item>"""))
        assertTrue(v31.contains("""<item name="windowSplashScreenAnimatedIcon">@null</item>"""))
    }

    @Test fun theSplashSettlesATouchSmaller() = assertEquals(112f / 128f, SETTLE_SCALE, 0.0001f)
}
