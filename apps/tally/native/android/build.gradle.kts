// Tally RS's Android shell. The app is Rust (native/crates/tally-rs, built into src/main/jniLibs by
// tools/build-lib.sh); Gradle packs it with GameActivity, the AndroidX activity that gives a native
// app the soft keyboard. The one Java class, src/main/java/com/tally/rs/TallyActivity.java, adds
// only what Android gives through Java callbacks: the file picker's result and the share sheet's
// FileProvider (androidx.core). No Kotlin.
import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
}

// Release signing, as the Kotlin app does (app/build.gradle.kts): keystore.properties at the repo
// root (gitignored) with storeFile, storePassword, keyAlias and keyPassword. With a complete file
// the release is signed with that key, the one com.quietsoftware.tally2 must be signed with when
// Tally RS takes the package over. Without one (or with a half-filled one) the release falls back
// to the debug key, so it still installs on the owner's phone.
val keystorePropertiesFile = rootProject.file("keystore.properties")
val keystoreProperties = Properties().apply {
    if (keystorePropertiesFile.exists()) keystorePropertiesFile.inputStream().use { load(it) }
}
val hasReleaseKeystore = keystorePropertiesFile.exists() &&
    listOf("storeFile", "storePassword", "keyAlias", "keyPassword")
        .all { !keystoreProperties.getProperty(it).isNullOrBlank() }

android {
    namespace = "com.tally.rs"
    compileSdk = 37

    defaultConfig {
        // Its own id, so it installs beside the Kotlin Tally until it can do everything.
        applicationId = "com.quietsoftware.tally.rs"
        minSdk = 26
        targetSdk = 36
        versionCode = 6
        versionName = "0.3.3"
    }

    signingConfigs {
        if (hasReleaseKeystore) {
            create("release") {
                storeFile = rootProject.file(keystoreProperties.getProperty("storeFile"))
                storePassword = keystoreProperties.getProperty("storePassword")
                keyAlias = keystoreProperties.getProperty("keyAlias")
                keyPassword = keystoreProperties.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName(if (hasReleaseKeystore) "release" else "debug")
        }
    }
}

// The library is built outside Gradle (tools/build-lib.sh). Without it the APK would install and
// crash at launch, so the build stops and says what to run.
val checkNativeLib = tasks.register("checkNativeLib") {
    // A local, not a script property: the configuration cache cannot keep the script itself.
    val jniLibsDir = layout.projectDirectory.dir("src/main/jniLibs").asFile
    doLast {
        val libs = jniLibsDir.listFiles()?.filter { File(it, "libtally.so").isFile }.orEmpty()
        if (libs.isEmpty()) {
            throw GradleException("No libtally.so under ${jniLibsDir}: run native/tools/build-lib.sh [abi ...] first")
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
