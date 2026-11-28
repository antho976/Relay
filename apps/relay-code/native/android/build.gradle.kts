// Relay Code's Android shell. The app is Rust (native/crates/relay-code, built into
// src/main/jniLibs by tools/build-lib.sh); Gradle packs it with GameActivity, the AndroidX activity
// that gives a native app the soft keyboard. Two Java classes: CodeActivity (Back, the clipboard,
// notifications) and LinkService (keeps the link to the PC alive in the background). No Kotlin.
plugins {
    alias(libs.plugins.android.application)
}

android {
    namespace = "com.quietsoftware.relaycode"
    compileSdk = 37

    defaultConfig {
        // Its own id: it installs beside the Kotlin app (com.quietsoftware.relay.android) until
        // it replaces it (design 44, Q4).
        applicationId = "com.quietsoftware.relaycode"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            // No release key yet: the debug key, so it installs on the owner's phone.
            signingConfig = signingConfigs.getByName("debug")
        }
    }
}

// The library is built outside Gradle (tools/build-lib.sh). Without it the APK would install and
// crash at launch, so the build stops and says what to run.
val checkNativeLib = tasks.register("checkNativeLib") {
    val jniLibsDir = layout.projectDirectory.dir("src/main/jniLibs").asFile
    doLast {
        val libs = jniLibsDir.listFiles()?.filter { File(it, "librelaycode.so").isFile }.orEmpty()
        if (libs.isEmpty()) {
            throw GradleException("No librelaycode.so under ${jniLibsDir}: run native/tools/build-lib.sh [abi ...] first")
        }
    }
}
tasks.named("preBuild") { dependsOn(checkNativeLib) }

dependencies {
    // The version android-activity 0.6 is written against.
    implementation("androidx.games:games-activity:4.4.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.core:core:1.17.0")
}
