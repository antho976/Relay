// Tally RS's Android shell. The app is Rust (native/crates/tally-rs, built into src/main/jniLibs by
// tools/build-lib.sh); Gradle packs it with GameActivity, the AndroidX activity that gives a native
// app the soft keyboard. The one Java class, src/main/java/com/tally/rs/TallyActivity.java, adds
// only what Android gives through Java callbacks: the file picker's result and the share sheet's
// FileProvider (androidx.core). No Kotlin.
plugins {
    alias(libs.plugins.android.application)
}

android {
    namespace = "com.tally.rs"
    compileSdk = 37

    defaultConfig {
        // Its own id, so it installs beside the Kotlin Tally until it can do everything.
        applicationId = "com.quietsoftware.tally.rs"
        minSdk = 26
        targetSdk = 36
        versionCode = 5
        versionName = "0.3.2"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("debug")
        }
    }
}

dependencies {
    // The version android-activity 0.6 is written against.
    implementation("androidx.games:games-activity:4.4.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.core:core:1.17.0")
}
