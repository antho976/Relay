import org.jetbrains.kotlin.gradle.dsl.JvmTarget

// Pure Kotlin/JVM: Relay's door (the handshake, the bus lines) and the request that lets this phone
// sync with the owner's server. No Android types, so it tests on the plain JVM against a real door.
plugins {
    alias(libs.plugins.kotlin.jvm)
    alias(libs.plugins.kotlin.serialization)
}

java {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
}

kotlin {
    compilerOptions { jvmTarget.set(JvmTarget.JVM_17) }
}

dependencies {
    api(libs.kotlinx.serialization.json)
    api(libs.kotlinx.coroutines.core)
    api(libs.okhttp)
    testImplementation(libs.junit)
    testImplementation(libs.kotlinx.coroutines.test)
    testImplementation(libs.okhttp.mockwebserver)
}

tasks.test {
    // The live test runs only when pointed at a server (TALLY_E2E_SERVER, see CloudLiveTest).
    environment("TALLY_E2E_SERVER", System.getenv("TALLY_E2E_SERVER") ?: "")
    testLogging {
        exceptionFormat = org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
        events("failed")
    }
}
