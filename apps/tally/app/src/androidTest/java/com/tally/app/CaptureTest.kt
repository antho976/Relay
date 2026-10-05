package com.tally.app

import android.graphics.Bitmap
import android.os.ParcelFileDescriptor
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ComposeTimeoutException
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.RuleChain
import org.junit.rules.TestRule
import org.junit.runner.RunWith
import org.junit.runners.model.Statement
import java.io.File

/**
 * A real device screenshot of Home for the design finish review: the status bar, the raised
 * navigation bar with its indicator pill, the add FAB and the system insets, none of which a
 * Robolectric golden draws. It writes `captures/<captureName>.png` under the app's external files
 * directory, where `.github/scripts/capture.sh` pulls it from.
 *
 * Not a check. It runs only when the instrumentation is given `-e captureName <name>`, so the
 * ordinary connectedDebugAndroidTest run in CI skips it.
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class CaptureTest {

    /** Null in a plain connectedDebugAndroidTest run; capture.sh passes it. */
    private val captureName: String? = InstrumentationRegistry.getArguments().getString("captureName")

    private val compose = createAndroidComposeRule<MainActivity>()

    /**
     * Asked before the compose rule applies: that rule launches MainActivity before the test body
     * runs, so an assumption inside the test would still start the app on every CI run only to skip.
     */
    private val onlyWhenAsked = TestRule { base, _ ->
        object : Statement() {
            override fun evaluate() {
                assumeTrue("No captureName argument, so there is nothing to capture.", !captureName.isNullOrBlank())
                base.evaluate()
            }
        }
    }

    @get:Rule
    val rules: TestRule = RuleChain.outerRule(onlyWhenAsked).around(compose)

    /** Every text and description on screen, so a timeout says what the app was showing instead. */
    private fun onScreen(): String {
        val labelled = SemanticsMatcher("has text or description") {
            SemanticsProperties.Text in it.config || SemanticsProperties.ContentDescription in it.config
        }
        return compose.onAllNodes(labelled).fetchSemanticsNodes().flatMap { node ->
            node.config.getOrElse(SemanticsProperties.Text) { emptyList() }.map { it.text } +
                node.config.getOrElse(SemanticsProperties.ContentDescription) { emptyList() }
        }.distinct().take(60).joinToString(" | ")
    }

    private fun waitOrExplain(what: String, timeout: Long, condition: () -> Boolean) {
        try {
            compose.waitUntil(timeout, condition)
        } catch (e: ComposeTimeoutException) {
            throw AssertionError("Waited ${timeout}ms for $what. On screen: ${onScreen()}", e)
        }
    }

    private fun tap(text: String) {
        waitOrExplain("\"$text\"", 10_000) { exists(text) }
        val node = compose.onAllNodesWithText(text, substring = true, ignoreCase = true).onFirst()
        // On a short screen or at a large font the target may sit below the fold.
        runCatching { node.performScrollTo() }
        node.performClick()
        compose.waitForIdle()
    }

    private fun exists(text: String): Boolean =
        compose.onAllNodes(hasText(text, substring = true, ignoreCase = true)).fetchSemanticsNodes().isNotEmpty()

    /** Home's top bar Settings button is always on screen, and only Home has one (as in AppFlowTest). */
    private fun onHome(): Boolean =
        compose.onAllNodes(hasContentDescription("Settings")).fetchSemanticsNodes().isNotEmpty()

    /**
     * Past first run by the labelled sample set, as AppFlowTest does. It waits for either screen
     * first: until the settings load the Activity draws nothing, so neither is there yet. A second
     * capture on the same install (the 200% font one) finds Home straight away.
     */
    private fun ensureHome() {
        waitOrExplain("first run or Home", 20_000) { onHome() || exists("sample data") }
        if (!onHome()) tap("sample data")
        waitOrExplain("Home (its Settings button)", 20_000) { onHome() }
    }

    /**
     * Home draws only its title row until its first load, and the Settings button lives in that
     * row, so reaching Home is not the same as Home being drawn. The hero panel is the first thing
     * the load brings; its label reads one of these three.
     */
    private fun homeDrawn(): Boolean =
        exists("Left to spend") || exists("Over budget") || exists("Left from income")

    /** Runs [command] as the shell user and waits for it to finish. */
    private fun shell(command: String) {
        val pfd = InstrumentationRegistry.getInstrumentation().uiAutomation.executeShellCommand(command)
        ParcelFileDescriptor.AutoCloseInputStream(pfd).use { it.readBytes() }
    }

    @Test
    fun captureHome() {
        val name = checkNotNull(captureName)
        ensureHome()
        waitOrExplain("Home's hero panel", 30_000) { homeDrawn() }
        compose.waitForIdle()
        // A software-rendered emulator on a CI runner can leave a "System UI isn't responding"
        // dialog over the app. Error dialogs close on this broadcast; the shell user may send it.
        shell("am broadcast -a android.intent.action.CLOSE_SYSTEM_DIALOGS")
        // Idle covers Compose only. The status and navigation bars settle their tint and scrim
        // after the window does, and the capture is about exactly those bars.
        Thread.sleep(2_500)

        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val shot = checkNotNull(instrumentation.uiAutomation.takeScreenshot()) { "The device returned no screenshot." }
        val files = checkNotNull(instrumentation.targetContext.getExternalFilesDir(null)) { "No external files directory." }
        val dir = File(files, "captures")
        check(dir.isDirectory || dir.mkdirs()) { "Could not create $dir." }
        File(dir, "$name.png").outputStream().use { out ->
            check(shot.compress(Bitmap.CompressFormat.PNG, 100, out)) { "PNG encoding failed for $name." }
        }
        shot.recycle()
    }
}
