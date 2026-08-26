import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

// Same keystore, same guard as :app — an unconfigured machine still builds (release unsigned).
val keystorePropertiesFile = rootProject.file("keystore.properties")
val keystoreProperties = Properties().apply {
    if (keystorePropertiesFile.exists()) keystorePropertiesFile.inputStream().use { load(it) }
}
val hasReleaseKeystore = keystorePropertiesFile.exists() &&
    listOf("storeFile", "storePassword", "keyAlias", "keyPassword")
        .all { !keystoreProperties.getProperty(it).isNullOrBlank() }

android {
    namespace = "com.forge.wear"
    compileSdk = 36

    defaultConfig {
        // SAME applicationId as the phone — required for Play wear-track distribution and for the
        // Data Layer to pair the two apps. The wear APK is distinguished by its own versionCode
        // scheme: phone versionCode + 100_000 (Play requires distinct codes per APK on one listing).
        applicationId = "com.quietsoftware.avex"
        minSdk = 30 // Wear OS 3+ (every Galaxy Watch since 2021; the plan's locked floor).
        // Matches :app and compileSdk. Play's annual target-API rule reaches API 36 for updates at
        // the end of August 2026, and a listing is only as current as its least-current APK — a
        // wear APK left at 35 would block the whole release.
        targetSdk = 36
        versionCode = 100_090
        versionName = "0.9"
    }

    signingConfigs {
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
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            if (hasReleaseKeystore) signingConfig = signingConfigs.getByName("release")
        }
        debug {
            // MUST match the phone's debug suffix — the Data Layer only pairs identical package names.
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

    buildFeatures { compose = true }
}

dependencies {
    implementation(project(":shared"))

    implementation(libs.androidx.core.ktx)
    // The wearable/ongoing libs pull a pre-1.3 fragment transitively, which trips lint-vital's
    // InvalidFragmentVersionForActivityResult on registerForActivityResult — pin a modern one.
    implementation("androidx.fragment:fragment:1.9.0")
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.activity.compose)

    // Compose for Wear OS (round-screen material + foundation) over the shared Compose BOM.
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.ui.graphics)
    implementation(libs.wear.compose.material)
    implementation(libs.wear.compose.foundation)

    // Wearable Data Layer (Bluetooth IPC to the phone; no network on either APK).
    implementation(libs.play.services.wearable)
    implementation(libs.kotlinx.coroutines.play.services)
    implementation(libs.kotlinx.coroutines.android)

    // Session presence on the watch face + one-tap return (W1).
    implementation(libs.wear.ongoing)
    implementation(libs.androidx.core.splashscreen)

    // Tiles + complications (W4).
    implementation(libs.wear.tiles)
    implementation(libs.protolayout)
    implementation(libs.protolayout.material)
    implementation(libs.protolayout.expression)
    implementation(libs.watchface.complications.data.source.ktx)
    implementation(libs.concurrent.futures.ktx)

    // Live HR during sessions via Health Services (W3). Its API returns Guava ListenableFutures,
    // so guava-android must be on the classpath for the await() bridge.
    implementation(libs.health.services.client)
    implementation("com.google.guava:guava:33.3.1-android")

    testImplementation(libs.junit)
}
