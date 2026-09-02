import java.time.Duration
import org.gradle.testing.jacoco.plugins.JacocoTaskExtension
import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.ksp)
    alias(libs.plugins.hilt)
    alias(libs.plugins.androidx.baselineprofile)
    alias(libs.plugins.roborazzi)
    // Gradle's built-in JaCoCo. Coverage here is a MEASUREMENT, not a gate: no threshold is
    // enforced, because a number that fails the build teaches people to write tests that touch
    // lines rather than tests that check behaviour. It exists so "are we covered?" has an answer
    // that is not somebody counting files per package.
    jacoco
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
        versionCode = 91
        versionName = "0.9"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        vectorDrawables { useSupportLibrary = true }
    }

    signingConfigs {
        // Created only when a COMPLETE keystore.properties exists, so an unconfigured or half-filled
        // machine still builds (the release just stays unsigned).
        if (hasReleaseKeystore) {
            create("release") {
                // rootProject.file, not file(...). `file(...)` is MODULE-relative, so one
                // documented path could not be right for both modules at once: the example says
                // storeFile is relative to forge-android/, which made it resolve under app/ here and
                // wear/ next door — and a wrong path fails configuration for EVERY build type,
                // debug included, because signingConfigs is evaluated at configuration time.
                storeFile = rootProject.file(keystoreProperties.getProperty("storeFile"))
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
            // Sign the release only when a complete keystore is configured; otherwise it stays
            // unsigned — EXCEPT under -PforgeCiSmokeSigning, which signs it with the DEBUG key so CI
            // can install and launch the minified APK on an emulator. Every R8-only defect (an empty
            // keep-rule file, an enum name asymmetry across the wire) is invisible to a pipeline that
            // only checks R8 finished, and an unsigned APK cannot be installed to find out. The flag
            // is opt-in and CI-only, so a real release can never be debug-signed by accident.
            if (!hasReleaseKeystore && providers.gradleProperty("forgeCiSmokeSigning").isPresent) {
                signingConfig = signingConfigs.getByName("debug")
            }
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
    kotlinOptions {
        jvmTarget = "17"
        // Pin the annotation-use-site default (KT-73255) instead of inheriting whatever a future
        // Kotlin release picks. The warnings this clears are spread across data/repo and service,
        // and the change they warn about is BEHAVIOURAL — an annotation silently moving from the
        // property to the constructor parameter (or back) rewires what Hilt, Room and
        // kotlinx-serialization see. Choosing it deliberately now means a Kotlin upgrade is a
        // version bump rather than a behaviour change.
        freeCompilerArgs += listOf("-Xannotation-default-target=param-property")
    }

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
            // A ceiling for the WHOLE task, as the backstop under runTest's per-test timeout. A
            // Robolectric test that wedges outside a coroutine (a native lock, a stuck file handle)
            // has nothing else to stop it, and the next thing that notices is the CI job timing out
            // an hour later with a log that ends mid-test.
            it.timeout.set(Duration.ofMinutes(20))

            // Without this, coverage silently omits every Robolectric test in the suite — which is
            // most of the data layer. Robolectric loads the classes under test through its own
            // sandbox classloader, and those classes arrive at JaCoCo with no code-source location;
            // JaCoCo skips such classes by default, so SettingsRepository read 0 of 325 lines while
            // nine tests were exercising it. A coverage number that quietly excludes a whole
            // category of test is worse than no number, because it gets believed.
            it.extensions.configure(JacocoTaskExtension::class.java) {
                isIncludeNoLocationClasses = true
                // JDK internals arrive the same way and are not ours to measure.
                excludes = listOf("jdk.internal.*")
            }

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

            // The goldens themselves, for exactly the same reason — and this half was MISSING even
            // though the comment above always claimed both. src/test/screenshots/ is not a resources
            // directory, so nothing put it on the task's inputs, and replacing a golden PNG with a
            // completely different image left :app:verifyRoborazziDebug UP-TO-DATE. The gate that
            // exists to catch clipping and overlap regressions would simply not have run, and a
            // skipped comparison reports as a pass.
            it.inputs.files(project.fileTree("src/test/screenshots"))
                .withPropertyName("screenshotGoldens")
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

/**
 * `./gradlew -p forge-android :app:coverageReport` — HTML at
 * app/build/reports/jacoco/coverageReport/html/index.html, XML beside it for CI.
 *
 * Deliberately scoped to what a unit test could plausibly cover. Generated code (Hilt components,
 * Room DAO implementations, Compose lambdas, R, BuildConfig) is excluded: it is written by a
 * processor, it is verified by the fact that the app compiles and runs, and leaving it in produces
 * a number that moves when a build tool changes rather than when the test suite does.
 *
 * Compose UI is excluded for the same reason in reverse — it IS covered, by the 41 Roborazzi
 * goldens, and those record pixels rather than executed lines, so counting them here would
 * understate coverage while adding thousands of untestable lambda classes to the denominator.
 */
val coverageReport = tasks.register<JacocoReport>("coverageReport") {
    group = "verification"
    description = "JaCoCo coverage for the JVM unit tests (:app)."
    dependsOn("testDebugUnitTest")

    reports {
        html.required.set(true)
        xml.required.set(true)
        csv.required.set(false)
    }

    val generated = listOf(
        "**/R.class", "**/R$*.class", "**/BuildConfig.*", "**/Manifest*.*",
        // Hilt / Dagger / KSP output
        "**/*_Factory*.*", "**/*_MembersInjector*.*", "**/*_HiltModules*.*",
        "**/Hilt_*.*", "**/*_Impl*.*", "**/DaggerForgeApp*.*", "**/*_Provide*Factory*.*",
        // Compose compiler output: ComposableSingletons holders and lambda classes
        "**/ComposableSingletons*.*", "**/*ComposableSingletons*",
        "**/*\$\$inlined\$*.*",
        // Screens and composables — covered by the Roborazzi goldens, not by line execution
        "**/ui/**/*Screen*.*", "**/ui/theme/**",
    )

    classDirectories.setFrom(
        files(
            fileTree(layout.buildDirectory.dir("tmp/kotlin-classes/debug")) { exclude(generated) },
            fileTree(layout.buildDirectory.dir("intermediates/javac/debug/classes")) { exclude(generated) }
        )
    )
    sourceDirectories.setFrom(files("src/main/java", "$rootDir/shared/src/main/kotlin"))
    executionData.setFrom(fileTree(layout.buildDirectory) { include("**/testDebugUnitTest.exec") })
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
    implementation("androidx.documentfile:documentfile:1.1.0")

    // Baseline Profile — installs the generated app/src/main/baseline-prof.txt at runtime (P2).
    // profileinstaller is a no-op until the profile is generated on a device.
    implementation(libs.androidx.profileinstaller)
    baselineProfile(project(":baselineprofile"))

    // Test
    testImplementation(libs.junit)
    // runTest, not runBlocking, for anything suspending: it carries a per-test timeout, so a test
    // that deadlocks on IO fails in seconds NAMING ITSELF, instead of hanging until the CI job's
    // own timeout kills the whole task with no idea which test was stuck.
    testImplementation(libs.kotlinx.coroutines.test)
    // Screenshot testing of the archetype recipes (DESIGN §14: the app must survive 200% font).
    testImplementation(libs.robolectric)
    testImplementation(libs.roborazzi)
    testImplementation(libs.roborazzi.compose)
    testImplementation(libs.roborazzi.junit.rule)
    testImplementation(libs.compose.ui.test.junit4)
    androidTestImplementation(libs.androidx.test.junit)
    androidTestImplementation("androidx.room:room-testing:2.8.4")
    androidTestImplementation(libs.androidx.test.espresso.core)
    androidTestImplementation(platform(libs.compose.bom))
    androidTestImplementation(libs.compose.ui.test.junit4)
    debugImplementation(libs.compose.ui.test.manifest)
}
