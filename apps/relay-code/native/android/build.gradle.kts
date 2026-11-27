// Relay Code's Android shell. The app is Rust (native/crates/relay-code, built into
// src/main/jniLibs by tools/build-lib.sh); Gradle packs it with GameActivity, the AndroidX activity
// that gives a native app the soft keyboard. Two Java classes: CodeActivity (Back, the clipboard,
// notifications) and LinkService (keeps the link to the PC alive in the background). No Kotlin.
plugins {
    alias(libs.plugins.android.application)
}

// The build number: the commit count, the same one tools/build-lib.sh builds the library with
// (RELAY_CODE_BUILD, which the app compares with the PC's phone.app.latest). RELAY_CODE_BUILD wins
// when set.
val relayCodeBuild: Int = (System.getenv("RELAY_CODE_BUILD")?.trim()?.toIntOrNull()
    ?: providers.exec {
        commandLine("git", "rev-list", "--count", "HEAD")
        workingDir = rootDir
        isIgnoreExitValue = true
    }.standardOutput.asText.get().trim().toIntOrNull()
    ?: 1)

android {
    namespace = "com.quietsoftware.relaycode"
    compileSdk = 37

    defaultConfig {
        // Its own id: it installs beside the Kotlin app (com.quietsoftware.relay.android) until
        // it replaces it (design 44, Q4).
        applicationId = "com.quietsoftware.relaycode"
        minSdk = 26
        targetSdk = 36
        versionCode = relayCodeBuild
        versionName = "0.$relayCodeBuild"
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
