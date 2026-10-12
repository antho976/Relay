pluginManagement {
    repositories {
        google {
            content {
                includeGroupByRegex("com\\.android.*")
                includeGroupByRegex("com\\.google.*")
                includeGroupByRegex("androidx.*")
            }
        }
        mavenCentral()
        gradlePluginPortal()
    }
}
plugins {
    id("org.gradle.toolchains.foojay-resolver-convention") version "1.0.0"
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "Tally"
include(":app")
// Pure Kotlin/JVM: money, periods, pace, recurrence, CSV and backup codecs. No Android types, so it
// tests in milliseconds and the app layer cannot leak into the arithmetic.
include(":core")
// Pure Kotlin/JVM: Relay's door and the request that connects this phone to the owner's server.
include(":link")
