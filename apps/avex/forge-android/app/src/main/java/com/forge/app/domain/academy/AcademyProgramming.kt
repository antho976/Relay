package com.forge.app.domain.academy

import com.forge.app.domain.academy.LessonBlock.Bullets
import com.forge.app.domain.academy.LessonBlock.Callout
import com.forge.app.domain.academy.LessonBlock.Heading
import com.forge.app.domain.academy.LessonBlock.Paragraph

/**
 * The Programming track's first batch (Coach v3 C) — P1–P4, the periodization lessons, each wired
 * to the block moment that makes it relevant.
 *
 * Honest framing throughout: the volume research is strong, the deload research is thin, and the
 * lessons say which is which rather than dressing practice up as evidence.
 */
internal object AcademyProgramming {

    val whatABlockIs = Lesson(
        id = "programming.what_a_block_is",
        track = LessonTrack.PROGRAMMING,
        title = "What a training block is",
        summary = "A few weeks with one intent, because the body adapts to trends rather than to single sessions.",
        unlock = LessonUnlock(
            label = "When your first block starts",
            detail = "The coach plans one once it can see far enough ahead to commit to a few weeks.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "A block is a stretch of weeks that share a purpose. Instead of asking every session " +
                    "to be maximal, a block builds toward something and then backs off so the work " +
                    "can turn into adaptation."
            ),
            Paragraph(
                "The alternative, going hard indefinitely, does not fail immediately. It fails " +
                    "slowly: fatigue accumulates faster than fitness, and performance drifts down " +
                    "while effort drifts up."
            ),
            Heading("What changes inside one"),
            Bullets(
                listOf(
                    "Volume rises while you can still recover from it",
                    "Then load takes over from volume as the demand",
                    "Then a week to express what you built, and measure it",
                    "Then a planned week back, before the next block starts higher"
                )
            ),
            Callout("Organised stress followed by planned rest beats a flat line of effort.")
        )
    )

    val fourPhases = Lesson(
        id = "programming.four_phases",
        track = LessonTrack.PROGRAMMING,
        title = "Accumulate, intensify, peak, deload",
        summary = "Four phases, four different jobs. The coach changes what it asks of you in each.",
        unlock = LessonUnlock(
            label = "When a block changes phase",
            detail = "A few weeks in, as it moves from building volume to adding load.",
            byYou = false
        ),
        blocks = listOf(
            Heading("Accumulate"),
            Paragraph(
                "Build working volume at moderate effort. This is where most of the growth stimulus " +
                    "comes from, and where the coach is most willing to add a set."
            ),
            Heading("Intensify"),
            Paragraph(
                "Trade some of that volume for load. Fewer sets, heavier work, effort creeping up. " +
                    "The coach stops adding volume here."
            ),
            Heading("Peak"),
            Paragraph(
                "Express what you built. Volume drops so you can be fresh enough to actually show " +
                    "your strength, and this is where a test set is worth doing."
            ),
            Heading("Deload"),
            Paragraph(
                "Loads and volume both come down. You are not losing anything: you are letting the " +
                    "accumulated work finish becoming adaptation."
            ),
            Callout("Same programme, four different questions. That is what the phase is for.")
        )
    )

    val deloadsAreEarned = Lesson(
        id = "programming.deloads_are_earned",
        track = LessonTrack.PROGRAMMING,
        title = "Deloads are earned, not failures",
        summary = "A scheduled easy week cashes in the work you already did. It is not a step backwards.",
        unlock = LessonUnlock(
            label = "When a deload is scheduled",
            detail = "Planned at the end of a block, or pulled earlier if fatigue earns it.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Fatigue builds faster than fitness fades. That gap is why a week of reduced work " +
                    "usually leaves you stronger rather than weaker: the fitness stays, the fatigue " +
                    "clears, and what was underneath shows up."
            ),
            Paragraph(
                "The coach schedules one per block rather than waiting for you to break down. The " +
                    "fatigue tripwire is still there, and it can pull the deload earlier, but it is " +
                    "no longer the only way rest ever happens."
            ),
            Heading("Being honest about the evidence"),
            Paragraph(
                "Direct research on deloading specifically is thin. What is well supported is that " +
                    "reducing training stress before a performance improves it, and that chronic " +
                    "unrelieved load degrades it. The coach also checks the result on your own data."
            ),
            Callout("If a deload week feels unnecessary, that is usually the sign it arrived on time.")
        )
    )

    val readingYourBlock = Lesson(
        id = "programming.reading_your_block_card",
        track = LessonTrack.PROGRAMMING,
        title = "Reading your block card",
        summary = "Week in block, current phase, and how long until the planned deload.",
        unlock = LessonUnlock(
            label = "Open your block card",
            detail = "It's at the top of the Coach page once a block is running.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "The rail shows the four phases with the live one filled. Under it, the coach states " +
                    "which week of the block you are in and what this phase is asking for."
            ),
            Bullets(
                listOf(
                    "The countdown names the week your deload lands on",
                    "The intent line says which of your goals this block serves",
                    "A test week says so explicitly, because it changes what a good session looks like",
                    "Ending the block is one tap, and nothing is lost by doing it"
                )
            ),
            Callout("It is a plan, not a contract. You can end it whenever it stops fitting your life.")
        )
    )

    val all: List<Lesson> = listOf(whatABlockIs, fourPhases, deloadsAreEarned, readingYourBlock)
}
