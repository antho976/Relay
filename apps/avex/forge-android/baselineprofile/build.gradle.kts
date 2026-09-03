plugins {
    alias(libs.plugins.android.test)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.androidx.baselineprofile)
}

android {
    namespace = "com.forge.app.baselineprofile"
    // Tracks :app: this module builds against it, so it needs the same APIs visible.
    compileSdk = 37

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }

    defaultConfig {
        // Baseline-profile generation + cold-start Macrobenchmark need API 28+ (AOT control).
        minSdk = 28
        targetSdk = 36
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    // The app this module profiles + benchmarks.
    targetProjectPath = ":app"
}

// Generate the profile on a connected device/emulator:  ./gradlew :baselineprofile:generateBaselineProfile
baselineProfile {
    useConnectedDevices = true
    // NOTE: this plugin (1.4.1) prints a one-line "tested below AGP 9.0" warning on AGP 9.2.1. It's
    // harmless — the tasks below configure + run fine; revisit when a newer baselineprofile ships.
}

dependencies {
    implementation(libs.androidx.test.junit)
    implementation(libs.androidx.test.espresso.core)
    implementation(libs.androidx.test.uiautomator)
    implementation(libs.androidx.benchmark.macro.junit4)
}
