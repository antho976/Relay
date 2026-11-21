package com.forge.app.data.health

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File

/**
 * Every Health Connect permission Avex can ask for must be declared in the manifest, or Health
 * Connect refuses the grant and the feature behind it is a permanent no-op on a real install.
 *
 * That is exactly how body-fat sync shipped dead (H-04): [HealthConnectManager.bodyFatPermissions]
 * requested READ_BODY_FAT and WRITE_BODY_FAT, neither appeared in AndroidManifest.xml, so
 * `canReadBodyFat`/`canWriteBodyFat` stayed false forever while the Recovery row offered Connect.
 * Nothing else compared the two lists, so this test does. It reads the SOURCE manifest, not the
 * merged one, which is enough: the health permissions are only ever declared here.
 */
@RunWith(RobolectricTestRunner::class)
class HealthConnectManifestTest {

    private val manager by lazy { HealthConnectManager(ApplicationProvider.getApplicationContext<Context>()) }

    /** Every permission set the manager exposes; a new set must be added here to be covered. */
    private fun requestedPermissions(): Set<String> =
        manager.permissions + manager.weightPermissions + manager.historyPermissions +
            manager.bodyFatPermissions + manager.caloriePermissions + manager.stepsPermissions +
            manager.exercisePermissions + manager.hrvPermissions + manager.leanMassPermissions +
            manager.watchWorkoutPermissions + manager.sessionWritePermissions

    private val manifest: String by lazy {
        val moduleRoot = generateSequence(File(".").canonicalFile) { it.parentFile }
            .firstOrNull { File(it, "src/main/AndroidManifest.xml").isFile }
            ?: error("Could not locate the :app module from ${File(".").canonicalPath}")
        File(moduleRoot, "src/main/AndroidManifest.xml").readText()
    }

    @Test
    fun everyRequestedHealthPermissionIsDeclaredInTheManifest() {
        val missing = requestedPermissions().filterNot { manifest.contains("android:name=\"$it\"") }
        assertTrue(
            "Requested but not declared in AndroidManifest.xml, so Health Connect can never grant it: $missing",
            missing.isEmpty()
        )
    }

    @Test
    fun bodyFatSyncRequestsBothDirections() {
        assertEquals(
            setOf("android.permission.health.READ_BODY_FAT", "android.permission.health.WRITE_BODY_FAT"),
            manager.bodyFatPermissions
        )
    }

    @Test
    fun weightHistoryRequestsTheHistoryPermission() {
        // The first-connect backfill can only reach past Health Connect's 30-day window with this
        // grant (H-05); the weight row requests it together with the ordinary read + write.
        assertEquals(setOf("android.permission.health.READ_HEALTH_DATA_HISTORY"), manager.historyPermissions)
        assertTrue(manager.historyPermissions.none { it in manager.weightPermissions })
    }
}
