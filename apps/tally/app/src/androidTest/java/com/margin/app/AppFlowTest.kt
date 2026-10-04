package com.margin.app

import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.test.ext.junit.runners.AndroidJUnit4
import leakcanary.DetectLeaksAfterTestSuccess
import org.junit.Rule
import org.junit.Test
import org.junit.rules.RuleChain
import org.junit.rules.TestRule
import org.junit.runner.RunWith

/**
 * The app as a person uses it, on a real device, with LeakCanary watching: after each test passes,
 * [DetectLeaksAfterTestSuccess] dumps the heap and FAILS the test if any destroyed Activity,
 * ViewModel or View is still reachable. A leak is a red build, not a notification nobody reads.
 *
 * The flow walks first run (or uses the sample data path), logs an expense through the keypad,
 * visits every tab, opens settings, and rotates, which rebuilds the Activity and is where a held
 * Context leaks.
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class AppFlowTest {

    private val compose = createAndroidComposeRule<MainActivity>()

    @get:Rule
    val rules: TestRule = RuleChain.outerRule(DetectLeaksAfterTestSuccess()).around(compose)

    private fun waitForText(text: String, substring: Boolean = true, timeout: Long = 10_000) {
        compose.waitUntil(timeout) {
            compose.onAllNodesWithText(text, substring = substring, ignoreCase = true).fetchSemanticsNodes().isNotEmpty()
        }
    }

    private fun tap(text: String, substring: Boolean = true) {
        waitForText(text, substring)
        val node = compose.onAllNodesWithText(text, substring = substring, ignoreCase = true).onFirst()
        // Inside a scrolling form the target may sit below the fold; bring it on screen first.
        runCatching { node.performScrollTo() }
        node.performClick()
        compose.waitForIdle()
    }

    private fun tapDescription(description: String) {
        compose.waitUntil(10_000) {
            compose.onAllNodes(hasContentDescription(description, substring = true, ignoreCase = true)).fetchSemanticsNodes().isNotEmpty()
        }
        compose.onAllNodes(hasContentDescription(description, substring = true, ignoreCase = true)).onFirst().performClick()
        compose.waitForIdle()
    }

    private fun exists(text: String): Boolean =
        compose.onAllNodes(hasText(text, substring = true, ignoreCase = true)).fetchSemanticsNodes().isNotEmpty()

    /** Past first run by the quickest path the app offers: the labelled sample set. */
    private fun ensureHome() {
        compose.waitForIdle()
        if (exists("sample data") && !exists("Recent")) {
            tap("sample data")
        }
        waitForText("Recent", timeout = 20_000)
    }

    @Test
    fun everyTabOpensAndAnExpenseLogs() {
        ensureHome()

        // Log an expense: FAB, keypad 1 2, a category, save.
        tapDescription("Add entry")
        waitForText("Save")
        tap("1", substring = false)
        tap("2", substring = false)
        tap("Groceries")
        tap("Save expense")
        waitForText("Recent")

        // Every tab, then back home.
        tap("Activity", substring = false)
        tap("Plan", substring = false)
        tap("Insights", substring = false)
        tap("Home", substring = false)

        // Settings and back.
        tapDescription("Settings")
        waitForText("Appearance")
        compose.activityRule.scenario.onActivity { it.onBackPressedDispatcher.onBackPressed() }
        waitForText("Recent")
    }

    @Test
    fun rotationRebuildsWithoutLeaking() {
        ensureHome()
        compose.activityRule.scenario.recreate()
        waitForText("Recent")
        tap("Insights", substring = false)
        compose.activityRule.scenario.recreate()
        compose.waitForIdle()
    }
}
