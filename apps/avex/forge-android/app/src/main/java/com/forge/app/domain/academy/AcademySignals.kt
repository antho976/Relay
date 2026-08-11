package com.forge.app.domain.academy

import com.forge.app.domain.academy.LessonBlock.Bullets
import com.forge.app.domain.academy.LessonBlock.Callout
import com.forge.app.domain.academy.LessonBlock.Example
import com.forge.app.domain.academy.LessonBlock.Paragraph

/**
 * The Signals track (Coach v3 F) — one lesson per slot, written as each slot goes live.
 *
 * Only HRV ships here. Protein and hydration stay COMING_SOON in the registry because the app does
 * not log food, and a lesson about a signal the coach cannot read would be exactly the "teaches
 * more than the coach does" failure the plan rules out.
 */
internal object AcademySignals {

    val stressHrv = Lesson(
        id = "signals.stress_hrv",
        track = LessonTrack.SIGNALS,
        title = "What HRV tells you, and what it doesn't",
        summary = "A trend against your own baseline is useful. A single night is mostly noise.",
        unlock = LessonUnlock(
            label = "Connect a watch that records overnight HRV",
            detail = "About two weeks of nights before a trend is worth reading.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "Heart-rate variability is the variation in time between your heartbeats. Higher " +
                    "generally means your nervous system is in a recovered state; lower means it is " +
                    "still dealing with something, which might be training, illness, alcohol, poor " +
                    "sleep or a stressful week."
            ),
            Paragraph(
                "It is genuinely noisy night to night, and the absolute number means little across " +
                    "people. What is useful is YOUR trend against YOUR own recent normal."
            ),
            Bullets(
                listOf(
                    "The coach compares the last night or two against your prior fortnight",
                    "A meaningful drop shaves a point off readiness, nothing more",
                    "It never overrides what you said in your check-in",
                    "With no watch syncing it, nothing about your coaching changes"
                )
            ),
            Example(
                key = "hrv_trend",
                label = "Your trend",
                fallback = "connect a watch that reports HRV and your trend appears here"
            ),
            Callout("Treat it as one quiet input among several, not a verdict on your day.")
        )
    )

    val all: List<Lesson> = listOf(stressHrv)
}
