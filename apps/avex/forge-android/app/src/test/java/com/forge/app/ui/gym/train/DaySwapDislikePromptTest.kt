package com.forge.app.ui.gym.train

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The pure gate deciding whether a "Make default" swap raises the post-swap "dislike the swapped-out
 * exercise?" prompt. Every mute / no-op condition must suppress it; only a fully-eligible swap shows it.
 */
class DaySwapDislikePromptTest {

    @Test
    fun showsWhenEnabledFreshDifferentAndNotDisliked() {
        assertTrue(
            shouldShowDislikePrompt(
                promptEnabled = true,
                suppressedThisSession = false,
                originalId = "old-ex",
                swapId = "new-ex",
                alreadyDisliked = false
            )
        )
    }

    @Test
    fun hiddenWhenPromptDisabled() {
        assertFalse(
            shouldShowDislikePrompt(
                promptEnabled = false,
                suppressedThisSession = false,
                originalId = "old-ex",
                swapId = "new-ex",
                alreadyDisliked = false
            )
        )
    }

    @Test
    fun hiddenWhenSuppressedThisSession() {
        assertFalse(
            shouldShowDislikePrompt(
                promptEnabled = true,
                suppressedThisSession = true,
                originalId = "old-ex",
                swapId = "new-ex",
                alreadyDisliked = false
            )
        )
    }

    @Test
    fun hiddenWhenSwappedToTheSameExercise() {
        assertFalse(
            shouldShowDislikePrompt(
                promptEnabled = true,
                suppressedThisSession = false,
                originalId = "same-ex",
                swapId = "same-ex",
                alreadyDisliked = false
            )
        )
    }

    @Test
    fun hiddenWhenAlreadyDisliked() {
        assertFalse(
            shouldShowDislikePrompt(
                promptEnabled = true,
                suppressedThisSession = false,
                originalId = "old-ex",
                swapId = "new-ex",
                alreadyDisliked = true
            )
        )
    }
}
