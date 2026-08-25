import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.ksp)
    alias(libs.plugins.hilt)
    alias(libs.plugins.androidx.baselineprofile)
    alias(libs.plugins.roborazzi)
}

// Release signing — reads credentials from forge-android/keystore.properties (gitignored).
// If that file is absent (fresh clone, CI without secrets) the release build is simply left
// unsigned and debug builds are unaffected. Copy keystore.properties.example to get started.
val keystorePropertiesFile = rootProject.file("keystore.properties")
val keystoreProperties = Properties().apply {
    if (keystorePropertiesFile.exists()) keystorePropertiesFile.inputStream().use { load(it) }
}
// A keystore is usable only when ALL four credentials are present. Guard on that, not merely on the
// file existing: a half-filled keystore.properties would otherwise pass file(null) to the signing
// config and fail configuration for EVERY build type (debug included), since signingConfigs is
// evaluated at configuration time.
val hasReleaseKeystore = keystorePropertiesFile.exists() &&
    listOf("storeFile", "storePassword", "keyAlias", "keyPassword")
        .all { !keystoreProperties.getProperty(it).isNullOrBlank() }

android {
    namespace = "com.forge.app"
    // Android 16 is both the Health Connect compile floor and the phone release target.
    compileSdk = 36

    defaultConfig {
        applicationId = "com.quietsoftware.avex"
        minSdk = 26
        targetSdk = 36
        versionCode = 90
        versionName = "0.8.9"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        vectorDrawables { useSupportLibrary = true }
    }

    signingConfigs {
        // Created only when a COMPLETE keystore.properties exists, so an unconfigured or half-filled
        // machine still builds (the release just stays unsigned).
        if (hasReleaseKeystore) {
            create("release") {
                storeFile = file(keystoreProperties.getProperty("storeFile"))
                storePassword = keystoreProperties.getProperty("storePassword")
                keyAlias = keystoreProperties.getProperty("keyAlias")
                keyPassword = keystoreProperties.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        release {
            // R8 shrink + obfuscate + resource-shrink. Keep rules live in proguard-rules.pro;
            // the persistence-critical bit there is keeping enum constant names, since Room/DataStore
            // round-trip a lot of state by enum.name. Run a release APK on a device once after enabling.
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            // Sign the release only when a complete keystore is configured; otherwise it stays unsigned.
            if (hasReleaseKeystore) {
                signingConfig = signingConfigs.getByName("release")
            }
        }
        debug {
            applicationIdSuffix = ".debug"
            isDebuggable = true
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }

    buildFeatures {
        compose = true
        // Generates com.forge.app.BuildConfig (VERSION_NAME) — used by the JSON export's appVersion.
        buildConfig = true
    }

    packaging {
        resources.excludes += "/META-INF/{AL2.0,LGPL2.1}"
    }

    // Make the exported Room schemas available to the migration test (androidTest).
    sourceSets["androidTest"].assets.srcDir("$projectDir/schemas")

    testOptions {
        // Roborazzi renders real Compose through Robolectric, which needs packaged resources.
        unitTests.isIncludeAndroidResources = true
        unitTests.all {
            // Forward -Dforge.regen to the test JVM so RegenerateAllowlist can rewrite the frozen
            // design-doctrine baseline on demand (DesignDoctrineTest). Defaults to off, so a normal
            // CI run can never rewrite the baseline it is supposed to be enforcing.
            it.systemProperty("forge.regen", System.getProperty("forge.regen") ?: "false")
            it.systemProperty("forge.paydown", System.getProperty("forge.paydown") ?: "false")

            // The doctrine tests read .claude/ and the golden screenshots, which Gradle cannot infer
            // from the classpath. Without declaring them, editing DESIGN.md leaves the test task
            // UP-TO-DATE and the parity/self-check suites silently do not run — the doc could drift
            // all the way to a release without one of them firing.
            it.inputs.files(rootProject.fileTree("../.claude"))
                .withPropertyName("designDoctrine")
                .withPathSensitivity(PathSensitivity.RELATIVE)

            // Print the full assertion message on failure. DesignDoctrineTest's messages name the
            // rule, the offending lines and the exact allowlist edit to make — useless if the
            // console only says "AssertionError" and points at an HTML report.
            it.testLogging {
                exceptionFormat = org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
                showExceptions = true
                showCauses = true
                showStackTraces = false
                // The allowlist maintenance tasks report what they changed via println, which
                // Gradle hides by default. Surface it only when one of them was actually asked for,
                // so ordinary runs stay quiet.
                showStandardStreams = System.getProperty("forge.paydown") == "true" ||
                    System.getProperty("forge.regen") == "true"
            }
        }
    }
}

// Room schema export directory (required because we set exportSchema = true)
ksp {
    arg("room.schemaLocation", "$projectDir/schemas")
}

dependencies {
    // The pure protocol/timer core shared with the watch (W1). api: its coroutines/serialization
    // types appear in :app signatures.
    api(project(":shared"))

    // Wearable Data Layer — Bluetooth IPC to the watch app; no network involved (W1).
    implementation(libs.play.services.wearable)
    implementation(libs.kotlinx.coroutines.play.services)

    // Core + lifecycle
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.core.splashscreen)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.activity.compose)
    // AndroidX ExifInterface — reads capture dates from HEIC/PNG/WebP on every supported API level;
    // the framework class misses those on older devices, losing the real date of gallery imports.
    implementation(libs.androidx.exifinterface)

    // Compose (BOM-pinned)
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.ui.graphics)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.compose.material3)
    implementation(libs.compose.material.icons)
    debugImplementation(libs.compose.ui.tooling)

    // Navigation
    implementation(libs.androidx.navigation.compose)

    // Hilt
    implementation(libs.hilt.android)
    ksp(libs.hilt.compiler)
    implementation(libs.hilt.navigation.compose)

    // Room
    implementation(libs.room.runtime)
    implementation(libs.room.ktx)
    ksp(libs.room.compiler)

    // DataStore
    implementation(libs.androidx.datastore.preferences)

    // Health Connect — on-device IPC for recovery signals (sleep, resting HR). No INTERNET; the
    // data is read from the Health Connect system app and never leaves the device.
    implementation(libs.androidx.health.connect)

    // CameraX — in-app guided progress-photo camera (preview + capture; pose-ghost alignment overlay).
    // Photos are written straight to app-private storage, never the camera roll.
    implementation(libs.androidx.camera.core)
    implementation(libs.androidx.camera.camera2)
    implementation(libs.androidx.camera.lifecycle)
    implementation(libs.androidx.camera.view)

    // Biometric — BiometricPrompt (fingerprint/face + device PIN/pattern/password fallback) for the
    // app & photo-gallery lock (GYMAP-69). No app PIN is stored; the OS owns the credential. Requires
    // MainActivity to be a FragmentActivity.
    implementation(libs.androidx.biometric)

    // Coroutines
    implementation(libs.kotlinx.coroutines.android)

    // WorkManager + Hilt Worker injection
    implementation(libs.work.runtime.ktx)
    implementation(libs.hilt.work)
    ksp(libs.hilt.androidx.compiler)

    // Glance for home screen widget (#146)
    implementation("androidx.glance:glance-appwidget:1.1.1")
    implementation("androidx.glance:glance-material3:1.1.1")

    // DocumentFile — enumerate a user-granted folder (Downloads) to auto-find gym-app exports (#GYMAP-17).
    implementation("androidx.documentfile:documentfile:1.0.1")

    // Baseline Profile — installs the generated app/src/main/baseline-prof.txt at runtime (P2).
    // profileinstaller is a no-op until the profile is generated on a device.
    implementation(libs.androidx.profileinstaller)
    baselineProfile(project(":baselineprofile"))

    // Test
    testImplementation(libs.junit)
    // Screenshot testing of the archetype recipes (DESIGN §14: the app must survive 200% font).
    testImplementation(libs.robolectric)
    testImplementation(libs.roborazzi)
    testImplementation(libs.roborazzi.compose)
    testImplementation(libs.roborazzi.junit.rule)
    testImplementation(libs.compose.ui.test.junit4)
    androidTestImplementation(libs.androidx.test.junit)
    androidTestImplementation("androidx.room:room-testing:2.7.2")
    androidTestImplementation(libs.androidx.test.espresso.core)
    androidTestImplementation(platform(libs.compose.bom))
    androidTestImplementation(libs.compose.ui.test.junit4)
    debugImplementation(libs.compose.ui.test.manifest)
}
