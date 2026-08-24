package com.forge.app.domain.academy

import com.forge.app.domain.academy.LessonBlock.Bullets
import com.forge.app.domain.academy.LessonBlock.Callout
import com.forge.app.domain.academy.LessonBlock.Example
import com.forge.app.domain.academy.LessonBlock.Heading
import com.forge.app.domain.academy.LessonBlock.Paragraph

/**
 * The Fundamentals track (Coach v3 B3) — F1–F10, the only sequential track, and the one that makes
 * Decision Zero true for a day-one user: below the data gates these lessons ARE the directive.
 *
 * Written to the plan's bound: teach exactly what the coach does, no more. Every lesson here
 * explains something the app will actually do to you, so a user who reads all ten could override
 * any of it. Sources for each are in `docs/ACADEMY_LESSONS.md`; lessons themselves cite nothing.
 *
 * Prose rule (2026-08-23 pass): short sentences, everyday words, one idea per paragraph. A reader
 * halfway through a warm-up should never have to re-read a line. Terms of art are earned, not
 * assumed — if a lesson needs one, it defines it in the sentence that introduces it.
 */
internal object AcademyFundamentals {

    val whatAProgramIs = Lesson(
        id = "fundamentals.what_a_program_is",
        track = LessonTrack.FUNDAMENTALS,
        title = "What a program is",
        summary = "Repeating the same structure is what makes progress measurable. Change everything every week and there is nothing to compare.",
        unlock = LessonUnlock(
            label = "Open your first program day",
            detail = "Unlocks as soon as there's a plan to open.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "A program is a set of days you repeat, each with the same exercise slots in it. " +
                    "The exact movements matter less than the fact that they come back. Because " +
                    "they come back, what you log this week means something next to last week."
            ),
            Paragraph(
                "Change everything every session and there is nothing to compare. You would be " +
                    "exercising, which is fine, but you would not be training."
            ),
            Heading("Why this one"),
            Bullets(
                listOf(
                    "It fits the number of days a week you said you can train",
                    "It only uses equipment you said you have",
                    "It spreads the work across your muscles so nothing gets hammered",
                    "You can change any of it, and the coach adapts to what you actually do"
                )
            ),
            Callout("Repeatable beats optimal. A structure you follow beats a better one you don't.")
        )
    )

    val setsRepsRpe = Lesson(
        id = "fundamentals.sets_reps_rpe",
        track = LessonTrack.FUNDAMENTALS,
        title = "Sets, reps, and how hard is hard",
        summary = "Reps are what you did. RPE is how close to your limit you did it. The coach needs both.",
        unlock = LessonUnlock(
            label = "Log a set",
            detail = "Start any session and log one working set.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "3 by 8 to 10 means three working sets, each one landing somewhere between eight " +
                    "and ten reps. The range is there so you can add reps before you add weight."
            ),
            Heading("Effort, in one number"),
            Paragraph(
                "RPE scores how hard a set was, from 1 to 10, based on what you had left at the " +
                    "end of it. RPE 8 means two more clean reps were there. RPE 10 means none were."
            ),
            Bullets(
                listOf(
                    "RPE 6 to 7: comfortable, several reps left",
                    "RPE 8: two clean reps left, and the usual target for working sets",
                    "RPE 9: one rep left",
                    "RPE 10: you could not have done another"
                )
            ),
            Paragraph(
                "Most of your sets should land between 7 and 9. Going to your absolute limit on " +
                    "every set costs more recovery than it buys."
            ),
            Callout("Log effort honestly and the coach can tell a hard day from a heavy one.")
        )
    )

    val formVsLoad = Lesson(
        id = "fundamentals.form_vs_load",
        track = LessonTrack.FUNDAMENTALS,
        title = "Form first, load second",
        summary = "Weight only counts for the muscle that actually moved it.",
        unlock = LessonUnlock(
            label = "Tag a session as technique work",
            detail = "Finish a session and add the technique tag. The coach then leaves those sets out of stall detection.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "The weight is a tool, not the score. If you add plates by cutting the range short " +
                    "or letting other muscles help, the muscle you were training does not get more " +
                    "work. You just get a bigger number."
            ),
            Bullets(
                listOf(
                    "Lower the weight under control instead of dropping into it",
                    "Use the full range you can control, not the range that lets you add plates",
                    "Keep the working muscle working, rather than bouncing or swinging the weight",
                    "If your form has to change to finish a rep, that rep was past your limit"
                )
            ),
            Paragraph(
                "That is what the technique tag is for. Tag a session and the coach leaves it out " +
                    "of its progress reads, so a deliberately light day never looks like a stall."
            ),
            Callout("The coach never rewards an ugly personal record, because your body doesn't either.")
        )
    )

    val progressiveOverload = Lesson(
        id = "fundamentals.progressive_overload",
        track = LessonTrack.FUNDAMENTALS,
        title = "Progressive overload",
        summary = "Doing a little more over time is what forces your body to change. Everything else is detail.",
        unlock = LessonUnlock(
            label = "When the coach first suggests a weight",
            detail = "It needs two sessions of the same lift to compare before it will.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Your body changes in response to demands it has to keep meeting. Keep the demand " +
                    "the same and there is nothing left to adapt to, so you hold where you are."
            ),
            Heading("More has several meanings"),
            Bullets(
                listOf(
                    "More weight for the same reps",
                    "More reps at the same weight",
                    "More sets across the week",
                    "Better control or a fuller range at the same numbers"
                )
            ),
            Paragraph(
                "The coach works the rep range first. Once you hit the top of it on every set, it " +
                    "adds weight and you start climbing the range again. That way each jump is " +
                    "earned rather than guessed."
            ),
            Callout("Your job is showing up and logging honestly. Choosing which \"more\" is the coach's job.")
        )
    )

    val restAndRecovery = Lesson(
        id = "fundamentals.rest_and_recovery",
        track = LessonTrack.FUNDAMENTALS,
        title = "Rest is where you grow",
        summary = "Training is the stimulus. The growth happens in between sessions.",
        unlock = LessonUnlock(
            label = "Use a rest timer",
            detail = "It starts itself the moment you log a set in a session.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph("Two clocks matter. The one between sets, and the one between sessions."),
            Heading("Between sets"),
            Paragraph(
                "On heavy compound lifts, two to three minutes is what it takes to repeat a real " +
                    "effort. Cut that to one minute and your later sets get worse. It feels harder. " +
                    "It is not more training."
            ),
            Heading("Between sessions"),
            Paragraph(
                "A muscle spends roughly two to three days rebuilding after you train it. That is " +
                    "why the coach leaves a gap before coming back to it, and why a rest day is " +
                    "part of the plan rather than a day you missed."
            ),
            Callout("Sleep is the best recovery tool you own, and the cheapest.")
        )
    )

    val sorenessVsInjury = Lesson(
        id = "fundamentals.soreness_vs_injury",
        track = LessonTrack.FUNDAMENTALS,
        title = "Soreness vs injury",
        summary = "Soreness is dull, spread out, and fades once you warm up. Injury is sharp, in one spot, and worse under load.",
        unlock = LessonUnlock(
            label = "Flag soreness or illness",
            detail = "From the daily check-in, or by logging a rest day as sick.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "Soreness usually peaks a day or two after training. It feels dull, spreads across " +
                    "the whole muscle, turns up on both sides evenly, and eases once you warm up."
            ),
            Paragraph(
                "An injury feels different. Sharp instead of dull, in one spot, often close to a " +
                    "joint, and worse the more you load it."
            ),
            Bullets(
                listOf(
                    "Sore: train it lighter, or train something else, and it settles",
                    "Sharp, or right on a joint: stop loading it and flag it as restricted",
                    "Soreness is not proof of a good session, and no soreness is not proof of a bad one",
                    "You get less sore on the same movements over time, even while still progressing"
                )
            ),
            Paragraph(
                "Flag a muscle as sore and the coach eases that muscle's work for a day or two. " +
                    "Flag an injury and it routes around the movement entirely until you clear it."
            ),
            Callout("Soreness is information. It is not a score.")
        )
    )

    val warmups = Lesson(
        id = "fundamentals.warmups",
        track = LessonTrack.FUNDAMENTALS,
        title = "Why warm-ups are in your session",
        summary = "A warm-up gets the muscle moving and rehearses the lift. It should never tire you out.",
        unlock = LessonUnlock(
            label = "Start a session with its warm-up",
            detail = "Warm-up sets lead a session unless you hold the start button to skip them.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "A warm-up does two things. It gets the muscle warm and moving freely, and it " +
                    "rehearses the movement before it gets heavy."
            ),
            Paragraph(
                "The payoff you can actually measure is performance. Your first working set lands " +
                    "closer to what you are capable of, instead of being a second warm-up."
            ),
            Heading("Ramp sets"),
            Paragraph(
                "The simplest warm-up for a lift is the lift itself, getting heavier. A few light " +
                    "reps, then a few at something in between, then your working weight. Keep the " +
                    "reps low and stay well clear of failure."
            ),
            Callout("If your warm-up made you tired, it was training, and it will cost you.")
        )
    )

    val howTheCoachWorks = Lesson(
        id = "fundamentals.how_the_coach_works",
        track = LessonTrack.FUNDAMENTALS,
        title = "How your coach works",
        summary = "It reads your data, proposes a change, watches what happens, and keeps score of itself.",
        unlock = LessonUnlock(
            label = "When your first week brief lands",
            detail = "The coach writes one after a week with training in it.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "The coach runs the same loop over and over. It reads everything you have logged, " +
                    "proposes a change, waits for you to take it or ignore it, then comes back " +
                    "later and judges whether the change actually worked."
            ),
            Bullets(
                listOf(
                    "Nothing gets changed without being checked afterwards",
                    "Anything it changes can be undone in one tap",
                    "Changes that work earn it the right to make bigger ones",
                    "Changes that fail feed back into how it plans",
                    "When it doesn't have enough to go on, it says nothing rather than guessing"
                )
            ),
            Paragraph(
                "That last rule is why a new account sees fewer suggestions. Thin data makes for " +
                    "confident, wrong advice, so the coach waits until it has enough to be useful."
            ),
            Callout("It is a system you can check, not a black box. Every call shows its reason.")
        )
    )

    val whatReadinessMeans = Lesson(
        id = "fundamentals.what_readiness_means",
        track = LessonTrack.FUNDAMENTALS,
        title = "What readiness means",
        summary = "A small nudge to today's targets, built from how you slept, how you feel, and how you've been training.",
        unlock = LessonUnlock(
            label = "When a readiness score first appears",
            detail = "Needs recovery data: a check-in answer, or sleep and resting heart rate from a watch.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Readiness answers one question: how much should today ask of you? It comes from " +
                    "your check-in, your sleep, your resting heart rate against your own normal, " +
                    "how much you have trained lately, and anything you flagged."
            ),
            Paragraph(
                "It moves today's targets by a few percent at most. It never cancels your session " +
                    "and it never tells you something is wrong with you. A low score is about " +
                    "today, not about you."
            ),
            Example(
                key = "readiness_today",
                label = "Today's read",
                fallback = "log a few more sessions and today's number will appear here"
            ),
            Callout("A bad night is a reason to train slightly lighter, not a reason to skip.")
        )
    )

    val logHonestly = Lesson(
        id = "fundamentals.log_honestly",
        track = LessonTrack.FUNDAMENTALS,
        title = "Log honestly",
        summary = "Every decision the coach makes runs on your log. Inflated numbers don't cheat it, they aim it at the wrong thing.",
        unlock = LessonUnlock(
            label = "Finish the fundamentals track",
            detail = "Read the nine lessons above. This one closes it.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "The coach has no eyes. Everything it believes about you comes from what you " +
                    "logged: the weight, the reps, the effort, and whether you finished the set."
            ),
            Bullets(
                listOf(
                    "A rep you didn't really complete becomes a target you can't hit",
                    "An RPE lower than the truth makes the next jump too big",
                    "A soreness flag you skipped turns a rough week into an unexplained stall",
                    "A session you didn't log is a session the coach thinks you never did"
                )
            ),
            Paragraph(
                "Rating effort is a skill and it takes practice. Early on you will be guessing, " +
                    "and that is fine. Guess the same way every time and the guesses still show a " +
                    "trend the coach can use."
            ),
            Callout("Honest logging is the whole price of a coach that can actually help.")
        )
    )

    /** The track in reading order — it doubles as the cold-start directive sequence. */
    val ordered: List<Lesson> = listOf(
        whatAProgramIs,
        setsRepsRpe,
        formVsLoad,
        progressiveOverload,
        restAndRecovery,
        sorenessVsInjury,
        warmups,
        howTheCoachWorks,
        whatReadinessMeans,
        logHonestly
    )
}
