package com.tally.app

import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ComposeTimeoutException
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasScrollToNodeAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performScrollToNode
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

    private fun waitForText(text: String, substring: Boolean = true, timeout: Long = 10_000) {
        waitOrExplain("\"$text\"", timeout) {
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
        waitOrExplain("a control described \"$description\"", 10_000) {
            compose.onAllNodes(hasContentDescription(description, substring = true, ignoreCase = true)).fetchSemanticsNodes().isNotEmpty()
        }
        compose.onAllNodes(hasContentDescription(description, substring = true, ignoreCase = true)).onFirst().performClick()
        compose.waitForIdle()
    }

    /**
     * Screens are lazy lists: a row below the fold is not composed, so it cannot be waited for.
     * Scroll the screen's list to it first, as a person would.
     */
    private fun scrollToText(text: String) {
        compose.onAllNodes(hasScrollToNodeAction()).onFirst()
            .performScrollToNode(hasText(text, substring = true, ignoreCase = true))
        waitForText(text)
    }

    private fun exists(text: String): Boolean =
        compose.onAllNodes(hasText(text, substring = true, ignoreCase = true)).fetchSemanticsNodes().isNotEmpty()

    /**
     * Home is a lazy list, so a panel below the fold (Recent, Upcoming) is never composed and can't
     * be waited for. Its top bar's Settings button is always on screen, and only Home has one.
     */
    private fun onHome(): Boolean =
        compose.onAllNodes(hasContentDescription("Settings")).fetchSemanticsNodes().isNotEmpty()

    private fun waitForHome(timeout: Long = 20_000) {
        waitOrExplain("Home (its Settings button)", timeout) { onHome() }
    }

    /** Past first run by the quickest path the app offers: the labelled sample set. */
    private fun ensureHome() {
        compose.waitForIdle()
        if (!onHome() && exists("sample data")) {
            tap("sample data")
        }
        waitForHome()
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
        waitForHome()

        // Every tab, then back home.
        tap("Plan", substring = false)
        tap("Insights", substring = false)
        tap("Home", substring = false)

        // Settings and back. Its page title is always on screen; its rows depend on screen height.
        tapDescription("Settings")
        waitForText("Settings", substring = false)
        scrollToText("Appearance")
        compose.activityRule.scenario.onActivity { it.onBackPressedDispatcher.onBackPressed() }
        waitForHome()
    }

    @Test
    fun rotationRebuildsWithoutLeaking() {
        ensureHome()
        compose.activityRule.scenario.recreate()
        waitForHome()
        tap("Insights", substring = false)
        compose.activityRule.scenario.recreate()
        compose.waitForIdle()
    }
}
