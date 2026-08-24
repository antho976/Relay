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
        summary = "A few weeks pointed at one thing, because your body responds to the trend rather than to any single session.",
        unlock = LessonUnlock(
            label = "When your first block starts",
            detail = "The coach plans one once it can see far enough ahead to commit to a few weeks.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "A block is a run of weeks that share a purpose. Instead of asking every session " +
                    "to be your hardest, a block builds up to something and then backs off, so the " +
                    "work you did has time to turn into progress."
            ),
            Paragraph(
                "Going hard forever is the alternative, and it doesn't fail straight away. It " +
                    "fails slowly. Tiredness piles up faster than fitness does, so your numbers " +
                    "drift down while the sessions feel harder."
            ),
            Heading("What changes inside one"),
            Bullets(
                listOf(
                    "Volume goes up while you can still recover from it",
                    "Then weight takes over from volume as the main demand",
                    "Then a lighter stretch, to show what you built and measure it",
                    "Then a planned easy week, before the next block starts higher"
                )
            ),
            Callout("Hard work followed by planned rest beats a flat line of effort.")
        )
    )

    val fourPhases = Lesson(
        id = "programming.four_phases",
        track = LessonTrack.PROGRAMMING,
        title = "Accumulate, intensify, peak, deload",
        summary = "Four phases, four different jobs. The coach changes what it asks of you in each one.",
        unlock = LessonUnlock(
            label = "When a block changes phase",
            detail = "A few weeks in, as it moves from building volume to adding load.",
            byYou = false
        ),
        blocks = listOf(
            Heading("Accumulate"),
            Paragraph(
                "Build up your working sets at moderate effort. Most of the growth comes from " +
                    "here, and this is where the coach is most willing to add a set."
            ),
            Heading("Intensify"),
            Paragraph(
                "Trade some of those sets for weight. Fewer sets, heavier bars, effort creeping " +
                    "up. The coach stops adding volume here."
            ),
            Heading("Peak"),
            Paragraph(
                "Show what you built. Volume drops so you turn up fresh enough to actually lift " +
                    "near your best, which is why a test set belongs here and nowhere else."
            ),
            Heading("Deload"),
            Paragraph(
                "Weight and sets both come down. You aren't losing anything. You're giving the " +
                    "work you already did time to finish paying out."
            ),
            Callout("Same program, four different jobs. That is what the phase is telling you.")
        )
    )

    val deloadsAreEarned = Lesson(
        id = "programming.deloads_are_earned",
        track = LessonTrack.PROGRAMMING,
        title = "Deloads are earned, not failures",
        summary = "An easy week cashes in the work you already did. It is not a step backwards.",
        unlock = LessonUnlock(
            label = "When a deload is scheduled",
            detail = "Planned at the end of a block, or pulled earlier if fatigue earns it.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Tiredness clears faster than fitness fades. That gap is the whole trick. Cut the " +
                    "work for a week and the tiredness goes while the fitness stays, so what was " +
                    "underneath it finally shows up."
            ),
            Paragraph(
                "The coach schedules one per block rather than waiting for you to fall apart. It " +
                    "can still pull that week forward if fatigue builds early, but that is no " +
                    "longer the only way you ever get a break."
            ),
            Heading("Being honest about the evidence"),
            Paragraph(
                "Research on deload weeks specifically is thin, and it would be dishonest to say " +
                    "otherwise. What is well supported is that easing off before a performance " +
                    "improves it, and that months of hard training with no let-up wear you down. " +
                    "The coach also checks the result against your own numbers."
            ),
            Callout("If a deload week feels unnecessary, that is usually the sign it arrived on time.")
        )
    )

    val readingYourBlock = Lesson(
        id = "programming.reading_your_block_card",
        track = LessonTrack.PROGRAMMING,
        title = "Reading your block card",
        summary = "Which week you're in, which phase is live, and how long until the planned deload.",
        unlock = LessonUnlock(
            label = "Open your block card",
            detail = "It's at the top of the Coach page once a block is running.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "The bar across the top shows the four phases, with the one you're in filled. " +
                    "Under it, the coach says which week of the block this is and what the phase " +
                    "is asking for."
            ),
            Bullets(
                listOf(
                    "The countdown names the week your deload lands on",
                    "The intent line says which of your goals this block is serving",
                    "A test week says so plainly, because it changes what a good session looks like",
                    "Ending the block takes one tap, and nothing is lost by doing it"
                )
            ),
            Callout("It is a plan, not a contract. You can end it whenever it stops fitting your life.")
        )
    )

    val all: List<Lesson> = listOf(whatABlockIs, fourPhases, deloadsAreEarned, readingYourBlock)
}
