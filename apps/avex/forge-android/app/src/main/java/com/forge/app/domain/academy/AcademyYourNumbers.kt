package com.forge.app.domain.academy

import com.forge.app.domain.academy.LessonBlock.Bullets
import com.forge.app.domain.academy.LessonBlock.Callout
import com.forge.app.domain.academy.LessonBlock.Example
import com.forge.app.domain.academy.LessonBlock.Heading
import com.forge.app.domain.academy.LessonBlock.Paragraph

/**
 * The "your numbers" batch (Coach v3 D) — P5–P8 plus C4.
 *
 * These are the lessons that only make sense once the coach has measured YOU: your volume
 * landmarks, your recovery spacing, your best rep ranges, and what it does with them. Each one
 * shows the population default beside the personal number, because the comparison is the lesson.
 */
internal object AcademyYourNumbers {

    val volumeLandmarks = Lesson(
        id = "programming.your_volume_landmarks",
        track = LessonTrack.PROGRAMMING,
        title = "MEV and MRV: your volume landmarks",
        summary = "The least volume that still works, and the most you can recover from. The useful zone is between them.",
        unlock = LessonUnlock(
            label = "When it measures your own volume ceiling",
            detail = "It starts from population defaults and replaces them as your log shows what you recover from.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Two numbers bracket useful training for any muscle. The minimum effective volume " +
                    "is the least weekly work that still produces progress. The maximum recoverable " +
                    "volume is the most you can absorb before it stops paying back."
            ),
            Paragraph(
                "More volume helps until it doesn't. The dose-response curve bends, and past your " +
                    "own ceiling extra sets buy fatigue rather than growth."
            ),
            Heading("Where your numbers came from"),
            Paragraph(
                "Population defaults started your caps. The coach has now watched your own weeks: " +
                    "did the weeks where a muscle got more work produce more strength than the weeks " +
                    "it got less? If yes, your ceiling moved up. If not, it came down."
            ),
            Example(
                key = "volume_caps",
                label = "Your caps",
                fallback = "a couple more months of history and your own numbers appear here"
            ),
            Callout("Your cap can only move so far from the default. One noisy month should not rewrite your training.")
        )
    )

    val recoveryCurve = Lesson(
        id = "programming.your_recovery_curve",
        track = LessonTrack.PROGRAMMING,
        title = "Your recovery curve",
        summary = "How many days you personally need before a muscle performs again.",
        unlock = LessonUnlock(
            label = "When your recovery pace changes the schedule",
            detail = "Estimated from how you perform bout to bout on the same muscle.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Everyone is told to train a muscle every two to three days. That range exists " +
                    "because the real answer varies with load, volume, age, sleep and how much of " +
                    "your life is already physical."
            ),
            Paragraph(
                "The coach reads it from your sessions: at which spacing does your work actually " +
                    "hold up? That is the gap it plans around."
            ),
            Example(
                key = "recovery_days",
                label = "Your spacing",
                fallback = "a few more months of sessions and this will read your own gap"
            ),
            Callout("Frequency is a lever, not a commandment. Total weekly volume matters more than how you split it.")
        )
    )

    val sweetSpotReps = Lesson(
        id = "programming.sweet_spot_reps",
        track = LessonTrack.PROGRAMMING,
        title = "Your sweet-spot rep ranges",
        summary = "Muscle grows across a wide rep range. Within it, your log shows where you actually progress.",
        unlock = LessonUnlock(
            label = "When your best rep range emerges",
            detail = "Your log has to show where you actually progress before prescriptions lean there.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Growth happens across a wide span of rep ranges, roughly five to thirty, as long " +
                    "as the sets are taken close enough to failure. Strength is more specific: it " +
                    "prefers the loads you actually train at."
            ),
            Paragraph(
                "That freedom means your own data can decide. The coach buckets your bouts by rep " +
                    "count and asks which bucket carried the most strength gain, then leans " +
                    "prescriptions there."
            ),
            Callout("The best rep range is the one you progress and recover on. That's measurable, and it's yours.")
        )
    )

    val imbalances = Lesson(
        id = "programming.imbalances",
        track = LessonTrack.PROGRAMMING,
        title = "Imbalances, and why the coach hunts them",
        summary = "Ratios between opposing groups, measured in your own sets and strength.",
        unlock = LessonUnlock(
            label = "When it finds a side-to-side gap",
            detail = "It watches push against pull, and quads against hamstrings, in both volume and strength.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "An imbalance is a persistent gap between opposing groups: pushing against pulling, " +
                    "quads against hamstrings. The coach measures it in the work you actually log, " +
                    "not in how you look."
            ),
            Heading("Why it's worth fixing"),
            Bullets(
                listOf(
                    "The lagging side eventually caps the strong one, so the plateau arrives anyway",
                    "The joint-health argument is real but weaker than the evidence for volume, and it is stated as such here",
                    "It's usually the cheapest lever available: a couple of sets a week, not a new programme"
                )
            ),
            Callout("A catch-up block is boring and it works. That is the whole method.")
        )
    )

    val whatAProjectIs = Lesson(
        id = "coach.what_a_project_is",
        track = LessonTrack.COACH,
        title = "What a project is",
        summary = "One named improvement at a time, with a why, a plan and a finish line.",
        unlock = LessonUnlock(
            label = "When the coach proposes a project",
            detail = "It hunts for your biggest single lever and runs one at a time.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "The coach permanently scans for your single biggest available improvement: a " +
                    "lagging muscle, an imbalance, missing conditioning, short sleep, work you keep " +
                    "skipping. It proposes ONE at a time."
            ),
            Paragraph(
                "One is deliberate. A list of eight things to fix is a list nobody acts on. One " +
                    "project with a stated finish line is a thing that gets finished."
            ),
            Bullets(
                listOf(
                    "Every project names why it exists, in your own numbers",
                    "Every project states what ends it",
                    "You can drop one at any time, and the coach won't propose that kind again",
                    "Finished and dropped projects both stay on the record"
                )
            ),
            Callout("This is \"what should I improve?\" answered before you have to ask it.")
        )
    )

    val all: List<Lesson> = listOf(
        volumeLandmarks, recoveryCurve, sweetSpotReps, imbalances, whatAProjectIs
    )
}
